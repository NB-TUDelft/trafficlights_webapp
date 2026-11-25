// SPDX-License-Identifier: AGPL-3.0-or-later
//! Viewer versus creator: who may do what, what each of them is told, and
//! what reaches the statistics log.

use std::time::Duration;

use trafficlights_proto::{
    ChangeSource, Color, ColorCounts, FirmwareInfo, FirmwareKind, FirmwareMode, JsonlEvent,
    SerialCommand, SessionClientMsg, SessionServerMsg, TableRange, TapMode, UnlistedTable,
};
use trafficlights_server::ws::SET_TABLE_BURST;

use crate::support::{unstamped, TestServer};

fn error_code(message: &SessionServerMsg) -> &str {
    match message {
        SessionServerMsg::Error { code, .. } => code,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn is_error(message: &SessionServerMsg) -> bool {
    matches!(message, SessionServerMsg::Error { .. })
}

/// Messages that must never reach a viewer.
fn creator_only(message: &SessionServerMsg) -> bool {
    match message {
        SessionServerMsg::SerialCommand { .. }
        | SessionServerMsg::UnlistedTables { .. }
        | SessionServerMsg::AuthOk { .. } => true,
        SessionServerMsg::Snapshot {
            is_creator,
            unlisted_tables,
            ..
        } => *is_creator || !unlisted_tables.is_empty(),
        _ => false,
    }
}

fn state_changes(events: &[JsonlEvent]) -> Vec<JsonlEvent> {
    events
        .iter()
        .filter(|event| matches!(event, JsonlEvent::StateChange { .. }))
        .map(unstamped)
        .collect()
}

fn change(
    table: u16,
    prev: Option<Color>,
    next: Color,
    source: ChangeSource,
    in_range: bool,
) -> JsonlEvent {
    JsonlEvent::StateChange {
        ts_ms: 0,
        elapsed_ms: 0,
        table,
        prev,
        next,
        source,
        in_range,
    }
}

fn viewer_counts(events: &[JsonlEvent]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|event| match event {
            JsonlEvent::ViewerCount { viewers, .. } => Some(*viewers),
            _ => None,
        })
        .collect()
}

fn receiver_statuses(events: &[JsonlEvent]) -> Vec<bool> {
    events
        .iter()
        .filter_map(|event| match event {
            JsonlEvent::ReceiverStatus { connected, .. } => Some(*connected),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_viewer_click_is_relayed_to_the_creator_only() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;
    let mut other = server.viewer(&session.id).await;

    viewer
        .send(&SessionClientMsg::SetTable {
            table: 5,
            color: Color::Red,
        })
        .await;
    let command = creator
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SerialCommand { .. }))
        .await;
    assert_eq!(
        command,
        SessionServerMsg::SerialCommand {
            command: SerialCommand::SetTable {
                table: 5,
                color: Color::Red
            }
        }
    );
    let update = creator
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::TableUpdate { .. }))
        .await;
    let SessionServerMsg::TableUpdate {
        table: 5,
        color: Color::Red,
        red_since_ms: Some(_),
        counts,
    } = update
    else {
        panic!("unexpected update {update:?}");
    };
    assert_eq!(
        counts,
        ColorCounts {
            green: 9,
            orange: 0,
            red: 1
        }
    );

    for client in [&mut viewer, &mut other] {
        let seen = client.barrier().await;
        assert!(
            seen.iter().any(|msg| matches!(
                msg,
                SessionServerMsg::TableUpdate {
                    table: 5,
                    color: Color::Red,
                    ..
                }
            )),
            "{seen:?}"
        );
        assert!(
            !seen.iter().any(creator_only),
            "leaked to a viewer: {seen:?}"
        );
    }

    // Clicking the current color again re-sends the command (the station
    // may have missed it) but changes and logs nothing.
    viewer
        .send(&SessionClientMsg::SetTable {
            table: 5,
            color: Color::Red,
        })
        .await;
    creator
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SerialCommand { .. }))
        .await;
    let seen = viewer.barrier().await;
    assert!(seen.is_empty(), "{seen:?}");

    // Tables outside the range are refused.
    for table in [0, 11] {
        viewer
            .send(&SessionClientMsg::SetTable {
                table,
                color: Color::Green,
            })
            .await;
        let reply = viewer.recv_until(is_error).await;
        assert_eq!(error_code(&reply), "table_out_of_range");
    }
    let seen = creator.barrier().await;
    assert!(
        !seen
            .iter()
            .any(|msg| matches!(msg, SessionServerMsg::SerialCommand { .. })),
        "{seen:?}"
    );

    assert_eq!(
        state_changes(&server.log_events(&session.id)),
        vec![change(
            5,
            Some(Color::Green),
            Color::Red,
            ChangeSource::TeacherClick,
            true
        )]
    );
}

