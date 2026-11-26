// SPDX-License-Identifier: AGPL-3.0-or-later
//! WebSocket JSON data-transfer objects shared by the server and the
//! frontend for `/ws/lobby` and `/ws/sessions/{id}`.
//!
//! Every message enum uses an internal `type` tag with snake-case values,
//! so the Rust server (Slice S5) and the TypeScript frontend (Slice S6)
//! agree on one exact JSON schema from these Rust definitions alone:
//! `{"type": "table_update", ...}`. Field names are the Rust field names.
//!
//! Deserialization is deliberately permissive about *absent* optional
//! fields (`#[serde(default)]`) so older clients keep parsing, while
//! serialization omits them where the contract says so. Deserializing a
//! value does not validate it: a received [`TableRange`], table ID, or
//! session name must still pass through the validating constructors in
//! this crate ([`TableRange::new`], [`normalize_session_name`]) before
//! the server acts on it.

use serde::{Deserialize, Serialize};

use crate::error::ProtoError;
use crate::stats::EndReason;
use crate::wire::{Color, FirmwareInfo, SerialCommand, TableRange};

/// Maximum session name length, in Unicode scalar values.
const MAX_SESSION_NAME_CHARS: usize = 80;

/// Normalize a human-typed session name for storage and display.
///
/// Leading and trailing whitespace is trimmed (including Unicode
/// whitespace, so a pasted name behaves as typed). The result is rejected
/// with [`ProtoError::InvalidSessionName`] when it is empty after
/// trimming, longer than 80 Unicode scalar values, or contains an ASCII
/// control character (`0x00..=0x1F` or `0x7F`) — internal tabs and
/// newlines are rejected rather than collapsed, so the lobby never has
/// to render a multi-line or terminal-escaping name. All other Unicode
/// text (accents, dashes, emoji) is kept verbatim.
pub fn normalize_session_name(raw: &str) -> Result<String, ProtoError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProtoError::InvalidSessionName {
            reason: "session name must not be empty",
        });
    }
    if trimmed.chars().count() > MAX_SESSION_NAME_CHARS {
        return Err(ProtoError::InvalidSessionName {
            reason: "session name must be at most 80 characters",
        });
    }
    if trimmed.chars().any(|ch| ch.is_ascii_control()) {
        return Err(ProtoError::InvalidSessionName {
            reason: "session name must not contain control characters",
        });
    }
    Ok(trimmed.to_string())
}

/// Maximum viewer name length, in Unicode scalar values.
pub const MAX_VIEWER_NAME_CHARS: usize = 40;

/// The name the statistics log gives the session creator in a session
/// that collects viewer names. Reserved: no viewer may take it.
pub const HOST_ACTOR: &str = "host";

/// Normalize the name a viewer gives in a session that collects viewer
/// names.
///
/// Trimmed like [`normalize_session_name`], and rejected with
/// [`ProtoError::InvalidViewerName`] when it is empty after trimming,
/// longer than 40 Unicode scalar values, contains an ASCII control
/// character, or is the creator's reserved name [`HOST_ACTOR`] in any
/// ASCII letter case, so no viewer can pass for the host in the log.
pub fn normalize_viewer_name(raw: &str) -> Result<String, ProtoError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProtoError::InvalidViewerName {
            reason: "name must not be empty",
        });
    }
    if trimmed.chars().count() > MAX_VIEWER_NAME_CHARS {
        return Err(ProtoError::InvalidViewerName {
            reason: "name must be at most 40 characters",
        });
    }
    if trimmed.chars().any(|ch| ch.is_ascii_control()) {
        return Err(ProtoError::InvalidViewerName {
            reason: "name must not contain control characters",
        });
    }
    if trimmed.eq_ignore_ascii_case(HOST_ACTOR) {
        return Err(ProtoError::InvalidViewerName {
            reason: "the name \"host\" is reserved for the session creator",
        });
    }
    Ok(trimmed.to_string())
}

/// What a normal tap or click on a table tile does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapMode {
    /// Reset the tapped table to green immediately (default).
    #[default]
    Green,
    /// Cycle green -> orange -> red -> green.
    Cycle,
}

/// How many in-range tables currently show each color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ColorCounts {
    /// Tables on green.
    pub green: u32,
    /// Tables on orange.
    pub orange: u32,
    /// Tables on red.
    pub red: u32,
}

