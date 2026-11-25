// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from "vitest";
import { FIRST_TONE_HZ, playAlertBeep, SECOND_TONE_HZ } from "@/lib/audio";
import { endReasonLabel, firmwareBadgeLabel, firmwareSummary } from "@/lib/firmware";
import { handleServerData } from "@/pages/sessionHooks";
import type { SessionServerMsg } from "@/protocol/types";
import { CreatorSerialBridge } from "@/serial/bridge";
import {
  acquireWakeLock,
  closeSerialPort,
  freeWasmBridge,
  releaseSerialStreams,
  releaseWakeLock,
} from "@/serial/resources";
import type { SerialPortLike, WakeLockSentinelLike, WasmBridgeLike } from "@/serial/types";
import { snapshotMsg } from "./fixtures";

function fakeBridgeRef() {
  const calls: string[] = [];
  const bridge = new CreatorSerialBridge({
    onStationReport: () => undefined,
    onStatus: () => undefined,
  });
  bridge.isConnected = () => true;
  bridge.enqueueCommand = (command) => {
    calls.push(`command:${command.cmd}`);
  };
  bridge.resetDedup = () => {
    calls.push("resetDedup");
  };
  bridge.resendStatus = () => {
    calls.push("resendStatus");
  };
  bridge.disconnect = () => {
    calls.push("disconnect");
    return Promise.resolve();
  };
  return { bridgeRef: { current: bridge }, calls };
}

describe("handleServerData", () => {
  it("forwards serial commands to the bridge without dispatching", () => {
    const { bridgeRef, calls } = fakeBridgeRef();
    const dispatched: SessionServerMsg[] = [];
    const stop = { current: false };
    handleServerData(
      JSON.stringify({
        type: "serial_command",
        command: { cmd: "set_table", table: 3, color: "red" },
      }),
      (msg) => dispatched.push(msg),
      bridgeRef,
      stop,
    );
    expect(calls).toEqual(["command:set_table"]);
    expect(dispatched).toEqual([]);
  });

  it("resets dedup and resends status on auth_ok only", () => {
    const { bridgeRef, calls } = fakeBridgeRef();
    const dispatched: SessionServerMsg[] = [];
    const stop = { current: false };
    handleServerData(
      JSON.stringify({ type: "auth_ok", is_creator: true }),
      (msg) => dispatched.push(msg),
      bridgeRef,
      stop,
    );
    expect(calls).toEqual(["resetDedup", "resendStatus"]);
    expect(dispatched).toHaveLength(1);
  });

  it("does not reset dedup on snapshot", () => {
    const { bridgeRef, calls } = fakeBridgeRef();
    const dispatched: SessionServerMsg[] = [];
    const stop = { current: false };
    handleServerData(JSON.stringify(snapshotMsg()), (msg) => dispatched.push(msg), bridgeRef, stop);
    expect(calls).toEqual([]);
    expect(dispatched).toHaveLength(1);
    expect(dispatched[0]?.type).toBe("snapshot");
  });

  it("disconnects the bridge and stops reconnecting when superseded", () => {
    const { bridgeRef, calls } = fakeBridgeRef();
    const dispatched: SessionServerMsg[] = [];
    const stop = { current: false };
    handleServerData(
      JSON.stringify({ type: "error", code: "superseded", message: "moved" }),
      (msg) => dispatched.push(msg),
      bridgeRef,
      stop,
    );
    expect(calls).toEqual(["disconnect"]);
    expect(stop.current).toBe(true);
    expect(dispatched).toHaveLength(1);
  });

  it("disconnects the bridge and stops reconnecting when the session ends", () => {
    const { bridgeRef, calls } = fakeBridgeRef();
    const stop = { current: false };
    handleServerData(
      JSON.stringify({ type: "session_ended", reason: "manual", log_available: true }),
      () => undefined,
      bridgeRef,
      stop,
    );
    expect(calls).toEqual(["disconnect"]);
    expect(stop.current).toBe(true);
  });

  it("stops reconnecting for an unknown session and ignores garbage", () => {
    const { bridgeRef, calls } = fakeBridgeRef();
    const dispatched: SessionServerMsg[] = [];
    const stop = { current: false };
    handleServerData("not json", (msg) => dispatched.push(msg), bridgeRef, stop);
    expect(dispatched).toEqual([]);
    handleServerData(
      JSON.stringify({ type: "error", code: "session_not_found", message: "gone" }),
      (msg) => dispatched.push(msg),
      bridgeRef,
      stop,
    );
    expect(stop.current).toBe(true);
    expect(calls).toEqual([]);
  });
});

