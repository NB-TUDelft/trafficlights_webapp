// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { parseLobbyServerMsg } from "@/protocol/parse";
import { parseSessionServerMsg } from "@/protocol/parse-session";
import { initialSessionState, sessionReducer } from "@/protocol/reducers";
import { tapTargetColor } from "@/protocol/types";
import { serverInfo, snapshotMsg, summary } from "./fixtures";

describe("protocol hardening", () => {
  it("clears creator state when superseded", () => {
    let state = sessionReducer(initialSessionState, snapshotMsg());
    state = sessionReducer(state, {
      type: "auth_ok",
      is_creator: true,
      unlisted_tables: [{ table: 0, color: "red", last_seen_ms: 99 }],
    });
    state = sessionReducer(state, {
      type: "error",
      code: "superseded",
      message: "Creator controls moved to a newer connection.",
    });
    expect(state.isCreator).toBe(false);
    expect(state.unlistedTables).toEqual([]);
    expect(state.error?.message).toBe("Session controls moved to another tab.");
  });

  it("falls back to the default source URL for non-https values", () => {
    const parsed = parseLobbyServerMsg({
      type: "lobby_snapshot",
      sessions: [],
      server_info: { ...serverInfo, source_url: "http://evil.example/repo" },
    });
    expect(parsed?.type).toBe("lobby_snapshot");
    if (parsed?.type === "lobby_snapshot") {
      expect(parsed.server_info.source_url).toBe(
        "https://github.com/NB-TUDelft/trafficlights_webapp",
      );
    }
    const script = parseLobbyServerMsg({
      type: "lobby_snapshot",
      sessions: [],
      server_info: { ...serverInfo, source_url: "javascript:alert(1)" },
    });
    if (script?.type === "lobby_snapshot") {
      expect(script.server_info.source_url.startsWith("https://")).toBe(true);
    }
  });

  it("rejects fractional millisecond timestamps", () => {
    const update = parseSessionServerMsg({
      type: "table_update",
      table: 1,
      color: "red",
      red_since_ms: 1.5,
      counts: { green: 0, orange: 0, red: 1 },
    });
    expect(update?.type).toBe("table_update");
    if (update?.type === "table_update") {
      expect(update.red_since_ms).toBeUndefined();
    }
    expect(
      parseLobbyServerMsg({
        type: "lobby_update",
        sessions: [{ ...summary, created_at_ms: 1.5 }],
      }),
    ).toBeNull();
  });

  it("parses and reduces tap mode and lock state from snapshot and session_meta", () => {
    expect(tapTargetColor("red", "green")).toBe("green");
    expect(tapTargetColor("orange", "green")).toBe("green");
    expect(tapTargetColor("green", "green")).toBe("green");
    expect(tapTargetColor("green", "cycle")).toBe("orange");
    expect(tapTargetColor("orange", "cycle")).toBe("red");
    expect(tapTargetColor("red", "cycle")).toBe("green");

    const parsedSnap = parseSessionServerMsg({
      ...snapshotMsg(),
      tap_mode: "cycle",
      tap_mode_locked: true,
    });
    expect(parsedSnap?.type).toBe("snapshot");
    if (parsedSnap?.type !== "snapshot") {
      return;
    }
    let state = sessionReducer(initialSessionState, parsedSnap);
    expect(state.tapMode).toBe("cycle");
    expect(state.tapModeLocked).toBe(true);

    const parsedMeta = parseSessionServerMsg({
      type: "session_meta",
      name: "Studio 3B",
      range: { start: 1, end: 60 },
      viewer_count: 4,
      tap_mode: "green",
      tap_mode_locked: false,
    });
    expect(parsedMeta?.type).toBe("session_meta");
    if (parsedMeta?.type !== "session_meta") {
      return;
    }
    state = sessionReducer(state, parsedMeta);
    expect(state.tapMode).toBe("green");
    expect(state.tapModeLocked).toBe(false);
  });
});
