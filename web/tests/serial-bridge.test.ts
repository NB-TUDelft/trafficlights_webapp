// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from "vitest";
import type { Color, SerialEvent } from "@/protocol/types";
import { CreatorSerialBridge } from "@/serial/bridge";
import type {
  SerialPortLike,
  SerialStatusUpdate,
  WasmBridgeLike,
  WasmSerialApi,
} from "@/serial/types";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function sleep(ms: number): Promise<void> {
  return new Promise<void>((resolve) => {
    setTimeout(resolve, ms);
  });
}

interface FakeBridgeHandle {
  bridge: WasmBridgeLike;
  freed: () => boolean;
}

function createFakeBridge(): FakeBridgeHandle {
  let buffer = "";
  let freedFlag = false;
  const bridge: WasmBridgeLike = {
    feed_bytes: (bytes: Uint8Array): unknown => {
      buffer += decoder.decode(bytes);
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";
      const events: SerialEvent[] = [];
      for (const line of lines) {
        const event = parseFakeLine(line.trim());
        if (event !== null) {
          events.push(event);
        }
      }
      return events;
    },
    format_set_table: (table: number, color: string): string => {
      const codes: Record<string, number> = { green: 0, orange: 1, red: 2 };
      return `T,${table},${codes[color] ?? 0}\n`;
    },
    format_reset_all: (): string => "T,-1,0\n",
    reset_dedup: (): void => {
      buffer = "";
    },
    free: (): void => {
      freedFlag = true;
    },
  };
  return { bridge, freed: () => freedFlag };
}

function parseFakeLine(line: string): SerialEvent | null {
  if (line.startsWith("!ok mode=station")) {
    const match = /id=(?<stationId>\d+)/u.exec(line);
    return {
      type: "firmware_detected",
      info: {
        kind: "new",
        mode: "station",
        version: "draft-1",
        station_id: Number(match?.groups?.stationId ?? 0),
      },
    };
  }
  if (line.startsWith("!ok")) {
    return {
      type: "firmware_detected",
      info: { kind: "new", mode: "receiver", version: "draft-1" },
    };
  }
  if (line.startsWith("S,") || line.startsWith("RS,")) {
    const parts = line.split(",");
    const colors: Color[] = ["green", "orange", "red"];
    return {
      type: "station_status",
      table: Number(parts[1]),
      color: colors[Number(parts[2])] ?? "green",
      relayed: line.startsWith("RS,"),
    };
  }
  return null;
}

function createFakeApi() {
  let last: FakeBridgeHandle | null = null;
  const api: WasmSerialApi = {
    microbit_usb_vid: () => 0x0d_28,
    serial_baud_rate: () => 115_200,
    format_wake_up: () => "\n",
    format_probe_firmware: () => "!get\n",
    createBridge: () => {
      last = createFakeBridge();
      return last.bridge;
    },
  };
  return { api, lastFreed: (): boolean => last?.freed() ?? false };
}

function createFakePort() {
  const writes: string[] = [];
  const writeTimes: number[] = [];
  let openedBaud: number | null = null;
  let closed = false;
  let controller: ReadableStreamDefaultController<Uint8Array> | null = null;
  const readable = new ReadableStream<Uint8Array>({
    start: (nextController) => {
      controller = nextController;
    },
  });
  const writable = new WritableStream<Uint8Array>({
    write: (chunk) => {
      writes.push(decoder.decode(chunk));
      writeTimes.push(Date.now());
    },
  });
  const port: SerialPortLike = {
    readable,
    writable,
    open: (options) => {
      openedBaud = options.baudRate;
      return Promise.resolve();
    },
    close: () => {
      closed = true;
      try {
        controller?.close();
      } catch {
        // Already closed by reader cancel.
      }
      return Promise.resolve();
    },
    getInfo: () => ({ usbVendorId: 0x0d_28, usbProductId: 0x02_04 }),
  };
  return {
    port,
    writes,
    writeTimes,
    openedBaud: (): number | null => openedBaud,
    isClosed: (): boolean => closed,
    push: (text: string): void => {
      controller?.enqueue(encoder.encode(text));
    },
    endStream: (): void => {
      controller?.close();
    },
  };
}

function makeHarness(probeTimeoutMs = 40) {
  const fakeApi = createFakeApi();
  const fakePort = createFakePort();
  const statuses: SerialStatusUpdate[] = [];
  const reports: { table: number; color: Color }[] = [];
  const bridge = new CreatorSerialBridge(
    {
      onStationReport: (table, color) => reports.push({ table, color }),
      onStatus: (status) => statuses.push(status),
    },
    { wasmApi: fakeApi.api, probeTimeoutMs, wakeDelayMs: 1, paceMs: 25 },
  );
  return { fakeApi, fakePort, bridge, statuses, reports };
}

