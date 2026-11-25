// SPDX-License-Identifier: AGPL-3.0-or-later
// TypeScript mirror of crates/proto (wire.rs, ws.rs, stats.rs). The Rust
// definitions are the source of truth; JSON uses snake_case type tags.

export type Color = "green" | "orange" | "red";

export const COLOR_GREEN: Color = "green";
export const COLOR_ORANGE: Color = "orange";
export const COLOR_RED: Color = "red";

export const MIN_TABLE_ID = 1;
export const MAX_TABLE_ID = 9999;
export const MAX_STATION_ID = 9999;
export const MAX_SESSION_NAME_CHARS = 80;
export const DEFAULT_RANGE_START = 1;
export const DEFAULT_RANGE_END = 60;
export const EXPECTED_PROTO_VERSION = 1;
export const DEFAULT_SOURCE_URL = "https://github.com/NB-TUDelft/trafficlights_webapp";

export function nextCycleColor(color: Color): Color {
  if (color === COLOR_GREEN) {
    return COLOR_ORANGE;
  }
  if (color === COLOR_ORANGE) {
    return COLOR_RED;
  }
  return COLOR_GREEN;
}

export interface TableRange {
  start: number;
  end: number;
}

export function isValidTableRange(start: number, end: number): boolean {
  return (
    Number.isInteger(start) &&
    Number.isInteger(end) &&
    start >= MIN_TABLE_ID &&
    end <= MAX_TABLE_ID &&
    start <= end
  );
}

export function tableRangeContains(range: TableRange, table: number): boolean {
  return table >= range.start && table <= range.end;
}

const CONTROL_CHAR_MAX = 0x1f;
const DELETE_CHAR = 0x7f;

function hasControlChar(text: string): boolean {
  for (const ch of text) {
    const code = ch.codePointAt(0) ?? 0;
    if (code <= CONTROL_CHAR_MAX || code === DELETE_CHAR) {
      return true;
    }
  }
  return false;
}

/** Mirrors proto normalize_session_name; returns null when invalid. */
export function normalizeSessionName(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return null;
  }
  if ([...trimmed].length > MAX_SESSION_NAME_CHARS) {
    return null;
  }
  if (hasControlChar(trimmed)) {
    return null;
  }
  return trimmed;
}

export type FirmwareKind = "new" | "legacy";
export type FirmwareMode = "receiver" | "station";

export interface FirmwareInfo {
  kind: FirmwareKind;
  mode?: FirmwareMode;
  version?: string;
  station_id?: number;
  station_color?: Color;
}

export interface ColorCounts {
  green: number;
  orange: number;
  red: number;
}

export interface TableEntry {
  table: number;
  color: Color;
  red_since_ms?: number;
}

export interface UnlistedTable {
  table: number;
  color: Color;
  last_seen_ms: number;
}

export interface ServerBuildInfo {
  version: string;
  commit?: string;
  license: string;
  source_url: string;
  proto_version: number;
  pin_required: boolean;
}

export interface SessionSummary {
  session_id: string;
  name: string;
  range: TableRange;
  counts: ColorCounts;
  receiver_connected: boolean;
  firmware?: FirmwareInfo;
  viewer_count: number;
  created_at_ms: number;
}

export type EndReason = "manual" | "grace_timeout" | "shutdown";

export type SerialCommand =
  | { cmd: "set_table"; table: number; color: Color }
  | { cmd: "reset_all" }
  | { cmd: "wake_up" }
  | { cmd: "probe_firmware" };

export type SerialEvent =
  | { type: "station_status"; table: number; color: Color; relayed: boolean }
  | { type: "firmware_detected"; info: FirmwareInfo }
  | { type: "control_error"; reason: string }
  | { type: "ignored_echo" };

export type LobbyClientMsg =
  | { type: "create_session"; name: string; start: number; end: number; pin?: string | null }
  | { type: "ping" };

export type LobbyServerMsg =
  | { type: "lobby_snapshot"; sessions: SessionSummary[]; server_info: ServerBuildInfo }
  | { type: "lobby_update"; sessions: SessionSummary[] }
  | { type: "session_created"; session_id: string; creator_token: string; summary: SessionSummary }
  | { type: "error"; code: string; message: string }
  | { type: "pong" };

export type SessionClientMsg =
  | { type: "auth"; creator_token: string }
  | { type: "set_table"; table: number; color: Color }
  | { type: "station_report"; table: number; color: Color }
  | {
      type: "serial_status";
      connected: boolean;
      port_label?: string | null;
      firmware?: FirmwareInfo | null;
    }
  | { type: "reset_all" }
  | { type: "set_table_range"; start: number; end: number }
  | { type: "rename_session"; name: string }
  | { type: "end_session" }
  | { type: "ping" };

export type SessionServerMsg =
  | {
      type: "snapshot";
      session_id: string;
      name: string;
      range: TableRange;
      tables: TableEntry[];
      counts: ColorCounts;
      receiver_connected: boolean;
      port_label?: string;
      firmware?: FirmwareInfo;
      viewer_count: number;
      is_creator: boolean;
      unlisted_tables?: UnlistedTable[];
      server_now_ms: number;
      server_info: ServerBuildInfo;
    }
  | { type: "auth_ok"; is_creator: boolean; unlisted_tables?: UnlistedTable[] }
  | {
      type: "table_update";
      table: number;
      color: Color;
      red_since_ms?: number;
      counts: ColorCounts;
    }
  | {
      type: "receiver_status";
      connected: boolean;
      port_label?: string;
      firmware?: FirmwareInfo;
    }
  | { type: "session_meta"; name: string; range: TableRange; viewer_count: number }
  | { type: "serial_command"; command: SerialCommand }
  | { type: "unlisted_tables"; tables: UnlistedTable[] }
  | { type: "session_ended"; reason: EndReason; log_available: boolean }
  | { type: "error"; code: string; message: string }
  | { type: "pong" };
