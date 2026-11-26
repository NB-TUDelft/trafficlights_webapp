// SPDX-License-Identifier: AGPL-3.0-or-later

import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { Link, useParams } from "react-router";
import { Footer } from "@/components/Footer";
import { SessionView, type SessionViewProps } from "@/components/SessionView";
import type { ReconnectingSocket, SocketStatus } from "@/lib/socket";
import { initialSessionState, type SessionState, sessionReducer } from "@/protocol/reducers";
import type { Color, SessionClientMsg } from "@/protocol/types";
import { isWebSerialSupported } from "@/serial/loader";
import { useBeeps, useSessionHandlers, useSessionSocket } from "./sessionHooks";
import { resolveBeepsEnabled, resolveTapMode } from "./sessionModeHandlers";
import { usePendingPort, useSerialBridge, useSessionUiState } from "./sessionStateHooks";

const TICK_MS = 1000;

function SessionNotFound() {
  return (
    <main className="page">
      <h1>Session not found</h1>
      <Link to="/">Back to Lobby</Link>
      <Footer />
    </main>
  );
}

export function SessionPage() {
  const params = useParams();
  const sessionId = params.id;
  if (sessionId === undefined || sessionId.length === 0) {
    return <SessionNotFound />;
  }
  return <SessionConsole sessionId={sessionId} />;
}

function useNowTick(): number {
  const [nowMs, setNowMs] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNowMs(Date.now()), TICK_MS);
    return () => clearInterval(timer);
  }, []);
  return nowMs;
}

type UiState = ReturnType<typeof useSessionUiState>;
type SerialBridgeState = ReturnType<typeof useSerialBridge>;
type Handlers = ReturnType<typeof useSessionHandlers>;

interface ViewPropsArgs {
  state: SessionState;
  socketStatus: SocketStatus;
  nowMs: number;
  ui: UiState;
  serialBridge: SerialBridgeState;
  handlers: Handlers;
}

function undismissedError(state: SessionState, dismissed: string | null): string | null {
  if (state.error === null) {
    return null;
  }
  return `${state.error.code}:${state.error.message}` === dismissed ? null : state.error.message;
}

function buildViewProps(args: ViewPropsArgs): SessionViewProps {
  const { state, ui, serialBridge, handlers } = args;
  const tapModeLocked = state.isCreator ? ui.hostTapModeLocked : state.tapModeLocked;
  const tapMode = resolveTapMode(state, ui);
  const beepsLocked = state.isCreator ? ui.hostBeepsLocked : state.beepsLocked;
  const beepsEnabled = resolveBeepsEnabled(state, ui);
  return {
    state,
    socketStatus: args.socketStatus,
    nowMs: args.nowMs,
    beepsEnabled,
    beepsLocked,
    mutedTables: ui.mutedTables,
    tapMode,
    tapModeLocked,
    transientError: ui.transientError ?? undismissedError(state, ui.dismissedError),
    serialConnected: serialBridge.serial.connected,
    serialFirmware: serialBridge.serial.firmware,
    serialPortLabel: serialBridge.serial.portLabel,
    serialSupported: isWebSerialSupported(),
    serialBusy: serialBridge.serialBusy,
    showAllPorts: ui.showAllPorts,
    rangeStart: ui.rangeStart,
    rangeEnd: ui.rangeEnd,
    renameValue: ui.renameValue,
    confirmEnd: ui.confirmEnd,
    onDismissError: () => {
      ui.setTransientError(null);
      ui.setDismissedError(
        state.error === null ? null : `${state.error.code}:${state.error.message}`,
      );
    },
    onRangeStartChange: ui.setRangeStart,
    onRangeEndChange: ui.setRangeEnd,
    onRenameChange: ui.setRenameValue,
    onShowAllChange: ui.setShowAllPorts,
    onAskEnd: () => ui.setConfirmEnd(true),
    onCancelEnd: () => ui.setConfirmEnd(false),
    ...handlers,
  };
}

function SessionConsole({ sessionId }: { sessionId: string }) {
  const [state, dispatch] = useReducer(sessionReducer, initialSessionState);
  const [socketStatus, setSocketStatus] = useState<SocketStatus>("connecting");
  const ui = useSessionUiState(state);
  const nowMs = useNowTick();
  const socketRef = useRef<ReconnectingSocket | null>(null);
  const stopReconnectRef = useRef(false);
  const prevColorsRef = useRef<Map<number, Color>>(new Map());
  const send = useCallback((msg: SessionClientMsg) => socketRef.current?.send(msg) ?? false, []);
  const serialBridge = useSerialBridge(send, ui.setTransientError);
  const beepsEnabled = resolveBeepsEnabled(state, ui);
  useSessionSocket({
    sessionId,
    dispatch,
    setSocketStatus,
    socketRef,
    bridgeRef: serialBridge.bridgeRef,
    stopReconnectRef,
  });
  usePendingPort(state.isCreator, sessionId, serialBridge.connectToPort);
  useBeeps(state, beepsEnabled, ui.mutedTables, prevColorsRef);
  const handlers = useSessionHandlers({
    state,
    send,
    setTransientError: ui.setTransientError,
    setMutedTables: ui.setMutedTables,
    beepsEnabled,
    hostBeepsEnabled: ui.hostBeepsEnabled,
    hostBeepsLocked: ui.hostBeepsLocked,
    setLocalBeepsEnabled: ui.setLocalBeepsEnabled,
    setHostBeepsEnabled: ui.setHostBeepsEnabled,
    setHostBeepsLocked: ui.setHostBeepsLocked,
    hostTapMode: ui.hostTapMode,
    hostTapModeLocked: ui.hostTapModeLocked,
    setLocalTapMode: ui.setLocalTapMode,
    setHostTapMode: ui.setHostTapMode,
    setHostTapModeLocked: ui.setHostTapModeLocked,
    rangeStart: ui.rangeStart,
    rangeEnd: ui.rangeEnd,
    renameValue: ui.renameValue,
    sessionId,
    showAllPorts: ui.showAllPorts,
    connectToPort: serialBridge.connectToPort,
    bridgeRef: serialBridge.bridgeRef,
    setConfirmEnd: ui.setConfirmEnd,
  });
  return (
    <SessionView {...buildViewProps({ state, socketStatus, nowMs, ui, serialBridge, handlers })} />
  );
}
