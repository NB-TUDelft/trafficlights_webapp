// SPDX-License-Identifier: AGPL-3.0-or-later
// Creator serial bridge: owns one opened port, the WASM dedup session,
// paced writes, firmware probing, Wake Lock, and unload warning.

import { parseSerialEvents } from "@/protocol/parse";
import type { SerialCommand } from "@/protocol/types";
import { delay, describePort, loadWasmSerialApi } from "./loader";
import type {
  SerialBridgeCallbacks,
  SerialBridgeOptions,
  SerialPortLike,
  WakeLockSentinelLike,
  WasmBridgeLike,
  WasmSerialApi,
} from "./types";
import { getWakeLock } from "./types";

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
    this.firmwareResolved = false;
    await this.requestWakeLock();
    this.addUnloadWarning();
    this.callbacks.onStatus({ connected: true, portLabel: this.portLabel, firmware: null });
    this.readLoop();
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
      this.writeText(line);
    }
  }

  async disconnect(): Promise<void> {
    if (!this.connected && this.port === null) {
      return;
    }
    this.connected = false;
    this.clearProbeTimer();
    this.removeUnloadWarning();
    await this.releaseWakeLock();
    await this.releaseStreams();
    await this.closePort();
    this.freeBridge();
    this.portLabel = null;
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
      this.callbacks.onStatus({
        connected: true,
        portLabel: this.portLabel,
        firmware: { kind: "legacy" },
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
    try {
      const result = await this.reader.read();
      if (result.done) {
        return;
      }
      if (result.value !== undefined) {
        this.handleChunk(result.value);
      }
    } catch (error) {
      if (this.connected) {
        this.reportError(error);
      }
      return;
    }
    return this.readLoop();
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

  private async requestWakeLock(): Promise<void> {
    try {
      const wakeLock = getWakeLock();
      if (wakeLock !== null) {
        this.wakeLock = await wakeLock.request("screen");
      }
    } catch {
      this.wakeLock = null;
    }
  }

  private async releaseWakeLock(): Promise<void> {
    try {
      await this.wakeLock?.release();
    } catch {
      // Wake lock release is best-effort.
    }
    this.wakeLock = null;
  }

  private addUnloadWarning(): void {
    globalThis.window?.addEventListener("beforeunload", this.beforeUnloadHandler);
  }

  private removeUnloadWarning(): void {
    globalThis.window?.removeEventListener("beforeunload", this.beforeUnloadHandler);
  }

  private async releaseStreams(): Promise<void> {
    try {
      await this.reader?.cancel();
    } catch {
      // Ignore cancel failures during teardown.
    }
    try {
      this.reader?.releaseLock();
    } catch {
      // Ignore lock failures during teardown.
    }
    try {
      this.writer?.releaseLock();
    } catch {
      // Ignore lock failures during teardown.
    }
    this.reader = null;
    this.writer = null;
  }

  private async closePort(): Promise<void> {
    try {
      await this.port?.close();
    } catch {
      // Ignore close failures during teardown.
    }
    this.port = null;
  }

  private freeBridge(): void {
    try {
      this.bridge?.free();
    } catch {
      // Ignore free failures during teardown.
    }
    this.bridge = null;
  }

  private reportError(error: unknown): void {
    const message = error instanceof Error ? error.message : String(error);
    this.callbacks.onError?.(message);
  }
}
