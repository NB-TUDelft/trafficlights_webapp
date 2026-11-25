// SPDX-License-Identifier: AGPL-3.0-or-later
// Shape-based status icons from old/fastapi_receiver.py so state never
// relies on color alone: checkmark, exclamation mark, skull/help glyph.

import type { Color } from "@/protocol/types";

export const STATUS_LABELS: Record<Color, string> = {
  green: "OK",
  orange: "Question",
  red: "Needs help",
};

function GreenIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" aria-hidden="true">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}

function OrangeIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" aria-hidden="true">
      <line x1="12" y1="3" x2="12" y2="14" />
      <circle cx="12" cy="20" r="1.6" fill="currentColor" stroke="none" />
    </svg>
  );
}

function RedIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <path d="M12 1.5a6.5 6.5 0 0 0-6.5 6.5c0 2.2 1 3.7 2.3 4.6V15h8.4v-2.4c1.3-.9 2.3-2.4 2.3-4.6A6.5 6.5 0 0 0 12 1.5z" />
      <circle cx="9.4" cy="8" r="1.5" fill="currentColor" stroke="none" />
      <circle cx="14.6" cy="8" r="1.5" fill="currentColor" stroke="none" />
      <line x1="4" y1="17" x2="20" y2="22.5" />
      <line x1="20" y1="17" x2="4" y2="22.5" />
    </svg>
  );
}

export function StatusIcon({ color }: { color: Color }) {
  return (
    <span className="status-icon" aria-hidden="true">
      {color === "green" ? <GreenIcon /> : null}
      {color === "orange" ? <OrangeIcon /> : null}
      {color === "red" ? <RedIcon /> : null}
    </span>
  );
}
