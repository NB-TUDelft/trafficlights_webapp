// SPDX-License-Identifier: AGPL-3.0-or-later

import type { RedDuration } from "@/lib/time";
import { StatusIcon } from "./StatusIcon";

export function LongestOnRed({ entries }: Readonly<{ entries: readonly RedDuration[] }>) {
  return (
    <article className="control-card red-card">
      <h2 className="card-label">Longest On Red</h2>
      <div className="red-list-wrap">
        <div className="red-list-body">
          {entries.length === 0 ? (
            <p className="muted-text">All clear</p>
          ) : (
            <ol className="red-list">
              {entries.map((entry) => (
                <li key={entry.table}>
                  <StatusIcon color="red" />
                  <span>
                    Table {entry.table}: {entry.label}
                  </span>
                </li>
              ))}
            </ol>
          )}
        </div>
      </div>
    </article>
  );
}
