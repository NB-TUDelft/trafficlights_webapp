// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { parseLobbyServerMsg } from "@/protocol/parse";
import { parseSessionServerMsg } from "@/protocol/parse-session";
import { initialSessionState, sessionReducer } from "@/protocol/reducers";
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
});
