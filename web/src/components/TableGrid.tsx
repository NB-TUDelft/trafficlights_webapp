// SPDX-License-Identifier: AGPL-3.0-or-later

import type { Color, TableEntry } from "@/protocol/types";
import { nextCycleColor } from "@/protocol/types";
import { STATUS_LABELS, StatusIcon } from "./StatusIcon";

export interface TableGridProps {
  tables: readonly TableEntry[];
  mutedTables: ReadonlySet<number>;
  onCycleTable: (table: number, color: Color) => void;
  onToggleMute: (table: number) => void;
}

function MutedIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <path d="M11 5L6 9H2v6h4l5 4v-4" />
      <line x1="23" y1="9" x2="17" y2="15" />
      <line x1="17" y1="9" x2="23" y2="15" />
    </svg>
  );
}

function UnmutedIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
      <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
    </svg>
  );
}

interface TableCardProps {
  entry: TableEntry;
  isMuted: boolean;
  onCycleTable: (table: number, color: Color) => void;
  onToggleMute: (table: number) => void;
}

function TableCard({ entry, isMuted, onCycleTable, onToggleMute }: TableCardProps) {
  const nextColor = nextCycleColor(entry.color);
  const muteLabel = isMuted
    ? `Unmute beep for table ${entry.table}`
    : `Mute beep for table ${entry.table}`;
  return (
    <article className="table-card" data-color={entry.color}>
      <button
        type="button"
        className="table-action"
        aria-label={`Table ${entry.table}: ${STATUS_LABELS[entry.color]}. Activate to set ${nextColor}.`}
        onClick={() => onCycleTable(entry.table, nextColor)}
      >
        <span className="table-label">
          <strong>{entry.table}</strong>
          <StatusIcon color={entry.color} />
          <span className="table-status-text">{STATUS_LABELS[entry.color]}</span>
        </span>
      </button>
      <button
        type="button"
        className="mute-btn"
        aria-label={muteLabel}
        title={muteLabel}
        aria-pressed={isMuted}
        onClick={() => onToggleMute(entry.table)}
      >
        {isMuted ? <MutedIcon /> : <UnmutedIcon />}
      </button>
    </article>
  );
}

export function TableGrid({ tables, mutedTables, onCycleTable, onToggleMute }: TableGridProps) {
  return (
    <section className="table-grid" aria-label="Tables">
      {tables.map((entry) => (
        <TableCard
          key={entry.table}
          entry={entry}
          isMuted={mutedTables.has(entry.table)}
          onCycleTable={onCycleTable}
          onToggleMute={onToggleMute}
        />
      ))}
    </section>
  );
}
