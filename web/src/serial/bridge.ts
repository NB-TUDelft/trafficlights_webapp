// SPDX-License-Identifier: AGPL-3.0-or-later
// Creator serial bridge: owns one opened port, the WASM dedup session,
// paced writes, firmware probing, Wake Lock, and unload warning.

import { parseSerialEvents } from "@/protocol/parse";
import type { FirmwareInfo, SerialCommand } from "@/protocol/types";
import { delay, describePort, loadWasmSerialApi } from "./loader";
import {
  acquireWakeLock,
  closeSerialPort,
  freeWasmBridge,
  releaseSerialStreams,
  releaseWakeLock,
} from "./resources";
import type {
  SerialBridgeCallbacks,
  SerialBridgeOptions,
  SerialPortLike,
  WakeLockSentinelLike,
  WasmBridgeLike,
  WasmSerialApi,
} from "./types";

export const WAKE_DELAY_MS = 50;
export const PROBE_TIMEOUT_MS = 1000;
export const WRITE_PACE_MS = 25;

function preventUnload(event: BeforeUnloadEvent): void {
  event.preventDefault();
}

export class CreatorSerialBridge {
  private readonly callbacks: SerialBridgeCallbacks;
  private readonly options: SerialBridgeOptions;
  private readonly encoder = new TextEncoder();
  private api: WasmSerialApi | null = null;
  private bridge: WasmBridgeLike | null = null;
  private port: SerialPortLike | null = null;
  private reader: ReadableStreamDefaultReader<Uint8Array> | null = null;
  private writer: WritableStreamDefaultWriter<Uint8Array> | null = null;
  private connected = false;
  private portLabel: string | null = null;
  private lastFirmware: FirmwareInfo | null = null;
  private firmwareResolved = false;
  private probeTimer: ReturnType<typeof setTimeout> | null = null;
  private wakeLock: WakeLockSentinelLike | null = null;
  private writeChain: Promise<void> = Promise.resolve();
  private lastWriteAt = 0;
  private readonly beforeUnloadHandler = preventUnload;

  constructor(callbacks: SerialBridgeCallbacks, options?: SerialBridgeOptions) {
    this.callbacks = callbacks;
    this.options = options ?? {};
  }

  isConnected(): boolean {
    return this.connected;
  }

  resetDedup(): void {
    this.bridge?.reset_dedup();
  }

  resendStatus(): void {
    if (!this.connected) {
      return;
    }
    this.callbacks.onStatus({
      connected: true,
      portLabel: this.portLabel,
      firmware: this.lastFirmware,
    });
  }

  async connect(port: SerialPortLike): Promise<void> {
    if (this.connected) {
      return;
    }
    this.api = await (this.options.wasmApi ?? loadWasmSerialApi());
    this.bridge = this.api.createBridge();
    this.port = port;
    await port.open({ baudRate: this.api.serial_baud_rate() });
    this.acquireStreams(port);
    this.connected = true;
    this.portLabel = describePort(port);
    this.lastFirmware = null;
    this.firmwareResolved = false;
    this.wakeLock = await acquireWakeLock();
    this.addUnloadWarning();
    this.callbacks.onStatus({ connected: true, portLabel: this.portLabel, firmware: null });
    this.readLoop().catch(() => undefined);
    await this.writeText(this.api.format_wake_up());
    await delay(this.options.wakeDelayMs ?? WAKE_DELAY_MS);
    await this.writeText(this.api.format_probe_firmware());
    this.armProbeTimeout();
  }

  enqueueCommand(command: SerialCommand): void {
    if (this.bridge === null || !this.connected) {
      return;
    }
    const line = this.formatCommand(command);
    if (line !== null) {
      this.writeText(line).catch(() => undefined);
    }
  }

  async disconnect(): Promise<void> {
    if (!this.connected && this.port === null) {
      return;
    }
    this.connected = false;
    this.clearProbeTimer();
    this.removeUnloadWarning();
    const { wakeLock, reader, writer, port, bridge } = this;
    this.wakeLock = null;
    this.reader = null;
    this.writer = null;
    this.port = null;
    this.bridge = null;
    this.portLabel = null;
    this.lastFirmware = null;
    await releaseWakeLock(wakeLock);
    await releaseSerialStreams(reader, writer);
    await closeSerialPort(port);
    freeWasmBridge(bridge);
    this.callbacks.onStatus({ connected: false, portLabel: null, firmware: null });
  }

