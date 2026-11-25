// SPDX-License-Identifier: AGPL-3.0-or-later
// Reconnecting WebSocket helper with exponential backoff and Ping keepalive.

export type SocketStatus = "connecting" | "open" | "reconnecting" | "closed";

export const INITIAL_BACKOFF_MS = 500;
export const MAX_BACKOFF_MS = 10_000;
export const BACKOFF_MULTIPLIER = 2;
export const PING_INTERVAL_MS = 20_000;

export interface SocketHandlers {
  onMessage: (data: string) => void;
  onOpen?: () => void;
  onStatusChange?: (status: SocketStatus) => void;
}

export interface SocketOptions {
  pingMessage: () => string;
  shouldReconnect?: () => boolean;
}

export function websocketUrl(path: string): string {
  const win = globalThis.window;
  if (win === undefined) {
    return `ws://127.0.0.1${path}`;
  }
  const scheme = win.location.protocol === "https:" ? "wss" : "ws";
  return `${scheme}://${win.location.host}${path}`;
}

export class ReconnectingSocket {
  private readonly url: string;
  private readonly handlers: SocketHandlers;
  private readonly options: SocketOptions;
  private socket: WebSocket | null = null;
  private closedByUser = false;
  private attempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private pingTimer: ReturnType<typeof setInterval> | null = null;

  constructor(url: string, handlers: SocketHandlers, options: SocketOptions) {
    this.url = url;
    this.handlers = handlers;
    this.options = options;
  }

  connect(): void {
    if (typeof WebSocket === "undefined") {
      this.emitStatus("closed");
      return;
    }
    this.closedByUser = false;
    this.openSocket();
  }

  send(payload: unknown): boolean {
    if (this.socket?.readyState !== WebSocket.OPEN) {
      return false;
    }
    this.socket.send(typeof payload === "string" ? payload : JSON.stringify(payload));
    return true;
  }

  close(): void {
    this.closedByUser = true;
    this.clearTimers();
    if (this.socket !== null) {
      this.socket.close();
      this.socket = null;
    }
    this.emitStatus("closed");
  }

  private emitStatus(status: SocketStatus): void {
    this.handlers.onStatusChange?.(status);
  }

  private clearTimers(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.pingTimer !== null) {
      clearInterval(this.pingTimer);
      this.pingTimer = null;
    }
  }

  private openSocket(): void {
    this.emitStatus(this.attempt === 0 ? "connecting" : "reconnecting");
    const socket = new WebSocket(this.url);
    this.socket = socket;
    socket.addEventListener("open", () => this.handleOpen());
    socket.addEventListener("message", (event: MessageEvent<unknown>) => {
      if (typeof event.data === "string") {
        this.handlers.onMessage(event.data);
      }
    });
    socket.addEventListener("close", () => this.handleClose());
    socket.addEventListener("error", () => socket.close());
  }

  private handleOpen(): void {
    this.attempt = 0;
    this.emitStatus("open");
    this.handlers.onOpen?.();
    this.startPing();
  }

  private startPing(): void {
    if (this.pingTimer !== null) {
      clearInterval(this.pingTimer);
    }
    this.pingTimer = setInterval(() => {
      this.send(this.options.pingMessage());
    }, PING_INTERVAL_MS);
  }

  private handleClose(): void {
    if (this.pingTimer !== null) {
      clearInterval(this.pingTimer);
      this.pingTimer = null;
    }
    if (this.closedByUser || this.options.shouldReconnect?.() === false) {
      this.emitStatus("closed");
      return;
    }
    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    this.emitStatus("reconnecting");
    const delay = Math.min(MAX_BACKOFF_MS, INITIAL_BACKOFF_MS * BACKOFF_MULTIPLIER ** this.attempt);
    this.attempt += 1;
    this.reconnectTimer = setTimeout(() => this.openSocket(), delay);
  }
}
