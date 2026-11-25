// SPDX-License-Identifier: AGPL-3.0-or-later
//! How sessions end: the disconnect grace timeout, the creator's End
//! Session, log retention afterward, and graceful shutdown, both in process
//! and through the real binary on `SIGTERM`.

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use tokio::time::timeout;
use trafficlights_proto::{
    EndReason, JsonlEvent, LobbyClientMsg, LobbyServerMsg, SessionClientMsg, SessionServerMsg,
};

use crate::support::{
    assert_log_invariants, created, eventually, parse_log, stamps, temp_dir, TestServer, WsClient,
    STEP,
};

/// The `session_ended` message among `texts`, which must be there.
fn ended(texts: &[String]) -> SessionServerMsg {
    texts
        .iter()
        .filter_map(|text| serde_json::from_str::<SessionServerMsg>(text).ok())
        .find(|msg| matches!(msg, SessionServerMsg::SessionEnded { .. }))
        .unwrap_or_else(|| panic!("no session_ended in {texts:?}"))
}

fn end_reason(events: &[JsonlEvent]) -> Option<EndReason> {
    match events.last() {
        Some(JsonlEvent::SessionEnd { reason, .. }) => Some(*reason),
        _ => None,
    }
}

#[tokio::test]
async fn an_unattended_session_ends_after_the_grace_period() {
    let server = TestServer::start(|config| {
        config.disconnect_grace = Duration::from_millis(300);
    })
    .await;
    let (mut watcher, _) = server.lobby().await;
    let session = server.create("Room", 1, 10).await;
    let mut viewer = server.viewer(&session.id).await;

    // No receiver ever attaches.
    let (texts, code) = viewer.expect_close().await;
    assert_eq!(code, Some(1000));
    assert_eq!(
        ended(&texts),
        SessionServerMsg::SessionEnded {
            reason: EndReason::GraceTimeout,
            log_available: true
        }
    );
    watcher
        .recv_until(|msg: &LobbyServerMsg| {
            matches!(msg, LobbyServerMsg::LobbyUpdate { sessions } if sessions.is_empty())
        })
        .await;
    assert_eq!(server.state.active_sessions(), 0);
    let events = server.log_events(&session.id);
    assert_log_invariants(&session.id, &events);
    assert_eq!(end_reason(&events), Some(EndReason::GraceTimeout));
}

#[tokio::test]
async fn an_attached_receiver_holds_the_session_until_it_is_lost() {
    let grace = Duration::from_millis(500);
    let server = TestServer::start(|config| config.disconnect_grace = grace).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;

    // Well past the grace period, the attached receiver keeps it alive.
    tokio::time::sleep(grace * 2).await;
    assert_eq!(server.state.active_sessions(), 1);

    let lost = SessionClientMsg::SerialStatus {
        connected: false,
        port_label: None,
        firmware: None,
    };
    creator.send(&lost).await;
    // A repeated "still disconnected" report must not restart the clock.
    tokio::time::sleep(Duration::from_millis(350)).await;
    creator.send(&lost).await;

    let (texts, _) = viewer.expect_close().await;
    assert!(matches!(
        ended(&texts),
        SessionServerMsg::SessionEnded {
            reason: EndReason::GraceTimeout,
            ..
        }
    ));
    let events = server.log_events(&session.id);
    assert_log_invariants(&session.id, &events);
    let lost_at = events
        .iter()
        .find(|event| {
            matches!(
                event,
                JsonlEvent::ReceiverStatus {
                    connected: false,
                    ..
                }
            )
        })
        .map(stamps)
        .unwrap_or_else(|| panic!("no disconnect logged: {events:?}"));
    let end_at = events.last().map(stamps).unwrap_or_default();
    let waited = Duration::from_millis(u64::try_from(end_at.0 - lost_at.0).unwrap_or_default());
    assert!(waited >= grace, "ended after only {waited:?}");
    assert!(
        waited < grace + Duration::from_millis(350),
        "the repeated report restarted the grace period: {waited:?}"
    );
    let disconnects = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                JsonlEvent::ReceiverStatus {
                    connected: false,
                    ..
                }
            )
        })
        .count();
    assert_eq!(disconnects, 1, "{events:?}");
    assert_eq!(end_reason(&events), Some(EndReason::GraceTimeout));
}

