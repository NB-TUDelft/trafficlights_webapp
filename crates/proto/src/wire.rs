// SPDX-License-Identifier: AGPL-3.0-or-later
//! The USB serial wire protocol spoken with the micro:bit receiver.
//!
//! This module owns everything byte-shaped: the legacy comma protocol
//! (`T,<id>,<color>` teacher commands, `S`/`RS` station reports, `RT`
//! repeater echoes), the new firmware's `!`-prefixed control replies
//! (`!ok ...`, `!err ...`), line framing for arbitrary USB read chunks, and
//! the heartbeat deduplication session used by the creator's browser.
//!
//! Backward compatibility is a safety property here, not a nicety: the old
//! MakeCode student light indexes its color list with the received number
//! and performs no bounds check, so [`format_command`] can only ever emit
//! validated lines (colors strictly `0..=2`, always `\n`-terminated, never
//! containing `0x03`), and [`parse_line`] drops anything malformed instead
//! of guessing. See "Firmware serial interface" in the knowledge base.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::error::ProtoError;

/// Maximum serial line length in bytes, excluding the line ending.
///
/// Mirrors the firmware's `MAX_LINE = 64`: longer lines are silently
/// dropped by the micro:bit, and dropped here on the receiving side too.
pub const MAX_LINE_BYTES: usize = 64;

/// USB vendor ID of the micro:bit (NXP ARM mbed DAPLink CDC interface).
/// The browser's Web Serial picker filters on this value.
pub const MICROBIT_USB_VID: u16 = 0x0D28;

/// Serial link speed shared by the old MakeCode setup and the new firmware.
pub const SERIAL_BAUD_RATE: u32 = 115_200;

/// Smallest assignable station/table ID.
pub const MIN_TABLE_ID: u16 = 1;

/// Largest assignable station/table ID. Teacher commands may only target
/// table IDs in `MIN_TABLE_ID..=MAX_TABLE_ID`.
pub const MAX_TABLE_ID: u16 = 9999;

/// Largest station ID a *report* may carry. Reports accept `0..=9999`
/// because an unset station reports ID `0`; the value is kept (and shown
/// to the session creator as an unlisted table) rather than dropped.
pub const MAX_STATION_ID: u16 = 9999;

/// The color of one classroom traffic light.
///
/// Serializes as `"green"`, `"orange"`, or `"red"` in JSON, and as the
/// wire codes `0`, `1`, `2` on the serial/radio link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    /// Everything is fine (wire code `0`).
    Green,
    /// The table has a question (wire code `1`).
    Orange,
    /// The table needs help (wire code `2`).
    Red,
}

impl Color {
    /// The legacy radio/serial code for this color: `0`, `1`, or `2`.
    pub fn as_wire_code(self) -> u8 {
        match self {
            Color::Green => 0,
            Color::Orange => 1,
            Color::Red => 2,
        }
    }

    /// The color for a legacy wire code, or `None` outside `0..=2`.
    pub fn from_wire_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Color::Green),
            1 => Some(Color::Orange),
            2 => Some(Color::Red),
            _ => None,
        }
    }

    /// Parse a color name (`green`, `orange`, `red`).
    ///
    /// Matching ignores ASCII case and surrounding ASCII whitespace, so
    /// values arriving from a browser (`"Red"`, `" red "`) behave the same
    /// as the canonical lowercase names. Anything else is rejected with
    /// [`ProtoError::InvalidColor`] carrying the original input.
    pub fn from_name(name: &str) -> Result<Self, ProtoError> {
        let trimmed = name.trim_ascii();
        if trimmed.eq_ignore_ascii_case("green") {
            Ok(Color::Green)
        } else if trimmed.eq_ignore_ascii_case("orange") {
            Ok(Color::Orange)
        } else if trimmed.eq_ignore_ascii_case("red") {
            Ok(Color::Red)
        } else {
            Err(ProtoError::InvalidColor {
                value: name.to_string(),
            })
        }
    }

    /// The canonical lowercase name, matching the JSON representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Color::Green => "green",
            Color::Orange => "orange",
            Color::Red => "red",
        }
    }

    /// The next color when a table tile is clicked:
    /// green, then orange, then red, then green again.
    pub fn next_cycle(self) -> Self {
        match self {
            Color::Green => Color::Orange,
            Color::Orange => Color::Red,
            Color::Red => Color::Green,
        }
    }
}

/// An inclusive range of table IDs shown in one session, `start..=end`.
///
/// Values built with [`TableRange::new`] always satisfy
/// `1 <= start <= end <= 9999`. The fields are public because the range is
/// a plain wire DTO (`{"start": 1, "end": 60}`); deserialization therefore
/// cannot enforce the invariant, and any consumer that receives a range
/// from the network must re-validate it through `TableRange::new` before
/// trusting it. The query methods below stay total (never panic) even on
/// such an unvalidated value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRange {
    /// First table ID in the range.
    pub start: u16,
    /// Last table ID in the range.
    pub end: u16,
}

impl TableRange {
    /// Build a validated range; requires `1 <= start <= end <= 9999`.
    pub fn new(start: u16, end: u16) -> Result<Self, ProtoError> {
        if start < MIN_TABLE_ID || end > MAX_TABLE_ID || start > end {
            return Err(ProtoError::InvalidTableRange { start, end });
        }
        Ok(Self { start, end })
    }

    /// The range the original classroom setup used, tables `1..=60`.
    pub fn default_classroom() -> Self {
        Self { start: 1, end: 60 }
    }

    /// Whether `table` lies inside this range.
    pub fn contains(self, table: u16) -> bool {
        (self.start..=self.end).contains(&table)
    }

    /// Number of tables in the range. Always at least 1 for a range built
    /// by [`TableRange::new`]; an unvalidated deserialized range with
    /// `start > end` reports 0 instead of panicking.
    pub fn len(self) -> usize {
        if self.start > self.end {
            0
        } else {
            usize::from(self.end - self.start) + 1
        }
    }

