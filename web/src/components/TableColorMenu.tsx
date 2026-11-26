// SPDX-License-Identifier: AGPL-3.0-or-later

import { Check } from "lucide-react";
import { useEffect, useLayoutEffect, useRef } from "react";
import { COLOR_GREEN, COLOR_ORANGE, COLOR_RED, type Color } from "@/protocol/types";
import { STATUS_LABELS, StatusIcon } from "./StatusIcon";

const MENU_WIDTH_PX = 204;
const MENU_HEIGHT_PX = 196;
const VIEWPORT_MARGIN_PX = 12;
const CHECK_ICON_SIZE = 16;

const COLOR_OPTIONS: readonly { color: Color; title: string }[] = [
  { color: COLOR_GREEN, title: "Green" },
  { color: COLOR_ORANGE, title: "Orange" },
  { color: COLOR_RED, title: "Red" },
];

export interface TableColorMenuProps {
  table: number;
  currentColor: Color;
  x: number;
  y: number;
  onSelect: (table: number, color: Color) => void;
  onClose: () => void;
}

export function clampMenuPosition(
  x: number,
  y: number,
  viewportWidth: number,
  viewportHeight: number,
): { left: number; top: number } {
  const maxLeft = Math.max(VIEWPORT_MARGIN_PX, viewportWidth - MENU_WIDTH_PX - VIEWPORT_MARGIN_PX);
  const maxTop = Math.max(VIEWPORT_MARGIN_PX, viewportHeight - MENU_HEIGHT_PX - VIEWPORT_MARGIN_PX);
  return {
    left: Math.min(Math.max(VIEWPORT_MARGIN_PX, x), maxLeft),
    top: Math.min(Math.max(VIEWPORT_MARGIN_PX, y), maxTop),
  };
}

export function applyMenuPosition(
  style: { left: string; top: string },
  point: { x: number; y: number },
  viewport: { width: number; height: number },
): void {
  const pos = clampMenuPosition(point.x, point.y, viewport.width, viewport.height);
  style.left = `${pos.left}px`;
  style.top = `${pos.top}px`;
}

export function shouldDismissOnPointer(
  containsTarget: (target: unknown) => boolean,
  target: unknown,
): boolean {
  return !containsTarget(target);
}

export function shouldDismissOnKey(key: string): boolean {
  return key === "Escape";
}

function useMenuPosition(menuRef: { current: HTMLDivElement | null }, x: number, y: number): void {
  const useIsoLayoutEffect = typeof document === "undefined" ? useEffect : useLayoutEffect;
  useIsoLayoutEffect(() => {
    const el = menuRef.current;
    if (el !== null) {
      applyMenuPosition(
        el.style,
        { x, y },
        { width: globalThis.innerWidth, height: globalThis.innerHeight },
      );
    }
  }, [menuRef, x, y]);
}

export interface EventTargetLike {
  addEventListener: (type: string, listener: (event: unknown) => void, capture?: boolean) => void;
  removeEventListener: (
    type: string,
    listener: (event: unknown) => void,
    capture?: boolean,
  ) => void;
}

/** Calls `onClose` on Escape or a press outside; returns the cleanup. */
export function attachOutsideDismiss(
  doc: EventTargetLike,
  isInside: (target: unknown) => boolean,
  onClose: () => void,
): () => void {
  const onPointerDown = (event: unknown) => {
    const target =
      typeof event === "object" && event !== null ? Reflect.get(event, "target") : null;
    if (shouldDismissOnPointer(isInside, target)) {
      onClose();
    }
  };
  const onKeyDown = (event: unknown) => {
    const key = typeof event === "object" && event !== null ? Reflect.get(event, "key") : null;
    if (typeof key === "string" && shouldDismissOnKey(key)) {
      onClose();
    }
  };
  doc.addEventListener("pointerdown", onPointerDown, true);
  doc.addEventListener("keydown", onKeyDown);
  return () => {
    doc.removeEventListener("pointerdown", onPointerDown, true);
    doc.removeEventListener("keydown", onKeyDown);
  };
}

/** Like `attachOutsideDismiss`, and also closes the menu on resize or scroll. */
export function attachMenuDismiss(
  doc: EventTargetLike,
  win: EventTargetLike,
  isInside: (target: unknown) => boolean,
  onClose: () => void,
): () => void {
  const detachOutside = attachOutsideDismiss(doc, isInside, onClose);
  win.addEventListener("resize", onClose);
  win.addEventListener("scroll", onClose, true);
  return () => {
    detachOutside();
    win.removeEventListener("resize", onClose);
    win.removeEventListener("scroll", onClose, true);
  };
}

function useMenuDismiss(menuRef: { current: HTMLDivElement | null }, onClose: () => void): void {
  useEffect(() => {
    menuRef.current?.querySelector<HTMLButtonElement>('[role="menuitemradio"]')?.focus();
    const isInside = (target: unknown) =>
      target instanceof Node && (menuRef.current?.contains(target) ?? false);
    return attachMenuDismiss(document, globalThis, isInside, onClose);
  }, [menuRef, onClose]);
}

export function selectMenuColor(
  table: number,
  color: Color,
  onSelect: (table: number, color: Color) => void,
  onClose: () => void,
): void {
  onSelect(table, color);
  onClose();
}

export function TableColorMenu({
  table,
  currentColor,
  x,
  y,
  onSelect,
  onClose,
}: Readonly<TableColorMenuProps>) {
  const menuRef = useRef<HTMLDivElement | null>(null);
  useMenuPosition(menuRef, x, y);
  useMenuDismiss(menuRef, onClose);
  return (
    <div
      ref={menuRef}
      className="table-context-menu"
      role="menu"
      tabIndex={-1}
      aria-label={`Set color for table ${table}`}
      onContextMenu={(event) => event.preventDefault()}
    >
      <div className="table-context-title">Table {table}</div>
      {COLOR_OPTIONS.map((option) => {
        const selected = option.color === currentColor;
        return (
          <button
            key={option.color}
            type="button"
            role="menuitemradio"
            aria-checked={selected}
            className="table-context-item"
            data-option={option.color}
            onClick={() => selectMenuColor(table, option.color, onSelect, onClose)}
          >
            <span className="table-context-item-main">
              <StatusIcon color={option.color} />
              <span>
                {option.title} ({STATUS_LABELS[option.color]})
              </span>
            </span>
            {selected ? (
              <Check className="table-context-check" size={CHECK_ICON_SIZE} aria-hidden="true" />
            ) : null}
          </button>
        );
      })}
    </div>
  );
}
