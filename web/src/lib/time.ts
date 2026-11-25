// SPDX-License-Identifier: AGPL-3.0-or-later
// Live longest-on-red computation anchored to server red_since_ms.

import type { TableEntry } from "@/protocol/types";

export const MAX_LONGEST_RED = 10;
export const MS_PER_SECOND = 1000;
export const SECONDS_PER_MINUTE = 60;
export const MINUTES_PER_HOUR = 60;
export const SECONDS_PER_HOUR = 3600;

export interface RedDuration {
  table: number;
  durationMs: number;
  label: string;
}

const PAD_LENGTH = 2;
const PAD_CHAR = "0";

function pad2(value: number): string {
  return String(value).padStart(PAD_LENGTH, PAD_CHAR);
}

/** Formats a duration as 42s, 2m 15s, or 1h 04m. */
export function formatDuration(durationMs: number): string {
  const clamped = Math.max(0, Math.floor(durationMs));
  const totalSeconds = Math.floor(clamped / MS_PER_SECOND);
  if (totalSeconds < SECONDS_PER_MINUTE) {
    return `${totalSeconds}s`;
  }
  if (totalSeconds < SECONDS_PER_HOUR) {
    const mins = Math.floor(totalSeconds / SECONDS_PER_MINUTE);
    const secs = totalSeconds % SECONDS_PER_MINUTE;
    return `${mins}m ${pad2(secs)}s`;
  }
  const hours = Math.floor(totalSeconds / SECONDS_PER_HOUR);
  const minutes = Math.floor((totalSeconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  return `${hours}h ${pad2(minutes)}m`;
}

function redDurationMs(entry: TableEntry, nowMs: number, clockOffsetMs: number): number | null {
  if (entry.color !== "red" || entry.red_since_ms === undefined || entry.red_since_ms === null) {
    return null;
  }
  return Math.max(0, nowMs + clockOffsetMs - entry.red_since_ms);
}

/** Top 10 red tables, longest first; ties broken by table number. */
export function computeLongestOnRed(
  tables: readonly TableEntry[],
  nowMs: number,
  clockOffsetMs: number,
): RedDuration[] {
  const durations: RedDuration[] = [];
  for (const entry of tables) {
    const durationMs = redDurationMs(entry, nowMs, clockOffsetMs);
    if (durationMs !== null) {
      durations.push({ table: entry.table, durationMs, label: formatDuration(durationMs) });
    }
  }
  durations.sort((left, right) => {
    if (right.durationMs !== left.durationMs) {
      return right.durationMs - left.durationMs;
    }
    return left.table - right.table;
  });
  return durations.slice(0, MAX_LONGEST_RED);
}