    /// Whether the range holds no tables. Always `false` for a range built
    /// by [`TableRange::new`]; present so `len` has its conventional pair.
    pub fn is_empty(self) -> bool {
        self.start > self.end
    }

    /// Iterate the table IDs from `start` to `end`, inclusive.
    pub fn iter(self) -> RangeInclusive<u16> {
        self.start..=self.end
    }
}

impl Default for TableRange {
    fn default() -> Self {
        Self::default_classroom()
    }
}

/// Which receiver firmware the creator's browser is talking to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FirmwareKind {
    /// The new `trafficlights_firmware`: it answered the `!get` probe.
    New,
    /// The old MakeCode teacher receiver: it cannot answer `!get`, so the
    /// browser assumes legacy operation when the probe times out.
    Legacy,
}

/// The running mode reported by the new firmware in its `!ok` reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FirmwareMode {
    /// Teacher receiver mode (the mode a session needs).
    Receiver,
    /// Student station mode (plugged in by mistake; the UI warns).
    Station,
}

/// What is known about the micro:bit attached to the creator's browser.
///
/// For the new firmware this comes from the `!ok` reply to the `!get`
/// probe; for a legacy MakeCode receiver only `kind` is known. Absent
/// fields are omitted from JSON rather than serialized as `null`, so the
/// statistics log stays byte-identical to the documented schema examples
/// (`{"kind": "new", "mode": "receiver", "version": "draft-1"}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareInfo {
    /// New firmware (answered `!get`) or legacy MakeCode receiver.
    pub kind: FirmwareKind,
    /// Running mode, when the firmware reported one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<FirmwareMode>,
    /// Firmware version string (the `fw=` value), when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Station ID, when a board in station mode reported one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station_id: Option<u16>,
    /// Current color, when a board in station mode reported one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station_color: Option<Color>,
}

impl FirmwareInfo {
    /// The assumed identity of a receiver that never answered `!get`:
    /// an old MakeCode receiver, about which nothing else is known.
    pub fn legacy() -> Self {
        Self {
            kind: FirmwareKind::Legacy,
            mode: None,
            version: None,
            station_id: None,
            station_color: None,
        }
    }
}

/// One meaningful line received from the receiver over USB serial.
///
/// Serialized with a `type` tag for the browser bridge: the WASM module
/// hands these to JavaScript as plain objects, and the creator's frontend
/// forwards station reports to the server over its WebSocket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SerialEvent {
    /// A station reported its color (`S,<id>,<color>`, or the relayed
    /// `RS,<id>,<color>`). `table` may be `0` for an unset station.
    StationStatus {
        /// Station/table ID, `0..=9999`.
        table: u16,
        /// The reported color.
        color: Color,
        /// `true` when an old MakeCode repeater relayed the report (`RS`).
        relayed: bool,
    },
    /// The attached board answered a `!`-control probe with `!ok ...`,
    /// identifying itself as the new firmware.
    FirmwareDetected {
        /// What the board reported about itself.
        info: FirmwareInfo,
    },
    /// The attached board rejected a `!`-control line (`!err <reason>`).
    ControlError {
        /// The firmware's reason text, verbatim (for example `bad id`).
        reason: String,
    },
    /// An `RT,<id>,<color>` repeater echo of a teacher command. Returned
    /// by [`parse_line`] so callers can tell "known line, deliberately
    /// ignored" apart from noise; [`SerialSession::feed_bytes`] filters it
    /// out and never forwards it.
    IgnoredEcho,
}

/// One command the web application may send to the receiver.
///
/// Serialized with a `cmd` tag because the server forwards teacher intent
/// to the creator's browser in this shape (see `ws::SessionServerMsg`);
/// the browser then turns it into bytes with [`format_command`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum SerialCommand {
    /// Set one station's color (`T,<table>,<color>`).
    SetTable {
        /// Target station/table ID; must be in `1..=9999`.
        table: u16,
        /// The color to set.
        color: Color,
    },
    /// Broadcast reset of every station to green (`T,-1,0`).
    ResetAll,
    /// The blank wake-up line sent right after opening the serial port,
    /// matching the original `fastapi_receiver.py` behavior.
    WakeUp,
    /// Probe for the new firmware (`!get`). Safe on legacy receivers:
    /// old stations ignore lines whose first field is not `T`/`RT`/`S`,
    /// and simply never reply.
    ProbeFirmware,
}

/// Format a command as the exact serial line to write to the receiver.
///
/// Safety guarantees, on every return path:
///
/// - the line ends with `'\n'` (the old MakeCode receiver only triggers
///   on a newline delimiter);
/// - a `SetTable` color code is strictly `0`, `1`, or `2`, because the old
///   MakeCode student light indexes `colorList[colorIndex]` with no bounds
///   check — a malformed `T,...` line must be impossible to produce here;
/// - the line never contains `0x03` (Ctrl-C), which the new firmware
///   deliberately treats as "stop and enter the REPL";
/// - the line is at most [`MAX_LINE_BYTES`] bytes long.
///
/// The only failure is a `SetTable` table outside `1..=9999`
/// ([`ProtoError::InvalidTableId`]); table `0` is a valid *report* value
/// (unset station) but never a valid command target.
pub fn format_command(command: SerialCommand) -> Result<String, ProtoError> {
    match command {
        SerialCommand::SetTable { table, color } => {
            if !(MIN_TABLE_ID..=MAX_TABLE_ID).contains(&table) {
                return Err(ProtoError::InvalidTableId { table });
            }
            Ok(format!("T,{table},{}\n", color.as_wire_code()))
        }
        SerialCommand::ResetAll => Ok("T,-1,0\n".to_string()),
        SerialCommand::WakeUp => Ok("\n".to_string()),
        SerialCommand::ProbeFirmware => Ok("!get\n".to_string()),
    }
}

