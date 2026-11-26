// SPDX-License-Identifier: AGPL-3.0-or-later
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AudioInfoCard } from "@/components/CreatorPanels";
import { isTableEffectivelyMuted, TableGrid, tableMuteLabel } from "@/components/TableGrid";
import {
  type ModeHandlersArgs,
  resolveBeepsEnabled,
  toggleBeeps,
  toggleBeepsLock,
  toggleMuted,
  toggleMuteWhenActive,
} from "@/pages/sessionModeHandlers";
import { initialSessionState } from "@/protocol/reducers";
import { DEFAULT_BEEPS_ENABLED, type SessionClientMsg, type TableEntry } from "@/protocol/types";

const noop = (): void => {
  // Test stub.
};

function makeModeArgs(overrides?: Partial<ModeHandlersArgs>) {
  const sent: SessionClientMsg[] = [];
  let localBeepsEnabled = false;
  let hostBeepsEnabled = false;
  let hostBeepsLocked = false;
  const args: ModeHandlersArgs = {
    state: initialSessionState,
    send: (msg) => {
      sent.push(msg);
      return true;
    },
    hostTapMode: "green",
    hostTapModeLocked: false,
    setLocalTapMode: noop,
    setHostTapMode: noop,
    setHostTapModeLocked: noop,
    hostBeepsEnabled,
    hostBeepsLocked,
    setLocalBeepsEnabled: (updater) => {
      localBeepsEnabled = updater(localBeepsEnabled);
    },
    setHostBeepsEnabled: (value) => {
      hostBeepsEnabled = value;
    },
    setHostBeepsLocked: (value) => {
      hostBeepsLocked = value;
    },
    ...overrides,
  };
  return { args, sent, read: () => ({ localBeepsEnabled, hostBeepsEnabled, hostBeepsLocked }) };
}

describe("beep defaults and host beep lock", () => {
  it("defaults beeps to off and unlocked", () => {
    expect(DEFAULT_BEEPS_ENABLED).toBe(false);
    expect(initialSessionState.beepsEnabled).toBe(false);
    expect(initialSessionState.beepsLocked).toBe(false);
  });

  it("syncs host beep toggles and blocks locked viewers", () => {
    const host = makeModeArgs({
      state: { ...initialSessionState, isCreator: true },
      hostBeepsEnabled: false,
      hostBeepsLocked: false,
    });
    toggleBeeps(host.args);
    expect(host.read().hostBeepsEnabled).toBe(true);
    expect(host.sent).toEqual([{ type: "set_beep_mode", enabled: true, locked: false }]);

    const hostLock = makeModeArgs({
      state: { ...initialSessionState, isCreator: true },
      hostBeepsEnabled: true,
      hostBeepsLocked: false,
    });
    toggleBeepsLock(hostLock.args);
    expect(hostLock.read().hostBeepsLocked).toBe(true);
    expect(hostLock.sent).toEqual([{ type: "set_beep_mode", enabled: true, locked: true }]);

    const viewerUnlocked = makeModeArgs({
      state: { ...initialSessionState, isCreator: false, beepsLocked: false },
    });
    toggleBeeps(viewerUnlocked.args);
    toggleBeepsLock(viewerUnlocked.args);
    expect(viewerUnlocked.read().localBeepsEnabled).toBe(true);
    expect(viewerUnlocked.sent).toEqual([]);

    const viewerLocked = makeModeArgs({
      state: { ...initialSessionState, isCreator: false, beepsLocked: true },
    });
    toggleBeeps(viewerLocked.args);
    expect(viewerLocked.read().localBeepsEnabled).toBe(false);
    expect(viewerLocked.sent).toEqual([]);

    expect(
      resolveBeepsEnabled(
        { isCreator: true, beepsEnabled: false, beepsLocked: false },
        { hostBeepsEnabled: true, localBeepsEnabled: false },
      ),
    ).toBe(true);
    expect(
      resolveBeepsEnabled(
        { isCreator: false, beepsEnabled: true, beepsLocked: true },
        { hostBeepsEnabled: false, localBeepsEnabled: false },
      ),
    ).toBe(true);
    expect(
      resolveBeepsEnabled(
        { isCreator: false, beepsEnabled: true, beepsLocked: false },
        { hostBeepsEnabled: false, localBeepsEnabled: false },
      ),
    ).toBe(false);
  });

  it("renders viewer beep switch locked and grayed out when locked by host", () => {
    const lockedOn = renderToStaticMarkup(
      <AudioInfoCard
        beepsEnabled={true}
        beepsLocked={true}
        viewerCount={2}
        tapMode="green"
        tapModeLocked={false}
        onToggleBeeps={noop}
        onToggleTapMode={noop}
      />,
    );
    expect(lockedOn).toContain('id="viewer-beeps"');
    expect(lockedOn).toContain('aria-checked="true"');
    expect(lockedOn).toContain("disabled");
    expect(lockedOn).toContain("Locked by host · Beeps On");

    const lockedOff = renderToStaticMarkup(
      <AudioInfoCard
        beepsEnabled={false}
        beepsLocked={true}
        viewerCount={2}
        tapMode="green"
        tapModeLocked={false}
        onToggleBeeps={noop}
        onToggleTapMode={noop}
      />,
    );
    expect(lockedOff).toContain('aria-checked="false"');
    expect(lockedOff).toContain("Locked by host · Beeps Off");
  });
});

