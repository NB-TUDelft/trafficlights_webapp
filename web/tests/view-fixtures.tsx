// SPDX-License-Identifier: AGPL-3.0-or-later
// Shared rendering helpers for the session view and Add session tests.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import type { SessionViewProps } from "@/components/SessionView";
import { NewSessionView } from "@/pages/NewSessionPage";
import { initialSessionState, type SessionState } from "@/protocol/reducers";
import type { ServerBuildInfo } from "@/protocol/types";

export const serverInfo: ServerBuildInfo = {
  version: "1.2.3",
  commit: "abcdef123456",
  license: "AGPL-3.0-or-later",
  source_url: "https://github.com/NB-TUDelft/trafficlights_webapp",
  proto_version: 1,
  pin_required: false,
};

export const noop = (): void => {
  // Test stub.
};

export function render(node: React.ReactNode): string {
  return renderToStaticMarkup(<MemoryRouter>{node}</MemoryRouter>);
}

export function baseState(overrides: Partial<SessionState>): SessionState {
  return {
    ...initialSessionState,
    sessionId: "sess-1",
    name: "Studio 3B",
    tables: [
      { table: 1, color: "green" },
      { table: 2, color: "red", red_since_ms: 1000 },
    ],
    counts: { green: 1, orange: 0, red: 1 },
    receiverConnected: true,
    viewerCount: 2,
    serverInfo,
    hasSnapshot: true,
    ...overrides,
  };
}

export function viewProps(
  state: SessionState,
  overrides?: Partial<SessionViewProps>,
): SessionViewProps {
  return {
    state,
    socketStatus: "open",
    nowMs: 61_000,
    beepsEnabled: state.beepsEnabled,
    beepsLocked: state.beepsLocked,
    mutedTables: new Set([1]),
    tapMode: state.tapMode,
    tapModeLocked: state.tapModeLocked,
    transientError: null,
    serialConnected: true,
    serialFirmware: null,
    serialPortLabel: "micro:bit USB (0D28:0204)",
    serialSupported: true,
    serialBusy: false,
    showAllPorts: false,
    rangeStart: "1",
    rangeEnd: "2",
    renameValue: "Studio 3B",
    confirmEnd: false,
    viewerName: null,
    namePromptOpen: false,
    nameDraft: "",
    nameError: null,
    nameCanCancel: false,
    onNameDraftChange: noop,
    onSubmitName: noop,
    onChangeName: noop,
    onCancelName: noop,
    onDismissError: noop,
    onSetTable: noop,
    onToggleMute: noop,
    onToggleBeeps: noop,
    onToggleBeepsLock: noop,
    onToggleTapMode: noop,
    onToggleTapModeLock: noop,
    onConnectSerial: noop,
    onDisconnectSerial: noop,
    onShowAllChange: noop,
    onRangeStartChange: noop,
    onRangeEndChange: noop,
    onApplyRange: noop,
    onResetAll: noop,
    onRenameChange: noop,
    onRename: noop,
    onDownloadLog: noop,
    onAskEnd: noop,
    onConfirmEnd: noop,
    onCancelEnd: noop,
    ...overrides,
  };
}

export function newSessionHtml(pinRequired: boolean, collectNames = false): string {
  return render(
    <NewSessionView
      serverInfo={{ ...serverInfo, pin_required: pinRequired }}
      name=""
      startValue="1"
      endValue="60"
      pin=""
      collectNames={collectNames}
      onCollectNamesChange={noop}
      showAllPorts={false}
      serialSupported={true}
      hasPendingPort={false}
      error={null}
      submitting={false}
      onNameChange={noop}
      onStartChange={noop}
      onEndChange={noop}
      onPinChange={noop}
      onShowAllChange={noop}
      onSelectPort={noop}
      onSubmit={noop}
    />,
  );
}