/// Incremental line framing for a USB serial byte stream.
///
/// USB reads arrive in arbitrary chunk sizes: a line, its ending, or even
/// one multi-byte UTF-8 character may be split across reads. Feed every
/// chunk to [`LineAccumulator::feed`]; it returns each line completed by
/// that chunk, without its ending.
///
/// Accepted endings are `\n`, `\r`, and `\r\n` (the firmware accepts all
/// three). A `\r\n` pair split across two chunks still counts as one
/// ending: the `\n` is absorbed, so no empty phantom line appears between
/// the chunks. Lines holding only whitespace are dropped.
///
/// Overflow handling is deliberately strict. When a line grows past
/// [`MAX_LINE_BYTES`] bytes before its ending arrives, every byte of that
/// line is discarded up to the next line ending — the tail of an over-long
/// line must never be mistaken for a fresh, valid line. Lines containing
/// invalid UTF-8 are dropped whole, mirroring the firmware.
#[derive(Debug, Clone, Default)]
pub struct LineAccumulator {
    /// Bytes of the line currently being assembled.
    buffer: Vec<u8>,
    /// `true` while discarding an over-long line up to its ending.
    discarding: bool,
    /// `true` when the previous byte was `\r`, so one directly following
    /// `\n` is absorbed as part of the same `\r\n` ending.
    after_cr: bool,
}

impl LineAccumulator {
    /// A fresh accumulator with no partial line buffered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop any partially buffered line and overflow state, as if newly
    /// constructed. Used when the serial device reconnects, so a fragment
    /// from the old connection cannot glue itself to new bytes.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.discarding = false;
        self.after_cr = false;
    }

    /// Feed one read chunk; returns the lines it completes, in order.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        for &byte in bytes {
            if self.after_cr {
                self.after_cr = false;
                if byte == b'\n' {
                    continue;
                }
            }
            match byte {
                b'\r' => {
                    self.after_cr = true;
                    self.finish_line(&mut lines);
                }
                b'\n' => self.finish_line(&mut lines),
                _ => {
                    if self.discarding {
                        continue;
                    }
                    if self.buffer.len() >= MAX_LINE_BYTES {
                        self.buffer.clear();
                        self.discarding = true;
                        continue;
                    }
                    self.buffer.push(byte);
                }
            }
        }
        lines
    }

    /// Close out the current line at a line ending: emit it unless it is
    /// being discarded for overflow, is not valid UTF-8, or is blank.
    fn finish_line(&mut self, lines: &mut Vec<String>) {
        if self.discarding {
            self.discarding = false;
            self.buffer.clear();
            return;
        }
        let raw = std::mem::take(&mut self.buffer);
        let Ok(line) = String::from_utf8(raw) else {
            return;
        };
        if line.trim().is_empty() {
            return;
        }
        lines.push(line);
    }
}

/// Parse one received serial line into a [`SerialEvent`], or `None` for
/// anything that is not a recognized, well-formed protocol line.
///
/// Radio and serial noise is expected in a classroom, so malformed input
/// is dropped quietly rather than reported as an error. Surrounding ASCII
/// whitespace is trimmed first.
///
/// Recognized lines:
///
/// - `S,<id>,<color>` — a station status report, `relayed: false`;
/// - `RS,<id>,<color>` — a report relayed once by a repeater,
///   `relayed: true`;
/// - `RT,...` — a repeater echo of a teacher command, returned as
///   [`SerialEvent::IgnoredEcho`] for any comma-bearing `RT` line (its
///   remaining fields are never trusted or parsed);
/// - `!ok mode=receiver|station ...` — a new-firmware control reply;
/// - `!err <reason>` — a new-firmware control rejection.
///
/// For the comma protocol, each field is trimmed of ASCII whitespace
/// before parsing, matching how the original `fastapi_receiver.py`
/// tolerated spacing through Python's `int()`. The station ID must be in
/// `0..=9999` (`0` is an unset station and is kept, not dropped) and the
/// color code in `0..=2`; negative numbers, non-numeric fields, wrong
/// field counts, and unknown roles (including our own outgoing `T` lines,
/// should one ever loop back) all yield `None`.
pub fn parse_line(line: &str) -> Option<SerialEvent> {
    let trimmed = line.trim_ascii();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('!') {
        return parse_control_reply(trimmed);
    }
    parse_legacy_line(trimmed)
}

/// Parse a legacy three-field `S`/`RS`/`RT` comma line.
fn parse_legacy_line(line: &str) -> Option<SerialEvent> {
    let parts: Vec<&str> = line.split(',').collect();
    let role = parts.first().copied().unwrap_or_default().trim_ascii();
    if role == "RT" && parts.len() > 1 {
        return Some(SerialEvent::IgnoredEcho);
    }
    if parts.len() != 3 {
        return None;
    }
    let relayed = match role {
        "S" => false,
        "RS" => true,
        _ => return None,
    };
    let table: u16 = parts[1].trim_ascii().parse().ok()?;
    if table > MAX_STATION_ID {
        return None;
    }
    let code: u8 = parts[2].trim_ascii().parse().ok()?;
    let color = Color::from_wire_code(code)?;
    Some(SerialEvent::StationStatus {
        table,
        color,
        relayed,
    })
}

