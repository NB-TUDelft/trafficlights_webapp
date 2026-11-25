// SPDX-License-Identifier: AGPL-3.0-or-later

import { useCallback, useEffect, useMemo, useState } from "react";
import { createLongPressController } from "@/lib/longPress";
import type { Color, TableEntry, TapMode } from "@/protocol/types";
import { tapTargetColor } from "@/protocol/types";
import { STATUS_LABELS, StatusIcon } from "./StatusIcon";
import { TableColorMenu } from "./TableColorMenu";

const HALF = 2;

export interface TableGridProps {
  tables: readonly TableEntry[];
  mutedTables: ReadonlySet<number>;
  tapMode: TapMode;
  onSetTable: (table: number, color: Color) => void;
  onToggleMute: (table: number) => void;
}

interface MenuAnchor {
  table: number;
  x: number;
  y: number;
}

export function resolveContextAnchor(
  clientX: number,
  clientY: number,
  rect: { left: number; top: number; width: number; height: number },
): { x: number; y: number } {
  if (clientX === 0 && clientY === 0) {
    return { x: rect.left + rect.width / HALF, y: rect.top + rect.height / HALF };
  }
  return { x: clientX, y: clientY };
}

export function handleTileActivate(
  consumeSuppressedClick: () => boolean,
  onSetTable: (table: number, color: Color) => void,
  table: number,
  targetColor: Color,
): void {
  if (!consumeSuppressedClick()) {
    onSetTable(table, targetColor);
  }
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
  tapMode: TapMode;
  onSetTable: (table: number, color: Color) => void;
  onToggleMute: (table: number) => void;
  onOpenMenu: (anchor: MenuAnchor) => void;
}

function TableCard({
  entry,
  isMuted,
  tapMode,
  onSetTable,
  onToggleMute,
  onOpenMenu,
}: Readonly<TableCardProps>) {
  const targetColor = tapTargetColor(entry.color, tapMode);
  const muteLabel = isMuted
    ? `Unmute beep for table ${entry.table}`
    : `Mute beep for table ${entry.table}`;
  const longPress = useMemo(
    () => createLongPressController((x, y) => onOpenMenu({ table: entry.table, x, y })),
    [entry.table, onOpenMenu],
  );
  useEffect(() => longPress.cancel, [longPress]);
  const handleContextMenu = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    longPress.cancel();
    const anchor = resolveContextAnchor(
      event.clientX,
      event.clientY,
      event.currentTarget.getBoundingClientRect(),
    );
    onOpenMenu({ table: entry.table, ...anchor });
  };
  return (
    <article
      className="table-card"
      data-color={entry.color}
      title={`Table ${entry.table}: ${STATUS_LABELS[entry.color]}`}
    >
      <button
        type="button"
        className="table-action"
        aria-label={`Table ${entry.table}: ${STATUS_LABELS[entry.color]}. Activate to set ${targetColor}. Hold or right-click for all colors.`}
        onPointerDown={(event) => longPress.start(event)}
        onPointerMove={(event) => longPress.move(event)}
        onPointerUp={longPress.cancel}
        onPointerCancel={longPress.cancel}
        onContextMenu={handleContextMenu}
        onClick={() =>
          handleTileActivate(longPress.consumeSuppressedClick, onSetTable, entry.table, targetColor)
        }
      >
        <span className="table-label">
          <strong>{entry.table}</strong>
          <StatusIcon color={entry.color} />
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

export function TableGrid({
  tables,
  mutedTables,
  tapMode,
  onSetTable,
  onToggleMute,
}: Readonly<TableGridProps>) {
  const [menu, setMenu] = useState<MenuAnchor | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const activeEntry =
    menu === null ? null : (tables.find((entry) => entry.table === menu.table) ?? null);
  return (
    <>
      <section className="table-grid" aria-label="Tables">
        {tables.map((entry) => (
          <TableCard
            key={entry.table}
            entry={entry}
            isMuted={mutedTables.has(entry.table)}
            tapMode={tapMode}
            onSetTable={onSetTable}
            onToggleMute={onToggleMute}
            onOpenMenu={setMenu}
          />
        ))}
      </section>
      {menu !== null && activeEntry !== null ? (
        <TableColorMenu
          table={activeEntry.table}
          currentColor={activeEntry.color}
          x={menu.x}
          y={menu.y}
          onSelect={onSetTable}
          onClose={closeMenu}
        />
      ) : null}
    </>
  );
}
