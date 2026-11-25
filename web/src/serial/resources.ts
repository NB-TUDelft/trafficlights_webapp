// SPDX-License-Identifier: AGPL-3.0-or-later
// Best-effort browser resource helpers for the creator serial bridge.

import type { SerialPortLike, WakeLockSentinelLike, WasmBridgeLike } from "./types";
import { getWakeLock } from "./types";

export async function acquireWakeLock(): Promise<WakeLockSentinelLike | null> {
  try {
    const wakeLock = getWakeLock();
    if (wakeLock !== null) {
      return await wakeLock.request("screen");
    }
  } catch {
    // Wake lock is optional and may fail on unsupported or background tabs.
  }
  return null;
}

export async function releaseWakeLock(sentinel: WakeLockSentinelLike | null): Promise<void> {
  try {
    await sentinel?.release();
  } catch {
    // Wake lock release is best-effort.
  }
}

export async function releaseSerialStreams(
  reader: ReadableStreamDefaultReader<Uint8Array> | null,
  writer: WritableStreamDefaultWriter<Uint8Array> | null,
): Promise<void> {
  try {
    await reader?.cancel();
  } catch {
    // Ignore cancel failures during teardown.
  }
  try {
    reader?.releaseLock();
  } catch {
    // Ignore lock failures during teardown.
  }
  try {
    writer?.releaseLock();
  } catch {
    // Ignore lock failures during teardown.
  }
}

export async function closeSerialPort(port: SerialPortLike | null): Promise<void> {
  try {
    await port?.close();
  } catch {
    // Ignore close failures during teardown.
  }
}

export function freeWasmBridge(bridge: WasmBridgeLike | null): void {
  try {
    bridge?.free();
  } catch {
    // Ignore free failures during teardown.
  }
}
