// SPDX-License-Identifier: AGPL-3.0-or-later

import { ChevronRight, Users } from "lucide-react";
import { Link } from "react-router";
import { firmwareSummary } from "@/lib/firmware";
import type { SessionSummary } from "@/protocol/types";
import { StatusIcon } from "./StatusIcon";

const META_ICON_SIZE = 15;
const CHEVRON_ICON_SIZE = 20;

export interface SessionCardProps {
  session: SessionSummary;
  isOwn: boolean;
}

export function SessionCard({ session, isOwn }: Readonly<SessionCardProps>) {
  const firmware = firmwareSummary(session.firmware);
  const viewerLabel = session.viewer_count === 1 ? "1 viewer" : `${session.viewer_count} viewers`;
  return (
    <article className="session-card">
      <div className="session-card-main">
        <div className="session-card-title-row">
          <h3>
            <Link className="session-card-link" to={`/s/${session.session_id}`}>
              {session.name}
            </Link>
          </h3>
          {isOwn ? <span className="badge badge-own">Your session</span> : null}
        </div>
        <div className="session-card-meta">
          <span className="session-meta-item">
            Tables {session.range.start}–{session.range.end}
          </span>
          <span
            className={session.receiver_connected ? "badge badge-online" : "badge badge-offline"}
          >
            {session.receiver_connected ? "Receiver online" : "Receiver disconnected"}
          </span>
          {firmware === null ? null : <span className="badge">{firmware}</span>}
          <span className="session-meta-item">
            <Users size={META_ICON_SIZE} aria-hidden="true" />
            <span>{viewerLabel}</span>
          </span>
        </div>
      </div>
      <div className="session-card-side">
        <div className="session-stat-group">
          <span className="session-stat orange">
            <StatusIcon color="orange" />
            <span>{session.counts.orange} orange</span>
          </span>
          <span className="session-stat red">
            <StatusIcon color="red" />
            <span>{session.counts.red} red</span>
          </span>
        </div>
        <span className="session-card-chevron" aria-hidden="true">
          <ChevronRight size={CHEVRON_ICON_SIZE} />
        </span>
      </div>
    </article>
  );
}