describe("CreatorSerialBridge", () => {
  it("opens at 115200 and sends wake-up then probe", async () => {
    const h = makeHarness();
    await h.bridge.connect(h.fakePort.port);
    expect(h.fakePort.openedBaud()).toBe(115_200);
    expect(h.fakePort.writes[0]).toBe("\n");
    expect(h.fakePort.writes[1]).toBe("!get\n");
    expect(h.statuses[0]).toEqual({
      connected: true,
      portLabel: "micro:bit USB (0D28:0204)",
      firmware: null,
    });
    await h.bridge.disconnect();
  });

  it("detects new receiver firmware and cancels the legacy fallback", async () => {
    const h = makeHarness(60);
    await h.bridge.connect(h.fakePort.port);
    h.fakePort.push("!ok mode=receiver fw=draft-1\n");
    await sleep(20);
    const detected = h.statuses.find((status) => status.firmware?.kind === "new");
    expect(detected?.firmware).toEqual({
      kind: "new",
      mode: "receiver",
      version: "draft-1",
    });
    await sleep(80);
    expect(h.statuses.some((status) => status.firmware?.kind === "legacy")).toBe(false);
    await h.bridge.disconnect();
  });

  it("detects a wrong-mode station board", async () => {
    const h = makeHarness(60);
    await h.bridge.connect(h.fakePort.port);
    h.fakePort.push("!ok mode=station id=7 fw=draft-1\n");
    await sleep(20);
    const detected = h.statuses.find((status) => status.firmware?.mode === "station");
    expect(detected?.firmware?.station_id).toBe(7);
    await h.bridge.disconnect();
  });

  it("falls back to legacy after the probe timeout", async () => {
    const h = makeHarness(30);
    await h.bridge.connect(h.fakePort.port);
    await sleep(70);
    expect(h.statuses.some((status) => status.firmware?.kind === "legacy")).toBe(true);
    await h.bridge.disconnect();
  });

  it("forwards station reports and paces outgoing commands", async () => {
    const h = makeHarness();
    await h.bridge.connect(h.fakePort.port);
    h.fakePort.push("S,5,2\n");
    await sleep(10);
    expect(h.reports).toEqual([{ table: 5, color: "red" }]);
    const writesBefore = h.fakePort.writes.length;
    h.bridge.enqueueCommand({ cmd: "set_table", table: 5, color: "green" });
    h.bridge.enqueueCommand({ cmd: "reset_all" });
    await sleep(120);
    expect(h.fakePort.writes[writesBefore]).toBe("T,5,0\n");
    expect(h.fakePort.writes[writesBefore + 1]).toBe("T,-1,0\n");
    const gap =
      (h.fakePort.writeTimes[writesBefore + 1] ?? 0) - (h.fakePort.writeTimes[writesBefore] ?? 0);
    expect(gap).toBeGreaterThanOrEqual(15);
    await h.bridge.disconnect();
  });

  it("disconnects cleanly, releases locks, and frees the WASM bridge", async () => {
    const h = makeHarness();
    await h.bridge.connect(h.fakePort.port);
    expect(h.fakePort.port.readable?.locked).toBe(true);
    await h.bridge.disconnect();
    expect(h.fakePort.isClosed()).toBe(true);
    expect(h.fakePort.port.readable?.locked).toBe(false);
    expect(h.fakePort.port.writable?.locked).toBe(false);
    expect(h.fakeApi.lastFreed()).toBe(true);
    expect(h.statuses.at(-1)).toEqual({ connected: false, portLabel: null, firmware: null });
    expect(h.bridge.isConnected()).toBe(false);
  });

  it("resends current status with detected firmware after WS reconnect", async () => {
    const h = makeHarness(60);
    await h.bridge.connect(h.fakePort.port);
    h.fakePort.push("!ok mode=receiver fw=draft-1\n");
    await sleep(20);
    const beforeCount = h.statuses.length;
    h.bridge.resetDedup();
    h.bridge.resendStatus();
    expect(h.statuses.length).toBe(beforeCount + 1);
    expect(h.statuses.at(-1)).toEqual({
      connected: true,
      portLabel: "micro:bit USB (0D28:0204)",
      firmware: { kind: "new", mode: "receiver", version: "draft-1" },
    });
    await h.bridge.disconnect();
  });

  it("auto-disconnects when the serial read stream closes unexpectedly", async () => {
    const h = makeHarness(60);
    await h.bridge.connect(h.fakePort.port);
    h.fakePort.endStream();
    await sleep(30);
    expect(h.bridge.isConnected()).toBe(false);
    expect(h.statuses.at(-1)).toEqual({ connected: false, portLabel: null, firmware: null });
    expect(h.fakeApi.lastFreed()).toBe(true);
  });
});
