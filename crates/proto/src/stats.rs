// SPDX-License-Identifier: AGPL-3.0-or-later
//! Classroom statistics events: one JSON object per line in the
//! append-only session `.jsonl` log (`schema_version: 1`).
//!
//! The on-disk format is the contract documented in "Classroom statistics
//! JSONL schema" in the knowledge base, byte-for-byte, field names,
//! values, and field order included — teachers load these files straight
//! into pandas, DuckDB, or R. Every line carries `event`, a UTC epoch
//! `ts_ms`, and `elapsed_ms` since the `session_start` at the top of the
//! file; the server (Slice S5) is responsible for monotonic timestamps
//! and transition-only logging, this module owns the shapes.
//!
//! Two details keep the files byte-identical to the documented
//! examples: the internally tagged enum writes `event` first, and
//! [`JsonlEvent::StateChange`] keeps `prev: null` for a first-heard
//! out-of-range table (an omitted key would be a different schema),
//! while an absent `firmware` object is omitted.

use serde::{Deserialize, Serialize};

use crate::wire::{Color, FirmwareInfo, TableRange};

/// The schema version written into every `session_start` line. Bump it
/// (and the schema document) before changing any field name or meaning.
pub const STATS_SCHEMA_VERSION: u32 = 1;

/// What caused one table's color change. Lets offline analysis tell
/// student-initiated updates apart from teacher or TA interventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSource {
    /// Reported by a student station over radio and serial.
    StudentRadio,
    /// A table tile was clicked in the web application.
    TeacherClick,
    /// The creator's Reset All To Green broadcast.
    TeacherResetAll,
    /// A table-range change reset the table's state.
    RangeReset,
}

/// Why a session, and with it its statistics file, ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// The creator clicked End Session.
    Manual,
    /// The creator stayed disconnected past the grace period.
    GraceTimeout,
    /// The server received `SIGTERM` or `SIGINT`.
    Shutdown,
}

/// One line of a session statistics `.jsonl` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum JsonlEvent {
    /// Line 1 of every file: schema version and session metadata.
    SessionStart {
        /// Always [`STATS_SCHEMA_VERSION`] (`1`) for this format.
        schema_version: u32,
        /// UTC epoch milliseconds when the session started.
        ts_ms: i64,
        /// Milliseconds since the session start; `0` on this line.
        elapsed_ms: u64,
        /// The session identifier.
        session_id: String,
        /// The human-readable session name.
        name: String,
        /// The initial inclusive table range.
        range: TableRange,
    },
    /// The creator's serial bridge connected or disconnected, so plots
    /// can shade the intervals where radio reports could not arrive.
    ReceiverStatus {
        /// UTC epoch milliseconds of the change.
        ts_ms: i64,
        /// Milliseconds since the session start.
        elapsed_ms: u64,
        /// Whether the serial bridge is now connected.
        connected: bool,
        /// Attached receiver firmware identity, when known; the key is
        /// omitted while unknown. Note the station-mode detail fields
        /// (`station_id`, `station_color`) ride along automatically
        /// whenever a fuller [`FirmwareInfo`] is logged.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        firmware: Option<FirmwareInfo>,
    },
    /// One table actually changed color. Repeated heartbeats never
    /// reach the log; only transitions (and explicit resets) do.
    StateChange {
        /// UTC epoch milliseconds of the change.
        ts_ms: i64,
        /// Milliseconds since the session start.
        elapsed_ms: u64,
        /// The table that changed; `0` is an unset or misnumbered station.
        table: u16,
        /// Previous color, or `null` when an out-of-range table is heard
        /// for the first time (in-range tables start at green). Always
        /// present in the JSON, per the documented schema.
        prev: Option<Color>,
        /// The new color.
        next: Color,
        /// What caused this transition.
        source: ChangeSource,
        /// Whether the table was inside the active session range.
        in_range: bool,
    },
    /// The creator changed the displayed table range; explains jumps in
    /// the denominator of average-state plots.
    RangeChange {
        /// UTC epoch milliseconds of the change.
        ts_ms: i64,
        /// Milliseconds since the session start.
        elapsed_ms: u64,
        /// The range in effect before the change.
        prev_range: TableRange,
        /// The range in effect after the change.
        next_range: TableRange,
    },
    /// The number of connected viewer clients (excluding the creator)
    /// changed, for correlating TA staffing with wait times.
    ViewerCount {
        /// UTC epoch milliseconds of the change.
        ts_ms: i64,
        /// Milliseconds since the session start.
        elapsed_ms: u64,
        /// Viewer count after the join or leave.
        viewers: u32,
    },
    /// The clean end marker of the file, for duration calculations.
    SessionEnd {
        /// UTC epoch milliseconds when the session ended.
        ts_ms: i64,
        /// Milliseconds since the session start.
        elapsed_ms: u64,
        /// Why the session closed.
        reason: EndReason,
    },
}

