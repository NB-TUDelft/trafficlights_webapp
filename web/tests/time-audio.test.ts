// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { enteredAlert, shouldBeepForTable } from "@/lib/audio";
import { computeLongestOnRed, formatDuration } from "@/lib/time";
import {
  CREATOR_TOKENS_KEY,
  getCreatorToken,
  hasCreatorToken,
  loadCreatorTokens,
  removeCreatorToken,
  type StorageLike,
  saveCreatorToken,
} from "@/protocol/storage";
import type { TableEntry } from "@/protocol/types";

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

describe("enteredAlert and shouldBeepForTable", () => {
  it("alerts only on newly entering orange or red", () => {
    expect(enteredAlert("green", "orange")).toBe(true);
    expect(enteredAlert("green", "red")).toBe(true);
    expect(enteredAlert(undefined, "red")).toBe(true);
    expect(enteredAlert(null, "orange")).toBe(true);
    expect(enteredAlert("orange", "red")).toBe(false);
    expect(enteredAlert("red", "orange")).toBe(false);
    expect(enteredAlert("green", "green")).toBe(false);
    expect(enteredAlert("red", "green")).toBe(false);
  });

  it("respects global beeps and per-table mute", () => {
    const base = { prev: "green" as const, next: "red" as const };
    expect(shouldBeepForTable({ ...base, beepsEnabled: true, muted: false })).toBe(true);
    expect(shouldBeepForTable({ ...base, beepsEnabled: false, muted: false })).toBe(false);
    expect(shouldBeepForTable({ ...base, beepsEnabled: true, muted: true })).toBe(false);
  });
});

describe("formatDuration", () => {
  it("formats seconds, minutes, and hours", () => {
    expect(formatDuration(0)).toBe("0s");
    expect(formatDuration(42_000)).toBe("42s");
    expect(formatDuration(135_000)).toBe("2m 15s");
    expect(formatDuration(125_000)).toBe("2m 05s");
    expect(formatDuration(3_840_000)).toBe("1h 04m");
    expect(formatDuration(-500)).toBe("0s");
  });
});

describe("computeLongestOnRed", () => {
  const now = 1_000_000;
  it("sorts longest first and ignores non-red or unanchored tables", () => {
    const tables: TableEntry[] = [
      { table: 1, color: "green", red_since_ms: now - 90_000 },
      { table: 2, color: "red", red_since_ms: now - 30_000 },
      { table: 3, color: "red", red_since_ms: now - 90_000 },
      { table: 4, color: "red" },
      { table: 5, color: "orange", red_since_ms: now - 999_999 },
    ];
    const result = computeLongestOnRed(tables, now, 0);
    expect(result.map((entry) => entry.table)).toEqual([3, 2]);
    expect(result[0]?.durationMs).toBe(90_000);
    expect(result[0]?.label).toBe("1m 30s");
  });

  it("applies clock offset and clamps negative durations", () => {
    const tables: TableEntry[] = [
      { table: 1, color: "red", red_since_ms: now + 5000 },
      { table: 2, color: "red", red_since_ms: now - 10_000 },
    ];
    const result = computeLongestOnRed(tables, now, 2000);
    expect(result[0]?.table).toBe(2);
    expect(result[0]?.durationMs).toBe(12_000);
    expect(result[1]?.durationMs).toBe(0);
  });

  it("keeps only the top 10", () => {
    const tables: TableEntry[] = Array.from({ length: 12 }, (_unused, index) => ({
      table: index + 1,
      color: "red" as const,
      red_since_ms: now - (index + 1) * 1000,
    }));
    const result = computeLongestOnRed(tables, now, 0);
    expect(result).toHaveLength(10);
    expect(result[0]?.table).toBe(12);
  });
});

describe("creator token storage", () => {
  it("round-trips tokens through injected storage", () => {
    const storage = createMemoryStorage();
    expect(getCreatorToken("a", storage)).toBeNull();
    saveCreatorToken("a", "tok-a", storage);
    saveCreatorToken("b", "tok-b", storage);
    expect(getCreatorToken("a", storage)).toBe("tok-a");
    expect(hasCreatorToken("b", storage)).toBe(true);
    expect(loadCreatorTokens(storage)).toEqual({ a: "tok-a", b: "tok-b" });
    expect(storage.getItem(CREATOR_TOKENS_KEY)).toContain("tok-a");
    removeCreatorToken("a", storage);
    expect(getCreatorToken("a", storage)).toBeNull();
    expect(getCreatorToken("b", storage)).toBe("tok-b");
  });

  it("survives corrupt data and unavailable storage", () => {
    const storage = createMemoryStorage();
    storage.setItem(CREATOR_TOKENS_KEY, "{broken");
    expect(loadCreatorTokens(storage)).toEqual({});
    storage.setItem(CREATOR_TOKENS_KEY, JSON.stringify({ good: "x", bad: 7 }));
    expect(loadCreatorTokens(storage)).toEqual({ good: "x" });
    const throwing: StorageLike = {
      getItem: () => {
        throw new Error("nope");
      },
      setItem: () => {
        throw new Error("nope");
      },
      removeItem: () => {
        throw new Error("nope");
      },
    };
    expect(getCreatorToken("a", throwing)).toBeNull();
    expect(() => saveCreatorToken("a", "t", throwing)).not.toThrow();
  });
});
