// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure state reducers for lobby and session WebSocket streams.

import type {
  ColorCounts,
  EndReason,
  FirmwareInfo,
  LobbyServerMsg,
  ServerBuildInfo,
  SessionServerMsg,
  SessionSummary,
  TableEntry,
  TableRange,
  TapMode,
  UnlistedTable,
} from "./types";
import {
  DEFAULT_BEEPS_ENABLED,
  DEFAULT_RANGE_END,
  DEFAULT_RANGE_START,
  DEFAULT_TAP_MODE,
} from "./types";

export interface CreatedSession {
  sessionId: string;
  creatorToken: string;
  summary: SessionSummary;
}

export interface LobbyState {
  sessions: SessionSummary[];
  serverInfo: ServerBuildInfo | null;
  created: CreatedSession | null;
  error: { code: string; message: string } | null;
}

export const initialLobbyState: LobbyState = {
  sessions: [],
  serverInfo: null,
  created: null,
  error: null,
};

function upsertSession(sessions: SessionSummary[], summary: SessionSummary): SessionSummary[] {
  const exists = sessions.some((item) => item.session_id === summary.session_id);
  if (!exists) {
    return [...sessions, summary];
  }
  return sessions.map((item) => (item.session_id === summary.session_id ? summary : item));
}

export function lobbyReducer(state: LobbyState, msg: LobbyServerMsg): LobbyState {
  if (msg.type === "lobby_snapshot") {
    return { ...state, sessions: msg.sessions, serverInfo: msg.server_info, error: null };
  }
  if (msg.type === "lobby_update") {
    return { ...state, sessions: msg.sessions };
  }
  if (msg.type === "session_created") {
    return {
      ...state,
      sessions: upsertSession(state.sessions, msg.summary),
      created: {
        sessionId: msg.session_id,
        creatorToken: msg.creator_token,
        summary: msg.summary,
      },
      error: null,
    };
  }
  if (msg.type === "error") {
    return { ...state, error: { code: msg.code, message: msg.message } };
  }
  return state;
}

export interface SessionEndedInfo {
  reason: EndReason;
  logAvailable: boolean;
}

export interface SessionState {
  sessionId: string | null;
  name: string;
  range: TableRange;
  tables: TableEntry[];
  counts: ColorCounts;
  receiverConnected: boolean;
  portLabel: string | null;
  firmware: FirmwareInfo | null;
  viewerCount: number;
  tapMode: TapMode;
  tapModeLocked: boolean;
  beepsEnabled: boolean;
  beepsLocked: boolean;
  isCreator: boolean;
  unlistedTables: UnlistedTable[];
  serverNowMs: number | null;
  clockOffsetMs: number;
  serverInfo: ServerBuildInfo | null;
  ended: SessionEndedInfo | null;
  error: { code: string; message: string } | null;
  hasSnapshot: boolean;
}

export const initialSessionState: SessionState = {
  sessionId: null,
  name: "",
  range: { start: DEFAULT_RANGE_START, end: DEFAULT_RANGE_END },
  tables: [],
  counts: { green: 0, orange: 0, red: 0 },
  receiverConnected: false,
  portLabel: null,
  firmware: null,
  viewerCount: 0,
  tapMode: DEFAULT_TAP_MODE,
  tapModeLocked: false,
  beepsEnabled: DEFAULT_BEEPS_ENABLED,
  beepsLocked: false,
  isCreator: false,
  unlistedTables: [],
  serverNowMs: null,
  clockOffsetMs: 0,
  serverInfo: null,
  ended: null,
  error: null,
  hasSnapshot: false,
};

type SnapshotMsg = Extract<SessionServerMsg, { type: "snapshot" }>;
type TableUpdateMsg = Extract<SessionServerMsg, { type: "table_update" }>;
type ReceiverStatusMsg = Extract<SessionServerMsg, { type: "receiver_status" }>;

function fromSnapshot(state: SessionState, msg: SnapshotMsg): SessionState {
  return {
    ...state,
    sessionId: msg.session_id,
    name: msg.name,
    range: msg.range,
    tables: msg.tables,
    counts: msg.counts,
    receiverConnected: msg.receiver_connected,
    portLabel: msg.port_label ?? null,
    firmware: msg.firmware ?? null,
    viewerCount: msg.viewer_count,
    tapMode: msg.tap_mode ?? DEFAULT_TAP_MODE,
    tapModeLocked: msg.tap_mode_locked ?? false,
    beepsEnabled: msg.beeps_enabled ?? DEFAULT_BEEPS_ENABLED,
    beepsLocked: msg.beeps_locked ?? false,
    isCreator: msg.is_creator,
    unlistedTables: msg.unlisted_tables ?? [],
    serverNowMs: msg.server_now_ms,
    clockOffsetMs: msg.server_now_ms - Date.now(),
    serverInfo: msg.server_info,
    hasSnapshot: true,
    error: null,
  };
}

function applyTableUpdate(state: SessionState, msg: TableUpdateMsg): SessionState {
  const tables = state.tables.map((entry) =>
    entry.table === msg.table
      ? { table: msg.table, color: msg.color, red_since_ms: msg.red_since_ms }
      : entry,
  );
  const cleaned = tables.map((entry) =>
    entry.red_since_ms === undefined
      ? { table: entry.table, color: entry.color }
      : { table: entry.table, color: entry.color, red_since_ms: entry.red_since_ms },
  );
  return { ...state, tables: cleaned, counts: msg.counts };
}

function applyReceiverStatus(state: SessionState, msg: ReceiverStatusMsg): SessionState {
  return {
    ...state,
    receiverConnected: msg.connected,
    portLabel: msg.port_label ?? (msg.connected ? state.portLabel : null),
    firmware: msg.firmware ?? (msg.connected ? state.firmware : null),
  };
}

export function sessionReducer(state: SessionState, msg: SessionServerMsg): SessionState {
  if (msg.type === "snapshot") {
    return fromSnapshot(state, msg);
  }
  if (msg.type === "auth_ok") {
    return { ...state, isCreator: msg.is_creator, unlistedTables: msg.unlisted_tables ?? [] };
  }
  if (msg.type === "table_update") {
    return applyTableUpdate(state, msg);
  }
  if (msg.type === "receiver_status") {
    return applyReceiverStatus(state, msg);
  }
  if (msg.type === "session_meta") {
    return {
      ...state,
      name: msg.name,
      range: msg.range,
      viewerCount: msg.viewer_count,
      tapMode: msg.tap_mode ?? state.tapMode,
      tapModeLocked: msg.tap_mode_locked ?? state.tapModeLocked,
      beepsEnabled: msg.beeps_enabled ?? state.beepsEnabled,
      beepsLocked: msg.beeps_locked ?? state.beepsLocked,
    };
  }
  if (msg.type === "unlisted_tables") {
    return { ...state, unlistedTables: msg.tables };
  }
  if (msg.type === "session_ended") {
    return { ...state, ended: { reason: msg.reason, logAvailable: msg.log_available } };
  }
  if (msg.type === "error") {
    if (msg.code === "superseded") {
      return {
        ...state,
        isCreator: false,
        unlistedTables: [],
        error: { code: msg.code, message: "Session controls moved to another tab." },
      };
    }
    return { ...state, error: { code: msg.code, message: msg.message } };
  }
  return state;
}
