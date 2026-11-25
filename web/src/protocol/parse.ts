// SPDX-License-Identifier: AGPL-3.0-or-later
// Runtime parsers for lobby WebSocket JSON and WASM serial events.
// Session message parsing lives in parse-session.ts.

import {
  asRecord,
  boolField,
  colorField,
  errorMsg,
  intField,
  type JsonRecord,
  parseFirmwareInfo,
  parseList,
  parseServerBuildInfo,
  parseSessionSummary,
  parseTextInput,
  strField,
} from "./guards";
import type { LobbyServerMsg, SerialEvent } from "./types";

function parseSerialEvent(value: unknown): SerialEvent | null {
  const rec = asRecord(value);
  if (rec === null) {
    return null;
  }
  const kind = rec.type;
  if (kind === "ignored_echo") {
    return { type: "ignored_echo" };
  }
  if (kind === "station_status") {
    return parseStationStatus(rec);
  }
  if (kind === "firmware_detected") {
    const info = parseFirmwareInfo(rec.info);
    return info === null ? null : { type: "firmware_detected", info };
  }
  if (kind === "control_error") {
    const reason = strField(rec, "reason");
    return reason === null ? null : { type: "control_error", reason };
  }
  return null;
}

function parseStationStatus(rec: JsonRecord): SerialEvent | null {
  const table = intField(rec, "table");
  const color = colorField(rec, "color");
  const relayed = boolField(rec, "relayed");
  if (table === null || color === null || relayed === null) {
    return null;
  }
  return { type: "station_status", table, color, relayed };
}

/** Validates the unknown return value of WASM bridge.feed_bytes(). */
export function parseSerialEvents(input: unknown): SerialEvent[] {
  const value = parseTextInput(input);
  if (!Array.isArray(value)) {
    return [];
  }
  const out: SerialEvent[] = [];
  for (const item of value) {
    const parsed = parseSerialEvent(item);
    if (parsed !== null) {
      out.push(parsed);
    }
  }
  return out;
}

export function parseLobbyServerMsg(input: unknown): LobbyServerMsg | null {
  const rec = asRecord(parseTextInput(input));
  if (rec === null) {
    return null;
  }
  const kind = rec.type;
  if (kind === "pong") {
    return { type: "pong" };
  }
  if (kind === "error") {
    const err = errorMsg(rec);
    return err === null ? null : { type: "error", code: err.code, message: err.message };
  }
  if (kind === "lobby_snapshot") {
    return parseLobbySnapshot(rec);
  }
  if (kind === "lobby_update") {
    const sessions = parseList(rec.sessions, parseSessionSummary);
    return sessions === null ? null : { type: "lobby_update", sessions };
  }
  if (kind === "session_created") {
    return parseSessionCreated(rec);
  }
  return null;
}

function parseLobbySnapshot(rec: JsonRecord): LobbyServerMsg | null {
  const sessions = parseList(rec.sessions, parseSessionSummary);
  const serverInfo = parseServerBuildInfo(rec.server_info);
  if (sessions === null || serverInfo === null) {
    return null;
  }
  return { type: "lobby_snapshot", sessions, server_info: serverInfo };
}

function parseSessionCreated(rec: JsonRecord): LobbyServerMsg | null {
  const sessionId = strField(rec, "session_id");
  const creatorToken = strField(rec, "creator_token");
  const summary = parseSessionSummary(rec.summary);
  if (sessionId === null || creatorToken === null || summary === null) {
    return null;
  }
  return {
    type: "session_created",
    session_id: sessionId,
    creator_token: creatorToken,
    summary,
  };
}
