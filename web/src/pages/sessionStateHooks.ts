// SPDX-License-Identifier: AGPL-3.0-or-later
// Local UI state and serial-bridge hooks for the session console.

import { useCallback, useEffect, useRef, useState } from "react";
import type { SessionState } from "@/protocol/reducers";
import type { SessionClientMsg, TapMode } from "@/protocol/types";
import { DEFAULT_BEEPS_ENABLED, DEFAULT_TAP_MODE } from "@/protocol/types";
import { CreatorSerialBridge } from "@/serial/bridge";
import { takePendingSerialPort } from "@/serial/pending";
import type { SerialPortLike, SerialStatusUpdate } from "@/serial/types";

export function useSessionUiState(state: SessionState) {
  const [localBeepsEnabled, setLocalBeepsEnabled] = useState(DEFAULT_BEEPS_ENABLED);
  const [hostBeepsEnabled, setHostBeepsEnabled] = useState(state.beepsEnabled);
  const [hostBeepsLocked, setHostBeepsLocked] = useState(state.beepsLocked);
  const [mutedTables, setMutedTables] = useState<ReadonlySet<number>>(new Set());
  const [localTapMode, setLocalTapMode] = useState<TapMode>(DEFAULT_TAP_MODE);
  const [hostTapMode, setHostTapMode] = useState<TapMode>(state.tapMode);
  const [hostTapModeLocked, setHostTapModeLocked] = useState(state.tapModeLocked);
  const [transientError, setTransientError] = useState<string | null>(null);
  const [showAllPorts, setShowAllPorts] = useState(false);
  const [rangeStart, setRangeStart] = useState(String(state.range.start));
  const [rangeEnd, setRangeEnd] = useState(String(state.range.end));
  const [renameValue, setRenameValue] = useState("");
  const [confirmEnd, setConfirmEnd] = useState(false);
  const [dismissedError, setDismissedError] = useState<string | null>(null);
  useEffect(() => setRangeStart(String(state.range.start)), [state.range.start]);
  useEffect(() => setRangeEnd(String(state.range.end)), [state.range.end]);
  useEffect(() => setRenameValue(state.name), [state.name]);
  useEffect(() => setHostTapMode(state.tapMode), [state.tapMode]);
  useEffect(() => setHostTapModeLocked(state.tapModeLocked), [state.tapModeLocked]);
  useEffect(() => setHostBeepsEnabled(state.beepsEnabled), [state.beepsEnabled]);
  useEffect(() => setHostBeepsLocked(state.beepsLocked), [state.beepsLocked]);
  return {
    localBeepsEnabled,
    setLocalBeepsEnabled,
    hostBeepsEnabled,
    setHostBeepsEnabled,
    hostBeepsLocked,
    setHostBeepsLocked,
    mutedTables,
    setMutedTables,
    localTapMode,
    setLocalTapMode,
    hostTapMode,
    setHostTapMode,
    hostTapModeLocked,
    setHostTapModeLocked,
    transientError,
    setTransientError,
    showAllPorts,
    setShowAllPorts,
    rangeStart,
    setRangeStart,
    rangeEnd,
    setRangeEnd,
    renameValue,
    setRenameValue,
    confirmEnd,
    setConfirmEnd,
    dismissedError,
    setDismissedError,
  };
}

export function useSerialBridge(
  send: (msg: SessionClientMsg) => boolean,
  setTransientError: (value: string | null) => void,
) {
  const [serial, setSerial] = useState<SerialStatusUpdate>({
    connected: false,
    portLabel: null,
    firmware: null,
  });
  const [serialBusy, setSerialBusy] = useState(false);
  const bridgeRef = useRef<CreatorSerialBridge | null>(null);
  const connectToPort = useCallback(
    async (port: SerialPortLike) => {
      bridgeRef.current ??= new CreatorSerialBridge({
        onStationReport: (table, color) => send({ type: "station_report", table, color }),
        onStatus: (status) => {
          setSerial(status);
          send({
            type: "serial_status",
            connected: status.connected,
            port_label: status.portLabel,
            firmware: status.firmware,
          });
        },
        onError: (message) => setTransientError(message),
      });
      setSerialBusy(true);
      try {
        await bridgeRef.current.connect(port);
      } catch {
        setTransientError("Could not open the selected serial port.");
      } finally {
        setSerialBusy(false);
      }
    },
    [send, setTransientError],
  );
  useEffect(
    () => () => {
      bridgeRef.current?.disconnect().catch(() => undefined);
    },
    [],
  );
  return { serial, serialBusy, bridgeRef, connectToPort };
}

export function usePendingPort(
  isCreator: boolean,
  sessionId: string,
  connectToPort: (port: SerialPortLike) => Promise<void>,
): void {
  useEffect(() => {
    if (!isCreator) {
      return;
    }
    const port = takePendingSerialPort(sessionId);
    if (port !== null) {
      connectToPort(port).catch(() => undefined);
    }
  }, [isCreator, sessionId, connectToPort]);
}
