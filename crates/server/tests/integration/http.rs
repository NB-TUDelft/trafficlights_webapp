// SPDX-License-Identifier: AGPL-3.0-or-later
//! The HTTP side: `PUBLIC_ORIGIN` enforcement before any upgrade, handling
//! of malformed WebSocket traffic, the creator-authorized statistics
//! download, including a whole-class `.jsonl` round trip, and the optional
//! custom script in the page head.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use trafficlights_proto::{
    ChangeSource, Color, EndReason, JsonlEvent, LobbyServerMsg, SessionClientMsg, SessionServerMsg,
    TableRange, STATS_SCHEMA_VERSION,
};
use trafficlights_server::ws::MAX_MESSAGE_BYTES;
use trafficlights_server::CustomScript;

use crate::support::{
    assert_log_invariants, new_receiver_firmware, parse_log, unstamped, TestServer,
};

const SITE: &str = "https://lights.example.com";

#[tokio::test]
async fn public_origin_guards_every_websocket_upgrade() {
    let server = TestServer::start(|config| {
        config.public_origins = vec![SITE.to_string()];
    })
    .await;
    let unknown = format!("/ws/sessions/{}", uuid::Uuid::now_v7());
    for path in ["/ws/lobby", unknown.as_str()] {
        for origin in [
            None,
            Some("https://evil.example"),
            Some("https://lights.example.com.evil.example"),
            Some("http://lights.example.com"),
            Some("null"),
        ] {
            assert_eq!(
                server.ws(path, origin).await.err(),
                Some(403),
                "{path} with {origin:?}"
            );
        }
    }

    // The refusal is a JSON error, decided before the upgrade is examined.
    let request = Request::builder()
        .uri("/ws/lobby")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .expect("request");
    let (status, _, body) = server.request(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains(r#""code":"forbidden_origin""#), "{body}");

    // The configured origin is accepted.
    let Ok(mut lobby) = server.ws("/ws/lobby", Some(SITE)).await else {
        panic!("the site's own origin was refused");
    };
    let snapshot: LobbyServerMsg = lobby.recv().await;
    assert!(matches!(snapshot, LobbyServerMsg::LobbySnapshot { .. }));
    let Ok(mut missing) = server.ws(&unknown, Some(SITE)).await else {
        panic!("the site's own origin was refused");
    };
    let (texts, code) = missing.expect_close().await;
    assert_eq!(code, Some(1000));
    assert_eq!(texts.len(), 1, "{texts:?}");
    assert!(
        texts[0].contains(r#""code":"session_not_found""#),
        "{texts:?}"
    );
}

#[tokio::test]
async fn unknown_and_malformed_session_ids_are_answered_then_closed() {
    let server = TestServer::start(|_| {}).await;
    let unknown = format!("/ws/sessions/{}", uuid::Uuid::now_v7());
    for path in [
        unknown.as_str(),
        "/ws/sessions/not.a.session",
        "/ws/sessions/..%2F..%2Fetc%2Fpasswd",
        "/ws/sessions/%00",
    ] {
        let mut client = server.ws_ok(path).await;
        let (texts, code) = client.expect_close().await;
        assert_eq!(code, Some(1000), "{path}");
        let replies: Vec<SessionServerMsg> = texts
            .iter()
            .map(|text| serde_json::from_str(text).expect("a session message"))
            .collect();
        assert!(
            matches!(
                replies.as_slice(),
                [SessionServerMsg::Error { code, .. }] if code == "session_not_found"
            ),
            "{path}: {replies:?}"
        );
    }

    // A plain request to a WebSocket route is not upgraded, and other
    // `/ws/...` paths are not the single-page app.
    let (status, _, _) = server.get("/ws/lobby", None).await;
    assert!(status.is_client_error(), "{status}");
    let (status, _, body) = server.get("/ws/elsewhere", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains(r#""code":"not_found""#), "{body}");
}

#[tokio::test]
async fn malformed_and_oversized_messages_are_refused() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut viewer = server.viewer(&session.id).await;

    for text in [
        "{",
        r#"{"type":"explode"}"#,
        r#"{"type":"set_table","table":1,"color":"blue"}"#,
        r#"{"type":"set_table","table":70000,"color":"red"}"#,
    ] {
        viewer.send_text(text).await;
        let reply: SessionServerMsg = viewer.recv().await;
        assert!(
            matches!(&reply, SessionServerMsg::Error { code, .. } if code == "bad_request"),
            "{text}: {reply:?}"
        );
    }
    viewer.send_binary(b"\x00\x01").await;
    let reply: SessionServerMsg = viewer.recv().await;
    assert!(
        matches!(&reply, SessionServerMsg::Error { code, .. } if code == "bad_request"),
        "{reply:?}"
    );
    // Still connected after those.
    assert!(viewer.barrier().await.is_empty());

    // One byte over the cap ends the connection.
    let oversized = "x".repeat(MAX_MESSAGE_BYTES + 1);
    viewer.try_send_text(&oversized).await;
    let (texts, _) = viewer.expect_close().await;
    assert!(texts.is_empty(), "{texts:?}");
    assert_eq!(server.state.active_sessions(), 1);
}

#[tokio::test]
async fn log_download_requires_the_creator_token() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Studio 3B", 1, 30).await;
    let other = server.create("Other", 1, 30).await;
    let uri = format!("/api/sessions/{}/log", session.id);

    let (status, headers, body) = server.get(&uri, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        headers
            .get(header::WWW_AUTHENTICATE)
            .map(|value| value.as_bytes()),
        Some(&b"Bearer"[..])
    );
    assert!(body.contains(r#""code":"unauthorized""#), "{body}");
    for wrong in [
        "0".repeat(64),
        session.token[..63].to_string(),
        format!("{}0", session.token),
        other.token.clone(),
        String::new(),
    ] {
        assert_eq!(
            server.get(&uri, Some(&wrong)).await.0,
            StatusCode::UNAUTHORIZED,
            "{wrong:?}"
        );
    }
    let basic = Request::builder()
        .uri(&uri)
        .header(header::AUTHORIZATION, format!("Basic {}", session.token))
        .body(Body::empty())
        .expect("request");
    assert_eq!(server.request(basic).await.0, StatusCode::UNAUTHORIZED);

    // The right token streams the live log.
    let (status, headers, body) = server.get(&uri, Some(&session.token)).await;
    assert_eq!(status, StatusCode::OK);
    let header_text = |name| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(
        header_text(header::CONTENT_TYPE),
        "application/x-ndjson; charset=utf-8"
    );
    assert_eq!(
        header_text(header::CONTENT_DISPOSITION),
        format!(
            "attachment; filename=\"trafficlights-{}.jsonl\"",
            session.id
        )
    );
    assert_eq!(header_text(header::CACHE_CONTROL), "no-store");
    assert_eq!(header_text(header::X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert_eq!(body, server.log_text(&session.id));
    assert!(
        body.starts_with(r#"{"event":"session_start","schema_version":1,"#),
        "{body}"
    );
    let events = parse_log(&body);
    assert_log_invariants(&session.id, &events);
    assert!(matches!(
        events.as_slice(),
        [JsonlEvent::SessionStart { name, range, .. }]
            if name == "Studio 3B" && *range == TableRange { start: 1, end: 30 }
    ));

    // Unknown, malformed, and traversal-shaped IDs name no log.
    for id in [
        uuid::Uuid::now_v7().to_string(),
        "a.b".to_string(),
        "..%2F..%2Fetc%2Fpasswd".to_string(),
        "x".repeat(65),
    ] {
        let (status, _, _) = server
            .get(&format!("/api/sessions/{id}/log"), Some(&session.token))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
    }
}

#[tokio::test]
async fn the_statistics_log_round_trips_a_whole_class() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Studio 3B", 1, 5).await;
    let mut creator = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;
    let table_update = |table: u16| move |msg: &SessionServerMsg| matches!(msg, SessionServerMsg::TableUpdate { table: t, .. } if *t == table);
    let snapshot = |msg: &SessionServerMsg| matches!(msg, SessionServerMsg::Snapshot { .. });
    let report = |table, color| SessionClientMsg::StationReport { table, color };

    creator.send(&report(2, Color::Red)).await;
    viewer.recv_until(table_update(2)).await;
    // A heartbeat repeating the same color is not a transition.
    creator.send(&report(2, Color::Red)).await;
    creator.barrier().await;
    viewer
        .send(&SessionClientMsg::SetTable {
            table: 4,
            color: Color::Orange,
        })
        .await;
    viewer.recv_until(table_update(4)).await;
    creator.send(&report(0, Color::Red)).await;
    creator.barrier().await;
    creator.send(&SessionClientMsg::ResetAll).await;
    viewer.recv_until(snapshot).await;
    creator.send(&report(5, Color::Orange)).await;
    viewer.recv_until(table_update(5)).await;
    creator
        .send(&SessionClientMsg::SetTableRange { start: 1, end: 3 })
        .await;
    viewer.recv_until(snapshot).await;
    creator
        .send(&SessionClientMsg::RenameSession {
            name: "Studio 3B (moved)".to_string(),
        })
        .await;
    viewer
        .recv_until(|msg: &SessionServerMsg| {
            matches!(msg, SessionServerMsg::SessionMeta { name, .. } if name == "Studio 3B (moved)")
        })
        .await;
    viewer.close().await;
    creator
        .recv_until(|msg: &SessionServerMsg| {
            matches!(
                msg,
                SessionServerMsg::SessionMeta {
                    viewer_count: 0,
                    ..
                }
            )
        })
        .await;
    creator
        .send(&SessionClientMsg::SerialStatus {
            connected: false,
            port_label: None,
            firmware: None,
        })
        .await;
    creator.barrier().await;
    creator.send(&SessionClientMsg::EndSession).await;
    creator.expect_close().await;

    let (status, _, body) = server
        .get(
            &format!("/api/sessions/{}/log", session.id),
            Some(&session.token),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.ends_with('\n'), "every line is LF-terminated");
    let events = parse_log(&body);
    assert_log_invariants(&session.id, &events);

    let change = |table, prev, next, source, in_range| JsonlEvent::StateChange {
        ts_ms: 0,
        elapsed_ms: 0,
        table,
        prev,
        next,
        source,
        in_range,
        actor: None,
    };
    let expected = vec![
        JsonlEvent::SessionStart {
            schema_version: STATS_SCHEMA_VERSION,
            ts_ms: 0,
            elapsed_ms: 0,
            session_id: session.id.clone(),
            name: "Studio 3B".to_string(),
            range: TableRange { start: 1, end: 5 },
            collect_names: false,
        },
        JsonlEvent::ReceiverStatus {
            ts_ms: 0,
            elapsed_ms: 0,
            connected: true,
            firmware: Some(new_receiver_firmware()),
        },
        JsonlEvent::ViewerCount {
            ts_ms: 0,
            elapsed_ms: 0,
            viewers: 1,
        },
        change(
            2,
            Some(Color::Green),
            Color::Red,
            ChangeSource::StudentRadio,
            true,
        ),
        change(
            4,
            Some(Color::Green),
            Color::Orange,
            ChangeSource::TeacherClick,
            true,
        ),
        change(0, None, Color::Red, ChangeSource::StudentRadio, false),
        change(
            2,
            Some(Color::Red),
            Color::Green,
            ChangeSource::TeacherResetAll,
            true,
        ),
        change(
            4,
            Some(Color::Orange),
            Color::Green,
            ChangeSource::TeacherResetAll,
            true,
        ),
        change(
            5,
            Some(Color::Green),
            Color::Orange,
            ChangeSource::StudentRadio,
            true,
        ),
        change(
            5,
            Some(Color::Orange),
            Color::Green,
            ChangeSource::RangeReset,
            true,
        ),
        JsonlEvent::RangeChange {
            ts_ms: 0,
            elapsed_ms: 0,
            prev_range: TableRange { start: 1, end: 5 },
            next_range: TableRange { start: 1, end: 3 },
        },
        JsonlEvent::ViewerCount {
            ts_ms: 0,
            elapsed_ms: 0,
            viewers: 0,
        },
        JsonlEvent::ReceiverStatus {
            ts_ms: 0,
            elapsed_ms: 0,
            connected: false,
            firmware: None,
        },
        JsonlEvent::SessionEnd {
            ts_ms: 0,
            elapsed_ms: 0,
            reason: EndReason::Manual,
        },
    ];
    let got: Vec<JsonlEvent> = events.iter().map(unstamped).collect();
    assert_eq!(got, expected);
}

#[tokio::test]
async fn the_embedded_app_carries_security_headers() {
    let server = TestServer::start(|_| {}).await;
    let (status, headers, _) = server.get("/", None).await;
    assert_eq!(status, StatusCode::OK);
    let header_text = |name| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(header_text(header::X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert_eq!(header_text(header::REFERRER_POLICY), "no-referrer");
    assert_eq!(header_text(header::X_FRAME_OPTIONS), "DENY");
    let csp = header_text(header::CONTENT_SECURITY_POLICY);
    assert!(csp.contains("default-src 'self'"), "{csp}");
    assert!(csp.contains("frame-ancestors 'none'"), "{csp}");
    assert!(csp.contains("'wasm-unsafe-eval'"), "{csp}");
}

#[tokio::test]
async fn websocket_upgrades_are_refused_past_max_connections() {
    let server = TestServer::start(|config| config.max_connections = 1).await;
    let _first = server.ws_ok("/ws/lobby").await;
    assert_eq!(server.ws("/ws/lobby", None).await.err(), Some(503));
    let session = uuid::Uuid::now_v7();
    assert_eq!(
        server
            .ws(&format!("/ws/sessions/{session}"), None)
            .await
            .err(),
        Some(503)
    );
}

#[tokio::test]
async fn a_custom_script_joins_every_page_head_and_the_csp() {
    let server = TestServer::start(|config| {
        config.custom_script = Some(
            CustomScript::parse("https://Scripts.Example.com/s.js?site=A&b=1")
                .expect("a valid custom script"),
        );
    })
    .await;
    let element =
        r#"<script src="https://Scripts.Example.com/s.js?site=A&amp;b=1" defer></script></head>"#;
    let session = format!("/s/{}", uuid::Uuid::now_v7());
    for path in ["/", "/index.html", "/new", session.as_str()] {
        let (status, headers, body) = server.get(path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(body.matches(element).count(), 1, "{path}: {body}");
        let csp = headers
            .get(header::CONTENT_SECURITY_POLICY)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        for directive in [
            "script-src 'self' 'wasm-unsafe-eval' https://scripts.example.com;",
            "connect-src 'self' ws: wss: https://scripts.example.com;",
            "img-src 'self' data: https://scripts.example.com;",
            "style-src 'self' https://scripts.example.com;",
            "frame-ancestors 'none'",
        ] {
            assert!(csp.contains(directive), "{path}: {csp}");
        }
    }

    // Unset, the page and its policy stay as they were.
    let plain = TestServer::start(|_| {}).await;
    let (_, headers, body) = plain.get("/", None).await;
    assert!(
        !body.to_ascii_lowercase().contains("scripts.example.com"),
        "{body}"
    );
    let csp = headers
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        csp.contains("script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' ws: wss:;"),
        "{csp}"
    );
}