/// Parse a new-firmware `!ok ...` or `!err <reason>` control reply.
///
/// `!ok` carries whitespace-separated `key=value` tokens. Only a reply
/// with a recognized `mode=receiver` or `mode=station` identifies the
/// board and becomes [`SerialEvent::FirmwareDetected`]; replies such as
/// `!ok restarting`, and unrecognized `key=value` pairs (whether from a
/// newer firmware or simply irrelevant here), are ignored so future
/// firmware fields stay forward compatible. Within a recognized reply, a
/// malformed `id=` or `color=` value is treated as absent rather than
/// discarding the identification. A bare `!err` with no reason still
/// surfaces as a control error with an empty reason.
fn parse_control_reply(line: &str) -> Option<SerialEvent> {
    // Exact token match: `!error ...` must not parse as `!err` + "or ...".
    if line == "!err" || line.starts_with("!err ") || line.starts_with("!err\t") {
        let reason = line
            .strip_prefix("!err")
            .unwrap_or_default()
            .trim_ascii()
            .to_string();
        return Some(SerialEvent::ControlError { reason });
    }
    let mut tokens = line.split_ascii_whitespace();
    match tokens.next()? {
        "!ok" => {
            let mut mode: Option<FirmwareMode> = None;
            let mut version: Option<String> = None;
            let mut station_id: Option<u16> = None;
            let mut station_color: Option<Color> = None;
            for token in tokens {
                let Some((key, value)) = token.split_once('=') else {
                    continue;
                };
                match key {
                    "mode" => {
                        mode = match value {
                            "receiver" => Some(FirmwareMode::Receiver),
                            "station" => Some(FirmwareMode::Station),
                            _ => mode,
                        };
                    }
                    "fw" => {
                        if !value.is_empty() {
                            version = Some(value.to_string());
                        }
                    }
                    "id" => {
                        station_id = value
                            .parse::<u16>()
                            .ok()
                            .filter(|id| *id <= MAX_STATION_ID)
                            .or(station_id);
                    }
                    "color" => {
                        station_color = Color::from_name(value).ok().or(station_color);
                    }
                    _ => {}
                }
            }
            let mode = mode?;
            Some(SerialEvent::FirmwareDetected {
                info: FirmwareInfo {
                    kind: FirmwareKind::New,
                    mode: Some(mode),
                    version,
                    station_id,
                    station_color,
                },
            })
        }
        _ => None,
    }
}

/// A live serial conversation in the session creator's browser: line
/// framing plus heartbeat deduplication.
///
/// Student stations re-broadcast their color every 4–6 seconds whether it
/// changed or not. Forwarding every heartbeat would flood the session
/// WebSocket with no-ops, so the session remembers the last color seen
/// (or deliberately set) per station and only forwards changes.
/// [`SerialEvent::FirmwareDetected`] and [`SerialEvent::ControlError`]
/// always pass through, and `RT` echoes never do.
///
/// The deduplication cache doubles as an RF packet-loss safeguard, which
/// is what [`SerialSession::record_set_table`] and
/// [`SerialSession::record_reset_all`] are for: they write the *intended*
/// color into the cache at command time. If the radio command reached the
/// station, its next heartbeat confirms the same color and stays
/// deduplicated; if the packet was lost over the air, the station is
/// still on its old color, so its next heartbeat *differs* from the cache
/// and is forwarded immediately, correcting the dashboard within one
/// heartbeat interval instead of leaving it wrong until the student next
/// presses a button.
#[derive(Debug, Clone, Default)]
pub struct SerialSession {
    accumulator: LineAccumulator,
    last_seen: BTreeMap<u16, Color>,
}

impl SerialSession {
    /// A fresh session with an empty line buffer and deduplication cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one raw read chunk from the serial device; returns the events
    /// worth forwarding, in arrival order.
    ///
    /// Unchanged station heartbeats and `RT` echoes are suppressed; every
    /// actual color change, firmware detection, and control error is
    /// returned.
    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Vec<SerialEvent> {
        let mut events = Vec::new();
        for line in self.accumulator.feed(bytes) {
            let Some(event) = parse_line(&line) else {
                continue;
            };
            match event {
                SerialEvent::IgnoredEcho => {}
                SerialEvent::StationStatus {
                    table,
                    color,
                    relayed,
                } => {
                    if self.last_seen.get(&table) == Some(&color) {
                        continue;
                    }
                    self.last_seen.insert(table, color);
                    events.push(SerialEvent::StationStatus {
                        table,
                        color,
                        relayed,
                    });
                }
                firmware_or_error => events.push(firmware_or_error),
            }
        }
        events
    }

    /// Record that a `T,<table>,<color>` command was just sent, so the
    /// deduplication cache holds the intended color (see the RF
    /// packet-loss safeguard in the type documentation). Call this for
    /// every successfully written set-table command, whether it came from
    /// a local click or from the server relaying another viewer's click.
    pub fn record_set_table(&mut self, table: u16, color: Color) {
        self.last_seen.insert(table, color);
    }

    /// Record that a `T,-1,0` reset-all broadcast was just sent: every
    /// station already in the cache is now expected to be green. Any
    /// station that missed the broadcast and stays orange or red will
    /// differ from the cache on its next heartbeat and re-report at once.
    /// Stations never heard from are not invented here; their first
    /// heartbeat reports normally.
    pub fn record_reset_all(&mut self) {
        for color in self.last_seen.values_mut() {
            *color = Color::Green;
        }
    }

    /// Forget every deduplicated station color. Called on serial
    /// reconnect so each station's first heartbeat after the reconnect is
    /// forwarded and the dashboard re-syncs from scratch. Any partially
    /// buffered line from the previous connection is dropped too, so it
    /// cannot glue itself to the first bytes of the new connection.
    pub fn reset_dedup(&mut self) {
        self.last_seen.clear();
        self.accumulator.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_command, parse_line, Color, FirmwareInfo, FirmwareKind, FirmwareMode,
        LineAccumulator, SerialCommand, SerialEvent, SerialSession, TableRange, MAX_LINE_BYTES,
        MAX_STATION_ID, MAX_TABLE_ID, MICROBIT_USB_VID, MIN_TABLE_ID, SERIAL_BAUD_RATE,
    };
    use crate::error::ProtoError;

    fn status(table: u16, color: Color, relayed: bool) -> SerialEvent {
        SerialEvent::StationStatus {
            table,
            color,
            relayed,
        }
    }