impl JsonlEvent {
    /// Serialize this event as one single-line JSON object, with no
    /// trailing newline. The caller writing the `.jsonl` file appends
    /// the `\n` line terminator.
    pub fn to_jsonl_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{ChangeSource, EndReason, JsonlEvent, STATS_SCHEMA_VERSION};
    use crate::wire::{Color, FirmwareInfo, FirmwareKind, FirmwareMode, TableRange};

    fn line(event: &JsonlEvent) -> String {
        event.to_jsonl_line().unwrap_or_default()
    }

    #[test]
    fn schema_version_is_one() {
        assert_eq!(STATS_SCHEMA_VERSION, 1);
    }

    // The six examples below are copied from the "Classroom statistics
    // JSONL schema" knowledge-base document and must match byte-for-byte.

    #[test]
    fn session_start_matches_the_documented_example() {
        let event = JsonlEvent::SessionStart {
            schema_version: STATS_SCHEMA_VERSION,
            ts_ms: 1_764_086_400_000,
            elapsed_ms: 0,
            session_id: "0195f1a2-3b4c-7d8e-9f0a-1b2c3d4e5f60".to_string(),
            name: "Studio 3B \u{2014} Week 4".to_string(),
            range: TableRange::default_classroom(),
        };
        assert_eq!(
            line(&event),
            "{\"event\":\"session_start\",\"schema_version\":1,\"ts_ms\":1764086400000,\"elapsed_ms\":0,\"session_id\":\"0195f1a2-3b4c-7d8e-9f0a-1b2c3d4e5f60\",\"name\":\"Studio 3B \u{2014} Week 4\",\"range\":{\"start\":1,\"end\":60}}"
        );
    }

    #[test]
    fn receiver_status_matches_the_documented_example() {
        let event = JsonlEvent::ReceiverStatus {
            ts_ms: 1_764_086_401_200,
            elapsed_ms: 1200,
            connected: true,
            firmware: Some(FirmwareInfo {
                kind: FirmwareKind::New,
                mode: Some(FirmwareMode::Receiver),
                version: Some("draft-1".to_string()),
                station_id: None,
                station_color: None,
            }),
        };
        assert_eq!(
            line(&event),
            "{\"event\":\"receiver_status\",\"ts_ms\":1764086401200,\"elapsed_ms\":1200,\"connected\":true,\"firmware\":{\"kind\":\"new\",\"mode\":\"receiver\",\"version\":\"draft-1\"}}"
        );
        // Unknown firmware omits the key entirely.
        let offline = JsonlEvent::ReceiverStatus {
            ts_ms: 1_764_086_401_200,
            elapsed_ms: 1200,
            connected: false,
            firmware: None,
        };
        assert_eq!(
            line(&offline),
            "{\"event\":\"receiver_status\",\"ts_ms\":1764086401200,\"elapsed_ms\":1200,\"connected\":false}"
        );
    }

    #[test]
    fn state_change_matches_the_documented_example() {
        let event = JsonlEvent::StateChange {
            ts_ms: 1_764_086_415_500,
            elapsed_ms: 15_500,
            table: 12,
            prev: Some(Color::Green),
            next: Color::Red,
            source: ChangeSource::StudentRadio,
            in_range: true,
        };
        assert_eq!(
            line(&event),
            "{\"event\":\"state_change\",\"ts_ms\":1764086415500,\"elapsed_ms\":15500,\"table\":12,\"prev\":\"green\",\"next\":\"red\",\"source\":\"student_radio\",\"in_range\":true}"
        );
        // A first-heard out-of-range table keeps an explicit null prev.
        let first_heard = JsonlEvent::StateChange {
            ts_ms: 1_764_086_415_500,
            elapsed_ms: 15_500,
            table: 0,
            prev: None,
            next: Color::Orange,
            source: ChangeSource::StudentRadio,
            in_range: false,
        };
        assert_eq!(
            line(&first_heard),
            "{\"event\":\"state_change\",\"ts_ms\":1764086415500,\"elapsed_ms\":15500,\"table\":0,\"prev\":null,\"next\":\"orange\",\"source\":\"student_radio\",\"in_range\":false}"
        );
    }

