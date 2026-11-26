// SPDX-License-Identifier: AGPL-3.0-or-later

import { ArrowLeft } from "lucide-react";
import { Link } from "react-router";
import type { ColorCounts } from "@/protocol/types";
import { StatusIcon } from "./StatusIcon";

const BACK_ICON_SIZE = 16;

export type SessionStatusKind = "live" | "disconnected" | "reconnecting" | "ended";

export interface SessionHeaderProps {
  name: string;
  counts: ColorCounts;
  statusLabel: string;
  statusKind?: SessionStatusKind;
}

export function SessionHeader({
  name,
  counts,
  statusLabel,
  statusKind = "reconnecting",
}: Readonly<SessionHeaderProps>) {
  const title = name.length > 0 ? name : "Session";
  return (
    <header className="session-nav">
      <Link to="/" className="lobby-link">
        <span className="brand-mark">
          <img src="/icon.svg" alt="" />
        </span>
        <span className="lobby-link-label">
          <ArrowLeft size={BACK_ICON_SIZE} aria-hidden="true" />
          <span className="wide-only">Lobby</span>
        </span>
      </Link>
      <span className="session-nav-divider" aria-hidden="true" />
      {/* Title, counters and status chip: one row, or on phones the chip under the title. */}
      <div className="session-nav-main">
        <h1 className="session-title" title={title}>
          {title}
        </h1>
        <div className="session-nav-metrics">
          <div className="counter orange">
            <span>
              <StatusIcon color="orange" /> <span className="wide-only">Orange</span>
            </span>
            <strong>{counts.orange}</strong>
          </div>
          <div className="counter red">
            <span>
              <StatusIcon color="red" /> <span className="wide-only">Red</span>
            </span>
            <strong>{counts.red}</strong>
          </div>
        </div>
        <span className="status-chip" data-status={statusKind} aria-live="polite">
          <span className="status-chip-label">{statusLabel}</span>
        </span>
      </div>
    </header>
  );
}
