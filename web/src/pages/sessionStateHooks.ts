// SPDX-License-Identifier: AGPL-3.0-or-later
// Local UI state and serial-bridge hooks for the session console.

import { useCallback, useEffect, useRef, useState } from "react";
import type { SessionState } from "@/protocol/reducers";
import {
  getLastViewerName,
  getViewerKey,
  getViewerName,
  hasCreatorToken,
  saveViewerName,
} from "@/protocol/storage";
import type { SessionClientMsg, TableRange, TapMode } from "@/protocol/types";
import {
  DEFAULT_BEEPS_ENABLED,
  DEFAULT_TAP_MODE,
  HOST_ACTOR,
  MAX_VIEWER_NAME_CHARS,
  normalizeViewerName,
} from "@/protocol/types";
import { CreatorSerialBridge } from "@/serial/bridge";
import { takePendingSerialPort } from "@/serial/pending";
import type { SerialPortLike, SerialStatusUpdate } from "@/serial/types";

/** The creator's table-range popup: open flag, input drafts, and inline error. */
function useRangeDialogState(range: TableRange) {
  const [rangeStart, setRangeStart] = useState(String(range.start));
  const [rangeEnd, setRangeEnd] = useState(String(range.end));
  const [rangeDialogOpen, setRangeDialogOpen] = useState(false);
  const [rangeError, setRangeError] = useState<string | null>(null);
  useEffect(() => setRangeStart(String(range.start)), [range.start]);
  useEffect(() => setRangeEnd(String(range.end)), [range.end]);
  return {
    rangeStart,
    setRangeStart,
    rangeEnd,
    setRangeEnd,
    rangeDialogOpen,
    setRangeDialogOpen,
    rangeError,
    setRangeError,
  };
}

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
  const rangeDialog = useRangeDialogState(state.range);
  const [renameValue, setRenameValue] = useState("");
  const [confirmEnd, setConfirmEnd] = useState(false);
  const [dismissedError, setDismissedError] = useState<string | null>(null);
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
    rangeDialog,
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

const NAME_ERROR_CODES: ReadonlySet<string> = new Set([
  "invalid_viewer_name",
  "name_required",
  "name_taken",
]);

/** Whether a server error is about this viewer's name (shown in the name prompt). */
export function isNameError(code: string): boolean {
  return NAME_ERROR_CODES.has(code);
}

/**
 * The viewer's name in a session that collects names: remembered per
 * session, suggested from the last session, asked for in a prompt, and asked
 * for again when the server refuses it (for example a name already in use).
 */
export function useViewerName(
  sessionId: string,
  state: SessionState,
  send: (msg: SessionClientMsg) => boolean,
) {
  const [name, setName] = useState<string | null>(() => getViewerName(sessionId));
  const [draft, setDraft] = useState(() => getViewerName(sessionId) ?? getLastViewerName() ?? "");
  const [editing, setEditing] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [holdsCreatorToken] = useState(() => hasCreatorToken(sessionId));
  useEffect(() => {
    if (state.error !== null && isNameError(state.error.code)) {
      setName(null);
      setFormError(state.error.message);
    }
  }, [state.error]);
  const submit = useCallback(() => {
    const normalized = normalizeViewerName(draft);
    if (normalized === null) {
      setFormError(
        `Enter a name of 1 to ${MAX_VIEWER_NAME_CHARS} characters other than "${HOST_ACTOR}".`,
      );
      return;
    }
    saveViewerName(sessionId, normalized);
    setName(normalized);
    setDraft(normalized);
    setFormError(null);
    setEditing(false);
    send({ type: "identify", name: normalized, key: getViewerKey() });
  }, [draft, sessionId, send]);
  const change = useCallback(() => setEditing(true), []);
  const cancel = useCallback(() => {
    setEditing(false);
    setFormError(null);
    setDraft(name ?? "");
  }, [name]);
  const promptOpen =
    state.hasSnapshot &&
    state.collectNames &&
    !state.isCreator &&
    !holdsCreatorToken &&
    state.ended === null &&
    (name === null || editing);
  return {
    name,
    draft,
    setDraft,
    formError,
    promptOpen,
    canCancel: name !== null,
    submit,
    change,
    cancel,
  };
}
