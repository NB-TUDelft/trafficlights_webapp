// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! Shared, I/O-free protocol and domain types for trafficlights-webapp.
//!
//! This crate is compiled into both the Axum server (`trafficlights-server`)
//! and the browser WebAssembly module (`trafficlights-wasm`), so the two
//! sides can never disagree about the serial wire format, the WebSocket
//! JSON messages, or the statistics file format. It performs no I/O: every
//! function maps values to values, which keeps the whole contract unit
//! testable on a native target.
//!
//! - [`wire`] — the USB serial protocol with the micro:bit receiver:
//!   domain types ([`Color`], [`TableRange`], [`FirmwareInfo`]), the strict
//!   `T,<table>,<color>` command formatter, legacy and `!`-control line
//!   parsing, [`LineAccumulator`] chunk framing, and the heartbeat
//!   deduplicating [`SerialSession`].
//! - [`ws`] — the `/ws/lobby` and `/ws/sessions/{id}` JSON messages shared
//!   with the TypeScript frontend, plus [`normalize_session_name`].
//! - [`stats`] — the classroom statistics `.jsonl` events
//!   (`schema_version: 1`) the server streams to disk.
//! - [`error`] — the structured [`ProtoError`] returned by the validating
//!   constructors and formatters.
//!
//! The most-used types are re-exported at the crate root; anything else is
//! reachable through its module.

pub mod error;
pub mod stats;
pub mod wire;
pub mod ws;

pub use error::ProtoError;
pub use stats::{ChangeSource, EndReason, JsonlEvent, STATS_SCHEMA_VERSION};
pub use wire::{
    format_command, parse_line, Color, FirmwareInfo, FirmwareKind, FirmwareMode, LineAccumulator,
    SerialCommand, SerialEvent, SerialSession, TableRange, MAX_LINE_BYTES, MAX_STATION_ID,
    MAX_TABLE_ID, MICROBIT_USB_VID, MIN_TABLE_ID, SERIAL_BAUD_RATE,
};
pub use ws::{
    normalize_session_name, normalize_viewer_name, ColorCounts, LobbyClientMsg, LobbyServerMsg,
    ServerBuildInfo, SessionClientMsg, SessionServerMsg, SessionSummary, TableEntry, TapMode,
    UnlistedTable, HOST_ACTOR, MAX_VIEWER_NAME_CHARS,
};

/// Version of the structured event and command protocol shared by the
/// server and the browser module. Bump it when either side must reject
/// the other; it travels in `ws::ServerBuildInfo` so a stale cached
/// frontend or WASM module is detected before messages fail to parse.
/// Optional fields added with `#[serde(default)]` keep both sides
/// compatible and need no bump.
pub const PROTO_VERSION: u32 = 1;

/// `skip_serializing_if` helper: leave a `false` flag out of the JSON, so
/// an optional flag added later keeps older documents byte-identical.
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::{Color, ProtoError, TableRange, PROTO_VERSION, STATS_SCHEMA_VERSION};
    use crate::ws::normalize_session_name;

    #[test]
    fn protocol_version_starts_at_one() {
        assert_eq!(PROTO_VERSION, 1);
        assert_eq!(STATS_SCHEMA_VERSION, 1);
    }

    #[test]
    fn crate_root_reexports_the_shared_vocabulary() {
        // Compiling at all is most of the test; these uses keep every
        // re-export honest if a module is reorganized later.
        assert_eq!(Color::from_name("red"), Ok(Color::Red));
        assert_eq!(TableRange::default().len(), 60);
        assert_eq!(
            normalize_session_name(" Studio 3B "),
            Ok("Studio 3B".to_string())
        );
        assert!(matches!(
            TableRange::new(0, 1),
            Err(ProtoError::InvalidTableRange { .. })
        ));
    }
}