    #[test]
    fn range_change_viewer_count_and_session_end_match_the_document() {
        assert_eq!(
            line(&JsonlEvent::RangeChange {
                ts_ms: 1_764_086_520_000,
                elapsed_ms: 120_000,
                prev_range: TableRange { start: 1, end: 60 },
                next_range: TableRange { start: 1, end: 30 },
            }),
            "{\"event\":\"range_change\",\"ts_ms\":1764086520000,\"elapsed_ms\":120000,\"prev_range\":{\"start\":1,\"end\":60},\"next_range\":{\"start\":1,\"end\":30}}"
        );
        assert_eq!(
            line(&JsonlEvent::ViewerCount {
                ts_ms: 1_764_086_525_000,
                elapsed_ms: 125_000,
                viewers: 5,
            }),
            "{\"event\":\"viewer_count\",\"ts_ms\":1764086525000,\"elapsed_ms\":125000,\"viewers\":5}"
        );
        assert_eq!(
            line(&JsonlEvent::SessionEnd {
                ts_ms: 1_764_090_000_000,
                elapsed_ms: 3_600_000,
                reason: EndReason::Manual,
            }),
            "{\"event\":\"session_end\",\"ts_ms\":1764090000000,\"elapsed_ms\":3600000,\"reason\":\"manual\"}"
        );
    }

    #[test]
    fn enum_values_use_the_documented_spellings() {
        for (source, text) in [
            (ChangeSource::StudentRadio, "\"student_radio\""),
            (ChangeSource::TeacherClick, "\"teacher_click\""),
            (ChangeSource::TeacherResetAll, "\"teacher_reset_all\""),
            (ChangeSource::RangeReset, "\"range_reset\""),
        ] {
            assert_eq!(serde_json::to_string(&source).unwrap_or_default(), text);
            assert_eq!(
                serde_json::from_str::<ChangeSource>(text).ok(),
                Some(source)
            );
        }
        for (reason, text) in [
            (EndReason::Manual, "\"manual\""),
            (EndReason::GraceTimeout, "\"grace_timeout\""),
            (EndReason::Shutdown, "\"shutdown\""),
        ] {
            assert_eq!(serde_json::to_string(&reason).unwrap_or_default(), text);
            assert_eq!(serde_json::from_str::<EndReason>(text).ok(), Some(reason));
        }
    }

    #[test]
    fn every_line_is_one_json_object_without_a_trailing_newline() {
        let events = [
            JsonlEvent::SessionStart {
                schema_version: STATS_SCHEMA_VERSION,
                ts_ms: 1,
                elapsed_ms: 0,
                session_id: "id".to_string(),
                name: "n".to_string(),
                range: TableRange::default_classroom(),
            },
            JsonlEvent::StateChange {
                ts_ms: 2,
                elapsed_ms: 1,
                table: 1,
                prev: Some(Color::Green),
                next: Color::Red,
                source: ChangeSource::TeacherClick,
                in_range: true,
            },
            JsonlEvent::SessionEnd {
                ts_ms: 3,
                elapsed_ms: 2,
                reason: EndReason::Shutdown,
            },
        ];
        for event in &events {
            let text = line(event);
            assert!(!text.contains('\n'), "line must be single: {text}");
            assert!(!text.ends_with('\n'));
            // A full file is exactly the lines joined with LF terminators,
            // and every line parses back to the same event.
            assert_eq!(
                serde_json::from_str::<JsonlEvent>(&text).ok(),
                Some(event.clone())
            );
        }
        assert!(serde_json::from_str::<JsonlEvent>("{\"event\":\"explode\"}").is_err());
    }
}