/// One in-range table in a session snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableEntry {
    /// Table/station ID.
    pub table: u16,
    /// Current color.
    pub color: Color,
    /// UTC epoch milliseconds when this table turned red, while it is
    /// red; omitted otherwise. Clients tick the red duration locally
    /// from this anchor instead of waiting for the next server event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red_since_ms: Option<i64>,
}

/// A station heard over the radio outside the session's table range
/// (for example an unset station reporting ID `0`). Shown to the session
/// creator only, as a setup debugging aid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlistedTable {
    /// Station/table ID as reported (`0..=9999`).
    pub table: u16,
    /// Most recently reported color.
    pub color: Color,
    /// UTC epoch milliseconds when this station was last heard.
    pub last_seen_ms: i64,
}

/// Build and source information about the running server, shown in the
/// UI footer (including the AGPL Section 13 source link) and used by the
/// frontend to detect a protocol mismatch before it becomes a parse
/// failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerBuildInfo {
    /// Server package version.
    pub version: String,
    /// Source commit the binary was built from, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// SPDX license identifier (`AGPL-3.0-or-later`).
    pub license: String,
    /// URL of the Corresponding Source (AGPL Section 13).
    pub source_url: String,
    /// Shared protocol version (`trafficlights_proto::PROTO_VERSION`).
    pub proto_version: u32,
    /// Whether creating a session requires the `CREATE_SESSION_PIN`.
    pub pin_required: bool,
}

/// One live session as listed on the homepage lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Opaque session identifier, also used in `/ws/sessions/{id}`.
    pub session_id: String,
    /// Human-readable session name, already normalized.
    pub name: String,
    /// Inclusive table range displayed in the session.
    pub range: TableRange,
    /// Current in-range color counts.
    pub counts: ColorCounts,
    /// Whether the creator's browser serial bridge is connected.
    pub receiver_connected: bool,
    /// Attached receiver firmware identity, once known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<FirmwareInfo>,
    /// Number of connected viewer clients, excluding the creator.
    pub viewer_count: u32,
    /// UTC epoch milliseconds when the session was created.
    pub created_at_ms: i64,
}

/// Messages a client sends on the lobby socket (`/ws/lobby`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyClientMsg {
    /// Ask the server to create a session. The server re-validates the
    /// name with [`normalize_session_name`] and the range with
    /// [`TableRange::new`]; `pin` is only checked when the server was
    /// configured with a `CREATE_SESSION_PIN`.
    CreateSession {
        /// Requested session name (not yet normalized).
        name: String,
        /// First table ID of the requested range.
        start: u16,
        /// Last table ID of the requested range.
        end: u16,
        /// Deployment PIN, when the server requires one.
        #[serde(default)]
        pin: Option<String>,
        /// Whether viewers must give a name (see
        /// [`SessionClientMsg::Identify`]) before they may change tables,
        /// so the statistics log can name who made each change; the
        /// creator is logged as [`HOST_ACTOR`]. Fixed for the session's
        /// lifetime. Absent means `false`, and `false` is not written.
        #[serde(default, skip_serializing_if = "crate::is_false")]
        collect_names: bool,
    },
    /// Liveness check; answered with [`LobbyServerMsg::Pong`].
    Ping,
}

/// Messages the server sends on the lobby socket (`/ws/lobby`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyServerMsg {
    /// The full lobby state plus server build info, sent on connect.
    LobbySnapshot {
        /// Every live session, in lobby display order.
        sessions: Vec<SessionSummary>,
        /// Build, license, and protocol information for the footer.
        server_info: ServerBuildInfo,
    },
    /// The full session list after any create, update, or end. The list
    /// is classroom-sized, so it is resent whole instead of diffed.
    LobbyUpdate {
        /// Every live session, in lobby display order.
        sessions: Vec<SessionSummary>,
    },
    /// Confirmation to the creating client only. The `creator_token` is
    /// the per-session capability credential: the creator's browser
    /// stores it and presents it in the session socket's `auth`
    /// message, never in a URL.
    SessionCreated {
        /// The new session's identifier.
        session_id: String,
        /// Unguessable per-session creator capability token.
        creator_token: String,
        /// The lobby summary of the new session.
        summary: SessionSummary,
    },
    /// A rejected request. `code` is stable for client branching (for
    /// example `invalid_pin`); `message` is human-readable detail.
    Error {
        /// Stable machine-readable error code.
        code: String,
        /// Human-readable explanation.
        message: String,
    },
    /// Answer to [`LobbyClientMsg::Ping`].
    Pong,
}

