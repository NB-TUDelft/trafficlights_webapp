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
  UnlistedTable,
} from "./types";

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
  const redSince = numField(rec, "red_since_ms");
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
  return { type: "session_meta", name, range, viewer_count: viewerCount };
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
