// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from "vitest";
import { ReconnectingSocket, type SocketStatus, websocketUrl } from "@/lib/socket";

type Listener = (event: { data?: unknown }) => void;

class FakeWebSocket {
  static readonly OPEN = 1;
  static instances: FakeWebSocket[] = [];
  readonly url: string;
  readyState = 0;
  readonly sent: string[] = [];
  private readonly listeners = new Map<string, Listener[]>();

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }

  addEventListener(type: string, listener: Listener): void {
    const list = this.listeners.get(type) ?? [];
    list.push(listener);
    this.listeners.set(type, list);
  }

  emit(type: string, event: { data?: unknown } = {}): void {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
  }

  send(data: string): void {
    this.sent.push(data);
  }

  close(): void {
    this.readyState = 3;
  }
}

describe("ReconnectingSocket", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    FakeWebSocket.instances = [];
  });

  function setup(shouldReconnect?: () => boolean) {
    vi.useFakeTimers();
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const messages: string[] = [];
    const statuses: SocketStatus[] = [];
    let opened = 0;
    const socket = new ReconnectingSocket(
      "ws://127.0.0.1/ws/lobby",
      {
        onMessage: (data) => messages.push(data),
        onOpen: () => {
          opened += 1;
        },
        onStatusChange: (status) => statuses.push(status),
      },
      {
        pingMessage: () => JSON.stringify({ type: "ping" }),
        ...(shouldReconnect === undefined ? {} : { shouldReconnect }),
      },
    );
    return { socket, messages, statuses, opened: () => opened };
  }

  it("builds a loopback URL without a window", () => {
    expect(websocketUrl("/ws/lobby")).toBe("ws://127.0.0.1/ws/lobby");
  });

  it("connects, reports status, sends, pings, and receives text only", () => {
    const h = setup();
    h.socket.connect();
    const [ws] = FakeWebSocket.instances;
    expect(ws?.url).toBe("ws://127.0.0.1/ws/lobby");
    expect(h.statuses).toEqual(["connecting"]);
    expect(h.socket.send("early")).toBe(false);
    ws.readyState = FakeWebSocket.OPEN;
    ws.emit("open");
    expect(h.statuses).toEqual(["connecting", "open"]);
    expect(h.opened()).toBe(1);
    expect(h.socket.send({ type: "ping" })).toBe(true);
    expect(h.socket.send("raw")).toBe(true);
    expect(ws.sent).toEqual(['{"type":"ping"}', "raw"]);
    ws.emit("message", { data: "hello" });
    ws.emit("message", { data: 42 });
    expect(h.messages).toEqual(["hello"]);
    vi.advanceTimersByTime(20_000);
    expect(ws.sent.at(-1)).toBe('{"type":"ping"}');
    h.socket.close();
    expect(h.statuses.at(-1)).toBe("closed");
    expect(ws.readyState).toBe(3);
  });

  it("reconnects with backoff after a drop", () => {
    const h = setup();
    h.socket.connect();
    const [first] = FakeWebSocket.instances;
    first.readyState = FakeWebSocket.OPEN;
    first.emit("open");
    first.emit("close");
    expect(h.statuses.at(-1)).toBe("reconnecting");
    vi.advanceTimersByTime(499);
    expect(FakeWebSocket.instances).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(FakeWebSocket.instances).toHaveLength(2);
    h.socket.close();
  });

  it("does not reconnect when shouldReconnect is false", () => {
    const h = setup(() => false);
    h.socket.connect();
    const [first] = FakeWebSocket.instances;
    first.readyState = FakeWebSocket.OPEN;
    first.emit("open");
    first.emit("error");
    expect(first.readyState).toBe(3);
    first.emit("close");
    expect(h.statuses.at(-1)).toBe("closed");
    vi.advanceTimersByTime(60_000);
    expect(FakeWebSocket.instances).toHaveLength(1);
  });
});