#[tokio::test]
async fn viewers_are_forbidden_from_every_creator_control() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;
    creator.barrier().await;
    let before = server.log_text(&session.id);

    let forbidden = [
        SessionClientMsg::StationReport {
            table: 1,
            color: Color::Red,
        },
        SessionClientMsg::StationReport {
            table: 0,
            color: Color::Red,
        },
        SessionClientMsg::SerialStatus {
            connected: false,
            port_label: None,
            firmware: None,
        },
        SessionClientMsg::ResetAll,
        SessionClientMsg::SetTableRange { start: 1, end: 5 },
        SessionClientMsg::RenameSession {
            name: "Hijacked".to_string(),
        },
        SessionClientMsg::SetTapMode {
            tap_mode: TapMode::Cycle,
            locked: true,
        },
        SessionClientMsg::EndSession,
    ];
    for message in &forbidden {
        viewer.send(message).await;
        let reply = viewer.recv().await;
        assert_eq!(error_code(&reply), "forbidden", "{message:?}");
    }

    // A wrong token is refused, and the viewer stays a viewer.
    viewer
        .send(&SessionClientMsg::Auth {
            creator_token: "0".repeat(64),
        })
        .await;
    assert_eq!(error_code(&viewer.recv().await), "unauthorized");
    viewer.send(&SessionClientMsg::ResetAll).await;
    assert_eq!(error_code(&viewer.recv().await), "forbidden");

    // Nothing happened: no log line, nothing for the creator.
    assert_eq!(server.log_text(&session.id), before);
    let seen = creator.barrier().await;
    assert!(seen.is_empty(), "{seen:?}");
    assert_eq!(server.state.active_sessions(), 1);
    let mut fresh = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let snapshot: SessionServerMsg = fresh.recv().await;
    let SessionServerMsg::Snapshot {
        name,
        range,
        receiver_connected,
        counts,
        ..
    } = snapshot
    else {
        panic!("expected a snapshot, got {snapshot:?}");
    };
    assert_eq!(name, "Room");
    assert_eq!(range, TableRange { start: 1, end: 10 });
    assert!(receiver_connected);
    assert_eq!(counts.green, 10);
}

