// SPDX-License-Identifier: AGPL-3.0-or-later
// Hooks and handlers for the session console page.

import { useCallback, useEffect } from "react";
import { enteredAlert, playAlertBeep } from "@/lib/audio";
import { ReconnectingSocket, type SocketStatus, websocketUrl } from "@/lib/socket";
import { parseSessionServerMsg } from "@/protocol/parse-session";
import type { SessionState } from "@/protocol/reducers";
import { getCreatorToken } from "@/protocol/storage";
import type { Color, SessionServerMsg } from "@/protocol/types";
import { isValidTableRange, normalizeSessionName } from "@/protocol/types";
import type { CreatorSerialBridge } from "@/serial/bridge";
import { requestMicrobitPort } from "@/serial/loader";
import type { SerialPortLike } from "@/serial/types";
import {
  type ModeHandlersArgs,
  toggleBeeps,
  toggleBeepsLock,
  toggleMuteWhenActive,
  toggleTapMode,
  toggleTapModeLock,
} from "./sessionModeHandlers";

const PARSE_RADIX = 10;

export interface Ref<T> {
  current: T;
}

export interface SocketHookArgs {
  sessionId: string;
  dispatch: (msg: SessionServerMsg) => void;
  setSocketStatus: (status: SocketStatus) => void;
  socketRef: Ref<ReconnectingSocket | null>;
  bridgeRef: Ref<CreatorSerialBridge | null>;
  stopReconnectRef: Ref<boolean>;
}

export function useSessionSocket(args: SocketHookArgs): void {
  const { sessionId, dispatch, setSocketStatus, socketRef, bridgeRef, stopReconnectRef } = args;
  useEffect(() => {
    stopReconnectRef.current = false;
    const socket = new ReconnectingSocket(
      websocketUrl(`/ws/sessions/${encodeURIComponent(sessionId)}`),
      {
        onOpen: () => {
          const token = getCreatorToken(sessionId);
          if (token !== null) {
            socket.send({ type: "auth", creator_token: token });
          }
        },
        onMessage: (data) => handleServerData(data, dispatch, bridgeRef, stopReconnectRef),
        onStatusChange: setSocketStatus,
      },
      {
        pingMessage: () => JSON.stringify({ type: "ping" }),
        shouldReconnect: () => !stopReconnectRef.current,
      },
    );
    socketRef.current = socket;
    socket.connect();
    return () => socket.close();
  }, [sessionId, dispatch, setSocketStatus, socketRef, bridgeRef, stopReconnectRef]);
}

export function handleServerData(
  data: string,
  dispatch: SocketHookArgs["dispatch"],
  bridgeRef: SocketHookArgs["bridgeRef"],
  stopReconnectRef: SocketHookArgs["stopReconnectRef"],
): void {
  const msg = parseSessionServerMsg(data);
  if (msg === null) {
    return;
  }
  if (msg.type === "serial_command") {
    bridgeRef.current?.enqueueCommand(msg.command);
    return;
  }
  if (msg.type === "auth_ok" && msg.is_creator && bridgeRef.current?.isConnected()) {
    bridgeRef.current.resetDedup();
    bridgeRef.current.resendStatus();
  }
  if (msg.type === "session_ended" || (msg.type === "error" && msg.code === "superseded")) {
    stopReconnectRef.current = true;
    bridgeRef.current?.disconnect().catch(() => undefined);
  }
  if (msg.type === "error" && msg.code === "session_not_found") {
    stopReconnectRef.current = true;
  }
  dispatch(msg);
}

export function useBeeps(
  state: SessionState,
  beepsEnabled: boolean,
  mutedTables: ReadonlySet<number>,
  prevColorsRef: Ref<Map<number, Color>>,
): void {
  useEffect(() => {
    let shouldBeep = false;
    for (const entry of state.tables) {
      const prev = prevColorsRef.current.get(entry.table);
      if (beepsEnabled && !mutedTables.has(entry.table) && enteredAlert(prev, entry.color)) {
        shouldBeep = true;
      }
    }
    prevColorsRef.current = new Map(state.tables.map((entry) => [entry.table, entry.color]));
    if (shouldBeep) {
      playAlertBeep();
    }
  }, [state.tables, beepsEnabled, mutedTables, prevColorsRef]);
}

