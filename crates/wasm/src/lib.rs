// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! Browser WebAssembly bridge for trafficlights-webapp.
//!
//! A thin `wasm-bindgen` wrapper over `trafficlights-proto`, loaded lazily
//! by the creator's browser only, so viewers never download it. Build it
//! with `scripts/build-wasm.sh`; the generated glue lands in
//! `web/src/wasm/`.
//!
//! The JavaScript surface is deliberately small:
//!
//! - [`proto_version`], [`microbit_usb_vid`], and [`serial_baud_rate`]
//!   expose the shared constants (the Web Serial picker filters on the
//!   vendor ID and opens the port at the baud rate);
//! - [`format_wake_up`] and [`format_probe_firmware`] hand back the exact
//!   bytes to write after opening the port (decode the returned string
//!   with a `TextEncoder`, like every line this module returns);
//! - [`SerialBridge`] owns one serial conversation: feed it raw
//!   `Uint8Array` read chunks, get back deduplicated protocol events as
//!   plain JavaScript objects, and ask it to format teacher commands so
//!   the deduplication cache always matches what was sent (the RF
//!   packet-loss safeguard in `trafficlights_proto::SerialSession`).
//!
//! Every behavior lives in a pure `*_inner` helper next to its
//! `wasm-bindgen` export, so `cargo test -p trafficlights-wasm` verifies
//! the bridge on a native target without a browser runtime.

use serde::Serialize;
use trafficlights_proto::{
    format_command, Color, ProtoError, SerialCommand, SerialEvent, SerialSession,
};
use wasm_bindgen::prelude::{wasm_bindgen, JsValue};

/// The protocol version this module was built against. The frontend can
/// compare it with the server's `server_info.proto_version` to detect a
/// stale cached module before any message fails to parse.
#[wasm_bindgen]
pub fn proto_version() -> u32 {
    trafficlights_proto::PROTO_VERSION
}

/// The micro:bit USB vendor ID (`0x0D28`) for `navigator.serial`
/// `requestPort` filters.
#[wasm_bindgen]
pub fn microbit_usb_vid() -> u16 {
    trafficlights_proto::MICROBIT_USB_VID
}

/// The serial baud rate (`115200`) to open the receiver port with.
#[wasm_bindgen]
pub fn serial_baud_rate() -> u32 {
    trafficlights_proto::SERIAL_BAUD_RATE
}

/// Format one infallible command, keeping the literal fallback identical
/// to the command's own line. Going through
/// `trafficlights_proto::format_command` keeps the wire bytes defined in
/// exactly one place; `WakeUp`, `ProbeFirmware`, and `ResetAll` cannot
/// fail validation, so the fallback is unreachable by construction.
fn format_infallible(command: SerialCommand, fallback: &str) -> String {
    format_command(command).unwrap_or_else(|_| fallback.to_string())
}

/// The blank wake-up line (`"\n"`) written right after opening the port,
/// matching the original `fastapi_receiver.py` connect behavior.
#[wasm_bindgen]
pub fn format_wake_up() -> String {
    format_infallible(SerialCommand::WakeUp, "\n")
}

/// The new-firmware probe line (`"!get\n"`). Legacy MakeCode receivers
/// ignore it harmlessly and simply never reply.
#[wasm_bindgen]
pub fn format_probe_firmware() -> String {
    format_infallible(SerialCommand::ProbeFirmware, "!get\n")
}

/// One serial conversation with a classroom receiver, for the session
/// creator's browser.
///
/// Construct one bridge per opened serial port. Feed every read chunk to
/// [`SerialBridge::feed_bytes`]; write teacher commands only through
/// [`SerialBridge::format_set_table`] and
/// [`SerialBridge::format_reset_all`], so each command actually sent is
/// recorded in the deduplication cache. That pairing is what makes a
/// lost radio packet self-correct on the station's next heartbeat (see
/// `trafficlights_proto::SerialSession`).
#[wasm_bindgen]
pub struct SerialBridge {
    session: SerialSession,
}

/// Pure bridge behavior, shared by the `wasm-bindgen` exports below and
/// the native unit tests.
impl SerialBridge {
    /// Feed one raw read chunk; returns the events worth forwarding to
    /// the server, in arrival order. Unchanged station heartbeats and
    /// `RT` echoes are already suppressed.
    pub fn feed_bytes_inner(&mut self, bytes: &[u8]) -> Vec<SerialEvent> {
        self.session.feed_bytes(bytes)
    }

