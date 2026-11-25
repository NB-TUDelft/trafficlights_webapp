// SPDX-License-Identifier: AGPL-3.0-or-later

import { Link } from "react-router";
import type { ColorCounts } from "@/protocol/types";
import { StatusIcon } from "./StatusIcon";

const HEADER_ICON_SIZE = 40;

export interface SessionHeaderProps {
  name: string;
  counts: ColorCounts;
  statusLabel: string;
}

export function SessionHeader({ name, counts, statusLabel }: SessionHeaderProps) {
  return (
    <header className="session-header">
      <Link to="/" className="lobby-link">
        <img src="/icon.svg" width={HEADER_ICON_SIZE} height={HEADER_ICON_SIZE} alt="" />
        <span>Lobby</span>
      </Link>
      <h1>{name.length > 0 ? name : "Session"}</h1>
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
      <span className="status-chip" aria-live="polite">
        {statusLabel}
      </span>
    </header>
  );
}
