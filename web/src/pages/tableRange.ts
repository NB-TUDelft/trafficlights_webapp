// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure handlers for the creator's table-range popup.

import type { SessionClientMsg, TableRange } from "@/protocol/types";
import { isValidTableRange } from "@/protocol/types";

const PARSE_RADIX = 10;

export const INVALID_RANGE_MESSAGE = "Enter a valid table range (start <= end, 1-9999).";

export interface RangeDialogArgs {
  /** The session's current range, which the popup starts from. */
  range: TableRange;
  rangeStart: string;
  rangeEnd: string;
  send: (msg: SessionClientMsg) => boolean;
  setRangeStart: (value: string) => void;
  setRangeEnd: (value: string) => void;
  setRangeError: (value: string | null) => void;
  setRangeDialogOpen: (value: boolean) => void;
}

/** Opens the popup with the inputs reset to the session's current range. */
export function openRangeDialog(args: Readonly<RangeDialogArgs>): void {
  args.setRangeStart(String(args.range.start));
  args.setRangeEnd(String(args.range.end));
  args.setRangeError(null);
  args.setRangeDialogOpen(true);
}

export function closeRangeDialog(args: Readonly<RangeDialogArgs>): void {
  args.setRangeDialogOpen(false);
  args.setRangeError(null);
}

/** Sends a valid range and closes the popup, or keeps it open with an error. */
export function applyRangeDialog(args: Readonly<RangeDialogArgs>): void {
  const start = Number.parseInt(args.rangeStart, PARSE_RADIX);
  const end = Number.parseInt(args.rangeEnd, PARSE_RADIX);
  if (!isValidTableRange(start, end)) {
    args.setRangeError(INVALID_RANGE_MESSAGE);
    return;
  }
  args.send({ type: "set_table_range", start, end });
  closeRangeDialog(args);
}