#[tokio::test]
async fn losing_the_creator_socket_starts_the_grace_period() {
    let server = TestServer::start(|config| {
        config.disconnect_grace = Duration::from_millis(300);
    })
    .await;
    let session = server.create("Room", 1, 10).await;
    let creator = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;

    creator.close().await;
    let (texts, _) = viewer.expect_close().await;
    let receiver_lost = texts
        .iter()
        .filter_map(|text| serde_json::from_str::<SessionServerMsg>(text).ok())
        .any(|msg| {
            matches!(
                msg,
                SessionServerMsg::ReceiverStatus {
                    connected: false,
                    ..
                }
            )
        });
    assert!(receiver_lost, "{texts:?}");
    assert!(matches!(
        ended(&texts),
        SessionServerMsg::SessionEnded {
            reason: EndReason::GraceTimeout,
            ..
        }
    ));
    let events = server.log_events(&session.id);
    assert_log_invariants(&session.id, &events);
    assert!(matches!(
        events.as_slice(),
        [
            ..,
            JsonlEvent::ReceiverStatus {
                connected: false,
                firmware: None,
                ..
            },
            JsonlEvent::SessionEnd {
                reason: EndReason::GraceTimeout,
                ..
            }
        ]
    ));
}

#[tokio::test]
async fn an_ended_session_log_is_kept_for_the_retention_window() {
    let server = TestServer::start(|config| {
        config.log_retention = Duration::from_millis(1_500);
    })
    .await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator(&session).await;
    let mut viewer = server.viewer(&session.id).await;

    creator.send(&SessionClientMsg::EndSession).await;
    let expected = SessionServerMsg::SessionEnded {
        reason: EndReason::Manual,
        log_available: true,
    };
    let (texts, _) = creator.expect_close().await;
    assert_eq!(ended(&texts), expected);
    let (texts, _) = viewer.expect_close().await;
    assert_eq!(ended(&texts), expected);

    // Still downloadable, by its creator only.
    let uri = format!("/api/sessions/{}/log", session.id);
    let (status, _, body) = server.get(&uri, Some(&session.token)).await;
    assert_eq!(status, StatusCode::OK);
    let events = parse_log(&body);
    assert_log_invariants(&session.id, &events);
    assert_eq!(end_reason(&events), Some(EndReason::Manual));
    assert_eq!(server.get(&uri, None).await.0, StatusCode::UNAUTHORIZED);

    // Late joiners learn that it ended.
    let mut late = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let (texts, _) = late.expect_close().await;
    assert_eq!(ended(&texts), expected);

    // After the window, the log and the session are gone.
    eventually("the log to expire", || async {
        server.get(&uri, Some(&session.token)).await.0 == StatusCode::NOT_FOUND
    })
    .await;
    assert!(!server.log_path(&session.id).exists());
    let mut gone = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let (texts, code) = gone.expect_close().await;
    assert_eq!(code, Some(1000));
    assert_eq!(texts.len(), 1, "{texts:?}");
    assert!(
        texts[0].contains(r#""code":"session_not_found""#),
        "{texts:?}"
    );
}

#[tokio::test]
async fn zero_retention_deletes_the_log_at_once() {
    let server = TestServer::start(|config| config.log_retention = Duration::ZERO).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator(&session).await;
    creator.send(&SessionClientMsg::EndSession).await;
    let (texts, _) = creator.expect_close().await;
    assert_eq!(
        ended(&texts),
        SessionServerMsg::SessionEnded {
            reason: EndReason::Manual,
            log_available: false
        }
    );
    let path = server.log_path(&session.id);
    eventually("the log to be deleted", || {
        let exists = path.exists();
        async move { !exists }
    })
    .await;
}

#[tokio::test]
async fn shutdown_ends_every_session_and_finishes_its_log() {
    let server = TestServer::start(|_| {}).await;
    let (mut lobby, _) = server.lobby().await;
    let first = server.create("One", 1, 10).await;
    let second = server.create("Two", 1, 10).await;
    let mut creator = server.creator_with_receiver(&first).await;
    let mut viewer = server.viewer(&second.id).await;

    let state = Arc::clone(&server.state);
    let shutdown = tokio::spawn(async move { state.shutdown().await });
    for client in [&mut creator, &mut viewer] {
        let (texts, _) = client.expect_close().await;
        // The download route stops with the server, so no log is offered.
        assert_eq!(
            ended(&texts),
            SessionServerMsg::SessionEnded {
                reason: EndReason::Shutdown,
                log_available: false
            }
        );
    }
    lobby.expect_close().await;
    assert!(timeout(STEP, shutdown).await.is_ok(), "shutdown hung");
    assert!(server.state.is_shutting_down());
    // A second call has nothing left to do and returns at once.
    assert!(
        timeout(Duration::from_millis(500), server.state.shutdown())
            .await
            .is_ok(),
        "a repeated shutdown waited"
    );
    assert_eq!(server.state.active_sessions(), 0);
    for session in [&first, &second] {
        let events = server.log_events(&session.id);
        assert_log_invariants(&session.id, &events);
        assert_eq!(end_reason(&events), Some(EndReason::Shutdown));
    }

    // No new sessions after shutdown began.
    let (mut late, _) = server.lobby().await;
    late.expect_close().await;
}

/// The spawned binary and its log directory, cleaned up even when the test
/// fails halfway: a still-running server is killed, the directory removed.
#[cfg(unix)]
struct Spawned {
    child: std::process::Child,
    log_dir: std::path::PathBuf,
}

#[cfg(unix)]
impl Drop for Spawned {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.log_dir);
    }
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn sigterm_ends_active_sessions_with_shutdown() {
    use std::process::{Command, Stdio};

    let log_dir = temp_dir("sigterm-logs");
    let bind = {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("free port");
        probe.local_addr().expect("local address")
    };
    let child = Command::new(env!("CARGO_BIN_EXE_trafficlights-webapp"))
        .env("BIND", bind.to_string())
        .env("LOG_DIR", &log_dir)
        .env("LOG", "error")
        .env_remove("PUBLIC_ORIGIN")
        .env_remove("CREATE_SESSION_PIN")
        .env_remove("DISCONNECT_GRACE_SECONDS")
        .env_remove("LOG_RETENTION_SECONDS")
        .env_remove("MAX_SESSIONS")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the server");
    let mut server = Spawned {
        child,
        log_dir: log_dir.clone(),
    };

    let probe = bind.to_string();
    eventually("the server to become healthy", || {
        let probe = probe.clone();
        async move {
            tokio::task::spawn_blocking(move || trafficlights_server::healthcheck(&probe))
                .await
                .unwrap_or(false)
        }
    })
    .await;

    let mut lobby = WsClient::connect(bind, "/ws/lobby", None)
        .await
        .expect("lobby upgrade");
    let _snapshot: LobbyServerMsg = lobby.recv().await;
    lobby
        .send(&LobbyClientMsg::CreateSession {
            name: "Room".to_string(),
            start: 1,
            end: 10,
            pin: None,
        })
        .await;
    let session = created(&lobby.recv().await);
    let mut viewer = WsClient::connect(bind, &format!("/ws/sessions/{}", session.id), None)
        .await
        .expect("session upgrade");
    let _snapshot: SessionServerMsg = viewer.recv().await;

    // A slow HTTP client is mid-request when the signal arrives; it must
    // not hold up ending the sessions, nor the exit.
    let mut slow = tokio::net::TcpStream::connect(bind).await.expect("connect");
    tokio::io::AsyncWriteExt::write_all(&mut slow, b"GET /api/health HTTP/1.1\r\nHost: test\r\n")
        .await
        .expect("send a partial request");

    // The shell's built-in kill, as in `tests/cli.rs`.
    let sent = Command::new("sh")
        .args(["-c", &format!("kill -TERM {}", server.child.id())])
        .status()
        .expect("sh");
    assert!(sent.success());

    let (texts, _) = viewer.expect_close().await;
    assert!(matches!(
        ended(&texts),
        SessionServerMsg::SessionEnded {
            reason: EndReason::Shutdown,
            ..
        }
    ));
    lobby.expect_close().await;
    let text = std::fs::read_to_string(log_dir.join(format!("{}.jsonl", session.id)))
        .expect("the session log");
    assert_eq!(
        end_reason(&parse_log(&text)),
        Some(EndReason::Shutdown),
        "session_end must be written while the slow request is still open"
    );

    let mut status = None;
    for _ in 0..500 {
        status = server.child.try_wait().expect("wait for the server");
        if status.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let Some(status) = status else {
        panic!("the server ignored SIGTERM");
    };
    assert!(status.success(), "unclean exit after SIGTERM: {status}");

    let text = std::fs::read_to_string(log_dir.join(format!("{}.jsonl", session.id)))
        .expect("the session log");
    let events = parse_log(&text);
    assert_log_invariants(&session.id, &events);
    assert_eq!(end_reason(&events), Some(EndReason::Shutdown));
    drop(slow);
}
