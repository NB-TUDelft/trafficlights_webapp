// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure state resolution and toggle handlers for tap mode, global beeps, and per-table mute.

import type { SessionState } from "@/protocol/reducers";
import type { SessionClientMsg, TapMode } from "@/protocol/types";

export interface TapModeStateSlice {
  isCreator: boolean;
  tapMode: TapMode;
  tapModeLocked: boolean;
}

export interface TapModeUiSlice {
  hostTapMode: TapMode;
  localTapMode: TapMode;
}

export interface BeepStateSlice {
  isCreator: boolean;
  beepsEnabled: boolean;
  beepsLocked: boolean;
}

export interface BeepUiSlice {
  hostBeepsEnabled: boolean;
  localBeepsEnabled: boolean;
}

export interface ModeHandlersArgs {
  state: Pick<SessionState, "isCreator" | "tapModeLocked" | "beepsLocked">;
  send: (msg: SessionClientMsg) => boolean;
  hostTapMode: TapMode;
  hostTapModeLocked: boolean;
  setLocalTapMode: (updater: (prev: TapMode) => TapMode) => void;
  setHostTapMode: (value: TapMode) => void;
  setHostTapModeLocked: (value: boolean) => void;
  hostBeepsEnabled: boolean;
  hostBeepsLocked: boolean;
  setLocalBeepsEnabled: (updater: (prev: boolean) => boolean) => void;
  setHostBeepsEnabled: (value: boolean) => void;
  setHostBeepsLocked: (value: boolean) => void;
}

export function resolveTapMode(
  state: Readonly<TapModeStateSlice>,
  ui: Readonly<TapModeUiSlice>,
): TapMode {
  if (state.isCreator) {
    return ui.hostTapMode;
  }
  return state.tapModeLocked ? state.tapMode : ui.localTapMode;
}

export function resolveBeepsEnabled(
  state: Readonly<BeepStateSlice>,
  ui: Readonly<BeepUiSlice>,
): boolean {
  if (state.isCreator) {
    return ui.hostBeepsEnabled;
  }
  return state.beepsLocked ? state.beepsEnabled : ui.localBeepsEnabled;
}

export function toggleTapMode(args: Readonly<ModeHandlersArgs>): void {
  if (args.state.isCreator) {
    const next: TapMode = args.hostTapMode === "cycle" ? "green" : "cycle";
    args.setHostTapMode(next);
    args.send({ type: "set_tap_mode", tap_mode: next, locked: args.hostTapModeLocked });
    return;
  }
  if (args.state.tapModeLocked) {
    return;
  }
  args.setLocalTapMode((prev) => (prev === "cycle" ? "green" : "cycle"));
}

export function toggleTapModeLock(args: Readonly<ModeHandlersArgs>): void {
  if (!args.state.isCreator) {
    return;
  }
  const nextLocked = !args.hostTapModeLocked;
  args.setHostTapModeLocked(nextLocked);
  args.send({ type: "set_tap_mode", tap_mode: args.hostTapMode, locked: nextLocked });
}

export function toggleBeeps(args: Readonly<ModeHandlersArgs>): void {
  if (args.state.isCreator) {
    const next = !args.hostBeepsEnabled;
    args.setHostBeepsEnabled(next);
    args.send({ type: "set_beep_mode", enabled: next, locked: args.hostBeepsLocked });
    return;
  }
  if (args.state.beepsLocked) {
    return;
  }
  args.setLocalBeepsEnabled((prev) => !prev);
}

export function toggleBeepsLock(args: Readonly<ModeHandlersArgs>): void {
  if (!args.state.isCreator) {
    return;
  }
  const nextLocked = !args.hostBeepsLocked;
  args.setHostBeepsLocked(nextLocked);
  args.send({ type: "set_beep_mode", enabled: args.hostBeepsEnabled, locked: nextLocked });
}

export function toggleMuted(prev: ReadonlySet<number>, table: number): ReadonlySet<number> {
  const next = new Set(prev);
  if (next.has(table)) {
    next.delete(table);
  } else {
    next.add(table);
  }
  return next;
}

export function toggleMuteWhenActive(
  beepsEnabled: boolean,
  table: number,
  setMutedTables: (updater: (prev: ReadonlySet<number>) => ReadonlySet<number>) => void,
): void {
  if (!beepsEnabled) {
    return;
  }
  setMutedTables((prev) => toggleMuted(prev, table));
}
