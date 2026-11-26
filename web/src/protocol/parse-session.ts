// SPDX-License-Identifier: AGPL-3.0-or-later
// Session WebSocket message parsing built on the shared guards.

import {
  asRecord,
  boolField,
  colorField,
  errorMsg,
  intField,
  type JsonRecord,
  numField,
  optionalFirmware,
  parseColorCounts,
  parseList,
  parseSerialCommand,
  parseServerBuildInfo,
  parseTableEntry,
  parseTableRange,
  parseTextInput,
  parseUnlistedTable,
  strField,
} from "./guards";
import type {
  ColorCounts,
  EndReason,
  ServerBuildInfo,
  SessionServerMsg,
  TableEntry,
  TableRange,
  TapMode,
  UnlistedTable,
} from "./types";

function optionalTapMode(rec: JsonRecord, key: string): TapMode | undefined {
  const value = rec[key];
  if (value === "green" || value === "cycle") {
    return value;
  }
  return undefined;
}

interface ModeFields {
  tap_mode?: TapMode;
  tap_mode_locked?: boolean;
  beeps_enabled?: boolean;
  beeps_locked?: boolean;
}

function applyModeFields(rec: JsonRecord, target: ModeFields): void {
  const tapMode = optionalTapMode(rec, "tap_mode");
  if (tapMode !== undefined) {
    target.tap_mode = tapMode;
  }
  const tapModeLocked = boolField(rec, "tap_mode_locked");
  if (tapModeLocked !== null) {
    target.tap_mode_locked = tapModeLocked;
  }
  const beepsEnabled = boolField(rec, "beeps_enabled");
  if (beepsEnabled !== null) {
    target.beeps_enabled = beepsEnabled;
  }
  const beepsLocked = boolField(rec, "beeps_locked");
  if (beepsLocked !== null) {
    target.beeps_locked = beepsLocked;
  }
}

function endReason(value: unknown): EndReason | null {
  if (value === "manual" || value === "grace_timeout" || value === "shutdown") {
    return value;
  }
  return null;
}

export function parseSessionServerMsg(input: unknown): SessionServerMsg | null {
  const rec = asRecord(parseTextInput(input));
  if (rec === null) {
    return null;
  }
  const kind = rec.type;
  if (typeof kind !== "string") {
    return null;
  }
  const parsers: Record<string, (r: JsonRecord) => SessionServerMsg | null> = {
    pong: () => ({ type: "pong" }),
    error: parseSessionError,
    snapshot: parseSnapshot,
    auth_ok: parseAuthOk,
    table_update: parseTableUpdate,
    receiver_status: parseReceiverStatus,
    session_meta: parseSessionMeta,
    serial_command: parseSerialCommandMsg,
    unlisted_tables: parseUnlistedMsg,
    session_ended: parseSessionEnded,
  };
  const parser = parsers[kind];
  return parser === undefined ? null : parser(rec);
}

function parseSessionError(rec: JsonRecord): SessionServerMsg | null {
  const err = errorMsg(rec);
  return err === null ? null : { type: "error", code: err.code, message: err.message };
}

function optionalUnlisted(rec: JsonRecord): UnlistedTable[] {
  if (!("unlisted_tables" in rec) || rec.unlisted_tables === undefined) {
    return [];
  }
  return parseList(rec.unlisted_tables, parseUnlistedTable) ?? [];
}

function parseSnapshot(rec: JsonRecord): SessionServerMsg | null {
  const sessionId = strField(rec, "session_id");
  const name = strField(rec, "name");
  const range = parseTableRange(rec.range);
  const tables = parseList(rec.tables, parseTableEntry);
  const counts = parseColorCounts(rec.counts);
  const receiverConnected = boolField(rec, "receiver_connected");
  const viewerCount = intField(rec, "viewer_count");
  const isCreator = boolField(rec, "is_creator");
  const serverNow = numField(rec, "server_now_ms");
  const serverInfo = parseServerBuildInfo(rec.server_info);
  if (
    sessionId === null ||
    name === null ||
    range === null ||
    tables === null ||
    counts === null ||
    receiverConnected === null ||
    viewerCount === null ||
    isCreator === null ||
    serverNow === null ||
    serverInfo === null
  ) {
    return null;
  }
  return buildSnapshot(rec, {
    sessionId,
    name,
    range,
    tables,
    counts,
    receiverConnected,
    viewerCount,
    isCreator,
    serverNow,
    serverInfo,
  });
}

