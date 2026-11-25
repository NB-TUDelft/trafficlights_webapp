// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { parseLobbyServerMsg, parseSerialEvents } from "@/protocol/parse";
import { parseSessionServerMsg } from "@/protocol/parse-session";
import {
  initialLobbyState,
  initialSessionState,
  lobbyReducer,
  sessionReducer,
} from "@/protocol/reducers";
import { isValidTableRange, nextCycleColor, normalizeSessionName } from "@/protocol/types";
import { serverInfo, snapshotMsg, summary } from "./fixtures";

describe("nextCycleColor and validation helpers", () => {
  it("cycles green to orange to red to green", () => {
    expect(nextCycleColor("green")).toBe("orange");
    expect(nextCycleColor("orange")).toBe("red");
    expect(nextCycleColor("red")).toBe("green");
  });

  it("normalizes session names like the Rust proto crate", () => {
    expect(normalizeSessionName("  Studio 3B — Week 4  ")).toBe("Studio 3B — Week 4");
    expect(normalizeSessionName("x".repeat(80))).toBe("x".repeat(80));
    expect(normalizeSessionName("🚦".repeat(80))).toBe("🚦".repeat(80));
    expect(normalizeSessionName("")).toBeNull();
    expect(normalizeSessionName("   ")).toBeNull();
    expect(normalizeSessionName("x".repeat(81))).toBeNull();
    expect(normalizeSessionName("two\nlines")).toBeNull();
    expect(normalizeSessionName("has\ttab")).toBeNull();
  });

  it("validates table ranges at the boundaries", () => {
    expect(isValidTableRange(1, 60)).toBe(true);
    expect(isValidTableRange(1, 1)).toBe(true);
    expect(isValidTableRange(9999, 9999)).toBe(true);
    expect(isValidTableRange(0, 60)).toBe(false);
    expect(isValidTableRange(5, 4)).toBe(false);
    expect(isValidTableRange(1, 10_000)).toBe(false);
    expect(isValidTableRange(1.5, 60)).toBe(false);
  });
});

describe("parseLobbyServerMsg", () => {
  it("parses snapshot, update, created, error, and pong", () => {
    const snapshot = {
      type: "lobby_snapshot",
      sessions: [summary],
      server_info: serverInfo,
    };
    expect(parseLobbyServerMsg(snapshot)).toEqual(snapshot);
    expect(parseLobbyServerMsg(JSON.stringify(snapshot))).toEqual(snapshot);
    expect(parseLobbyServerMsg({ type: "lobby_update", sessions: [] })).toEqual({
      type: "lobby_update",
      sessions: [],
    });
    expect(
      parseLobbyServerMsg({
        type: "session_created",
        session_id: "sess-1",
        creator_token: "tok",
        summary,
      }),
    ).toEqual({
      type: "session_created",
      session_id: "sess-1",
      creator_token: "tok",
      summary,
    });
    expect(parseLobbyServerMsg({ type: "error", code: "invalid_pin", message: "wrong" })).toEqual({
      type: "error",
      code: "invalid_pin",
      message: "wrong",
    });
    expect(parseLobbyServerMsg({ type: "pong" })).toEqual({ type: "pong" });
  });

  it("rejects malformed and unknown lobby messages", () => {
    expect(parseLobbyServerMsg(null)).toBeNull();
    expect(parseLobbyServerMsg("not json")).toBeNull();
    expect(parseLobbyServerMsg({ type: "explode" })).toBeNull();
    expect(parseLobbyServerMsg({ type: "lobby_snapshot", sessions: [] })).toBeNull();
    expect(
      parseLobbyServerMsg({
        type: "lobby_update",
        sessions: [{ ...summary, counts: { green: "many", orange: 0, red: 0 } }],
      }),
    ).toBeNull();
  });
});

describe("parseSessionServerMsg", () => {
  it("parses a full snapshot with omitted optional fields", () => {
    const parsed = parseSessionServerMsg({
      type: "snapshot",
      session_id: "sess-1",
      name: "Studio 3B",
      range: { start: 1, end: 2 },
      tables: [
        { table: 1, color: "green" },
        { table: 2, color: "red", red_since_ms: 42 },
      ],
      counts: { green: 1, orange: 0, red: 1 },
      receiver_connected: true,
      viewer_count: 2,
      is_creator: true,
      server_now_ms: 1000,
      server_info: serverInfo,
    });
    expect(parsed?.type).toBe("snapshot");
    if (parsed?.type === "snapshot") {
      expect(parsed.unlisted_tables).toEqual([]);
      expect(parsed.tables[1]?.red_since_ms).toBe(42);
      expect(parsed.port_label).toBeUndefined();
    }
  });

  it("parses incremental and creator messages", () => {
    expect(
      parseSessionServerMsg({
        type: "table_update",
        table: 12,
        color: "red",
        red_since_ms: 42,
        counts: { green: 59, orange: 0, red: 1 },
      }),
    ).toEqual({
      type: "table_update",
      table: 12,
      color: "red",
      red_since_ms: 42,
      counts: { green: 59, orange: 0, red: 1 },
    });
    expect(
      parseSessionServerMsg({
        type: "serial_command",
        command: { cmd: "set_table", table: 12, color: "green" },
      }),
    ).toEqual({
      type: "serial_command",
      command: { cmd: "set_table", table: 12, color: "green" },
    });
    expect(
      parseSessionServerMsg({ type: "serial_command", command: { cmd: "reset_all" } }),
    ).toEqual({ type: "serial_command", command: { cmd: "reset_all" } });
    expect(
      parseSessionServerMsg({
        type: "session_ended",
        reason: "grace_timeout",
        log_available: true,
      }),
    ).toEqual({ type: "session_ended", reason: "grace_timeout", log_available: true });
    expect(parseSessionServerMsg({ type: "auth_ok", is_creator: true })).toEqual({
      type: "auth_ok",
      is_creator: true,
      unlisted_tables: [],
    });
  });

  it("rejects unknown types and bad colors", () => {
    expect(parseSessionServerMsg({ type: "explode" })).toBeNull();
    expect(
      parseSessionServerMsg({
        type: "table_update",
        table: 1,
        color: "blue",
        counts: { green: 0, orange: 0, red: 0 },
      }),
    ).toBeNull();
    expect(parseSessionServerMsg(42)).toBeNull();
  });
});

