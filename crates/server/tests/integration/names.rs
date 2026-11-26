// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sessions that collect viewer names: viewers must identify before they
//! change tables, names are unique within a session, and the statistics log
//! records who made each web-app change (the creator as `host`).

use trafficlights_proto::{
    ChangeSource, Color, JsonlEvent, SessionClientMsg, SessionServerMsg, HOST_ACTOR,
};

use crate::support::{assert_log_invariants, unstamped, TestServer, WsClient};

fn identify(name: &str, key: Option<&str>) -> SessionClientMsg {
    SessionClientMsg::Identify {
        name: name.to_string(),
        key: key.map(str::to_string),
    }
}

fn set(table: u16, color: Color) -> SessionClientMsg {
    SessionClientMsg::SetTable { table, color }
}

/// The error codes among the messages queued before a barrier.
fn errors(messages: &[SessionServerMsg]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| match message {
            SessionServerMsg::Error { code, .. } => Some(code.clone()),
            _ => None,
        })
        .collect()
}

/// Send `message` and return the error codes it caused (if any).
async fn errors_after(client: &mut WsClient, message: &SessionClientMsg) -> Vec<String> {
    client.send(message).await;
    errors(&client.barrier().await)
}

fn change(
    table: u16,
    prev: Color,
    next: Color,
    source: ChangeSource,
    actor: Option<&str>,
) -> JsonlEvent {
    JsonlEvent::StateChange {
        ts_ms: 0,
        elapsed_ms: 0,
        table,
        prev: Some(prev),
        next,
        source,
        in_range: true,
        actor: actor.map(str::to_string),
    }
}

fn state_changes(events: &[JsonlEvent]) -> Vec<JsonlEvent> {
    events
        .iter()
        .filter(|event| matches!(event, JsonlEvent::StateChange { .. }))
        .map(unstamped)
        .collect()
}

#[tokio::test]
async fn the_log_names_who_changed_each_table() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create_with("Room", 1, 5, true).await;
    let mut creator = server.creator_with_receiver(&session).await;

    // Every connection learns that this session collects names.
    let mut watcher = server.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
    let snapshot: SessionServerMsg = watcher.recv().await;
    assert!(
        matches!(
            snapshot,
            SessionServerMsg::Snapshot {
                collect_names: true,
                ..
            }
        ),
        "{snapshot:?}"
    );
    watcher.close().await;

    let mut viewer = server.viewer(&session.id).await;
    // No name yet: refused, and nothing changes.
    assert_eq!(
        errors_after(&mut viewer, &set(2, Color::Red)).await,
        ["name_required"]
    );
    assert!(errors_after(&mut viewer, &identify("  Alice ", None))
        .await
        .is_empty());
    viewer.send(&set(2, Color::Red)).await;
    viewer
        .recv_until(|msg: &SessionServerMsg| {
            matches!(msg, SessionServerMsg::TableUpdate { table: 2, .. })
        })
        .await;

    // The creator needs no name; one it sends is ignored, and it stays host.
    assert!(errors_after(&mut creator, &identify("Alice", None))
        .await
        .is_empty());
    creator.send(&set(3, Color::Orange)).await;
    creator
        .recv_until(|msg: &SessionServerMsg| {
            matches!(msg, SessionServerMsg::TableUpdate { table: 3, .. })
        })
        .await;
    creator.send(&SessionClientMsg::ResetAll).await;
    creator.barrier().await;
    creator
        .send(&SessionClientMsg::StationReport {
            table: 1,
            color: Color::Red,
        })
        .await;
    creator.barrier().await;
    creator
        .send(&SessionClientMsg::SetTableRange { start: 2, end: 5 })
        .await;
    creator.barrier().await;
    creator.send(&SessionClientMsg::EndSession).await;
    creator.expect_close().await;

    let events = server.log_events(&session.id);
    assert_log_invariants(&session.id, &events);
    assert!(
        matches!(
            events.first(),
            Some(JsonlEvent::SessionStart {
                collect_names: true,
                ..
            })
        ),
        "{events:?}"
    );
    let host = Some(HOST_ACTOR);
    assert_eq!(
        state_changes(&events),
        vec![
            change(
                2,
                Color::Green,
                Color::Red,
                ChangeSource::TeacherClick,
                Some("Alice")
            ),
            change(
                3,
                Color::Green,
                Color::Orange,
                ChangeSource::TeacherClick,
                host
            ),
            change(
                2,
                Color::Red,
                Color::Green,
                ChangeSource::TeacherResetAll,
                host
            ),
            change(
                3,
                Color::Orange,
                Color::Green,
                ChangeSource::TeacherResetAll,
                host
            ),
            change(
                1,
                Color::Green,
                Color::Red,
                ChangeSource::StudentRadio,
                None
            ),
            change(1, Color::Red, Color::Green, ChangeSource::RangeReset, host),
        ]
    );
}