    #[test]
    fn constants_match_the_firmware_contract() {
        assert_eq!(MAX_LINE_BYTES, 64);
        assert_eq!(MICROBIT_USB_VID, 0x0D28);
        assert_eq!(SERIAL_BAUD_RATE, 115_200);
        assert_eq!(MIN_TABLE_ID, 1);
        assert_eq!(MAX_TABLE_ID, 9999);
        assert_eq!(MAX_STATION_ID, 9999);
    }

    #[test]
    fn color_wire_codes_round_trip_and_reject_out_of_range() {
        for (color, code) in [(Color::Green, 0), (Color::Orange, 1), (Color::Red, 2)] {
            assert_eq!(color.as_wire_code(), code);
            assert_eq!(Color::from_wire_code(code), Some(color));
        }
        assert_eq!(Color::from_wire_code(3), None);
        assert_eq!(Color::from_wire_code(u8::MAX), None);
    }

    #[test]
    fn color_names_parse_case_insensitively_with_trimming() {
        assert_eq!(Color::from_name("green"), Ok(Color::Green));
        assert_eq!(Color::from_name("ORANGE"), Ok(Color::Orange));
        assert_eq!(Color::from_name(" Red "), Ok(Color::Red));
        assert_eq!(Color::from_name("\tgReEn\n"), Ok(Color::Green));
        assert_eq!(
            Color::from_name("blue"),
            Err(ProtoError::InvalidColor {
                value: "blue".to_string()
            })
        );
        assert_eq!(
            Color::from_name(""),
            Err(ProtoError::InvalidColor {
                value: String::new()
            })
        );
        assert_eq!(
            Color::from_name("redd"),
            Err(ProtoError::InvalidColor {
                value: "redd".to_string()
            })
        );
    }

    #[test]
    fn color_names_and_cycle_cover_the_whole_ring() {
        assert_eq!(Color::Green.as_str(), "green");
        assert_eq!(Color::Orange.as_str(), "orange");
        assert_eq!(Color::Red.as_str(), "red");
        assert_eq!(Color::Green.next_cycle(), Color::Orange);
        assert_eq!(Color::Orange.next_cycle(), Color::Red);
        assert_eq!(Color::Red.next_cycle(), Color::Green);
        assert_eq!(
            Color::Green.next_cycle().next_cycle().next_cycle(),
            Color::Green
        );
        for color in [Color::Green, Color::Orange, Color::Red] {
            assert_eq!(Color::from_name(color.as_str()), Ok(color));
        }
    }

    #[test]
    fn color_serializes_as_a_lowercase_name() {
        assert_eq!(
            serde_json::to_string(&Color::Orange).unwrap_or_default(),
            "\"orange\""
        );
        assert_eq!(
            serde_json::from_str::<Color>("\"red\"").ok(),
            Some(Color::Red)
        );
        assert!(serde_json::from_str::<Color>("\"Red\"").is_err());
        assert!(serde_json::from_str::<Color>("2").is_err());
    }

    #[test]
    fn table_range_accepts_the_full_legal_span() {
        assert_eq!(
            TableRange::new(1, 9999),
            Ok(TableRange {
                start: 1,
                end: 9999
            })
        );
        assert_eq!(TableRange::new(7, 7), Ok(TableRange { start: 7, end: 7 }));
        for (start, end) in [
            (0, 60),
            (0, 0),
            (1, 10_000),
            (60, 1),
            (2, 1),
            (10_000, 10_000),
        ] {
            assert_eq!(
                TableRange::new(start, end),
                Err(ProtoError::InvalidTableRange { start, end }),
                "range {start}..={end} must be rejected"
            );
        }
    }

    #[test]
    fn table_range_queries_cover_boundaries() {
        let range = TableRange::default_classroom();
        assert_eq!(range, TableRange { start: 1, end: 60 });
        assert_eq!(TableRange::default(), range);
        assert_eq!(range.len(), 60);
        assert!(!range.is_empty());
        assert!(range.contains(1));
        assert!(range.contains(60));
        assert!(!range.contains(0));
        assert!(!range.contains(61));
        assert_eq!(range.iter().count(), 60);
        assert_eq!(range.iter().next(), Some(1));
        assert_eq!(range.iter().last(), Some(60));

        let single = TableRange { start: 5, end: 5 };
        assert_eq!(single.len(), 1);
        assert_eq!(single.iter().collect::<Vec<_>>(), vec![5]);

        // An unvalidated (for example deserialized) inverted range must
        // stay total: no panic, no members.
        let inverted = TableRange { start: 9, end: 2 };
        assert_eq!(inverted.len(), 0);
        assert!(inverted.is_empty());
        assert_eq!(inverted.iter().count(), 0);
        assert!(!inverted.contains(5));
    }

    #[test]
    fn table_range_and_firmware_info_match_the_documented_json() {
        let range = TableRange::default_classroom();
        assert_eq!(
            serde_json::to_string(&range).unwrap_or_default(),
            "{\"start\":1,\"end\":60}"
        );
        assert_eq!(
            serde_json::from_str::<TableRange>("{\"start\":3,\"end\":4}").ok(),
            Some(TableRange { start: 3, end: 4 })
        );

        let legacy = FirmwareInfo::legacy();
        assert_eq!(legacy.kind, FirmwareKind::Legacy);
        assert_eq!(
            serde_json::to_string(&legacy).unwrap_or_default(),
            "{\"kind\":\"legacy\"}"
        );

        let receiver = FirmwareInfo {
            kind: FirmwareKind::New,
            mode: Some(FirmwareMode::Receiver),
            version: Some("draft-1".to_string()),
            station_id: None,
            station_color: None,
        };
        assert_eq!(
            serde_json::to_string(&receiver).unwrap_or_default(),
            "{\"kind\":\"new\",\"mode\":\"receiver\",\"version\":\"draft-1\"}"
        );
        // Absent optional fields are accepted on the way back in.
        assert_eq!(
            serde_json::from_str::<FirmwareInfo>("{\"kind\":\"legacy\"}").ok(),
            Some(FirmwareInfo::legacy())
        );
    }

