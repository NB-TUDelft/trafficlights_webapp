// SPDX-License-Identifier: AGPL-3.0-or-later

import { Link } from "react-router";
import { endReasonLabel } from "@/lib/firmware";
import type { SessionEndedInfo } from "@/protocol/reducers";
import type { FirmwareInfo, UnlistedTable } from "@/protocol/types";
import { STATUS_LABELS } from "./StatusIcon";

export function ErrorBanner({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div className="banner banner-error" role="alert">
      <span>{message}</span>
      <button type="button" className="ghost-button" onClick={onDismiss}>
        Dismiss
      </button>
    </div>
  );
}

export function WrongModeBanner({ firmware }: { firmware: FirmwareInfo }) {
  const stationText = firmware.station_id === undefined ? "" : ` (station ${firmware.station_id})`;
  return (
    <div className="banner banner-warning" role="alert">
      <strong>Wrong board plugged in.</strong>
      <span>
        A Student Station{stationText} is connected instead of a Teacher Receiver. Plug in the
        receiver micro:bit to stream table status.
      </span>
    </div>
  );
}

function unlistedLabel(entry: UnlistedTable): string {
  if (entry.table === 0) {
    return `Table 0 (unset ID): ${STATUS_LABELS[entry.color]}`;
  }
  return `Table ${entry.table}: ${STATUS_LABELS[entry.color]}`;
}

export function UnlistedBanner({ tables }: { tables: readonly UnlistedTable[] }) {
  if (tables.length === 0) {
    return null;
  }
  return (
    <div className="banner banner-info">
      <strong>Unlisted tables heard</strong>
      <ul>
        {tables.map((entry) => (
          <li key={entry.table}>{unlistedLabel(entry)}</li>
        ))}
      </ul>
    </div>
  );
}

export interface EndedBannerProps {
  ended: SessionEndedInfo;
  isCreator: boolean;
  onDownloadLog: () => void;
}

export function EndedBanner({ ended, isCreator, onDownloadLog }: EndedBannerProps) {
  return (
    <div className="banner banner-ended" role="alert">
      <strong>Session ended</strong>
      <span>Reason: {endReasonLabel(ended.reason)}.</span>
      <Link to="/">Back to Lobby</Link>
      {isCreator && ended.logAvailable ? (
        <button type="button" onClick={onDownloadLog}>
          Download statistics (.jsonl)
        </button>
      ) : null}
    </div>
  );
}
