// SPDX-License-Identifier: AGPL-3.0-or-later
// In-memory holder for a SerialPort picked on /new, consumed on /s/:id.

import type { SerialPortLike } from "./types";

interface PendingEntry {
  port: SerialPortLike;
  sessionId: string | null;
}

let pending: PendingEntry | null = null;

export function setPendingSerialPort(port: SerialPortLike): void {
  pending = { port, sessionId: null };
}

export function bindPendingSerialPort(sessionId: string): void {
  if (pending !== null) {
    pending = { port: pending.port, sessionId };
  }
}

export function peekPendingSerialPort(sessionId?: string): SerialPortLike | null {
  if (pending === null) {
    return null;
  }
  if (sessionId !== undefined && pending.sessionId !== null && pending.sessionId !== sessionId) {
    return null;
  }
  return pending.port;
}

export function takePendingSerialPort(sessionId: string): SerialPortLike | null {
  const port = peekPendingSerialPort(sessionId);
  if (port !== null) {
    pending = null;
  }
  return port;
}

export function clearPendingSerialPort(): void {
  pending = null;
}