interface SnapshotParts {
  sessionId: string;
  name: string;
  range: TableRange;
  tables: TableEntry[];
  counts: ColorCounts;
  receiverConnected: boolean;
  viewerCount: number;
  isCreator: boolean;
  serverNow: number;
  serverInfo: ServerBuildInfo;
}

function buildSnapshot(rec: JsonRecord, parts: SnapshotParts): SessionServerMsg {
  const msg: Extract<SessionServerMsg, { type: "snapshot" }> = {
    type: "snapshot",
    session_id: parts.sessionId,
    name: parts.name,
    range: parts.range,
    tables: parts.tables,
    counts: parts.counts,
    receiver_connected: parts.receiverConnected,
    viewer_count: parts.viewerCount,
    is_creator: parts.isCreator,
    unlisted_tables: optionalUnlisted(rec),
    server_now_ms: parts.serverNow,
    server_info: parts.serverInfo,
  };
  const portLabel = strField(rec, "port_label");
  if (portLabel !== null) {
    msg.port_label = portLabel;
  }
  const firmware = optionalFirmware(rec, "firmware");
  if (firmware !== undefined) {
    msg.firmware = firmware;
  }
  applyModeFields(rec, msg);
  return msg;
}

function parseAuthOk(rec: JsonRecord): SessionServerMsg | null {
  const isCreator = boolField(rec, "is_creator");
  if (isCreator === null) {
    return null;
  }
  return { type: "auth_ok", is_creator: isCreator, unlisted_tables: optionalUnlisted(rec) };
}

function parseTableUpdate(rec: JsonRecord): SessionServerMsg | null {
  const table = intField(rec, "table");
  const color = colorField(rec, "color");
  const counts = parseColorCounts(rec.counts);
  if (table === null || color === null || counts === null) {
    return null;
  }
  const msg: Extract<SessionServerMsg, { type: "table_update" }> = {
    type: "table_update",
    table,
    color,
    counts,
  };
  const redSince = intField(rec, "red_since_ms");
  if (redSince !== null) {
    msg.red_since_ms = redSince;
  }
  return msg;
}

function parseReceiverStatus(rec: JsonRecord): SessionServerMsg | null {
  const connected = boolField(rec, "connected");
  if (connected === null) {
    return null;
  }
  const msg: Extract<SessionServerMsg, { type: "receiver_status" }> = {
    type: "receiver_status",
    connected,
  };
  const portLabel = strField(rec, "port_label");
  if (portLabel !== null) {
    msg.port_label = portLabel;
  }
  const firmware = optionalFirmware(rec, "firmware");
  if (firmware !== undefined) {
    msg.firmware = firmware;
  }
  return msg;
}

function parseSessionMeta(rec: JsonRecord): SessionServerMsg | null {
  const name = strField(rec, "name");
  const range = parseTableRange(rec.range);
  const viewerCount = intField(rec, "viewer_count");
  if (name === null || range === null || viewerCount === null) {
    return null;
  }
  const msg: Extract<SessionServerMsg, { type: "session_meta" }> = {
    type: "session_meta",
    name,
    range,
    viewer_count: viewerCount,
  };
  applyModeFields(rec, msg);
  return msg;
}

function parseSerialCommandMsg(rec: JsonRecord): SessionServerMsg | null {
  const command = parseSerialCommand(rec.command);
  return command === null ? null : { type: "serial_command", command };
}

function parseUnlistedMsg(rec: JsonRecord): SessionServerMsg | null {
  const tables = parseList(rec.tables, parseUnlistedTable);
  return tables === null ? null : { type: "unlisted_tables", tables };
}

function parseSessionEnded(rec: JsonRecord): SessionServerMsg | null {
  const reason = endReason(rec.reason);
  const logAvailable = boolField(rec, "log_available");
  if (reason === null || logAvailable === null) {
    return null;
  }
  return { type: "session_ended", reason, log_available: logAvailable };
}