#[tokio::test]
async fn offline_set_table_and_reset_all_change_nothing() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;

    // No creator at all yet.
    let mut viewer = server.viewer(&session.id).await;
    viewer
        .send(&SessionClientMsg::SetTable {
            table: 3,
            color: Color::Red,
        })
        .await;
    assert_eq!(error_code(&viewer.recv().await), "receiver_disconnected");

    // The creator is here, but no receiver is attached.
    let mut creator = server.creator(&session).await;
    creator.barrier().await;
    let before = server.log_text(&session.id);
    viewer
        .send(&SessionClientMsg::SetTable {
            table: 3,
            color: Color::Red,
        })
        .await;
    assert_eq!(error_code(&viewer.recv().await), "receiver_disconnected");
    creator.send(&SessionClientMsg::ResetAll).await;
    assert_eq!(error_code(&creator.recv().await), "receiver_disconnected");
    creator
        .send(&SessionClientMsg::SetTable {
            table: 3,
            color: Color::Red,
        })
        .await;
    assert_eq!(error_code(&creator.recv().await), "receiver_disconnected");
    assert!(creator.barrier().await.is_empty());
    assert!(viewer.barrier().await.is_empty());
    assert_eq!(server.log_text(&session.id), before);

    // Attach, light table 3, then lose the receiver again.
    creator.attach_receiver().await;
    viewer
        .send(&SessionClientMsg::SetTable {
            table: 3,
            color: Color::Red,
        })
        .await;
    creator
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SerialCommand { .. }))
        .await;
    creator
        .send(&SessionClientMsg::SerialStatus {
            connected: false,
            port_label: None,
            firmware: None,
        })
        .await;
    viewer
        .recv_until(|msg: &SessionServerMsg| {
            matches!(
                msg,
                SessionServerMsg::ReceiverStatus {
                    connected: false,
                    ..
                }
            )
        })
        .await;
    creator.barrier().await;
    let before = server.log_text(&session.id);

    viewer
        .send(&SessionClientMsg::SetTable {
            table: 3,
            color: Color::Green,
        })
        .await;
    assert_eq!(
        error_code(&viewer.recv_until(is_error).await),
        "receiver_disconnected"
    );
    creator.send(&SessionClientMsg::ResetAll).await;
    assert_eq!(
        error_code(&creator.recv_until(is_error).await),
        "receiver_disconnected"
    );
    let seen = creator.barrier().await;
    assert!(
        !seen
            .iter()
            .any(|msg| matches!(msg, SessionServerMsg::SerialCommand { .. })),
        "{seen:?}"
    );
    assert_eq!(server.log_text(&session.id), before, "no statistics line");

    // The board is untouched: table 3 is still red.
    let mut fresh = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let snapshot: SessionServerMsg = fresh.recv().await;
    let SessionServerMsg::Snapshot {
        tables,
        counts,
        receiver_connected: false,
        ..
    } = snapshot
    else {
        panic!("unexpected snapshot {snapshot:?}");
    };
    assert_eq!(
        counts,
        ColorCounts {
            green: 9,
            orange: 0,
            red: 1
        }
    );
    assert!(tables
        .iter()
        .any(|entry| entry.table == 3 && entry.color == Color::Red));
}