    #[test]
    fn format_command_emits_exact_legacy_lines() {
        assert_eq!(
            format_command(SerialCommand::SetTable {
                table: 5,
                color: Color::Red
            }),
            Ok("T,5,2\n".to_string())
        );
        assert_eq!(
            format_command(SerialCommand::SetTable {
                table: 9999,
                color: Color::Green
            }),
            Ok("T,9999,0\n".to_string())
        );
        assert_eq!(
            format_command(SerialCommand::ResetAll),
            Ok("T,-1,0\n".to_string())
        );
        assert_eq!(format_command(SerialCommand::WakeUp), Ok("\n".to_string()));
        assert_eq!(
            format_command(SerialCommand::ProbeFirmware),
            Ok("!get\n".to_string())
        );
    }

    #[test]
    fn format_command_rejects_tables_that_reports_may_carry() {
        // Table 0 is a legal *report* (unset station) but never a command
        // target; table 10000 is beyond the firmware ID space.
        for table in [0, 10_000, u16::MAX] {
            assert_eq!(
                format_command(SerialCommand::SetTable {
                    table,
                    color: Color::Green
                }),
                Err(ProtoError::InvalidTableId { table })
            );
        }
    }

    #[test]
    fn every_formatted_command_is_safe_for_legacy_student_lights() {
        // The sweep covers boundary tables and every color: the old
        // MakeCode student indexes colorList unchecked, so each line must
        // be newline terminated, Ctrl-C free, within the line budget, and
        // carry a color digit inside 0..=2 only.
        let mut commands = vec![
            SerialCommand::ResetAll,
            SerialCommand::WakeUp,
            SerialCommand::ProbeFirmware,
        ];
        for table in [1, 2, 60, 61, 9998, 9999] {
            for color in [Color::Green, Color::Orange, Color::Red] {
                commands.push(SerialCommand::SetTable { table, color });
            }
        }
        for command in commands {
            let line = format_command(command).unwrap_or_default();
            assert!(line.ends_with('\n'), "line {line:?} must end with newline");
            assert!(!line.contains('\u{3}'), "line {line:?} contains Ctrl-C");
            assert!(line.len() <= MAX_LINE_BYTES, "line {line:?} too long");
            assert!(!line[..line.len() - 1].contains('\n'));
            if let SerialCommand::SetTable { table, color } = command {
                assert_eq!(line, format!("T,{table},{}\n", color.as_wire_code()));
                assert!(["0\n", "1\n", "2\n"]
                    .iter()
                    .any(|tail| line.ends_with(tail)));
            }
        }
    }

    #[test]
    fn accumulator_splits_lines_on_every_ending_style() {
        let mut acc = LineAccumulator::new();
        assert_eq!(acc.feed(b"S,1,0\n"), ["S,1,0"]);
        assert_eq!(acc.feed(b"S,2,1\r"), ["S,2,1"]);
        assert_eq!(acc.feed(b"S,3,2\r\n"), ["S,3,2"]);
        assert_eq!(acc.feed(b"S,4,0\nS,5,1\nS,6,2"), ["S,4,0", "S,5,1"]);
        assert_eq!(acc.feed(b"\n"), ["S,6,2"]);
        assert!(acc.feed(b"").is_empty());
        assert!(acc.feed(b"no ending yet").is_empty());
        assert_eq!(acc.feed(b"\r\n"), ["no ending yet"]);
    }

    #[test]
    fn accumulator_never_makes_a_phantom_line_from_split_crlf() {
        let mut acc = LineAccumulator::new();
        assert_eq!(acc.feed(b"S,1,2\r"), ["S,1,2"]);
        // The LF half of the CRLF pair arrives in the next chunk: it
        // belongs to the previous ending, not to an empty new line. The
        // next real line must still parse in the same chunk.
        assert_eq!(acc.feed(b"\nS,2,0\n"), ["S,2,0"]);
        // And a CR not followed by LF must not eat the following byte.
        assert_eq!(acc.feed(b"S,3,1\rS,4,2\n"), ["S,3,1", "S,4,2"]);
    }

    #[test]
    fn accumulator_drops_blank_and_invalid_utf8_lines_only() {
        let mut acc = LineAccumulator::new();
        assert!(acc.feed(b"\n\r\n  \t \n").is_empty());
        assert!(acc.feed(b"S,1,\xff\xfe\n").is_empty());
        assert_eq!(acc.feed(b"S,1,0\n"), ["S,1,0"]);
        // A multi-byte character split across chunks survives intact.
        assert!(acc.feed(b"caf\xc3").is_empty());
        assert_eq!(acc.feed(b"\xa9\n"), ["caf\u{e9}"]);
    }

    #[test]
    fn accumulator_discards_the_whole_overlong_line() {
        let mut acc = LineAccumulator::new();
        let at_limit = format!("{}\n", "x".repeat(MAX_LINE_BYTES));
        assert_eq!(acc.feed(at_limit.as_bytes()).len(), 1);

        // The tail of this over-long line looks like a valid report; it
        // must not masquerade as one. Only the line after it survives.
        let over = format!("{}S,1,0\nS,2,1\n", "y".repeat(MAX_LINE_BYTES));
        assert_eq!(acc.feed(over.as_bytes()), ["S,2,1"]);

        // Overflow also works byte by byte across chunk boundaries.
        let mut acc = LineAccumulator::new();
        for _ in 0..(MAX_LINE_BYTES + 5) {
            assert!(acc.feed(b"z").is_empty());
        }
        assert!(acc.feed(b",1,0\n").is_empty());
        assert_eq!(acc.feed(b"S,9,2\n"), ["S,9,2"]);
    }

