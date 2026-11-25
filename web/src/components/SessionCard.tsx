// SPDX-License-Identifier: AGPL-3.0-or-later

import { Link } from "react-router";
import { firmwareSummary } from "@/lib/firmware";
import type { SessionSummary } from "@/protocol/types";
import { StatusIcon } from "./StatusIcon";

export interface SessionCardProps {
  session: SessionSummary;
  isOwn: boolean;
}

export function SessionCard({ session, isOwn }: SessionCardProps) {
  const firmware = firmwareSummary(session.firmware);
  return (
    <article className="session-card">
      <h3>
        <Link to={`/s/${session.session_id}`}>{session.name}</Link>
        {isOwn ? <span className="badge badge-own">Your session</span> : null}
      </h3>
      <p className="muted-text">
        Tables {session.range.start}–{session.range.end}
      </p>
      <p className="session-counts">
        <span>
          <StatusIcon color="orange" /> {session.counts.orange} orange
        </span>
        <span>
          <StatusIcon color="red" /> {session.counts.red} red
        </span>
      </p>
      <p>
        <span className={session.receiver_connected ? "badge badge-online" : "badge badge-offline"}>
          {session.receiver_connected ? "Receiver online" : "Receiver disconnected"}
        </span>
        {firmware === null ? null : <span className="badge">{firmware}</span>}
      </p>
      <p className="muted-text">{session.viewer_count} viewers</p>
    </article>
  );
}