describe("parseSerialEvents", () => {
  it("validates WASM feed_bytes output and drops garbage entries", () => {
    const events = parseSerialEvents([
      { type: "station_status", table: 5, color: "red", relayed: false },
      {
        type: "firmware_detected",
        info: { kind: "new", mode: "receiver", version: "draft-1" },
      },
      { type: "control_error", reason: "bad id" },
      { type: "ignored_echo" },
      { type: "station_status", table: "five", color: "red", relayed: false },
      null,
    ]);
    expect(events).toHaveLength(4);
    expect(events[0]).toEqual({
      type: "station_status",
      table: 5,
      color: "red",
      relayed: false,
    });
    expect(events[1]?.type).toBe("firmware_detected");
    expect(parseSerialEvents("nope")).toEqual([]);
    expect(parseSerialEvents(undefined)).toEqual([]);
  });
});

describe("lobbyReducer", () => {
  it("applies snapshots, updates, creation, and errors", () => {
    let state = lobbyReducer(initialLobbyState, {
      type: "lobby_snapshot",
      sessions: [summary],
      server_info: serverInfo,
    });
    expect(state.sessions).toHaveLength(1);
    expect(state.serverInfo?.proto_version).toBe(1);
    state = lobbyReducer(state, { type: "lobby_update", sessions: [] });
    expect(state.sessions).toEqual([]);
    state = lobbyReducer(state, {
      type: "session_created",
      session_id: "sess-1",
      creator_token: "tok",
      summary,
    });
    expect(state.created?.creatorToken).toBe("tok");
    expect(state.sessions[0]?.session_id).toBe("sess-1");
    state = lobbyReducer(state, { type: "error", code: "invalid_pin", message: "nope" });
    expect(state.error?.code).toBe("invalid_pin");
    expect(lobbyReducer(state, { type: "pong" })).toBe(state);
  });
});

describe("sessionReducer", () => {
  it("applies a snapshot and anchors the clock offset", () => {
    const msg = snapshotMsg();
    const state = sessionReducer(initialSessionState, msg);
    expect(state.name).toBe("Studio 3B");
    expect(state.tables).toHaveLength(2);
    expect(state.firmware).toEqual({ kind: "legacy" });
    expect(state.hasSnapshot).toBe(true);
    expect(Math.abs(state.clockOffsetMs)).toBeLessThan(5000);
  });

  it("promotes to creator on auth_ok and applies updates", () => {
    let state = sessionReducer(initialSessionState, snapshotMsg());
    state = sessionReducer(state, {
      type: "auth_ok",
      is_creator: true,
      unlisted_tables: [{ table: 0, color: "red", last_seen_ms: 99 }],
    });
    expect(state.isCreator).toBe(true);
    expect(state.unlistedTables[0]?.table).toBe(0);
    state = sessionReducer(state, {
      type: "table_update",
      table: 1,
      color: "orange",
      counts: { green: 0, orange: 1, red: 1 },
    });
    expect(state.tables[0]?.color).toBe("orange");
    expect(state.tables[0]?.red_since_ms).toBeUndefined();
    state = sessionReducer(state, {
      type: "receiver_status",
      connected: true,
      port_label: "micro:bit USB",
      firmware: { kind: "new", mode: "receiver", version: "draft-1" },
    });
    expect(state.receiverConnected).toBe(true);
    expect(state.firmware?.version).toBe("draft-1");
    state = sessionReducer(state, {
      type: "session_meta",
      name: "Renamed",
      range: { start: 1, end: 9 },
      viewer_count: 7,
    });
    expect(state.name).toBe("Renamed");
    expect(state.viewerCount).toBe(7);
  });

  it("records unlisted tables, end, and errors; ignores pong and serial commands", () => {
    let state = sessionReducer(initialSessionState, snapshotMsg());
    state = sessionReducer(state, {
      type: "unlisted_tables",
      tables: [{ table: 99, color: "orange", last_seen_ms: 5 }],
    });
    expect(state.unlistedTables[0]?.table).toBe(99);
    expect(sessionReducer(state, { type: "pong" })).toBe(state);
    expect(
      sessionReducer(state, {
        type: "serial_command",
        command: { cmd: "reset_all" },
      }),
    ).toBe(state);
    state = sessionReducer(state, { type: "error", code: "receiver_offline", message: "off" });
    expect(state.error?.code).toBe("receiver_offline");
    state = sessionReducer(state, {
      type: "session_ended",
      reason: "manual",
      log_available: true,
    });
    expect(state.ended).toEqual({ reason: "manual", logAvailable: true });
  });
});