    #[test]
    fn accumulator_reset_forgets_the_partial_line() {
        let mut acc = LineAccumulator::new();
        assert!(acc.feed(b"S,1,").is_empty());
        acc.reset();
        assert_eq!(acc.feed(b"S,2,1\n"), ["S,2,1"]);
    }

    #[test]
    fn parse_line_reads_legacy_station_reports() {
        assert_eq!(parse_line("S,12,2"), Some(status(12, Color::Red, false)));
        assert_eq!(parse_line("RS,12,1"), Some(status(12, Color::Orange, true)));
        assert_eq!(parse_line("S,0,0"), Some(status(0, Color::Green, false)));
        assert_eq!(
            parse_line("S,9999,2"),
            Some(status(9999, Color::Red, false))
        );
        assert_eq!(
            parse_line("  S,3,1  \r"),
            Some(status(3, Color::Orange, false))
        );
        // Field-level padding is tolerated exactly like the original
        // fastapi_receiver.py parser tolerated it through int().
        assert_eq!(parse_line("S, 7 , 2 "), Some(status(7, Color::Red, false)));
        assert_eq!(parse_line("S,007,00"), Some(status(7, Color::Green, false)));
    }

    #[test]
    fn parse_line_marks_every_rt_line_as_an_ignored_echo() {
        assert_eq!(parse_line("RT,4,1"), Some(SerialEvent::IgnoredEcho));
        assert_eq!(parse_line("RT,garbage"), Some(SerialEvent::IgnoredEcho));
        assert_eq!(parse_line("RT,4,1,extra"), Some(SerialEvent::IgnoredEcho));
        assert_eq!(parse_line("RT"), None);
        assert_eq!(parse_line("RTX,4,1"), None);
    }

    #[test]
    fn parse_line_rejects_malformed_legacy_lines() {
        for line in [
            "",
            "   ",
            "garbage",
            "S,1",
            "S,1,0,extra",
            "S,,0",
            "S,1,",
            "S,-1,0",
            "S,1,-1",
            "S,10000,0",
            "S,65536,0",
            "S,1,3",
            "S,1,256",
            "S,one,0",
            "S,1,red",
            "S,1.5,0",
            "X,1,0",
            "s,1,0",
            "T,1,0",
            "T,-1,0",
            "SS,1,0",
        ] {
            assert_eq!(parse_line(line), None, "line {line:?} must be rejected");
        }
    }

    #[test]
    fn parse_line_reads_new_firmware_ok_replies() {
        assert_eq!(
            parse_line("!ok mode=receiver fw=draft-1"),
            Some(SerialEvent::FirmwareDetected {
                info: FirmwareInfo {
                    kind: FirmwareKind::New,
                    mode: Some(FirmwareMode::Receiver),
                    version: Some("draft-1".to_string()),
                    station_id: None,
                    station_color: None,
                }
            })
        );
        assert_eq!(
            parse_line("!ok mode=station id=7 color=green fw=draft-1"),
            Some(SerialEvent::FirmwareDetected {
                info: FirmwareInfo {
                    kind: FirmwareKind::New,
                    mode: Some(FirmwareMode::Station),
                    version: Some("draft-1".to_string()),
                    station_id: Some(7),
                    station_color: Some(Color::Green),
                }
            })
        );
        // !set and !clear answers identify the board too.
        assert_eq!(
            parse_line("!ok mode=station id=12"),
            Some(SerialEvent::FirmwareDetected {
                info: FirmwareInfo {
                    kind: FirmwareKind::New,
                    mode: Some(FirmwareMode::Station),
                    version: None,
                    station_id: Some(12),
                    station_color: None,
                }
            })
        );
        assert_eq!(
            parse_line("!ok mode=station id=0"),
            Some(SerialEvent::FirmwareDetected {
                info: FirmwareInfo {
                    kind: FirmwareKind::New,
                    mode: Some(FirmwareMode::Station),
                    version: None,
                    station_id: Some(0),
                    station_color: None,
                }
            })
        );
    }

    #[test]
    fn parse_line_keeps_control_replies_forward_compatible() {
        // Unknown keys and a malformed id do not hide a recognized mode.
        assert_eq!(
            parse_line("!ok fw=next-9 mode=receiver mesh=on id=nope extra"),
            Some(SerialEvent::FirmwareDetected {
                info: FirmwareInfo {
                    kind: FirmwareKind::New,
                    mode: Some(FirmwareMode::Receiver),
                    version: Some("next-9".to_string()),
                    station_id: None,
                    station_color: None,
                }
            })
        );
        // Replies without a recognized mode identify nothing.
        for line in [
            "!ok restarting",
            "!ok",
            "!ok fw=draft-1",
            "!ok mode=mesh fw=draft-9",
            "!get",
            "!set mode=station id=12",
            "!okfoo mode=receiver",
            "!unknown mode=receiver",
        ] {
            assert_eq!(parse_line(line), None, "line {line:?} must be ignored");
        }
    }

    #[test]
    fn parse_line_reads_new_firmware_error_replies() {
        assert_eq!(
            parse_line("!err bad id"),
            Some(SerialEvent::ControlError {
                reason: "bad id".to_string()
            })
        );
        assert_eq!(
            parse_line("  !err receiver has no id  "),
            Some(SerialEvent::ControlError {
                reason: "receiver has no id".to_string()
            })
        );
        assert_eq!(
            parse_line("!err"),
            Some(SerialEvent::ControlError {
                reason: String::new()
            })
        );
        assert_eq!(parse_line("!error bad id"), None);
        assert_eq!(parse_line("!errx bad id"), None);
        assert_eq!(parse_line("!errors"), None);
    }