#[tokio::test]
async fn out_of_range_stations_are_shown_to_the_creator_only() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;
    creator.barrier().await;

    let reports = [
        (0, Color::Orange),
        (0, Color::Orange),
        (0, Color::Red),
        (42, Color::Green),
    ];
    for (table, color) in reports {
        creator
            .send(&SessionClientMsg::StationReport { table, color })
            .await;
        let SessionServerMsg::UnlistedTables { tables } = creator.recv().await else {
            panic!("expected unlisted tables");
        };
        assert!(tables
            .iter()
            .any(|entry| entry.table == table && entry.color == color));
    }
    creator
        .send(&SessionClientMsg::StationReport {
            table: 10_000,
            color: Color::Red,
        })
        .await;
    assert_eq!(error_code(&creator.recv().await), "invalid_table");

    let seen = viewer.barrier().await;
    assert!(
        seen.is_empty(),
        "a viewer heard about unlisted tables: {seen:?}"
    );

    // First sight logs `prev: null`, a repeat logs nothing, a change logs
    // the previous color; all with `in_range: false`.
    assert_eq!(
        state_changes(&server.log_events(&session.id)),
        vec![
            change(0, None, Color::Orange, ChangeSource::StudentRadio, false),
            change(
                0,
                Some(Color::Orange),
                Color::Red,
                ChangeSource::StudentRadio,
                false
            ),
            change(42, None, Color::Green, ChangeSource::StudentRadio, false),
        ]
    );
    let raw = server.log_text(&session.id);
    assert!(
        raw.contains(
            r#""table":0,"prev":null,"next":"orange","source":"student_radio","in_range":false"#
        ),
        "{raw}"
    );

    // A new viewer's snapshot carries no unlisted tables at all.
    let mut fresh = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let text = fresh.recv_text().await;
    assert!(text.contains(r#""type":"snapshot""#), "{text}");
    assert!(!text.contains("unlisted"), "{text}");

    // Reset All: everyone gets a snapshot, only the creator's lists them.
    creator.send(&SessionClientMsg::ResetAll).await;
    let command = creator
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SerialCommand { .. }))
        .await;
    assert_eq!(
        command,
        SessionServerMsg::SerialCommand {
            command: SerialCommand::ResetAll
        }
    );
    let snapshot = creator
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::Snapshot { .. }))
        .await;
    let SessionServerMsg::Snapshot {
        is_creator: true,
        unlisted_tables,
        ..
    } = snapshot
    else {
        panic!("unexpected creator snapshot {snapshot:?}");
    };
    let listed: Vec<u16> = unlisted_tables.iter().map(|entry| entry.table).collect();
    assert_eq!(listed, vec![0, 42]);
    let seen = viewer.barrier().await;
    assert!(
        seen.iter()
            .any(|msg| matches!(msg, SessionServerMsg::Snapshot { .. })),
        "{seen:?}"
    );
    assert!(
        !seen.iter().any(creator_only),
        "leaked to a viewer: {seen:?}"
    );

    // A range that now covers 42 forgets it; again only the creator hears.
    creator
        .send(&SessionClientMsg::SetTableRange { start: 1, end: 50 })
        .await;
    let snapshot = creator
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::Snapshot { .. }))
        .await;
    let SessionServerMsg::Snapshot {
        unlisted_tables,
        range,
        ..
    } = snapshot
    else {
        panic!("unexpected creator snapshot {snapshot:?}");
    };
    assert_eq!(range, TableRange { start: 1, end: 50 });
    assert_eq!(
        unlisted_tables
            .iter()
            .map(|entry| entry.table)
            .collect::<Vec<_>>(),
        vec![0]
    );
    let SessionServerMsg::UnlistedTables { tables } = creator.recv().await else {
        panic!("expected the pruned unlisted tables");
    };
    assert_eq!(
        tables.iter().map(|entry| entry.table).collect::<Vec<_>>(),
        vec![0]
    );
    let seen = viewer.barrier().await;
    assert!(
        seen.iter().any(|msg| matches!(
            msg,
            SessionServerMsg::SessionMeta {
                range: TableRange { start: 1, end: 50 },
                ..
            }
        )),
        "{seen:?}"
    );
    assert!(
        !seen.iter().any(creator_only),
        "leaked to a viewer: {seen:?}"
    );

    // A reconnecting creator gets the list with its authentication.
    let mut again = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let _snapshot: SessionServerMsg = again.recv().await;
    again
        .send(&SessionClientMsg::Auth {
            creator_token: session.token.clone(),
        })
        .await;
    let auth = again
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::AuthOk { .. }))
        .await;
    let SessionServerMsg::AuthOk {
        is_creator: true,
        unlisted_tables,
    } = auth
    else {
        panic!("unexpected auth reply {auth:?}");
    };
    assert!(matches!(
        unlisted_tables.as_slice(),
        [UnlistedTable {
            table: 0,
            color: Color::Red,
            ..
        }]
    ));
}

