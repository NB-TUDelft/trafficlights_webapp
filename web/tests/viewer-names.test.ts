// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { isNameError } from "@/pages/sessionStateHooks";
import { parseSessionServerMsg } from "@/protocol/parse-session";
import { initialSessionState, sessionReducer } from "@/protocol/reducers";
import {
  getLastViewerName,
  getViewerKey,
  getViewerName,
  LAST_VIEWER_NAME_KEY,
  type StorageLike,
  saveViewerName,
  VIEWER_KEY_KEY,
} from "@/protocol/storage";
import { HOST_ACTOR, MAX_VIEWER_NAME_CHARS, normalizeViewerName } from "@/protocol/types";
import { snapshotMsg } from "./fixtures";

function createMemoryStorage(): StorageLike {
  const map = new Map<string, string>();
  return {
    getItem: (key: string): string | null => map.get(key) ?? null,
    setItem: (key: string, value: string): void => {
      map.set(key, value);
    },
    removeItem: (key: string): void => {
      map.delete(key);
    },
  };
}

describe("normalizeViewerName", () => {
  it("trims and keeps classroom names like the Rust proto crate", () => {
    expect(normalizeViewerName("  Alice  ")).toBe("Alice");
    expect(normalizeViewerName("Ms. Núñez")).toBe("Ms. Núñez");
    expect(normalizeViewerName("Host Bob")).toBe("Host Bob");
    expect(normalizeViewerName("x".repeat(MAX_VIEWER_NAME_CHARS))).toBe("x".repeat(40));
    expect(normalizeViewerName("🚦".repeat(40))).toBe("🚦".repeat(40));
  });

  it("rejects empty, overlong, control, and host names", () => {
    expect(HOST_ACTOR).toBe("host");
    for (const name of ["", "   ", "x".repeat(41), "two\nlines", "bell\u0007", "host", " HOST "]) {
      expect(normalizeViewerName(name)).toBeNull();
    }
  });
});

describe("collect_names in session messages", () => {
  it("parses the flag and keeps it in session state", () => {
    const parsed = parseSessionServerMsg({ ...snapshotMsg(), collect_names: true });
    expect(parsed?.type === "snapshot" && parsed.collect_names).toBe(true);
    if (parsed !== null) {
      expect(sessionReducer(initialSessionState, parsed).collectNames).toBe(true);
    }
  });

  it("treats an absent or malformed flag as no name collection", () => {
    for (const extra of [{}, { collect_names: "yes" }]) {
      const parsed = parseSessionServerMsg({ ...snapshotMsg(), ...extra });
      expect(parsed?.type === "snapshot" && parsed.collect_names).toBeFalsy();
      if (parsed !== null) {
        expect(sessionReducer(initialSessionState, parsed).collectNames).toBe(false);
      }
    }
  });

  it("recognizes the name errors the name prompt shows", () => {
    for (const code of ["name_taken", "name_required", "invalid_viewer_name"]) {
      expect(isNameError(code)).toBe(true);
    }
    expect(isNameError("receiver_disconnected")).toBe(false);
  });
});

describe("viewer name storage", () => {
  it("remembers names per session and suggests the last one", () => {
    const storage = createMemoryStorage();
    expect(getViewerName("sess-1", storage)).toBeNull();
    expect(getLastViewerName(storage)).toBeNull();
    saveViewerName("sess-1", "Alice", storage);
    saveViewerName("sess-2", "Bokwa", storage);
    expect(getViewerName("sess-1", storage)).toBe("Alice");
    expect(getViewerName("sess-2", storage)).toBe("Bokwa");
    expect(getViewerName("sess-3", storage)).toBeNull();
    expect(getLastViewerName(storage)).toBe("Bokwa");
    expect(storage.getItem(LAST_VIEWER_NAME_KEY)).toBe("Bokwa");
  });

  it("keeps one random reconnect key per tab", () => {
    const storage = createMemoryStorage();
    const key = getViewerKey(storage);
    expect(key).toMatch(/^[0-9a-f]{32}$/u);
    expect(getViewerKey(storage)).toBe(key);
    expect(storage.getItem(VIEWER_KEY_KEY)).toBe(key);
  });

  it("survives unavailable storage", () => {
    const throwing: StorageLike = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
      removeItem: () => {
        throw new Error("blocked");
      },
    };
    expect(() => saveViewerName("sess-1", "Alice", throwing)).not.toThrow();
    expect(getViewerName("sess-1", throwing)).toBeNull();
    expect(getLastViewerName(throwing)).toBeNull();
    expect(getViewerKey(throwing)).toMatch(/^[0-9a-f]{32}$/u);
  });
});
