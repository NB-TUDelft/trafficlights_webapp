// SPDX-License-Identifier: AGPL-3.0-or-later

import { firmwareBadgeLabel } from "@/lib/firmware";
import type { FirmwareInfo, TapMode } from "@/protocol/types";
import { MAX_TABLE_ID, MIN_TABLE_ID } from "@/protocol/types";
import { ToggleSwitch } from "./ToggleSwitch";

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

export function SerialConnectionCard(props: Readonly<SerialCardProps>) {
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

export function TableRangeCard(props: Readonly<RangeCardProps>) {
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

function tapModeHint(tapMode: TapMode, lockedForViewer: boolean): string {
  const modeText = tapMode === "cycle" ? "Tap cycles colors" : "Tap resets to green";
  return lockedForViewer ? `Locked by host · ${modeText}` : modeText;
}

function beepHint(beepsEnabled: boolean, lockedForViewer: boolean): string {
  const statusText = beepsEnabled ? "Beeps On" : "Beeps Off";
  return lockedForViewer ? `Locked by host · ${statusText}` : statusText;
}

export interface QuickActionsProps {
  receiverConnected: boolean;
  beepsEnabled: boolean;
  beepsLocked: boolean;
  viewerCount: number;
  tapMode: TapMode;
  tapModeLocked: boolean;
  onResetAll: () => void;
  onToggleBeeps: () => void;
  onToggleBeepsLock: () => void;
  onToggleTapMode: () => void;
  onToggleTapModeLock: () => void;
}

export function QuickActionsCard(props: Readonly<QuickActionsProps>) {
  return (
    <article className="control-card">
      <h2 className="card-label">Quick Actions</h2>
      <button type="button" disabled={!props.receiverConnected} onClick={props.onResetAll}>
        Reset All To Green
      </button>
      <ToggleSwitch
        id="host-beeps"
        label="Alert beeps"
        hint={beepHint(props.beepsEnabled, false)}
        checked={props.beepsEnabled}
        onToggle={props.onToggleBeeps}
      />
      <ToggleSwitch
        id="host-beeps-lock"
        label="Lock beeps"
        hint={props.beepsLocked ? "Viewers follow host" : "Viewers choose own"}
        checked={props.beepsLocked}
        onToggle={props.onToggleBeepsLock}
      />
      <ToggleSwitch
        id="host-tap-mode"
        label="Cycle on tap"
        hint={tapModeHint(props.tapMode, false)}
        checked={props.tapMode === "cycle"}
        onToggle={props.onToggleTapMode}
      />
      <ToggleSwitch
        id="host-tap-lock"
        label="Lock tap mode"
        hint={props.tapModeLocked ? "Viewers follow host" : "Viewers choose own"}
        checked={props.tapModeLocked}
        onToggle={props.onToggleTapModeLock}
      />
      <p className="muted-text">{props.viewerCount} viewers connected</p>
    </article>
  );
}

export interface AudioInfoProps {
  beepsEnabled: boolean;
  beepsLocked: boolean;
  viewerCount: number;
  tapMode: TapMode;
  tapModeLocked: boolean;
  /** The name this viewer gave, in a session that collects names. */
  viewerName?: string | null;
  onToggleBeeps: () => void;
  onToggleTapMode: () => void;
  onChangeName?: () => void;
}

export function AudioInfoCard(props: Readonly<AudioInfoProps>) {
  const tapInstructions =
    props.tapMode === "cycle"
      ? "Tap a table tile to cycle colors. Hold or right-click for all colors."
      : "Tap a table tile to reset to green. Hold or right-click for all colors.";
  return (
    <article className="control-card">
      <h2 className="card-label">Audio &amp; Session Info</h2>
      <ToggleSwitch
        id="viewer-beeps"
        label="Alert beeps"
        hint={beepHint(props.beepsEnabled, props.beepsLocked)}
        checked={props.beepsEnabled}
        disabled={props.beepsLocked}
        onToggle={props.onToggleBeeps}
      />
      <ToggleSwitch
        id="viewer-tap-mode"
        label="Cycle on tap"
        hint={tapModeHint(props.tapMode, props.tapModeLocked)}
        checked={props.tapMode === "cycle"}
        disabled={props.tapModeLocked}
        onToggle={props.onToggleTapMode}
      />
      <p className="muted-text">{props.viewerCount} viewers connected</p>
      <p className="muted-text">{tapInstructions}</p>
      {props.viewerName === undefined || props.viewerName === null ? null : (
        <p className="muted-text viewer-name-line">
          <span>
            Your name: <strong>{props.viewerName}</strong>
          </span>
          <button type="button" className="ghost-button" onClick={props.onChangeName}>
            Change name
          </button>
        </p>
      )}
    </article>
  );
}

export interface SessionManagementProps {
  /** Whether the session collects viewer names. */
  collectNames?: boolean;
  renameValue: string;
  confirmEnd: boolean;
  onRenameChange: (value: string) => void;
  onRename: () => void;
  onDownloadLog: () => void;
  onAskEnd: () => void;
  onConfirmEnd: () => void;
  onCancelEnd: () => void;
}

export function SessionManagementCard(props: Readonly<SessionManagementProps>) {
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
      {props.collectNames === true ? (
        <p className="muted-text">
          Viewer names are collected: the statistics log names who changed each table, with you as
          "host".
        </p>
      ) : null}
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