#[tokio::test]
async fn in_range_reports_update_everyone_and_log_transitions_only() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;

    for (color, red) in [
        (Color::Red, true),
        (Color::Red, true),
        (Color::Green, false),
    ] {
        creator
            .send(&SessionClientMsg::StationReport { table: 3, color })
            .await;
        creator.barrier().await;
        let updates: Vec<SessionServerMsg> = viewer
            .barrier()
            .await
            .into_iter()
            .filter(|msg| matches!(msg, SessionServerMsg::TableUpdate { .. }))
            .collect();
        if updates.is_empty() {
            // Only the repeated red heartbeat may be silent.
            assert!(red, "a color change must be broadcast");
            continue;
        }
        let [SessionServerMsg::TableUpdate {
            table: 3,
            color: got,
            red_since_ms,
            ..
        }] = updates.as_slice()
        else {
            panic!("unexpected updates {updates:?}");
        };
        assert_eq!(*got, color);
        assert_eq!(red_since_ms.is_some(), red);
    }

    assert_eq!(
        state_changes(&server.log_events(&session.id)),
        vec![
            change(
                3,
                Some(Color::Green),
                Color::Red,
                ChangeSource::StudentRadio,
                true
            ),
            change(
                3,
                Some(Color::Red),
                Color::Green,
                ChangeSource::StudentRadio,
                true
            ),
        ]
    );
}

#[tokio::test]
async fn a_refreshed_creator_tab_takes_over_without_ending_the_session() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut first = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;

    // A second tab authenticates while the first is still open.
    let mut second = server.creator(&session).await;
    let (texts, code) = first.expect_close().await;
    assert_eq!(code, Some(1000));
    assert!(
        texts
            .iter()
            .any(|text| text.contains(r#""code":"superseded""#)),
        "{texts:?}"
    );
    // The old tab held the serial bridge, so the receiver is unreachable
    // until the new tab attaches its own.
    viewer
        .recv_until(|msg: &SessionServerMsg| {
            matches!(
                msg,
                SessionServerMsg::ReceiverStatus {
                    connected: false,
                    ..
                }
            )
        })
        .await;
    viewer
        .send(&SessionClientMsg::SetTable {
            table: 2,
            color: Color::Orange,
        })
        .await;
    assert_eq!(
        error_code(&viewer.recv_until(is_error).await),
        "receiver_disconnected"
    );

    second.attach_receiver().await;
    viewer
        .recv_until(|msg: &SessionServerMsg| {
            matches!(
                msg,
                SessionServerMsg::ReceiverStatus {
                    connected: true,
                    ..
                }
            )
        })
        .await;
    viewer
        .send(&SessionClientMsg::SetTable {
            table: 2,
            color: Color::Orange,
        })
        .await;
    let command = second
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SerialCommand { .. }))
        .await;
    assert_eq!(
        command,
        SessionServerMsg::SerialCommand {
            command: SerialCommand::SetTable {
                table: 2,
                color: Color::Orange
            }
        }
    );

    // A plain refresh: the old socket goes away first, then a new one
    // authenticates and reattaches.
    second.close().await;
    viewer
        .recv_until(|msg: &SessionServerMsg| {
            matches!(
                msg,
                SessionServerMsg::ReceiverStatus {
                    connected: false,
                    ..
                }
            )
        })
        .await;
    let mut third = server.creator_with_receiver(&session).await;
    viewer
        .recv_until(|msg: &SessionServerMsg| {
            matches!(
                msg,
                SessionServerMsg::ReceiverStatus {
                    connected: true,
                    ..
                }
            )
        })
        .await;
    third.barrier().await;
    viewer.barrier().await;

    assert_eq!(server.state.active_sessions(), 1);
    let events = server.log_events(&session.id);
    // Creator connections were never counted as viewers.
    assert_eq!(viewer_counts(&events), vec![1]);
    assert_eq!(
        receiver_statuses(&events),
        vec![true, false, true, false, true]
    );
    assert!(!events
        .iter()
        .any(|event| matches!(event, JsonlEvent::SessionEnd { .. })));
}