/// Messages a client sends on a session socket (`/ws/sessions/{id}`).
///
/// `Identify`, `SetTable`, and `Ping` are open to every viewer. All other
/// stateful messages are creator-only: the server must verify that the
/// sending connection authenticated with [`SessionClientMsg::Auth`]
/// before acting on them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionClientMsg {
    /// Present the creator capability token obtained at session
    /// creation. Sent in the socket body, never in the URL, so the token
    /// stays out of proxy logs and browser history.
    Auth {
        /// The per-session creator capability token.
        creator_token: String,
    },
    /// A viewer gives its name, in a session that collects viewer names
    /// (`collect_names`). Required there before the viewer's `SetTable`
    /// is accepted, and sent again after every reconnect: the server keeps
    /// the name for this connection only and never broadcasts it. The
    /// server re-validates it with [`normalize_viewer_name`] and refuses a
    /// name another connection of the session holds (ignoring letter
    /// case). The creator never needs it; its changes are logged as
    /// [`HOST_ACTOR`].
    Identify {
        /// The viewer's chosen name (not yet normalized).
        name: String,
        /// A random key the browser keeps for itself (at most 64 ASCII
        /// letters, digits, or hyphens). A connection presenting the same
        /// key as the current holder of the name takes the name over, so
        /// a browser that reconnects before the server noticed its old
        /// connection drop is not refused its own name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    /// A viewer or the creator clicked a table tile. The server checks
    /// the table is in range and the receiver is connected before
    /// relaying a [`SerialCommand`] to the creator's browser.
    SetTable {
        /// Table to set.
        table: u16,
        /// Color to set.
        color: Color,
    },
    /// Creator-only: a color reported by a station through the browser
    /// WASM serial bridge (already heartbeat-deduplicated there).
    StationReport {
        /// Reporting station/table ID; may be `0` or out of range.
        table: u16,
        /// Reported color.
        color: Color,
    },
    /// Creator-only: the browser Web Serial bridge connected or
    /// disconnected, and what it is attached to.
    SerialStatus {
        /// Whether a serial device is currently open.
        connected: bool,
        /// Human-readable port label for the creator panel.
        #[serde(default)]
        port_label: Option<String>,
        /// Attached board identity, once probed or assumed legacy.
        #[serde(default)]
        firmware: Option<FirmwareInfo>,
    },
    /// Creator-only: broadcast `T,-1,0` and reset every table to green.
    ResetAll,
    /// Creator-only: change the displayed inclusive table range.
    SetTableRange {
        /// First table ID of the new range.
        start: u16,
        /// Last table ID of the new range.
        end: u16,
    },
    /// Creator-only: rename the session.
    RenameSession {
        /// Requested new name (not yet normalized).
        name: String,
    },
    /// Creator-only: set the host tap mode and whether viewers are locked to
    /// the host's tap mode.
    SetTapMode {
        /// Host's chosen tap mode.
        tap_mode: TapMode,
        /// When true, all viewer clients follow `tap_mode` and cannot toggle
        /// their own tap-mode switch.
        locked: bool,
    },
    /// Creator-only: set the host global beep state and whether viewers are
    /// locked to the host's beep setting.
    SetBeepMode {
        /// Whether alert beeps are enabled on the host.
        enabled: bool,
        /// When true, all viewer clients follow `enabled` and cannot toggle
        /// their own global beep switch.
        locked: bool,
    },
    /// Creator-only: close the session for everyone now.
    EndSession,
    /// Liveness check; answered with [`SessionServerMsg::Pong`].
    Ping,
}