describe("smart per-table beep icons", () => {
  const sampleTables: TableEntry[] = [
    { table: 1, color: "green" },
    { table: 2, color: "orange" },
  ];

  it("shows all icons off when global beeps are off and restores manual mutes when re-enabled", () => {
    expect(isTableEffectivelyMuted(false, false)).toBe(true);
    expect(isTableEffectivelyMuted(false, true)).toBe(true);
    expect(isTableEffectivelyMuted(true, false)).toBe(false);
    expect(isTableEffectivelyMuted(true, true)).toBe(true);
    expect(tableMuteLabel(3, false, false)).toBe("Beep off for table 3 (global beeps are off)");
    expect(tableMuteLabel(3, true, false)).toBe("Mute beep for table 3");
    expect(tableMuteLabel(3, true, true)).toBe("Unmute beep for table 3");

    let muted: ReadonlySet<number> = new Set();
    const updateMuted = (updater: (prev: ReadonlySet<number>) => ReadonlySet<number>) => {
      muted = updater(muted);
    };
    toggleMuteWhenActive(false, 1, updateMuted);
    expect(muted.size).toBe(0);
    toggleMuteWhenActive(true, 1, updateMuted);
    expect(muted.has(1)).toBe(true);
    expect(toggleMuted(muted, 1).has(1)).toBe(false);

    // With global beep OFF, both Table 1 (manually muted) and Table 2 (unmuted) show off + disabled.
    const globalOffHtml = renderToStaticMarkup(
      <TableGrid
        tables={sampleTables}
        beepsEnabled={false}
        mutedTables={muted}
        tapMode="green"
        onSetTable={noop}
        onToggleMute={noop}
      />,
    );
    expect(globalOffHtml).toContain("Beep off for table 1 (global beeps are off)");
    expect(globalOffHtml).toContain("Beep off for table 2 (global beeps are off)");
    expect(globalOffHtml).not.toContain("M15.54");

    // When global beep turns back ON, Table 1 stays muted by hand while Table 2 goes back on.
    const globalOnHtml = renderToStaticMarkup(
      <TableGrid
        tables={sampleTables}
        beepsEnabled={true}
        mutedTables={muted}
        tapMode="green"
        onSetTable={noop}
        onToggleMute={noop}
      />,
    );
    expect(globalOnHtml).toContain("Unmute beep for table 1");
    expect(globalOnHtml).toContain("Mute beep for table 2");
    expect(globalOnHtml).toContain("M15.54");
    expect(globalOnHtml).toContain('x1="23"');
  });
});