#[tokio::test]
async fn rejected_messages_inside_the_auth_window_change_nothing() {
    // A long window keeps the new connection uncounted for the whole test.
    let server = TestServer::start(|config| config.auth_window = Duration::from_secs(30)).await;
    let session = server.create("Room", 1, 10).await;
    let (mut lobby, _) = server.lobby().await;
    let before = server.log_text(&session.id);

    let mut fresh = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let _snapshot: SessionServerMsg = fresh.recv().await;
    let rejected = [
        (
            SessionClientMsg::SetTable {
                table: 3,
                color: Color::Red,
            },
            "receiver_disconnected",
        ),
        (SessionClientMsg::ResetAll, "forbidden"),
        (
            SessionClientMsg::StationReport {
                table: 3,
                color: Color::Red,
            },
            "forbidden",
        ),
        (
            SessionClientMsg::Auth {
                creator_token: "0".repeat(64),
            },
            "unauthorized",
        ),
    ];
    for (message, code) in &rejected {
        fresh.send(message).await;
        assert_eq!(error_code(&fresh.recv().await), *code, "{message:?}");
    }
    assert!(fresh.barrier().await.is_empty());
    assert_eq!(server.log_text(&session.id), before, "no statistics line");
    assert!(lobby.lobby_barrier().await.is_empty(), "no lobby update");
}

#[tokio::test]
async fn set_table_floods_are_rate_limited_per_connection() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;
    creator.barrier().await;

    let sent = 20;
    for click in 0..sent {
        let color = if click % 2 == 0 {
            Color::Red
        } else {
            Color::Green
        };
        viewer
            .send(&SessionClientMsg::SetTable { table: 1, color })
            .await;
    }
    let limited = viewer
        .barrier()
        .await
        .iter()
        .filter(|msg| matches!(msg, SessionServerMsg::Error { code, .. } if code == "rate_limited"))
        .count();
    let relayed = creator
        .barrier()
        .await
        .iter()
        .filter(|msg| matches!(msg, SessionServerMsg::SerialCommand { .. }))
        .count();
    let burst = usize::try_from(SET_TABLE_BURST).expect("burst");
    assert_eq!(relayed + limited, sent, "every click relayed or refused");
    assert!(
        (burst..=burst + 4).contains(&relayed),
        "{relayed} of {sent} clicks relayed"
    );
    // Refused clicks never reach the statistics.
    let changes = state_changes(&server.log_events(&session.id)).len();
    assert!((burst..=relayed).contains(&changes), "{changes} changes");

    // The bucket refills: after a pause the viewer may click again.
    tokio::time::sleep(Duration::from_millis(500)).await;
    viewer
        .send(&SessionClientMsg::SetTable {
            table: 2,
            color: Color::Orange,
        })
        .await;
    let command = creator
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SerialCommand { .. }))
        .await;
    assert_eq!(
        command,
        SessionServerMsg::SerialCommand {
            command: SerialCommand::SetTable {
                table: 2,
                color: Color::Orange
            }
        }
    );
}

#[tokio::test]
async fn quiet_connections_count_as_viewers_after_the_auth_window() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator(&session).await;

    let mut quiet = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let snapshot: SessionServerMsg = quiet.recv().await;
    assert!(matches!(
        snapshot,
        SessionServerMsg::Snapshot {
            viewer_count: 0,
            ..
        }
    ));
    // It sends nothing; once the window passes it is counted.
    let meta = quiet
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SessionMeta { .. }))
        .await;
    assert!(matches!(
        meta,
        SessionServerMsg::SessionMeta {
            viewer_count: 1,
            ..
        }
    ));
    creator
        .recv_until(|msg: &SessionServerMsg| {
            matches!(
                msg,
                SessionServerMsg::SessionMeta {
                    viewer_count: 1,
                    ..
                }
            )
        })
        .await;

    quiet.close().await;
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
    assert_eq!(viewer_counts(&server.log_events(&session.id)), vec![1, 0]);
}

