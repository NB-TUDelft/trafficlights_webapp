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
async fn pin_guessing_closes_socket_and_trips_global_limiter() {
    let server = TestServer::start(|config| {
        config.create_session_pin = Some("4321".to_string());
    })
    .await;
    let burst = usize::try_from(CREATE_SESSION_BURST).expect("burst");

    // Ten failed PIN attempts on one socket close that lobby socket.
    let (mut guesser_one, _) = server.lobby().await;
    for guess in 0..burst {
        guesser_one
            .send(&create("Room", 1, 10, Some(&format!("{guess:04}"))))
            .await;
        assert_eq!(error_code(&guesser_one.recv().await), "invalid_pin");
    }
    let _ = guesser_one.expect_close().await;
    assert_eq!(server.log_files(), 0);

    // Below the global 20/min cap, a valid PIN on a fresh socket still works.
    let (mut teacher, _) = server.lobby().await;
    teacher.send(&create("Room", 1, 10, Some("4321"))).await;
    let _session = created(&teacher.recv().await);

    // Ten more failures across a second socket exhaust the global PIN bucket.
    let (mut guesser_two, _) = server.lobby().await;
    for guess in 0..burst {
        guesser_two
            .send(&create("Room", 1, 10, Some(&format!("{guess:04}"))))
            .await;
        assert_eq!(error_code(&guesser_two.recv().await), "invalid_pin");
    }
    let _ = guesser_two.expect_close().await;

    // A third socket is now globally rate-limited even before checking the PIN.
    let (mut blocked, _) = server.lobby().await;
    blocked.send(&create("Room", 1, 10, Some("4321"))).await;
    assert_eq!(error_code(&blocked.recv().await), "rate_limited");
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

#[tokio::test]
async fn a_wrong_pin_hides_name_and_range_validation() {
    let server = TestServer::start(|config| {
        config.create_session_pin = Some("4321".to_string());
    })
    .await;
    let (mut lobby, _) = server.lobby().await;
    // Invalid name and range, but the PIN is checked first.
    lobby.send(&create("", 0, 0, Some("0000"))).await;
    assert_eq!(error_code(&lobby.recv().await), "invalid_pin");
    lobby.send(&create("Room", 1, 513, None)).await;
    assert_eq!(error_code(&lobby.recv().await), "invalid_pin");
    assert_eq!(server.state.active_sessions(), 0);
}

#[tokio::test]
async fn displayed_ranges_are_capped_at_512_tables() {
    let server = TestServer::start(|_| {}).await;
    let (mut lobby, _) = server.lobby().await;
    lobby.send(&create("Too wide", 1, 513, None)).await;
    assert_eq!(error_code(&lobby.recv().await), "invalid_range");
    // A 512-table window anywhere in the ID space is fine.
    let ok = server.create("Window", 100, 200).await;
    assert_eq!(server.state.active_sessions(), 1);
    let _ = ok;
    let full = server.create("Full512", 1, 512).await;
    assert_eq!(server.state.active_sessions(), 2);
    let _ = full;
}