describe("serial resources", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("acquires and releases a wake lock, tolerating failures", async () => {
    const sentinel: WakeLockSentinelLike = { release: () => Promise.resolve() };
    vi.stubGlobal("navigator", {
      wakeLock: { request: () => Promise.resolve(sentinel) },
    });
    await expect(acquireWakeLock()).resolves.toBe(sentinel);
    await expect(releaseWakeLock(sentinel)).resolves.toBeUndefined();
    await expect(releaseWakeLock(null)).resolves.toBeUndefined();

    vi.stubGlobal("navigator", {
      wakeLock: { request: () => Promise.reject(new Error("denied")) },
    });
    await expect(acquireWakeLock()).resolves.toBeNull();
    const failing: WakeLockSentinelLike = {
      release: () => Promise.reject(new Error("gone")),
    };
    await expect(releaseWakeLock(failing)).resolves.toBeUndefined();

    vi.stubGlobal("navigator", {});
    await expect(acquireWakeLock()).resolves.toBeNull();
  });

  it("releases streams, closes ports, and frees bridges best-effort", async () => {
    const calls: string[] = [];
    const reader = {
      cancel: () => {
        calls.push("cancel");
        return Promise.resolve();
      },
      releaseLock: () => {
        calls.push("readerLock");
      },
    } as unknown as ReadableStreamDefaultReader<Uint8Array>;
    const writer = {
      releaseLock: () => {
        calls.push("writerLock");
      },
    } as unknown as WritableStreamDefaultWriter<Uint8Array>;
    await releaseSerialStreams(reader, writer);
    expect(calls).toEqual(["cancel", "readerLock", "writerLock"]);
    await releaseSerialStreams(null, null);

    const throwingReader = {
      cancel: () => Promise.reject(new Error("stuck")),
      releaseLock: () => {
        throw new Error("stuck");
      },
    } as unknown as ReadableStreamDefaultReader<Uint8Array>;
    const throwingWriter = {
      releaseLock: () => {
        throw new Error("stuck");
      },
    } as unknown as WritableStreamDefaultWriter<Uint8Array>;
    await expect(releaseSerialStreams(throwingReader, throwingWriter)).resolves.toBeUndefined();

    let closed = false;
    const port: SerialPortLike = {
      readable: null,
      writable: null,
      open: () => Promise.resolve(),
      close: () => {
        closed = true;
        return Promise.resolve();
      },
      getInfo: () => ({}),
    };
    await closeSerialPort(port);
    expect(closed).toBe(true);
    await closeSerialPort(null);
    await closeSerialPort({
      ...port,
      close: () => Promise.reject(new Error("busy")),
    });

    let freed = false;
    const wasm: WasmBridgeLike = {
      feed_bytes: () => [],
      format_set_table: () => "",
      format_reset_all: () => "",
      reset_dedup: () => undefined,
      free: () => {
        freed = true;
      },
    };
    freeWasmBridge(wasm);
    expect(freed).toBe(true);
    freeWasmBridge(null);
    freeWasmBridge({
      ...wasm,
      free: () => {
        throw new Error("gone");
      },
    });
  });
});

describe("firmware labels", () => {
  it("covers every branch", () => {
    expect(firmwareSummary(null)).toBeNull();
    expect(firmwareSummary(undefined)).toBeNull();
    expect(firmwareSummary({ kind: "legacy" })).toBe("Legacy MakeCode receiver");
    expect(firmwareSummary({ kind: "new", version: "draft-1" })).toBe("New firmware (draft-1)");
    expect(firmwareSummary({ kind: "new" })).toBe("New firmware");
    expect(firmwareSummary({ kind: "new", version: "" })).toBe("New firmware");
    expect(firmwareBadgeLabel(false, { kind: "new" })).toBe("Disconnected");
    expect(firmwareBadgeLabel(true, null)).toBe("Probing...");
    expect(firmwareBadgeLabel(true, { kind: "legacy" })).toBe("Legacy MakeCode receiver");
    expect(endReasonLabel("manual")).toBe("Manual");
    expect(endReasonLabel("grace_timeout")).toBe("Grace timeout");
    expect(endReasonLabel("shutdown")).toBe("Server shutdown");
    expect(endReasonLabel("other")).toBe("Server shutdown");
  });
});

describe("alert beep tones", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("plays an 880 Hz tone followed by a 660 Hz tone", () => {
    const frequencies: number[] = [];
    const fakeAudioContext = vi.fn(function fakeAudioContextImpl() {
      return {
        state: "running",
        currentTime: 0,
        destination: {},
        resume: () => Promise.resolve(),
        createOscillator: () => {
          const osc = {
            type: "sine",
            frequency: { value: 0 },
            connect: () => undefined,
            start: () => {
              frequencies.push(osc.frequency.value);
            },
            stop: () => undefined,
          };
          return osc;
        },
        createGain: () => ({
          gain: {
            setValueAtTime: () => undefined,
            exponentialRampToValueAtTime: () => undefined,
          },
          connect: () => undefined,
        }),
      };
    });
    vi.stubGlobal("AudioContext", fakeAudioContext);
    playAlertBeep();
    expect(frequencies).toEqual([FIRST_TONE_HZ, SECOND_TONE_HZ]);
    expect(FIRST_TONE_HZ).toBe(880);
    expect(SECOND_TONE_HZ).toBe(660);
  });
});
