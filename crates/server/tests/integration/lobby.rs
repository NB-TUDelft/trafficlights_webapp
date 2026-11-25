// SPDX-License-Identifier: AGPL-3.0-or-later
//! The lobby: its snapshot and live updates, session creation and its
//! validation, the optional creation PIN, and the `MAX_SESSIONS` cap.

use trafficlights_proto::{
    Color, ColorCounts, LobbyClientMsg, LobbyServerMsg, SessionClientMsg, SessionServerMsg,
    SessionSummary, TableRange, PROTO_VERSION,
};

use trafficlights_server::ws::CREATE_SESSION_BURST;

use crate::support::{created, new_receiver_firmware, TestServer};

fn create(name: &str, start: u16, end: u16, pin: Option<&str>) -> LobbyClientMsg {
    LobbyClientMsg::CreateSession {
        name: name.to_string(),
        start,
        end,
        pin: pin.map(str::to_string),
    }
}

fn error_code(message: &LobbyServerMsg) -> &str {
    match message {
        LobbyServerMsg::Error { code, .. } => code,
        other => panic!("expected an error, got {other:?}"),
    }
}

/// The session list of a lobby update.
fn listed(message: &LobbyServerMsg) -> &[SessionSummary] {
    match message {
        LobbyServerMsg::LobbyUpdate { sessions }
        | LobbyServerMsg::LobbySnapshot { sessions, .. } => sessions,
        _ => &[],
    }
}

fn is_update(message: &LobbyServerMsg) -> bool {
    matches!(message, LobbyServerMsg::LobbyUpdate { .. })
}

#[tokio::test]
async fn the_lobby_tracks_sessions_live_and_only_the_creator_gets_the_token() {
    let server = TestServer::start(|_| {}).await;
    let (mut watcher, snapshot) = server.lobby().await;
    let LobbyServerMsg::LobbySnapshot {
        sessions,
        server_info,
    } = snapshot
    else {
        panic!("expected a lobby snapshot");
    };
    assert!(sessions.is_empty());
    assert_eq!(server_info.license, "AGPL-3.0-or-later");
    assert_eq!(
        server_info.source_url,
        "https://github.com/NB-TUDelft/trafficlights_webapp"
    );
    assert_eq!(server_info.proto_version, PROTO_VERSION);
    assert!(!server_info.pin_required);

    let (mut requester, _) = server.lobby().await;
    requester.send(&create("  Studio 3B  ", 1, 30, None)).await;
    let reply: LobbyServerMsg = requester.recv().await;
    let session = created(&reply);
    let LobbyServerMsg::SessionCreated {
        creator_token,
        summary,
        ..
    } = &reply
    else {
        panic!("expected session_created");
    };
    assert_eq!(creator_token.len(), 64);
    assert!(creator_token
        .chars()
        .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch)));
    assert_eq!(summary.name, "Studio 3B");
    assert_eq!(summary.range, TableRange { start: 1, end: 30 });
    assert_eq!(
        summary.counts,
        ColorCounts {
            green: 30,
            orange: 0,
            red: 0
        }
    );
    assert!(!summary.receiver_connected);
    assert_eq!(summary.viewer_count, 0);
    let update: LobbyServerMsg = requester.recv().await;
    assert_eq!(listed(&update).len(), 1);

    // Everyone else sees the session, never its token.
    let seen = watcher.lobby_barrier().await;
    assert!(seen.iter().all(is_update), "{seen:?}");
    let last = seen.last().map(listed).unwrap_or_default();
    assert_eq!(
        last.iter()
            .map(|s| s.session_id.as_str())
            .collect::<Vec<_>>(),
        vec![session.id.as_str()]
    );

    // Receiver status, firmware, counts, viewers, and renames propagate.
    let mut creator = server.creator_with_receiver(&session).await;
    watcher
        .recv_until(|msg: &LobbyServerMsg| {
            listed(msg).first().is_some_and(|s| {
                s.receiver_connected && s.firmware == Some(new_receiver_firmware())
            })
        })
        .await;
    creator
        .send(&SessionClientMsg::StationReport {
            table: 4,
            color: Color::Red,
        })
        .await;
    watcher
        .recv_until(|msg: &LobbyServerMsg| listed(msg).first().is_some_and(|s| s.counts.red == 1))
        .await;
    let _viewer = server.viewer(&session.id).await;
    watcher
        .recv_until(|msg: &LobbyServerMsg| listed(msg).first().is_some_and(|s| s.viewer_count == 1))
        .await;
    creator
        .send(&SessionClientMsg::RenameSession {
            name: "Studio 3C".to_string(),
        })
        .await;
    watcher
        .recv_until(|msg: &LobbyServerMsg| {
            listed(msg).first().is_some_and(|s| s.name == "Studio 3C")
        })
        .await;

    // Ending the session removes it.
    creator.send(&SessionClientMsg::EndSession).await;
    watcher
        .recv_until(|msg: &LobbyServerMsg| is_update(msg) && listed(msg).is_empty())
        .await;
    let (_, snapshot) = server.lobby().await;
    assert!(listed(&snapshot).is_empty());
}