  private acquireStreams(port: SerialPortLike): void {
    if (port.readable === null || port.writable === null) {
      throw new Error("Serial port streams are unavailable.");
    }
    this.reader = port.readable.getReader();
    this.writer = port.writable.getWriter();
  }

  private armProbeTimeout(): void {
    const timeoutMs = this.options.probeTimeoutMs ?? PROBE_TIMEOUT_MS;
    this.probeTimer = setTimeout(() => {
      if (!this.connected || this.firmwareResolved) {
        return;
      }
      this.firmwareResolved = true;
      this.lastFirmware = { kind: "legacy" };
      this.callbacks.onStatus({
        connected: true,
        portLabel: this.portLabel,
        firmware: this.lastFirmware,
      });
    }, timeoutMs);
  }

  private clearProbeTimer(): void {
    if (this.probeTimer !== null) {
      clearTimeout(this.probeTimer);
      this.probeTimer = null;
    }
  }

  private formatCommand(command: SerialCommand): string | null {
    if (this.bridge === null) {
      return null;
    }
    try {
      if (command.cmd === "set_table") {
        return this.bridge.format_set_table(command.table, command.color);
      }
      if (command.cmd === "reset_all") {
        return this.bridge.format_reset_all();
      }
    } catch (error) {
      this.reportError(error);
    }
    return null;
  }

  private async readLoop(): Promise<void> {
    if (!this.connected || this.reader === null) {
      return;
    }
    let endedUnexpectedly = false;
    try {
      const result = await this.reader.read();
      if (result.done) {
        endedUnexpectedly = true;
      } else if (result.value !== undefined) {
        this.handleChunk(result.value);
      }
    } catch (error) {
      if (this.connected) {
        this.reportError(error);
        endedUnexpectedly = true;
      }
    }
    if (endedUnexpectedly) {
      if (this.connected) {
        await this.disconnect();
      }
      return;
    }
    await this.readLoop();
  }

  private handleChunk(chunk: Uint8Array): void {
    if (this.bridge === null) {
      return;
    }
    const raw: unknown = this.bridge.feed_bytes(chunk);
    for (const event of parseSerialEvents(raw)) {
      this.handleEvent(event);
    }
  }

  private handleEvent(event: ReturnType<typeof parseSerialEvents>[number]): void {
    if (event.type === "station_status") {
      this.callbacks.onStationReport(event.table, event.color);
    } else if (event.type === "firmware_detected") {
      this.firmwareResolved = true;
      this.lastFirmware = event.info;
      this.clearProbeTimer();
      this.callbacks.onStatus({
        connected: true,
        portLabel: this.portLabel,
        firmware: event.info,
      });
    } else if (event.type === "control_error") {
      this.callbacks.onError?.(event.reason);
    }
  }

  private writeText(text: string): Promise<void> {
    const paceMs = this.options.paceMs ?? WRITE_PACE_MS;
    this.writeChain = this.writeChain.then(async () => {
      await this.waitForPace(paceMs);
      await this.writeNow(text);
    });
    return this.writeChain;
  }

  private async waitForPace(paceMs: number): Promise<void> {
    const elapsed = Date.now() - this.lastWriteAt;
    if (this.lastWriteAt > 0 && elapsed < paceMs) {
      await delay(paceMs - elapsed);
    }
  }

  private async writeNow(text: string): Promise<void> {
    if (this.writer === null) {
      return;
    }
    try {
      await this.writer.write(this.encoder.encode(text));
      this.lastWriteAt = Date.now();
    } catch (error) {
      this.reportError(error);
    }
  }

  private addUnloadWarning(): void {
    globalThis.window?.addEventListener("beforeunload", this.beforeUnloadHandler);
  }

  private removeUnloadWarning(): void {
    globalThis.window?.removeEventListener("beforeunload", this.beforeUnloadHandler);
  }

  private reportError(error: unknown): void {
    const message = error instanceof Error ? error.message : String(error);
    this.callbacks.onError?.(message);
  }
}
