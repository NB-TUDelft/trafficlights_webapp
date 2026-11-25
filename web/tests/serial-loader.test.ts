// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from "vitest";
import {
  delay,
  describePort,
  isWebSerialSupported,
  loadWasmSerialApi,
  requestMicrobitPort,
} from "@/serial/loader";
import type { SerialPortLike } from "@/serial/types";

function fakePort(info: () => { usbVendorId?: number; usbProductId?: number }): SerialPortLike {
  return {
    readable: null,
    writable: null,
    open: () => Promise.resolve(),
    close: () => Promise.resolve(),
    getInfo: info,
  };
}

describe("WASM loader cache", () => {
  it("does not cache a rejected load, so a retry can succeed later", async () => {
    const first = loadWasmSerialApi();
    await expect(first).rejects.toThrow();
    const second = loadWasmSerialApi();
    expect(second).not.toBe(first);
    await expect(second).rejects.toThrow();
  });
});

describe("loader helpers", () => {
  it("describes ports with and without USB info", () => {
    expect(describePort(fakePort(() => ({ usbVendorId: 0x0d_28, usbProductId: 0x02_04 })))).toBe(
      "micro:bit USB (0D28:0204)",
    );
    expect(describePort(fakePort(() => ({ usbVendorId: 0x0d_28 })))).toBe(
      "micro:bit USB (0D28:????)",
    );
    expect(describePort(fakePort(() => ({})))).toBe("micro:bit USB serial");
    expect(
      describePort(
        fakePort(() => {
          throw new Error("gone");
        }),
      ),
    ).toBe("micro:bit USB serial");
  });

  it("delays, reports unsupported Web Serial, and refuses to pick a port", async () => {
    const started = Date.now();
    await delay(5);
    expect(Date.now() - started).toBeGreaterThanOrEqual(4);
    expect(isWebSerialSupported()).toBe(false);
    await expect(requestMicrobitPort()).rejects.toThrow("Web Serial is not available");
    await expect(requestMicrobitPort({ showAll: true })).rejects.toThrow(
      "Web Serial is not available",
    );
  });
});