    /// Validate and format a set-one-table command from a color name
    /// (`"green"`, `"orange"`, `"red"`, ASCII case-insensitive), record
    /// it as sent, and return the exact serial line to write.
    ///
    /// On any validation failure nothing is recorded, so a rejected
    /// command can never poison the deduplication cache.
    pub fn format_set_table_inner(
        &mut self,
        table: u16,
        color: &str,
    ) -> Result<String, ProtoError> {
        let color = Color::from_name(color)?;
        let line = format_command(SerialCommand::SetTable { table, color })?;
        self.session.record_set_table(table, color);
        Ok(line)
    }

    /// Record a reset-all broadcast as sent and return its serial line
    /// (`"T,-1,0\n"`). Stations that missed the broadcast re-report
    /// their unchanged color on the next heartbeat.
    pub fn format_reset_all_inner(&mut self) -> String {
        self.session.record_reset_all();
        format_infallible(SerialCommand::ResetAll, "T,-1,0\n")
    }
}

#[wasm_bindgen]
impl SerialBridge {
    /// A bridge with an empty line buffer and deduplication cache.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            session: SerialSession::new(),
        }
    }

    /// Feed one `Uint8Array` read chunk from the serial port.
    ///
    /// Returns a JavaScript array of plain event objects (never `Map`
    /// instances), for example
    /// `[{type: "station_status", table: 5, color: "red", relayed: false}]`.
    /// Firmware detections arrive as
    /// `{type: "firmware_detected", info: {kind: "new", mode: "receiver",
    /// version: "draft-1"}}` with absent optional fields omitted, and
    /// control rejections as `{type: "control_error", reason: "bad id"}`.
    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Result<JsValue, JsValue> {
        let events = self.feed_bytes_inner(bytes);
        // json_compatible sets serialize_maps_as_objects(true) (plain
        // objects, never JS Map) and renders any missing value as null
        // instead of undefined, so JSON.stringify on the result is
        // lossless for the WebSocket hop to the server.
        let serializer = serde_wasm_bindgen::Serializer::json_compatible();
        events
            .serialize(&serializer)
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Format `T,<table>,<color>\n` from a color name and record it as
    /// sent. Throws a string error for an unknown color name or a table
    /// outside `1..=9999`; nothing is recorded in that case.
    pub fn format_set_table(&mut self, table: u16, color: &str) -> Result<String, JsValue> {
        self.format_set_table_inner(table, color)
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Record a reset-all broadcast as sent and return `"T,-1,0\n"`.
    pub fn format_reset_all(&mut self) -> String {
        self.format_reset_all_inner()
    }

    /// Forget every deduplicated station color. Call on serial reconnect
    /// so the first heartbeat of each station is forwarded again and the
    /// dashboard re-syncs from scratch.
    pub fn reset_dedup(&mut self) {
        self.session.reset_dedup();
    }
}

impl Default for SerialBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_probe_firmware, format_wake_up, microbit_usb_vid, proto_version, serial_baud_rate,
        SerialBridge,
    };
    use trafficlights_proto::{
        Color, FirmwareInfo, FirmwareKind, FirmwareMode, ProtoError, SerialEvent,
    };

    fn status(table: u16, color: Color, relayed: bool) -> SerialEvent {
        SerialEvent::StationStatus {
            table,
            color,
            relayed,
        }
    }

    #[test]
    fn exports_the_shared_constants_and_connect_lines() {
        assert_eq!(proto_version(), 1);
        assert_eq!(microbit_usb_vid(), 0x0D28);
        assert_eq!(serial_baud_rate(), 115_200);
        assert_eq!(format_wake_up(), "\n");
        assert_eq!(format_probe_firmware(), "!get\n");
    }

    #[test]
    fn bridge_decodes_split_chunks_and_dedupes_heartbeats() {
        let mut bridge = SerialBridge::new();
        assert!(bridge.feed_bytes_inner(b"S,5,").is_empty());
        assert_eq!(
            bridge.feed_bytes_inner(b"2\r"),
            [status(5, Color::Red, false)]
        );
        assert!(bridge.feed_bytes_inner(b"\nS,5,2\nRT,5,2\n").is_empty());
        assert_eq!(
            bridge.feed_bytes_inner(b"RS,5,0\n"),
            [status(5, Color::Green, true)]
        );
    }

    #[test]
    fn bridge_surfaces_firmware_detection_and_control_errors() {
        let mut bridge = SerialBridge::default();
        assert_eq!(
            bridge.feed_bytes_inner(b"!ok mode=receiver fw=draft-1\n!err bad mode\n"),
            [
                SerialEvent::FirmwareDetected {
                    info: FirmwareInfo {
                        kind: FirmwareKind::New,
                        mode: Some(FirmwareMode::Receiver),
                        version: Some("draft-1".to_string()),
                        station_id: None,
                        station_color: None,
                    }
                },
                SerialEvent::ControlError {
                    reason: "bad mode".to_string()
                },
            ]
        );
    }

    #[test]
    fn format_set_table_writes_and_records_the_command() {
        let mut bridge = SerialBridge::new();
        assert_eq!(
            bridge.feed_bytes_inner(b"S,5,0\n"),
            [status(5, Color::Green, false)]
        );
        assert_eq!(
            bridge.format_set_table_inner(5, "red"),
            Ok("T,5,2\n".to_string())
        );
        // The station heard the command: its confirming red heartbeat is
        // deduplicated against what the bridge recorded as sent.
        assert!(bridge.feed_bytes_inner(b"S,5,2\n").is_empty());
    }

    #[test]
    fn format_set_table_exposes_a_lost_radio_packet() {
        let mut bridge = SerialBridge::new();
        assert_eq!(
            bridge.feed_bytes_inner(b"S,5,2\n"),
            [status(5, Color::Red, false)]
        );
        assert_eq!(
            bridge.format_set_table_inner(5, "GREEN"),
            Ok("T,5,0\n".to_string())
        );
        // The command never arrived: the station is still red, differs
        // from the recorded intent, and re-reports on its next heartbeat.
        assert_eq!(
            bridge.feed_bytes_inner(b"S,5,2\n"),
            [status(5, Color::Red, false)]
        );
    }

    #[test]
    fn rejected_commands_record_nothing() {
        let mut bridge = SerialBridge::new();
        assert_eq!(
            bridge.feed_bytes_inner(b"S,5,0\n"),
            [status(5, Color::Green, false)]
        );
        assert_eq!(
            bridge.format_set_table_inner(5, "blue"),
            Err(ProtoError::InvalidColor {
                value: "blue".to_string()
            })
        );
        assert_eq!(
            bridge.format_set_table_inner(0, "red"),
            Err(ProtoError::InvalidTableId { table: 0 })
        );
        assert_eq!(
            bridge.format_set_table_inner(10_000, "red"),
            Err(ProtoError::InvalidTableId { table: 10_000 })
        );
        // The cache still holds the green the station actually reported:
        // its repeat heartbeat stays deduplicated, a change still flows.
        assert!(bridge.feed_bytes_inner(b"S,5,0\n").is_empty());
        assert_eq!(
            bridge.feed_bytes_inner(b"S,5,1\n"),
            [status(5, Color::Orange, false)]
        );
    }

    #[test]
    fn format_reset_all_records_green_for_every_known_station() {
        let mut bridge = SerialBridge::new();
        assert_eq!(bridge.feed_bytes_inner(b"S,1,2\nS,2,1\n").len(), 2);
        assert_eq!(bridge.format_reset_all_inner(), "T,-1,0\n");
        // Table 1 accepted the reset; table 2 missed the broadcast and
        // re-reports its orange on the next heartbeat.
        assert_eq!(
            bridge.feed_bytes_inner(b"S,1,0\nS,2,1\n"),
            [status(2, Color::Orange, false)]
        );
    }

    #[test]
    fn reset_dedup_forwards_the_first_heartbeat_after_reconnect() {
        let mut bridge = SerialBridge::new();
        assert_eq!(
            bridge.feed_bytes_inner(b"S,9,2\n"),
            [status(9, Color::Red, false)]
        );
        bridge.reset_dedup();
        assert_eq!(
            bridge.feed_bytes_inner(b"S,9,2\n"),
            [status(9, Color::Red, false)]
        );
    }
}