    #[test]
    fn session_forwards_first_report_and_suppresses_heartbeats() {
        let mut session = SerialSession::new();
        assert_eq!(
            session.feed_bytes(b"S,4,2\n"),
            [status(4, Color::Red, false)]
        );
        // Identical 4-6 second heartbeats, direct or relayed, are noise.
        assert!(session.feed_bytes(b"S,4,2\n").is_empty());
        assert!(session.feed_bytes(b"RS,4,2\n").is_empty());
        // A change is forwarded at once, per table independently.
        assert_eq!(
            session.feed_bytes(b"S,4,1\nS,5,2\n"),
            [
                status(4, Color::Orange, false),
                status(5, Color::Red, false)
            ]
        );
        assert!(session.feed_bytes(b"S,5,2\n").is_empty());
    }

    #[test]
    fn session_handles_split_chunks_echoes_and_noise() {
        let mut session = SerialSession::new();
        assert!(session.feed_bytes(b"S,8,").is_empty());
        assert_eq!(
            session.feed_bytes(b"1\r"),
            [status(8, Color::Orange, false)]
        );
        assert!(session.feed_bytes(b"\nRT,8,1\nnoise\nS,x,9\n").is_empty());
        assert_eq!(
            session.feed_bytes(b"S,0,2\n"),
            [status(0, Color::Red, false)]
        );
    }

    #[test]
    fn session_always_forwards_firmware_and_control_errors() {
        let mut session = SerialSession::new();
        let detected = || SerialEvent::FirmwareDetected {
            info: FirmwareInfo {
                kind: FirmwareKind::New,
                mode: Some(FirmwareMode::Receiver),
                version: Some("draft-1".to_string()),
                station_id: None,
                station_color: None,
            },
        };
        assert_eq!(
            session.feed_bytes(b"!ok mode=receiver fw=draft-1\n"),
            [detected()]
        );
        assert_eq!(
            session.feed_bytes(b"!ok mode=receiver fw=draft-1\n!err bad mode\n"),
            [
                detected(),
                SerialEvent::ControlError {
                    reason: "bad mode".to_string()
                }
            ]
        );
    }

    #[test]
    fn recorded_set_table_dedupes_success_and_exposes_rf_loss() {
        let mut session = SerialSession::new();
        assert_eq!(
            session.feed_bytes(b"S,6,0\n"),
            [status(6, Color::Green, false)]
        );
        // Teacher sets table 6 to red and the station received it: the
        // confirming heartbeat matches the cache and stays deduplicated.
        session.record_set_table(6, Color::Red);
        assert!(session.feed_bytes(b"S,6,2\n").is_empty());

        // Teacher sets table 6 back to green but the RF packet is lost:
        // the station is still red, differs from the cache, and its very
        // next heartbeat is forwarded to correct the dashboard.
        session.record_set_table(6, Color::Green);
        assert_eq!(
            session.feed_bytes(b"S,6,2\n"),
            [status(6, Color::Red, false)]
        );
    }

    #[test]
    fn recorded_reset_all_exposes_stations_that_missed_the_broadcast() {
        let mut session = SerialSession::new();
        assert_eq!(session.feed_bytes(b"S,1,2\nS,2,1\n").len(), 2);
        session.record_reset_all();
        // Table 1 heard the reset (green heartbeat deduplicated); table 2
        // missed it and is still orange, so it re-reports immediately.
        assert_eq!(
            session.feed_bytes(b"S,1,0\nS,2,1\n"),
            [status(2, Color::Orange, false)]
        );
        // A never-heard station is not invented by the reset; its first
        // heartbeat reports normally.
        assert_eq!(
            session.feed_bytes(b"S,3,0\n"),
            [status(3, Color::Green, false)]
        );
    }

    #[test]
    fn reset_dedup_resyncs_after_reconnect() {
        let mut session = SerialSession::new();
        assert_eq!(session.feed_bytes(b"S,9,2\n").len(), 1);
        assert!(session.feed_bytes(b"S,9,").is_empty());
        session.reset_dedup();
        // The partial line from before the reconnect is gone, and the
        // same color reports again because the cache is empty.
        assert_eq!(
            session.feed_bytes(b"S,9,2\n"),
            [status(9, Color::Red, false)]
        );
    }

    #[test]
    fn serial_events_and_commands_use_the_tagged_json_shape() {
        assert_eq!(
            serde_json::to_string(&status(12, Color::Red, true)).unwrap_or_default(),
            "{\"type\":\"station_status\",\"table\":12,\"color\":\"red\",\"relayed\":true}"
        );
        assert_eq!(
            serde_json::to_string(&SerialEvent::IgnoredEcho).unwrap_or_default(),
            "{\"type\":\"ignored_echo\"}"
        );
        assert_eq!(
            serde_json::to_string(&SerialEvent::ControlError {
                reason: "bad id".to_string()
            })
            .unwrap_or_default(),
            "{\"type\":\"control_error\",\"reason\":\"bad id\"}"
        );
        assert_eq!(
            serde_json::to_string(&SerialCommand::SetTable {
                table: 5,
                color: Color::Orange
            })
            .unwrap_or_default(),
            "{\"cmd\":\"set_table\",\"table\":5,\"color\":\"orange\"}"
        );
        assert_eq!(
            serde_json::to_string(&SerialCommand::ResetAll).unwrap_or_default(),
            "{\"cmd\":\"reset_all\"}"
        );
        assert_eq!(
            serde_json::from_str::<SerialCommand>("{\"cmd\":\"probe_firmware\"}").ok(),
            Some(SerialCommand::ProbeFirmware)
        );
        assert_eq!(
            serde_json::from_str::<SerialEvent>(
                "{\"type\":\"firmware_detected\",\"info\":{\"kind\":\"legacy\"}}"
            )
            .ok(),
            Some(SerialEvent::FirmwareDetected {
                info: FirmwareInfo::legacy()
            })
        );
    }
}
