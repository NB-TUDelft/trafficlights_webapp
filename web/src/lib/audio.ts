// SPDX-License-Identifier: AGPL-3.0-or-later
// Web Audio double beep from old/fastapi_receiver.py: 880 Hz then 660 Hz.

import type { Color } from "@/protocol/types";

export const FIRST_TONE_HZ = 880;
export const SECOND_TONE_HZ = 660;
export const TONE_DURATION_S = 0.12;
export const SECOND_TONE_OFFSET_S = 0.16;
export const TONE_START_DELAY_S = 0.01;
export const TONE_GAIN = 0.18;
export const TONE_GAIN_FLOOR = 0.001;
export const TONE_STOP_PAD_S = 0.02;

const ALERT_COLORS: ReadonlySet<Color> = new Set<Color>(["orange", "red"]);

/** True when a table newly enters orange/red from green or absent. */
export function enteredAlert(prev: Color | null | undefined, next: Color): boolean {
  if (!ALERT_COLORS.has(next)) {
    return false;
  }
  if (prev === null || prev === undefined) {
    return true;
  }
  return !ALERT_COLORS.has(prev);
}

export function shouldBeepForTable(args: {
  prev: Color | null | undefined;
  next: Color;
  beepsEnabled: boolean;
  muted: boolean;
}): boolean {
  if (!args.beepsEnabled || args.muted) {
    return false;
  }
  return enteredAlert(args.prev, args.next);
}

let sharedContext: AudioContext | null = null;

function resolveAudioConstructor(): typeof AudioContext | null {
  const standard: unknown = Reflect.get(globalThis, "AudioContext");
  if (typeof standard === "function") {
    return standard as typeof AudioContext;
  }
  const webkit: unknown = Reflect.get(globalThis, "webkitAudioContext");
  return typeof webkit === "function" ? (webkit as typeof AudioContext) : null;
}

function ensureAudioContext(): AudioContext | null {
  try {
    const Ctor = resolveAudioConstructor();
    if (Ctor === null) {
      return null;
    }
    sharedContext ??= new Ctor();
    if (sharedContext.state === "suspended") {
      sharedContext.resume().catch(() => undefined);
    }
    return sharedContext;
  } catch {
    return null;
  }
}

function playTone(ctx: AudioContext, freq: number, start: number): void {
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  osc.type = "square";
  osc.frequency.value = freq;
  gain.gain.setValueAtTime(TONE_GAIN, start);
  gain.gain.exponentialRampToValueAtTime(TONE_GAIN_FLOOR, start + TONE_DURATION_S);
  osc.connect(gain);
  gain.connect(ctx.destination);
  osc.start(start);
  osc.stop(start + TONE_DURATION_S + TONE_STOP_PAD_S);
}

/** Plays the 880/660 Hz double beep; silently no-ops without Web Audio. */
export function playAlertBeep(): void {
  const ctx = ensureAudioContext();
  if (ctx === null) {
    return;
  }
  try {
    const now = ctx.currentTime + TONE_START_DELAY_S;
    playTone(ctx, FIRST_TONE_HZ, now);
    playTone(ctx, SECOND_TONE_HZ, now + SECOND_TONE_OFFSET_S);
  } catch {
    // Audio is best-effort feedback only.
  }
}