/// Messages the server sends on a session socket (`/ws/sessions/{id}`).
///
/// Everything is broadcast to all connected clients of the session
/// except [`SessionServerMsg::SerialCommand`] and
/// [`SessionServerMsg::UnlistedTables`], which go to the authenticated
/// creator connection only, and [`SessionServerMsg::AuthOk`], which
/// answers the authenticating connection only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionServerMsg {
    /// The full session state, sent on connect. `is_creator` tells this
    /// connection whether it may use creator controls;
    /// `unlisted_tables` is only populated for the creator and is
    /// omitted entirely while empty.
    Snapshot {
        /// The session's identifier.
        session_id: String,
        /// Human-readable session name.
        name: String,
        /// Inclusive table range on display.
        range: TableRange,
        /// One entry per in-range table, in table order.
        tables: Vec<TableEntry>,
        /// Current in-range color counts.
        counts: ColorCounts,
        /// Whether the creator's serial bridge is connected.
        receiver_connected: bool,
        /// Human-readable serial port label, when connected.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port_label: Option<String>,
        /// Attached receiver firmware identity, once known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        firmware: Option<FirmwareInfo>,
        /// Connected viewer count, excluding the creator.
        viewer_count: u32,
        /// Host's current tap mode (`green` by default).
        #[serde(default)]
        tap_mode: TapMode,
        /// Whether viewer clients are locked to the host's tap mode.
        #[serde(default)]
        tap_mode_locked: bool,
        /// Host's current global beep setting (`false` by default).
        #[serde(default)]
        beeps_enabled: bool,
        /// Whether viewer clients are locked to the host's global beep setting.
        #[serde(default)]
        beeps_locked: bool,
        /// Whether this session collects viewer names: a viewer must send
        /// [`SessionClientMsg::Identify`] before its `SetTable` is accepted.
        #[serde(default)]
        collect_names: bool,
        /// Whether this connection authenticated as the creator.
        is_creator: bool,
        /// Out-of-range stations heard; creator-only debugging data.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unlisted_tables: Vec<UnlistedTable>,
        /// Server UTC epoch milliseconds at snapshot time, so clients
        /// can anchor ticking red durations to server time.
        server_now_ms: i64,
        /// Build, license, and protocol information for the footer.
        /// Boxed only to keep this message enum small on the stack
        /// (`Snapshot` is by far its largest variant); the JSON is
        /// identical to an unboxed field.
        server_info: Box<ServerBuildInfo>,
    },
    /// Answer to [`SessionClientMsg::Auth`], confirming whether the
    /// token matched. Carries the current unlisted tables so the
    /// creator panel is complete without waiting for a fresh snapshot.
    AuthOk {
        /// Whether this connection is now the session creator.
        is_creator: bool,
        /// Out-of-range stations heard so far.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unlisted_tables: Vec<UnlistedTable>,
    },
    /// One in-range table changed color, with refreshed counts.
    TableUpdate {
        /// The table that changed.
        table: u16,
        /// Its new color.
        color: Color,
        /// UTC epoch milliseconds when it turned red, while red.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        red_since_ms: Option<i64>,
        /// In-range color counts after the change.
        counts: ColorCounts,
    },
    /// The creator's serial bridge connected or disconnected.
    ReceiverStatus {
        /// Whether a serial device is currently open.
        connected: bool,
        /// Human-readable serial port label, when connected.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port_label: Option<String>,
        /// Attached receiver firmware identity, once known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        firmware: Option<FirmwareInfo>,
    },
    /// Session metadata changed: renamed, re-ranged, tap/beep mode changed, or
    /// a viewer joined or left.
    SessionMeta {
        /// Current session name.
        name: String,
        /// Current inclusive table range.
        range: TableRange,
        /// Connected viewer count, excluding the creator.
        viewer_count: u32,
        /// Host's current tap mode.
        #[serde(default)]
        tap_mode: TapMode,
        /// Whether viewer clients are locked to the host's tap mode.
        #[serde(default)]
        tap_mode_locked: bool,
        /// Host's current global beep setting.
        #[serde(default)]
        beeps_enabled: bool,
        /// Whether viewer clients are locked to the host's global beep setting.
        #[serde(default)]
        beeps_locked: bool,
    },
    /// Creator-only: write this command to the serial device. The
    /// browser formats it with `wire::format_command` (through the WASM
    /// bridge) and records it in the deduplication cache.
    SerialCommand {
        /// The validated command to send.
        command: SerialCommand,
    },
    /// Creator-only: the full current list of out-of-range stations
    /// heard over the radio.
    UnlistedTables {
        /// Every currently heard out-of-range station.
        tables: Vec<UnlistedTable>,
    },
    /// The session has ended. `log_available` tells the creator whether
    /// the finalized statistics `.jsonl` can still be downloaded during
    /// the post-end retention window.
    SessionEnded {
        /// Why the session closed.
        reason: EndReason,
        /// Whether the session statistics log remains downloadable.
        log_available: bool,
    },
    /// A rejected message. `code` is stable for client branching;
    /// `message` is human-readable detail.
    Error {
        /// Stable machine-readable error code.
        code: String,
        /// Human-readable explanation.
        message: String,
    },
    /// Answer to [`SessionClientMsg::Ping`].
    Pong,
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_session_name, normalize_viewer_name, ColorCounts, LobbyClientMsg, LobbyServerMsg,
        ServerBuildInfo, SessionClientMsg, SessionServerMsg, SessionSummary, TableEntry, TapMode,
        UnlistedTable, HOST_ACTOR,
    };
    use crate::error::ProtoError;
    use crate::stats::EndReason;
    use crate::wire::{Color, FirmwareInfo, SerialCommand, TableRange};

    fn server_info() -> ServerBuildInfo {
        ServerBuildInfo {
            version: "0.1.0".to_string(),
            commit: None,
            license: "AGPL-3.0-or-later".to_string(),
            source_url: "https://github.com/NB-TUDelft/trafficlights_webapp".to_string(),
            proto_version: 1,
            pin_required: false,
        }
    }

    fn summary() -> SessionSummary {
        SessionSummary {
            session_id: "sess-1".to_string(),
            name: "Studio 3B".to_string(),
            range: TableRange::default_classroom(),
            counts: ColorCounts {
                green: 58,
                orange: 1,
                red: 1,
            },
            receiver_connected: true,
            firmware: None,
            viewer_count: 3,
            created_at_ms: 1_764_086_400_000,
        }
    }

    fn json<T: serde::Serialize>(value: &T) -> String {
        serde_json::to_string(value).unwrap_or_default()
    }

    #[test]
    fn session_names_trim_and_keep_classroom_text() {
        assert_eq!(
            normalize_session_name("  Studio 3B \u{2014} Week 4  "),
            Ok("Studio 3B \u{2014} Week 4".to_string())
        );
        assert_eq!(
            normalize_session_name("Caf\u{e9}"),
            Ok("Caf\u{e9}".to_string())
        );
        assert_eq!(normalize_session_name("a  b"), Ok("a  b".to_string()));
        let at_limit = "x".repeat(80);
        assert_eq!(normalize_session_name(&at_limit), Ok(at_limit));
        // The limit counts Unicode scalar values, not bytes.
        let emoji: String = "\u{1f6a6}".repeat(80);
        assert_eq!(normalize_session_name(&emoji), Ok(emoji));
    }

    #[test]
    fn session_names_reject_empty_overlong_and_control_text() {
        for name in ["", "   ", "\t\n "] {
            assert_eq!(
                normalize_session_name(name),
                Err(ProtoError::InvalidSessionName {
                    reason: "session name must not be empty"
                }),
                "name {name:?} must be rejected as empty"
            );
        }
        assert_eq!(
            normalize_session_name(&"x".repeat(81)),
            Err(ProtoError::InvalidSessionName {
                reason: "session name must be at most 80 characters"
            })
        );
        for name in [
            "two\nlines",
            "has\ttab",
            "bell\u{7}",
            "del\u{7f}",
            "nul\u{0}byte",
        ] {
            assert_eq!(
                normalize_session_name(name),
                Err(ProtoError::InvalidSessionName {
                    reason: "session name must not contain control characters"
                }),
                "name {name:?} must be rejected for control characters"
            );
        }
    }

    #[test]
    fn viewer_names_trim_and_keep_classroom_text() {
        assert_eq!(normalize_viewer_name("  Alice  "), Ok("Alice".to_string()));
        assert_eq!(
            normalize_viewer_name("Ms. N\u{fa}\u{f1}ez"),
            Ok("Ms. N\u{fa}\u{f1}ez".to_string())
        );
        // Only the exact creator name is reserved, not names containing it.
        assert_eq!(
            normalize_viewer_name("Host Bob"),
            Ok("Host Bob".to_string())
        );
        let at_limit = "x".repeat(40);
        assert_eq!(normalize_viewer_name(&at_limit), Ok(at_limit));
        // The limit counts Unicode scalar values, not bytes.
        let emoji: String = "\u{1f6a6}".repeat(40);
        assert_eq!(normalize_viewer_name(&emoji), Ok(emoji));
    }

    #[test]
    fn viewer_names_reject_empty_overlong_control_text_and_the_host_name() {
        let reason = |name: &str| match normalize_viewer_name(name) {
            Err(ProtoError::InvalidViewerName { reason }) => reason,
            other => panic!("{name:?} must be rejected, got {other:?}"),
        };
        for name in ["", "   ", "\t\n "] {
            assert_eq!(reason(name), "name must not be empty", "{name:?}");
        }
        assert_eq!(
            reason(&"x".repeat(41)),
            "name must be at most 40 characters"
        );
        for name in ["two\nlines", "has\ttab", "bell\u{7}", "del\u{7f}"] {
            assert_eq!(
                reason(name),
                "name must not contain control characters",
                "{name:?}"
            );
        }
        assert_eq!(HOST_ACTOR, "host");
        for name in ["host", "HOST", "  Host ", "hOsT"] {
            assert_eq!(
                reason(name),
                "the name \"host\" is reserved for the session creator",
                "{name:?}"
            );
        }
    }

    #[test]
    fn lobby_messages_use_the_snake_case_type_tag() {
        assert_eq!(
            json(&LobbyClientMsg::CreateSession {
                name: "Studio 3B".to_string(),
                start: 1,
                end: 60,
                pin: None,
                collect_names: false,
            }),
            "{\"type\":\"create_session\",\"name\":\"Studio 3B\",\"start\":1,\"end\":60,\"pin\":null}"
        );
        // A missing pin is indistinguishable from an explicit null, and a
        // missing collect_names means the session collects no names.
        assert_eq!(
            serde_json::from_str::<LobbyClientMsg>(
                "{\"type\":\"create_session\",\"name\":\"A\",\"start\":1,\"end\":2}"
            )
            .ok(),
            Some(LobbyClientMsg::CreateSession {
                name: "A".to_string(),
                start: 1,
                end: 2,
                pin: None,
                collect_names: false,
            })
        );
        let collecting = LobbyClientMsg::CreateSession {
            name: "A".to_string(),
            start: 1,
            end: 2,
            pin: None,
            collect_names: true,
        };
        assert_eq!(
            json(&collecting),
            "{\"type\":\"create_session\",\"name\":\"A\",\"start\":1,\"end\":2,\"pin\":null,\"collect_names\":true}"
        );
        assert_eq!(
            serde_json::from_str::<LobbyClientMsg>(&json(&collecting)).ok(),
            Some(collecting)
        );
        assert_eq!(json(&LobbyClientMsg::Ping), "{\"type\":\"ping\"}");
        assert_eq!(json(&LobbyServerMsg::Pong), "{\"type\":\"pong\"}");
        assert_eq!(
            json(&LobbyServerMsg::Error {
                code: "invalid_pin".to_string(),
                message: "wrong PIN".to_string(),
            }),
            "{\"type\":\"error\",\"code\":\"invalid_pin\",\"message\":\"wrong PIN\"}"
        );
    }

    #[test]
    fn lobby_snapshot_omits_absent_optional_fields() {
        let text = json(&LobbyServerMsg::LobbySnapshot {
            sessions: vec![summary()],
            server_info: server_info(),
        });
        assert!(text.starts_with("{\"type\":\"lobby_snapshot\",\"sessions\":[{"));
        assert!(text.contains("\"range\":{\"start\":1,\"end\":60}"));
        assert!(text.contains("\"counts\":{\"green\":58,\"orange\":1,\"red\":1}"));
        assert!(
            !text.contains("\"firmware\""),
            "absent firmware must be omitted: {text}"
        );
        assert!(
            !text.contains("\"commit\""),
            "absent commit must be omitted: {text}"
        );
        assert!(text.contains("\"proto_version\":1"));
        assert!(text.contains("\"pin_required\":false"));
        // And it parses back into the same value.
        assert_eq!(
            serde_json::from_str::<LobbyServerMsg>(&text).ok(),
            Some(LobbyServerMsg::LobbySnapshot {
                sessions: vec![summary()],
                server_info: server_info(),
            })
        );
    }

    #[test]
    fn session_client_messages_match_the_wire_contract() {
        assert_eq!(
            json(&SessionClientMsg::Auth {
                creator_token: "tok".to_string()
            }),
            "{\"type\":\"auth\",\"creator_token\":\"tok\"}"
        );
        assert_eq!(
            json(&SessionClientMsg::Identify {
                name: "Alice".to_string(),
                key: None,
            }),
            "{\"type\":\"identify\",\"name\":\"Alice\"}"
        );
        assert_eq!(
            serde_json::from_str::<SessionClientMsg>(
                "{\"type\":\"identify\",\"name\":\"Alice\",\"key\":\"k-1\"}"
            )
            .ok(),
            Some(SessionClientMsg::Identify {
                name: "Alice".to_string(),
                key: Some("k-1".to_string()),
            })
        );
        assert_eq!(
            json(&SessionClientMsg::SetTable {
                table: 4,
                color: Color::Red
            }),
            "{\"type\":\"set_table\",\"table\":4,\"color\":\"red\"}"
        );
        assert_eq!(
            json(&SessionClientMsg::StationReport {
                table: 0,
                color: Color::Orange
            }),
            "{\"type\":\"station_report\",\"table\":0,\"color\":\"orange\"}"
        );
        assert_eq!(
            json(&SessionClientMsg::ResetAll),
            "{\"type\":\"reset_all\"}"
        );
        assert_eq!(
            json(&SessionClientMsg::EndSession),
            "{\"type\":\"end_session\"}"
        );
        // Optional serial-status fields default to absent on parse.
        assert_eq!(
            serde_json::from_str::<SessionClientMsg>(
                "{\"type\":\"serial_status\",\"connected\":true}"
            )
            .ok(),
            Some(SessionClientMsg::SerialStatus {
                connected: true,
                port_label: None,
                firmware: None,
            })
        );
        assert_eq!(
            serde_json::from_str::<SessionClientMsg>(
                "{\"type\":\"set_table_range\",\"start\":5,\"end\":9}"
            )
            .ok(),
            Some(SessionClientMsg::SetTableRange { start: 5, end: 9 })
        );
        assert_eq!(
            serde_json::from_str::<SessionClientMsg>(
                "{\"type\":\"rename_session\",\"name\":\"New\"}"
            )
            .ok(),
            Some(SessionClientMsg::RenameSession {
                name: "New".to_string()
            })
        );
        assert_eq!(
            json(&SessionClientMsg::SetTapMode {
                tap_mode: TapMode::Cycle,
                locked: true,
            }),
            "{\"type\":\"set_tap_mode\",\"tap_mode\":\"cycle\",\"locked\":true}"
        );
        assert_eq!(
            serde_json::from_str::<SessionClientMsg>(
                "{\"type\":\"set_tap_mode\",\"tap_mode\":\"green\",\"locked\":false}"
            )
            .ok(),
            Some(SessionClientMsg::SetTapMode {
                tap_mode: TapMode::Green,
                locked: false,
            })
        );
        assert_eq!(
            json(&SessionClientMsg::SetBeepMode {
                enabled: true,
                locked: true,
            }),
            "{\"type\":\"set_beep_mode\",\"enabled\":true,\"locked\":true}"
        );
        assert_eq!(
            serde_json::from_str::<SessionClientMsg>(
                "{\"type\":\"set_beep_mode\",\"enabled\":false,\"locked\":false}"
            )
            .ok(),
            Some(SessionClientMsg::SetBeepMode {
                enabled: false,
                locked: false,
            })
        );
    }

    #[test]
    fn session_snapshot_skips_empty_unlisted_and_absent_fields() {
        let snapshot = SessionServerMsg::Snapshot {
            session_id: "sess-1".to_string(),
            name: "Studio 3B".to_string(),
            range: TableRange { start: 1, end: 2 },
            tables: vec![
                TableEntry {
                    table: 1,
                    color: Color::Green,
                    red_since_ms: None,
                },
                TableEntry {
                    table: 2,
                    color: Color::Red,
                    red_since_ms: Some(1_764_086_415_500),
                },
            ],
            counts: ColorCounts {
                green: 1,
                orange: 0,
                red: 1,
            },
            receiver_connected: false,
            port_label: None,
            firmware: Some(FirmwareInfo::legacy()),
            viewer_count: 2,
            tap_mode: TapMode::Green,
            tap_mode_locked: false,
            beeps_enabled: false,
            beeps_locked: false,
            collect_names: false,
            is_creator: false,
            unlisted_tables: Vec::new(),
            server_now_ms: 1_764_086_420_000,
            server_info: Box::new(server_info()),
        };
        let text = json(&snapshot);
        assert!(text.starts_with("{\"type\":\"snapshot\""), "{text}");
        assert!(text.contains("\"red_since_ms\":1764086415500"), "{text}");
        assert_eq!(text.matches("\"red_since_ms\"").count(), 1, "{text}");
        assert!(!text.contains("\"port_label\""), "{text}");
        assert!(!text.contains("\"unlisted_tables\""), "{text}");
        assert!(text.contains("\"collect_names\":false"), "{text}");
        assert!(
            text.contains("\"firmware\":{\"kind\":\"legacy\"}"),
            "{text}"
        );
        assert_eq!(
            serde_json::from_str::<SessionServerMsg>(&text).ok(),
            Some(snapshot.clone())
        );
        // A snapshot from before viewer names existed still parses.
        let older = text.replace("\"collect_names\":false,", "");
        assert_ne!(older, text);
        assert_eq!(
            serde_json::from_str::<SessionServerMsg>(&older).ok(),
            Some(snapshot)
        );
    }

    #[test]
    fn session_server_updates_and_creator_messages_round_trip() {
        let update = SessionServerMsg::TableUpdate {
            table: 12,
            color: Color::Red,
            red_since_ms: Some(42),
            counts: ColorCounts {
                green: 59,
                orange: 0,
                red: 1,
            },
        };
        assert_eq!(
            json(&update),
            "{\"type\":\"table_update\",\"table\":12,\"color\":\"red\",\"red_since_ms\":42,\"counts\":{\"green\":59,\"orange\":0,\"red\":1}}"
        );

        let command = SessionServerMsg::SerialCommand {
            command: SerialCommand::SetTable {
                table: 12,
                color: Color::Green,
            },
        };
        assert_eq!(
            json(&command),
            "{\"type\":\"serial_command\",\"command\":{\"cmd\":\"set_table\",\"table\":12,\"color\":\"green\"}}"
        );
        assert_eq!(
            serde_json::from_str::<SessionServerMsg>(&json(&command)).ok(),
            Some(command)
        );

        let unlisted = SessionServerMsg::UnlistedTables {
            tables: vec![UnlistedTable {
                table: 0,
                color: Color::Red,
                last_seen_ms: 99,
            }],
        };
        assert_eq!(
            json(&unlisted),
            "{\"type\":\"unlisted_tables\",\"tables\":[{\"table\":0,\"color\":\"red\",\"last_seen_ms\":99}]}"
        );

        assert_eq!(
            json(&SessionServerMsg::AuthOk {
                is_creator: true,
                unlisted_tables: Vec::new(),
            }),
            "{\"type\":\"auth_ok\",\"is_creator\":true}"
        );
        assert_eq!(
            json(&SessionServerMsg::SessionEnded {
                reason: EndReason::GraceTimeout,
                log_available: true,
            }),
            "{\"type\":\"session_ended\",\"reason\":\"grace_timeout\",\"log_available\":true}"
        );
        assert_eq!(
            json(&SessionServerMsg::ReceiverStatus {
                connected: false,
                port_label: None,
                firmware: None,
            }),
            "{\"type\":\"receiver_status\",\"connected\":false}"
        );
    }

    #[test]
    fn unknown_message_types_and_colors_fail_to_parse() {
        assert!(serde_json::from_str::<SessionClientMsg>("{\"type\":\"explode\"}").is_err());
        assert!(serde_json::from_str::<LobbyServerMsg>("{\"type\":\"snapshot\"}").is_err());
        assert!(serde_json::from_str::<SessionClientMsg>(
            "{\"type\":\"set_table\",\"table\":1,\"color\":\"blue\"}"
        )
        .is_err());
        assert!(serde_json::from_str::<SessionClientMsg>("not json").is_err());
    }
}
