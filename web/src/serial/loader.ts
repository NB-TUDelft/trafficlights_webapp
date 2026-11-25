// SPDX-License-Identifier: AGPL-3.0-or-later
// Lazy WASM loading and Web Serial port picking. The dynamic import in
// loadRealWasm is the only WASM import in the app (requirement R7), so
// viewer routes never download the WASM chunk.

import type { SerialPortLike, WasmBridgeLike, WasmSerialApi } from "./types";
import { getSerialNavigator, isSecure } from "./types";

export const FALLBACK_BAUD_RATE = 115_200;
export const MICROBIT_USB_VID_FALLBACK = 0x0d_28;

const HEX_RADIX = 16;
const HEX_WIDTH = 4;

let wasmPromise: Promise<WasmSerialApi> | null = null;

async function loadRealWasm(): Promise<WasmSerialApi> {
  const mod = await import("@/wasm/trafficlights_wasm");
  await mod.default();
  return {
    proto_version: () => mod.proto_version(),
    microbit_usb_vid: () => mod.microbit_usb_vid(),
    serial_baud_rate: () => mod.serial_baud_rate(),
    format_wake_up: () => mod.format_wake_up(),
    format_probe_firmware: () => mod.format_probe_firmware(),
    createBridge: () => adaptBridge(new mod.SerialBridge()),
  };
}

function adaptBridge(inner: {
  feed_bytes: (bytes: Uint8Array) => unknown;
  format_set_table: (table: number, color: string) => string;
  format_reset_all: () => string;
  reset_dedup: () => void;
  free: () => void;
}): WasmBridgeLike {
  return {
    feed_bytes: (bytes: Uint8Array): unknown => {
      const raw: unknown = inner.feed_bytes(bytes);
      return raw;
    },
    format_set_table: (table: number, color: string) => inner.format_set_table(table, color),
    format_reset_all: () => inner.format_reset_all(),
    reset_dedup: () => inner.reset_dedup(),
    free: () => inner.free(),
  };
}

export function loadWasmSerialApi(): Promise<WasmSerialApi> {
  wasmPromise ??= loadRealWasm().catch((error: unknown) => {
    wasmPromise = null;
    throw error;
  });
  return wasmPromise;
}

export function isWebSerialSupported(): boolean {
  return isSecure() && getSerialNavigator() !== null;
}

export async function requestMicrobitPort(options?: {
  showAll?: boolean;
}): Promise<SerialPortLike> {
  const serial = getSerialNavigator();
  if (serial === null) {
    throw new Error("Web Serial is not available in this browser.");
  }
  if (options?.showAll === true) {
    return serial.requestPort();
  }
  let vid = MICROBIT_USB_VID_FALLBACK;
  try {
    const api = await loadWasmSerialApi();
    vid = api.microbit_usb_vid();
  } catch {
    vid = MICROBIT_USB_VID_FALLBACK;
  }
  return serial.requestPort({ filters: [{ usbVendorId: vid }] });
}

export function delay(ms: number): Promise<void> {
  return new Promise<void>((resolve) => {
    setTimeout(resolve, ms);
  });
}

function hexId(value: number | undefined): string {
  if (value === undefined) {
    return "????";
  }
  return value.toString(HEX_RADIX).toUpperCase().padStart(HEX_WIDTH, "0");
}

export function describePort(port: SerialPortLike): string {
  try {
    const info = port.getInfo();
    if (info.usbVendorId !== undefined) {
      return `micro:bit USB (${hexId(info.usbVendorId)}:${hexId(info.usbProductId)})`;
    }
  } catch {
    return "micro:bit USB serial";
  }
  return "micro:bit USB serial";
}