#[tokio::test]
async fn names_are_unique_and_host_is_reserved() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create_with("Room", 1, 5, true).await;
    let _creator = server.creator_with_receiver(&session).await;
    let mut first = server.viewer(&session.id).await;
    let mut second = server.viewer(&session.id).await;

    assert!(errors_after(&mut first, &identify("Bokwa", Some("key-a")))
        .await
        .is_empty());
    // Another browser cannot take a name in use, whatever its letter case.
    for (name, key) in [("bokwa", None), (" BOKWA ", Some("key-b"))] {
        assert_eq!(
            errors_after(&mut second, &identify(name, key)).await,
            ["name_taken"],
            "{name:?}"
        );
    }
    for name in ["host", "Host", "", "   ", "two\nlines"] {
        assert_eq!(
            errors_after(&mut second, &identify(name, None)).await,
            ["invalid_viewer_name"],
            "{name:?}"
        );
    }
    // Every refusal left the second viewer nameless.
    assert_eq!(
        errors_after(&mut second, &set(1, Color::Red)).await,
        ["name_required"]
    );
    // Re-identifying with one's own name is fine.
    assert!(errors_after(&mut first, &identify("Bokwa", Some("key-a")))
        .await
        .is_empty());

    // The same browser reconnecting (same key) takes its name back from its
    // stale connection, which can then no longer act under it.
    let mut again = server.viewer(&session.id).await;
    assert!(errors_after(&mut again, &identify("Bokwa", Some("key-a")))
        .await
        .is_empty());
    assert_eq!(
        errors_after(&mut first, &set(1, Color::Red)).await,
        ["name_required"]
    );
    assert!(errors_after(&mut again, &set(1, Color::Red))
        .await
        .is_empty());

    // A name frees up when its holder leaves.
    second.barrier().await;
    again.close().await;
    second
        .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::SessionMeta { .. }))
        .await;
    assert!(errors_after(&mut second, &identify("Bokwa", None))
        .await
        .is_empty());
    assert!(errors_after(&mut second, &set(2, Color::Red))
        .await
        .is_empty());
}

#[tokio::test]
async fn sessions_without_names_refuse_identify_and_log_no_actor() {
    let server = TestServer::start(|_| {}).await;
    let session = server.create("Room", 1, 5).await;
    let mut creator = server.creator_with_receiver(&session).await;
    let mut viewer = server.viewer(&session.id).await;

    assert_eq!(
        errors_after(&mut viewer, &identify("Alice", None)).await,
        ["names_not_collected"]
    );
    assert!(errors_after(&mut viewer, &set(2, Color::Red))
        .await
        .is_empty());
    creator.send(&SessionClientMsg::ResetAll).await;
    creator.barrier().await;

    let text = server.log_text(&session.id);
    assert!(!text.contains("collect_names"), "{text}");
    assert!(!text.contains("actor"), "{text}");
    let events = server.log_events(&session.id);
    assert_eq!(
        state_changes(&events),
        vec![
            change(
                2,
                Color::Green,
                Color::Red,
                ChangeSource::TeacherClick,
                None
            ),
            change(
                2,
                Color::Red,
                Color::Green,
                ChangeSource::TeacherResetAll,
                None
            ),
        ]
    );
}
