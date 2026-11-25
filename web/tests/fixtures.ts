// SPDX-License-Identifier: AGPL-3.0-or-later
import type { ServerBuildInfo, SessionServerMsg, SessionSummary } from "@/protocol/types";

export const serverInfo: ServerBuildInfo = {
  version: "0.1.0",
  license: "AGPL-3.0-or-later",
  source_url: "https://github.com/NB-TUDelft/trafficlights_webapp",
  proto_version: 1,
  pin_required: false,
};

export const summary: SessionSummary = {
  session_id: "sess-1",
  name: "Studio 3B",
  range: { start: 1, end: 60 },
  counts: { green: 58, orange: 1, red: 1 },
  receiver_connected: true,
  viewer_count: 3,
  created_at_ms: 1_764_086_400_000,
};

export function snapshotMsg(): Extract<SessionServerMsg, { type: "snapshot" }> {
  return {
    type: "snapshot",
    session_id: "sess-1",
    name: "Studio 3B",
    range: { start: 1, end: 2 },
    tables: [
      { table: 1, color: "green" },
      { table: 2, color: "red", red_since_ms: 1_764_086_415_500 },
    ],
    counts: { green: 1, orange: 0, red: 1 },
    receiver_connected: false,
    firmware: { kind: "legacy" },
    viewer_count: 2,
    is_creator: false,
    unlisted_tables: [],
    server_now_ms: Date.now(),
    server_info: serverInfo,
  };
}
