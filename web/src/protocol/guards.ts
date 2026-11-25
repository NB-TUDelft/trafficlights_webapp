// SPDX-License-Identifier: AGPL-3.0-or-later
// Shared runtime guards for untrusted JSON objects and protocol entities.
// No unchecked casts: every field is narrowed before use.

import type {
  Color,
  ColorCounts,
  FirmwareInfo,
  FirmwareKind,
  FirmwareMode,
  SerialCommand,
  ServerBuildInfo,
  SessionSummary,
  TableEntry,
  TableRange,
  UnlistedTable,
} from "./types";
import { DEFAULT_SOURCE_URL } from "./types";

export type JsonRecord = Record<string, unknown>;

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function asRecord(value: unknown): JsonRecord | null {
  return isRecord(value) ? value : null;
}

export function parseTextInput(input: unknown): unknown {
  if (typeof input !== "string") {
    return input;
  }
  try {
    const parsed: unknown = JSON.parse(input);
    return parsed;
  } catch {
    return null;
  }
}

export function strField(rec: JsonRecord, key: string): string | null {
  const value = rec[key];
  return typeof value === "string" ? value : null;
}

export function numField(rec: JsonRecord, key: string): number | null {
  const value = rec[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

export function intField(rec: JsonRecord, key: string): number | null {
  const value = numField(rec, key);
  return value !== null && Number.isInteger(value) ? value : null;
}

export function boolField(rec: JsonRecord, key: string): boolean | null {
  const value = rec[key];
  return typeof value === "boolean" ? value : null;
}

function isColor(value: unknown): value is Color {
  return value === "green" || value === "orange" || value === "red";
}

export function colorField(rec: JsonRecord, key: string): Color | null {
  const value = rec[key];
  return isColor(value) ? value : null;
}

export function parseTableRange(value: unknown): TableRange | null {
  const rec = asRecord(value);
  if (rec === null) {
    return null;
  }
  const start = intField(rec, "start");
  const end = intField(rec, "end");
  if (start === null || end === null) {
    return null;
  }
  return { start, end };
}

export function parseColorCounts(value: unknown): ColorCounts | null {
  const rec = asRecord(value);
  if (rec === null) {
    return null;
  }
  const green = intField(rec, "green");
  const orange = intField(rec, "orange");
  const red = intField(rec, "red");
  if (green === null || orange === null || red === null) {
    return null;
  }
  return { green, orange, red };
}

function firmwareKind(value: unknown): FirmwareKind | null {
  return value === "new" || value === "legacy" ? value : null;
}

function firmwareMode(value: unknown): FirmwareMode | null {
  return value === "receiver" || value === "station" ? value : null;
}

export function parseFirmwareInfo(value: unknown): FirmwareInfo | null {
  const rec = asRecord(value);
  if (rec === null) {
    return null;
  }
  const kind = firmwareKind(rec.kind);
  if (kind === null) {
    return null;
  }
  const info: FirmwareInfo = { kind };
  const mode = firmwareMode(rec.mode);
  if (mode !== null) {
    info.mode = mode;
  }
  const version = strField(rec, "version");
  if (version !== null) {
    info.version = version;
  }
  const stationId = intField(rec, "station_id");
  if (stationId !== null) {
    info.station_id = stationId;
  }
  const stationColor = colorField(rec, "station_color");
  if (stationColor !== null) {
    info.station_color = stationColor;
  }
  return info;
}

export function optionalFirmware(rec: JsonRecord, key: string): FirmwareInfo | undefined {
  if (!(key in rec) || rec[key] === undefined || rec[key] === null) {
    return undefined;
  }
  return parseFirmwareInfo(rec[key]) ?? undefined;
}

export function parseTableEntry(value: unknown): TableEntry | null {
  const rec = asRecord(value);
  if (rec === null) {
    return null;
  }
  const table = intField(rec, "table");
  const color = colorField(rec, "color");
  if (table === null || color === null) {
    return null;
  }
  const entry: TableEntry = { table, color };
  const redSince = intField(rec, "red_since_ms");
  if (redSince !== null) {
    entry.red_since_ms = redSince;
  }
  return entry;
}

export function parseUnlistedTable(value: unknown): UnlistedTable | null {
  const rec = asRecord(value);
  if (rec === null) {
    return null;
  }
  const table = intField(rec, "table");
  const color = colorField(rec, "color");
  const lastSeen = intField(rec, "last_seen_ms");
  if (table === null || color === null || lastSeen === null) {
    return null;
  }
  return { table, color, last_seen_ms: lastSeen };
}

export function parseList<T>(value: unknown, parseItem: (item: unknown) => T | null): T[] | null {
  if (!Array.isArray(value)) {
    return null;
  }
  const out: T[] = [];
  for (const item of value) {
    const parsed = parseItem(item);
    if (parsed === null) {
      return null;
    }
    out.push(parsed);
  }
  return out;
}

export function parseServerBuildInfo(value: unknown): ServerBuildInfo | null {
  const rec = asRecord(value);
  if (rec === null) {
    return null;
  }
  const version = strField(rec, "version");
  const license = strField(rec, "license");
  const sourceUrl = strField(rec, "source_url");
  const protoVersion = intField(rec, "proto_version");
  const pinRequired = boolField(rec, "pin_required");
  if (
    version === null ||
    license === null ||
    sourceUrl === null ||
    protoVersion === null ||
    pinRequired === null
  ) {
    return null;
  }
  const info: ServerBuildInfo = {
    version,
    license,
    source_url: sourceUrl.startsWith("https://") ? sourceUrl : DEFAULT_SOURCE_URL,
    proto_version: protoVersion,
    pin_required: pinRequired,
  };
  const commit = strField(rec, "commit");
  if (commit !== null) {
    info.commit = commit;
  }
  return info;
}

export function parseSessionSummary(value: unknown): SessionSummary | null {
  const rec = asRecord(value);
  if (rec === null) {
    return null;
  }
  const sessionId = strField(rec, "session_id");
  const name = strField(rec, "name");
  const range = parseTableRange(rec.range);
  const counts = parseColorCounts(rec.counts);
  const receiverConnected = boolField(rec, "receiver_connected");
  const viewerCount = intField(rec, "viewer_count");
  const createdAt = intField(rec, "created_at_ms");
  if (
    sessionId === null ||
    name === null ||
    range === null ||
    counts === null ||
    receiverConnected === null ||
    viewerCount === null ||
    createdAt === null
  ) {
    return null;
  }
  const summary: SessionSummary = {
    session_id: sessionId,
    name,
    range,
    counts,
    receiver_connected: receiverConnected,
    viewer_count: viewerCount,
    created_at_ms: createdAt,
  };
  const firmware = optionalFirmware(rec, "firmware");
  if (firmware !== undefined) {
    summary.firmware = firmware;
  }
  return summary;
}

export function parseSerialCommand(value: unknown): SerialCommand | null {
  const rec = asRecord(value);
  if (rec === null) {
    return null;
  }
  const { cmd } = rec;
  if (cmd === "reset_all") {
    return { cmd: "reset_all" };
  }
  if (cmd === "wake_up") {
    return { cmd: "wake_up" };
  }
  if (cmd === "probe_firmware") {
    return { cmd: "probe_firmware" };
  }
  if (cmd !== "set_table") {
    return null;
  }
  const table = intField(rec, "table");
  const color = colorField(rec, "color");
  if (table === null || color === null) {
    return null;
  }
  return { cmd: "set_table", table, color };
}

export function errorMsg(rec: JsonRecord): { code: string; message: string } | null {
  const code = strField(rec, "code");
  const message = strField(rec, "message");
  if (code === null || message === null) {
    return null;
  }
  return { code, message };
}
