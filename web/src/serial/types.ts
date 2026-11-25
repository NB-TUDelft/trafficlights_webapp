// SPDX-License-Identifier: AGPL-3.0-or-later
// Minimal Web Serial and Wake Lock declarations (no @types/w3c-web-serial).

import type { Color, FirmwareInfo } from "@/protocol/types";

export interface SerialPortInfoLike {
  usbVendorId?: number;
  usbProductId?: number;
}

export interface SerialPortFilter {
  usbVendorId?: number;
  usbProductId?: number;
}

export interface SerialPortRequestOptions {
  filters?: SerialPortFilter[];
}

export interface SerialOpenOptions {
  baudRate: number;
}

export interface SerialPortLike {
  readable: ReadableStream<Uint8Array> | null;
  writable: WritableStream<Uint8Array> | null;
  open: (options: SerialOpenOptions) => Promise<void>;
  close: () => Promise<void>;
  getInfo: () => SerialPortInfoLike;
}

export interface SerialNavigatorLike {
  requestPort: (options?: SerialPortRequestOptions) => Promise<SerialPortLike>;
  getPorts: () => Promise<SerialPortLike[]>;
}

export interface WakeLockSentinelLike {
  release: () => Promise<void>;
}

export interface WakeLockLike {
  request: (type: "screen") => Promise<WakeLockSentinelLike>;
}

export interface WasmBridgeLike {
  feed_bytes: (bytes: Uint8Array) => unknown;
  format_set_table: (table: number, color: string) => string;
  format_reset_all: () => string;
  reset_dedup: () => void;
  free: () => void;
}

export interface WasmSerialApi {
  microbit_usb_vid: () => number;
  serial_baud_rate: () => number;
  format_wake_up: () => string;
  format_probe_firmware: () => string;
  createBridge: () => WasmBridgeLike;
}

type AnyFunction = (...args: unknown[]) => unknown;

function isFunction(value: unknown): value is AnyFunction {
  return typeof value === "function";
}

export function getSerialNavigator(): SerialNavigatorLike | null {
  if (typeof navigator === "undefined") {
    return null;
  }
  const candidate: unknown = Reflect.get(navigator, "serial");
  if (typeof candidate !== "object" || candidate === null) {
    return null;
  }
  const requestPort: unknown = Reflect.get(candidate, "requestPort");
  const getPorts: unknown = Reflect.get(candidate, "getPorts");
  if (!(isFunction(requestPort) && isFunction(getPorts))) {
    return null;
  }
  return {
    requestPort: (options?: SerialPortRequestOptions) =>
      Reflect.apply(requestPort, candidate, [options]) as Promise<SerialPortLike>,
    getPorts: () => Reflect.apply(getPorts, candidate, []) as Promise<SerialPortLike[]>,
  };
}

export function getWakeLock(): WakeLockLike | null {
  if (typeof navigator === "undefined") {
    return null;
  }
  const candidate: unknown = Reflect.get(navigator, "wakeLock");
  if (typeof candidate !== "object" || candidate === null) {
    return null;
  }
  const request: unknown = Reflect.get(candidate, "request");
  if (!isFunction(request)) {
    return null;
  }
  return {
    request: (type: "screen") =>
      Reflect.apply(request, candidate, [type]) as Promise<WakeLockSentinelLike>,
  };
}

export function isSecure(): boolean {
  return globalThis.window?.isSecureContext === true;
}

export interface SerialStatusUpdate {
  connected: boolean;
  portLabel: string | null;
  firmware: FirmwareInfo | null;
}

export interface SerialBridgeCallbacks {
  onStationReport: (table: number, color: Color) => void;
  onStatus: (status: SerialStatusUpdate) => void;
  onError?: (message: string) => void;
}

export interface SerialBridgeOptions {
  wasmApi?: WasmSerialApi | Promise<WasmSerialApi>;
  probeTimeoutMs?: number;
  wakeDelayMs?: number;
  paceMs?: number;
}
