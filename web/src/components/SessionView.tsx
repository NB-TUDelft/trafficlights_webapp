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
import { SessionHeader } from "@/components/SessionHeader";
import { TableGrid } from "@/components/TableGrid";
import type { SocketStatus } from "@/lib/socket";
import { computeLongestOnRed } from "@/lib/time";
import type { SessionState } from "@/protocol/reducers";
import type { Color, FirmwareInfo } from "@/protocol/types";
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

export interface SessionViewProps {
  state: SessionState;
  socketStatus: SocketStatus;
  nowMs: number;
  beepsEnabled: boolean;
  mutedTables: ReadonlySet<number>;
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
  onCycleTable: (table: number, color: Color) => void;
  onToggleMute: (table: number) => void;
  onToggleBeeps: () => void;
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

function CreatorControls(props: SessionViewProps) {
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
        viewerCount={props.state.viewerCount}
        onResetAll={props.onResetAll}
        onToggleBeeps={props.onToggleBeeps}
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

export function SessionView(props: SessionViewProps) {
  const { state } = props;
  const displayError = props.transientError;
  const firmware = props.serialFirmware ?? state.firmware;
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
      />
      {displayError === null ? null : (
        <ErrorBanner message={displayError} onDismiss={props.onDismissError} />
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
      <section className="control-grid">
        {state.isCreator ? (
          <CreatorControls {...props} />
        ) : (
          <AudioInfoCard
            beepsEnabled={props.beepsEnabled}
            viewerCount={state.viewerCount}
            onToggleBeeps={props.onToggleBeeps}
          />
        )}
        <LongestOnRed entries={longest} />
      </section>
      <TableGrid
        tables={state.tables}
        mutedTables={props.mutedTables}
        onCycleTable={props.onCycleTable}
        onToggleMute={props.onToggleMute}
      />
      <Footer serverInfo={state.serverInfo} />
    </main>
  );
}
