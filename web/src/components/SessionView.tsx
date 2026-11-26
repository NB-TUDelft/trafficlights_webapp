// SPDX-License-Identifier: AGPL-3.0-or-later

import { EndedBanner, ErrorBanner, UnlistedBanner, WrongModeBanner } from "@/components/Banners";
import {
  AudioInfoCard,
  QuickActionsCard,
  SerialConnectionCard,
  SessionManagementCard,
  TableRangeCard,
} from "@/components/CreatorPanels";
import { Footer } from "@/components/Footer";
import { LongestOnRed } from "@/components/LongestOnRed";
import { SessionHeader, type SessionStatusKind } from "@/components/SessionHeader";
import { TableGrid } from "@/components/TableGrid";
import type { SocketStatus } from "@/lib/socket";
import { computeLongestOnRed } from "@/lib/time";
import type { SessionState } from "@/protocol/reducers";
import type { Color, FirmwareInfo, TapMode } from "@/protocol/types";
import { EXPECTED_PROTO_VERSION } from "@/protocol/types";

export function sessionStatusLabel(
  socketStatus: SocketStatus,
  receiverConnected: boolean,
  ended: boolean,
): string {
  if (ended) {
    return "Session ended";
  }
  if (socketStatus !== "open") {
    return "Reconnecting...";
  }
  return receiverConnected ? "Live" : "Receiver disconnected";
}

export function sessionStatusKind(
  socketStatus: SocketStatus,
  receiverConnected: boolean,
  ended: boolean,
): SessionStatusKind {
  if (ended) {
    return "ended";
  }
  if (socketStatus !== "open") {
    return "reconnecting";
  }
  return receiverConnected ? "live" : "disconnected";
}

export interface SessionViewProps {
  state: SessionState;
  socketStatus: SocketStatus;
  nowMs: number;
  beepsEnabled: boolean;
  beepsLocked: boolean;
  mutedTables: ReadonlySet<number>;
  tapMode: TapMode;
  tapModeLocked: boolean;
  transientError: string | null;
  serialConnected: boolean;
  serialFirmware: FirmwareInfo | null;
  serialPortLabel: string | null;
  serialSupported: boolean;
  serialBusy: boolean;
  showAllPorts: boolean;
  rangeStart: string;
  rangeEnd: string;
  renameValue: string;
  confirmEnd: boolean;
  onDismissError: () => void;
  onSetTable: (table: number, color: Color) => void;
  onToggleMute: (table: number) => void;
  onToggleBeeps: () => void;
  onToggleBeepsLock: () => void;
  onToggleTapMode: () => void;
  onToggleTapModeLock: () => void;
  onConnectSerial: () => void;
  onDisconnectSerial: () => void;
  onShowAllChange: (value: boolean) => void;
  onRangeStartChange: (value: string) => void;
  onRangeEndChange: (value: string) => void;
  onApplyRange: () => void;
  onResetAll: () => void;
  onRenameChange: (value: string) => void;
  onRename: () => void;
  onDownloadLog: () => void;
  onAskEnd: () => void;
  onConfirmEnd: () => void;
  onCancelEnd: () => void;
}

function CreatorControls(props: Readonly<SessionViewProps>) {
  return (
    <>
      <SerialConnectionCard
        connected={props.serialConnected}
        portLabel={props.serialPortLabel ?? props.state.portLabel}
        firmware={props.serialFirmware ?? props.state.firmware}
        showAllPorts={props.showAllPorts}
        serialSupported={props.serialSupported}
        busy={props.serialBusy}
        onShowAllChange={props.onShowAllChange}
        onConnect={props.onConnectSerial}
        onDisconnect={props.onDisconnectSerial}
      />
      <TableRangeCard
        startValue={props.rangeStart}
        endValue={props.rangeEnd}
        onStartChange={props.onRangeStartChange}
        onEndChange={props.onRangeEndChange}
        onApply={props.onApplyRange}
      />
      <QuickActionsCard
        receiverConnected={props.state.receiverConnected}
        beepsEnabled={props.beepsEnabled}
        beepsLocked={props.beepsLocked}
        viewerCount={props.state.viewerCount}
        tapMode={props.tapMode}
        tapModeLocked={props.tapModeLocked}
        onResetAll={props.onResetAll}
        onToggleBeeps={props.onToggleBeeps}
        onToggleBeepsLock={props.onToggleBeepsLock}
        onToggleTapMode={props.onToggleTapMode}
        onToggleTapModeLock={props.onToggleTapModeLock}
      />
      <SessionManagementCard
        renameValue={props.renameValue}
        confirmEnd={props.confirmEnd}
        onRenameChange={props.onRenameChange}
        onRename={props.onRename}
        onDownloadLog={props.onDownloadLog}
        onAskEnd={props.onAskEnd}
        onConfirmEnd={props.onConfirmEnd}
        onCancelEnd={props.onCancelEnd}
      />
    </>
  );
}

function SessionBanners(props: Readonly<SessionViewProps>) {
  const { state } = props;
  const firmware = props.serialFirmware ?? state.firmware;
  return (
    <>
      {props.transientError === null ? null : (
        <ErrorBanner message={props.transientError} onDismiss={props.onDismissError} />
      )}
      {state.serverInfo !== null && state.serverInfo.proto_version !== EXPECTED_PROTO_VERSION ? (
        <div className="banner banner-warning" role="alert">
          This page is out of date for the server. Please reload the page.
        </div>
      ) : null}
      {state.ended === null ? null : (
        <EndedBanner
          ended={state.ended}
          isCreator={state.isCreator}
          onDownloadLog={props.onDownloadLog}
        />
      )}
      {state.isCreator && firmware !== null && firmware.mode === "station" ? (
        <WrongModeBanner firmware={firmware} />
      ) : null}
      {state.isCreator ? <UnlistedBanner tables={state.unlistedTables} /> : null}
    </>
  );
}

export function SessionView(props: Readonly<SessionViewProps>) {
  const { state } = props;
  const longest = computeLongestOnRed(state.tables, props.nowMs, state.clockOffsetMs);
  return (
    <main className="page">
      <SessionHeader
        name={state.name}
        counts={state.counts}
        statusLabel={sessionStatusLabel(
          props.socketStatus,
          state.receiverConnected,
          state.ended !== null,
        )}
        statusKind={sessionStatusKind(
          props.socketStatus,
          state.receiverConnected,
          state.ended !== null,
        )}
      />
      <SessionBanners {...props} />
      <section className={state.isCreator ? "control-grid control-grid-creator" : "control-grid"}>
        {state.isCreator ? (
          <CreatorControls {...props} />
        ) : (
          <AudioInfoCard
            beepsEnabled={props.beepsEnabled}
            beepsLocked={props.beepsLocked}
            viewerCount={state.viewerCount}
            tapMode={props.tapMode}
            tapModeLocked={props.tapModeLocked}
            onToggleBeeps={props.onToggleBeeps}
            onToggleTapMode={props.onToggleTapMode}
          />
        )}
        <LongestOnRed entries={longest} />
      </section>
      <TableGrid
        tables={state.tables}
        beepsEnabled={props.beepsEnabled}
        mutedTables={props.mutedTables}
        tapMode={props.tapMode}
        onSetTable={props.onSetTable}
        onToggleMute={props.onToggleMute}
      />
      <Footer serverInfo={state.serverInfo} />
    </main>
  );
}