#[tokio::test]
async fn the_creator_renames_the_session_for_everyone() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator(&session).await;
    let mut viewer = server.viewer(&session.id).await;

    creator
        .send(&SessionClientMsg::RenameSession {
            name: "  Room 2 \u{2014} after lunch  ".to_string(),
        })
        .await;
    let meta = viewer
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SessionMeta { .. }))
        .await;
    let SessionServerMsg::SessionMeta { name, .. } = meta else {
        panic!("unexpected {meta:?}");
    };
    assert_eq!(name, "Room 2 \u{2014} after lunch");

    for bad in ["   ", "two\nlines"] {
        creator
            .send(&SessionClientMsg::RenameSession {
                name: bad.to_string(),
            })
            .await;
        assert_eq!(
            error_code(&creator.recv_until(is_error).await),
            "invalid_name"
        );
    }
    creator
        .send(&SessionClientMsg::SetTableRange { start: 5, end: 1 })
        .await;
    assert_eq!(
        error_code(&creator.recv_until(is_error).await),
        "invalid_range"
    );
}

#[tokio::test]
async fn a_station_mode_board_never_counts_as_a_receiver() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator(&session).await;
    let mut viewer = server.viewer(&session.id).await;
    let station = FirmwareInfo {
        kind: FirmwareKind::New,
        mode: Some(FirmwareMode::Station),
        version: Some("draft-1".to_string()),
        station_id: Some(7),
        station_color: Some(Color::Green),
    };
    creator
        .send(&SessionClientMsg::SerialStatus {
            connected: true,
            port_label: Some("micro:bit (test)".to_string()),
            firmware: Some(station.clone()),
        })
        .await;
    let status = viewer
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::ReceiverStatus { .. }))
        .await;
    assert!(
        matches!(
            &status,
            SessionServerMsg::ReceiverStatus {
                connected: false,
                firmware: Some(info),
                ..
            } if info.mode == Some(FirmwareMode::Station)
        ),
        "{status:?}"
    );
    // Clicks and reset stay refused while only a station is attached.
    viewer
        .send(&SessionClientMsg::SetTable {
            table: 3,
            color: Color::Red,
        })
        .await;
    assert_eq!(
        error_code(&viewer.recv_until(is_error).await),
        "receiver_disconnected"
    );
    creator.send(&SessionClientMsg::ResetAll).await;
    assert_eq!(
        error_code(&creator.recv_until(is_error).await),
        "receiver_disconnected"
    );
    // An over-wide range change is refused too.
    creator
        .send(&SessionClientMsg::SetTableRange {
            start: 1,
            end: 9999,
        })
        .await;
    assert_eq!(
        error_code(&creator.recv_until(is_error).await),
        "invalid_range"
    );
}

#[tokio::test]
async fn the_creator_sets_and_locks_tap_mode_for_viewers() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 10).await;
    let mut creator = server.creator(&session).await;
    let mut viewer = server.viewer(&session.id).await;

    creator
        .send(&SessionClientMsg::SetTapMode {
            tap_mode: TapMode::Cycle,
            locked: true,
        })
        .await;
    let meta = viewer
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SessionMeta { .. }))
        .await;
    assert!(
        matches!(
            meta,
            SessionServerMsg::SessionMeta {
                tap_mode: TapMode::Cycle,
                tap_mode_locked: true,
                ..
            }
        ),
        "unexpected {meta:?}"
    );

    // Late joiners immediately receive the locked tap mode in their snapshot.
    let mut late = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let snapshot: SessionServerMsg = late.recv().await;
    assert!(
        matches!(
            snapshot,
            SessionServerMsg::Snapshot {
                tap_mode: TapMode::Cycle,
                tap_mode_locked: true,
                ..
            }
        ),
        "unexpected {snapshot:?}"
    );

    // Unlocking broadcasts back to everyone.
    creator
        .send(&SessionClientMsg::SetTapMode {
            tap_mode: TapMode::Green,
            locked: false,
        })
        .await;
    let unlocked = viewer
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SessionMeta { .. }))
        .await;
    assert!(
        matches!(
            unlocked,
            SessionServerMsg::SessionMeta {
                tap_mode: TapMode::Green,
                tap_mode_locked: false,
                ..
            }
        ),
        "unexpected {unlocked:?}"
    );
}