#[tokio::test]
async fn invalid_create_requests_create_nothing() {
    let server = TestServer::start(|_| {}).await;
    let (mut lobby, _) = server.lobby().await;

    let cases = [
        (create("   ", 1, 10, None), "invalid_name"),
        (create("two\nlines", 1, 10, None), "invalid_name"),
        (create(&"x".repeat(81), 1, 10, None), "invalid_name"),
        (create("Room", 0, 10, None), "invalid_range"),
        (create("Room", 10, 5, None), "invalid_range"),
        (create("Room", 1, 10_000, None), "invalid_range"),
    ];
    for (request, code) in &cases {
        lobby.send(request).await;
        assert_eq!(error_code(&lobby.recv().await), *code, "{request:?}");
    }
    for text in [
        "{",
        r#"{"type":"explode"}"#,
        r#"{"type":"create_session","name":"A"}"#,
    ] {
        lobby.send_text(text).await;
        assert_eq!(error_code(&lobby.recv().await), "bad_request", "{text}");
    }
    lobby.send_binary(&[0, 1, 2]).await;
    assert_eq!(error_code(&lobby.recv().await), "bad_request");

    assert!(lobby.lobby_barrier().await.is_empty());
    assert_eq!(server.state.active_sessions(), 0);
    assert_eq!(server.log_files(), 0);
}

#[tokio::test]
async fn the_creation_pin_is_checked_when_configured() {
    let server = TestServer::start(|config| {
        config.create_session_pin = Some("4321".to_string());
    })
    .await;
    let (mut lobby, snapshot) = server.lobby().await;
    let LobbyServerMsg::LobbySnapshot { server_info, .. } = snapshot else {
        panic!("expected a lobby snapshot");
    };
    assert!(server_info.pin_required);

    for pin in [None, Some(""), Some("1234"), Some("43210"), Some("432")] {
        lobby.send(&create("Room", 1, 10, pin)).await;
        assert_eq!(error_code(&lobby.recv().await), "invalid_pin", "{pin:?}");
    }
    assert_eq!(server.log_files(), 0);

    lobby.send(&create("Room", 1, 10, Some("4321"))).await;
    let session = created(&lobby.recv().await);
    assert_eq!(server.state.active_sessions(), 1);
    assert!(server.log_path(&session.id).exists());
    assert_eq!(server.log_files(), 1);
}

#[tokio::test]
async fn pin_guessing_is_rate_limited_per_connection() {
    let server = TestServer::start(|config| {
        config.create_session_pin = Some("4321".to_string());
    })
    .await;
    let (mut guesser, _) = server.lobby().await;
    let attempts = 16;
    for guess in 0..attempts {
        guesser
            .send(&create("Room", 1, 10, Some(&format!("{guess:04}"))))
            .await;
    }
    let mut checked = 0;
    let mut limited = 0;
    for _ in 0..attempts {
        match error_code(&guesser.recv().await) {
            "invalid_pin" => checked += 1,
            "rate_limited" => limited += 1,
            other => panic!("unexpected error {other}"),
        }
    }
    let burst = usize::try_from(CREATE_SESSION_BURST).expect("burst");
    assert!(
        (burst..=burst + 2).contains(&checked),
        "{checked} of {attempts} PIN guesses were checked"
    );
    assert_eq!(checked + limited, attempts);
    assert_eq!(server.log_files(), 0);

    // The limit is per connection: another lobby still creates normally.
    let (mut teacher, _) = server.lobby().await;
    teacher.send(&create("Room", 1, 10, Some("4321"))).await;
    let _session = created(&teacher.recv().await);
}

#[tokio::test]
async fn max_sessions_caps_active_sessions() {
    let server = TestServer::start(|config| config.max_sessions = 2).await;
    let first = server.create("One", 1, 10).await;
    let _second = server.create("Two", 1, 10).await;

    let (mut lobby, _) = server.lobby().await;
    lobby.send(&create("Three", 1, 10, None)).await;
    assert_eq!(error_code(&lobby.recv().await), "max_sessions_reached");
    assert_eq!(server.log_files(), 2);

    // Ending a session frees its slot.
    let mut creator = server.creator(&first).await;
    creator.send(&SessionClientMsg::EndSession).await;
    let (texts, _) = creator.expect_close().await;
    assert!(
        texts
            .iter()
            .any(|text| text.contains(r#""type":"session_ended""#)),
        "{texts:?}"
    );
    lobby.send(&create("Three", 1, 10, None)).await;
    let reply = lobby
        .recv_until(|msg: &LobbyServerMsg| !is_update(msg))
        .await;
    let _third = created(&reply);
    assert_eq!(server.state.active_sessions(), 2);

    // Joining the ended session is answered with `session_ended`.
    let mut viewer = server.ws_ok(&format!("/ws/sessions/{}", first.id)).await;
    let ended: SessionServerMsg = viewer.recv().await;
    assert!(
        matches!(ended, SessionServerMsg::SessionEnded { .. }),
        "{ended:?}"
    );
}
