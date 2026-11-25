// SPDX-License-Identifier: AGPL-3.0-or-later

import { firmwareBadgeLabel } from "@/lib/firmware";
import type { FirmwareInfo } from "@/protocol/types";
import { MAX_TABLE_ID, MIN_TABLE_ID } from "@/protocol/types";

export interface SerialCardProps {
  connected: boolean;
  portLabel: string | null;
  firmware: FirmwareInfo | null;
  showAllPorts: boolean;
  serialSupported: boolean;
  busy: boolean;
  onShowAllChange: (value: boolean) => void;
  onConnect: () => void;
  onDisconnect: () => void;
}

export function SerialConnectionCard(props: SerialCardProps) {
  const label = props.portLabel ?? "No receiver selected";
  return (
    <article className="control-card">
      <h2 className="card-label">Serial Connection</h2>
      <p>
        <strong>{label}</strong>
      </p>
      <p>
        <span className={props.connected ? "badge badge-online" : "badge badge-offline"}>
          {props.connected ? "Connected" : "Disconnected"}
        </span>
        {props.connected ? (
          <>
            {" "}
            <span className="badge">{firmwareBadgeLabel(props.connected, props.firmware)}</span>
          </>
        ) : null}
      </p>
      <label className="check-label">
        <input
          type="checkbox"
          checked={props.showAllPorts}
          onChange={(event) => props.onShowAllChange(event.target.checked)}
        />{" "}
        Show all ports
      </label>
      {props.connected ? (
        <button type="button" disabled={props.busy} onClick={props.onDisconnect}>
          Disconnect
        </button>
      ) : (
        <button
          type="button"
          disabled={props.busy || !props.serialSupported}
          onClick={props.onConnect}
        >
          Connect micro:bit
        </button>
      )}
      {props.serialSupported ? null : (
        <p className="muted-text">
          Web Serial needs a Chromium-based browser over HTTPS or localhost.
        </p>
      )}
    </article>
  );
}

export interface RangeCardProps {
  startValue: string;
  endValue: string;
  onStartChange: (value: string) => void;
  onEndChange: (value: string) => void;
  onApply: () => void;
}

export function TableRangeCard(props: RangeCardProps) {
  return (
    <article className="control-card">
      <h2 className="card-label">Table Range</h2>
      <div className="range-inputs">
        <label htmlFor="range-start">
          <span>Start</span>
          <input
            id="range-start"
            type="number"
            min={MIN_TABLE_ID}
            max={MAX_TABLE_ID}
            value={props.startValue}
            onChange={(event) => props.onStartChange(event.target.value)}
          />
        </label>
        <label htmlFor="range-end">
          <span>End</span>
          <input
            id="range-end"
            type="number"
            min={MIN_TABLE_ID}
            max={MAX_TABLE_ID}
            value={props.endValue}
            onChange={(event) => props.onEndChange(event.target.value)}
          />
        </label>
      </div>
      <button type="button" onClick={props.onApply}>
        Apply Range
      </button>
    </article>
  );
}

export interface QuickActionsProps {
  receiverConnected: boolean;
  beepsEnabled: boolean;
  viewerCount: number;
  onResetAll: () => void;
  onToggleBeeps: () => void;
}

export function QuickActionsCard(props: QuickActionsProps) {
  return (
    <article className="control-card">
      <h2 className="card-label">Quick Actions</h2>
      <button type="button" disabled={!props.receiverConnected} onClick={props.onResetAll}>
        Reset All To Green
      </button>
      <button type="button" onClick={props.onToggleBeeps}>
        {props.beepsEnabled ? "Beeps On" : "Beeps Off"}
      </button>
      <p className="muted-text">{props.viewerCount} viewers connected</p>
    </article>
  );
}

export interface AudioInfoProps {
  beepsEnabled: boolean;
  viewerCount: number;
  onToggleBeeps: () => void;
}

export function AudioInfoCard(props: AudioInfoProps) {
  return (
    <article className="control-card">
      <h2 className="card-label">Audio &amp; Session Info</h2>
      <button type="button" onClick={props.onToggleBeeps}>
        {props.beepsEnabled ? "Beeps On" : "Beeps Off"}
      </button>
      <p className="muted-text">{props.viewerCount} viewers connected</p>
      <p className="muted-text">Tap any table tile to cycle green → orange → red.</p>
    </article>
  );
}

export interface SessionManagementProps {
  renameValue: string;
  confirmEnd: boolean;
  onRenameChange: (value: string) => void;
  onRename: () => void;
  onDownloadLog: () => void;
  onAskEnd: () => void;
  onConfirmEnd: () => void;
  onCancelEnd: () => void;
}

export function SessionManagementCard(props: SessionManagementProps) {
  return (
    <article className="control-card">
      <h2 className="card-label">Session Management</h2>
      <label htmlFor="rename-session">
        <span>Session name</span>
        <input
          id="rename-session"
          type="text"
          value={props.renameValue}
          onChange={(event) => props.onRenameChange(event.target.value)}
        />
      </label>
      <button type="button" onClick={props.onRename}>
        Rename
      </button>
      <button type="button" onClick={props.onDownloadLog}>
        Download statistics (.jsonl)
      </button>
      {props.confirmEnd ? (
        <span className="inline-actions">
          <button type="button" className="danger-button" onClick={props.onConfirmEnd}>
            Confirm end
          </button>
          <button type="button" className="ghost-button" onClick={props.onCancelEnd}>
            Cancel
          </button>
        </span>
      ) : (
        <button type="button" className="danger-button" onClick={props.onAskEnd}>
          End session
        </button>
      )}
    </article>
  );
}
