// SPDX-License-Identifier: AGPL-3.0-or-later

import type { ServerBuildInfo } from "@/protocol/types";

export const SOURCE_URL = "https://github.com/NB-TUDelft/trafficlights_webapp";
export const FALLBACK_VERSION = "0.1.0";
export const LICENSE_ID = "AGPL-3.0-or-later";

const SHORT_COMMIT_LENGTH = 7;
const COMMIT_START = 0;

function shortCommit(serverInfo: ServerBuildInfo | null | undefined): string | null {
  const commit = serverInfo?.commit ?? import.meta.env.VITE_BUILD_COMMIT;
  if (commit === undefined || commit === null || commit.length === 0) {
    return null;
  }
  return commit.slice(COMMIT_START, SHORT_COMMIT_LENGTH);
}

export function Footer({ serverInfo }: { serverInfo?: ServerBuildInfo | null }) {
  const version = serverInfo?.version ?? FALLBACK_VERSION;
  const commit = shortCommit(serverInfo);
  const rawSourceUrl = serverInfo?.source_url ?? SOURCE_URL;
  const sourceUrl = rawSourceUrl.startsWith("https://") ? rawSourceUrl : SOURCE_URL;
  return (
    <footer className="app-footer">
      <span>
        Classroom Traffic Lights {version}
        {commit === null ? "" : ` (${commit})`} · {LICENSE_ID} ·{" "}
        <a href={sourceUrl}>Corresponding Source</a>
      </span>
      <span>
        Hardware concept courtesy of RolfHut/ClassroomIndicatorLights, the original classroom
        indicator lights project.
      </span>
    </footer>
  );
}
