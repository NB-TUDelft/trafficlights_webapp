// SPDX-License-Identifier: AGPL-3.0-or-later
// Human-readable firmware labels shared by lobby and session views.

import type { FirmwareInfo } from "@/protocol/types";

export function firmwareSummary(info: FirmwareInfo | null | undefined): string | null {
  if (info === null || info === undefined) {
    return null;
  }
  if (info.kind === "legacy") {
    return "Legacy MakeCode receiver";
  }
  if (info.version !== undefined && info.version.length > 0) {
    return `New firmware (${info.version})`;
  }
  return "New firmware";
}

export function firmwareBadgeLabel(
  connected: boolean,
  info: FirmwareInfo | null | undefined,
): string {
  if (!connected) {
    return "Disconnected";
  }
  const summary = firmwareSummary(info);
  return summary ?? "Probing...";
}

export function endReasonLabel(reason: string): string {
  if (reason === "manual") {
    return "Manual";
  }
  if (reason === "grace_timeout") {
    return "Grace timeout";
  }
  return "Server shutdown";
}
