// SPDX-License-Identifier: AGPL-3.0-or-later

import { ArrowLeft } from "lucide-react";
import { Link } from "react-router";
import type { ColorCounts } from "@/protocol/types";
import { StatusIcon } from "./StatusIcon";

const HEADER_ICON_SIZE = 58;
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
    <header className="session-header">
      <div className="session-nav-identity">
        <Link to="/" className="lobby-link">
          <span className="brand-mark brand-mark-nav">
            <img src="/icon.svg" width={HEADER_ICON_SIZE} height={HEADER_ICON_SIZE} alt="" />
          </span>
          <span className="lobby-link-label">
            <ArrowLeft size={BACK_ICON_SIZE} aria-hidden="true" />
            <span>Lobby</span>
          </span>
        </Link>
        <span className="session-nav-divider" aria-hidden="true" />
        <h1 className="session-title" title={title}>
          {title}
        </h1>
      </div>
      <div className="session-nav-metrics">
        <div className="counter orange">
          <span>
            <StatusIcon color="orange" /> Orange
          </span>
          <strong>{counts.orange}</strong>
        </div>
        <div className="counter red">
          <span>
            <StatusIcon color="red" /> Red
          </span>
          <strong>{counts.red}</strong>
        </div>
        <span className="status-chip" data-status={statusKind} aria-live="polite">
          {statusLabel}
        </span>
      </div>
    </header>
  );
}