export interface HandlersArgs extends ModeHandlersArgs {
  state: SessionState;
  setTransientError: (value: string | null) => void;
  setMutedTables: (updater: (prev: ReadonlySet<number>) => ReadonlySet<number>) => void;
  beepsEnabled: boolean;
  rangeStart: string;
  rangeEnd: string;
  renameValue: string;
  sessionId: string;
  showAllPorts: boolean;
  connectToPort: (port: SerialPortLike) => Promise<void>;
  bridgeRef: Ref<CreatorSerialBridge | null>;
  setConfirmEnd: (value: boolean) => void;
}

export function useSessionHandlers(args: HandlersArgs) {
  const onSetTable = useCallback(
    (table: number, color: Color) => {
      if (!args.state.receiverConnected) {
        args.setTransientError("Receiver is disconnected. Table was not changed.");
        return;
      }
      args.send({ type: "set_table", table, color });
    },
    [args],
  );
  const onToggleMute = useCallback(
    (table: number) => toggleMuteWhenActive(args.beepsEnabled, table, args.setMutedTables),
    [args],
  );
  const onToggleBeeps = useCallback(() => toggleBeeps(args), [args]);
  const onToggleBeepsLock = useCallback(() => toggleBeepsLock(args), [args]);
  const onToggleTapMode = useCallback(() => toggleTapMode(args), [args]);
  const onToggleTapModeLock = useCallback(() => toggleTapModeLock(args), [args]);
  const onConnectSerial = useCallback(() => {
    connectFromPicker(args).catch(() => undefined);
  }, [args]);
  const onDisconnectSerial = useCallback(() => {
    args.bridgeRef.current?.disconnect().catch(() => undefined);
  }, [args]);
  const onApplyRange = useCallback(() => applyRange(args), [args]);
  const onResetAll = useCallback(() => args.send({ type: "reset_all" }), [args]);
  const onRename = useCallback(() => applyRename(args), [args]);
  const onDownloadLog = useCallback(() => {
    downloadLog(args).catch(() => undefined);
  }, [args]);
  const onConfirmEnd = useCallback(() => {
    args.setConfirmEnd(false);
    args.send({ type: "end_session" });
  }, [args]);
  return {
    onSetTable,
    onToggleMute,
    onToggleBeeps,
    onToggleBeepsLock,
    onToggleTapMode,
    onToggleTapModeLock,
    onConnectSerial,
    onDisconnectSerial,
    onApplyRange,
    onResetAll,
    onRename,
    onDownloadLog,
    onConfirmEnd,
  };
}

async function connectFromPicker(args: HandlersArgs): Promise<void> {
  try {
    const port = await requestMicrobitPort({ showAll: args.showAllPorts });
    await args.connectToPort(port);
  } catch {
    args.setTransientError("No receiver was selected.");
  }
}

function applyRange(args: HandlersArgs): void {
  const start = Number.parseInt(args.rangeStart, PARSE_RADIX);
  const end = Number.parseInt(args.rangeEnd, PARSE_RADIX);
  if (!isValidTableRange(start, end)) {
    args.setTransientError("Enter a valid table range (start <= end, 1-9999).");
    return;
  }
  args.send({ type: "set_table_range", start, end });
}

function applyRename(args: HandlersArgs): void {
  const normalized = normalizeSessionName(args.renameValue);
  if (normalized === null) {
    args.setTransientError("Enter a session name of at most 80 characters.");
    return;
  }
  args.send({ type: "rename_session", name: normalized });
}

async function downloadLog(args: HandlersArgs): Promise<void> {
  const token = getCreatorToken(args.sessionId);
  if (token === null) {
    args.setTransientError("Creator token is missing in this browser.");
    return;
  }
  try {
    const response = await fetch(`/api/sessions/${encodeURIComponent(args.sessionId)}/log`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (!response.ok) {
      throw new Error("download failed");
    }
    const blob = await response.blob();
    triggerBlobDownload(blob, `trafficlights-${args.sessionId}.jsonl`);
  } catch {
    args.setTransientError("Statistics download failed.");
  }
}

function triggerBlobDownload(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
