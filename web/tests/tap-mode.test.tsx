// SPDX-License-Identifier: AGPL-3.0-or-later
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { sessionStatusKind } from "@/components/SessionView";
import {
  applyMenuPosition,
  attachMenuDismiss,
  clampMenuPosition,
  type EventTargetLike,
  selectMenuColor,
  shouldDismissOnKey,
  shouldDismissOnPointer,
  TableColorMenu,
} from "@/components/TableColorMenu";
import { handleTileActivate, resolveContextAnchor } from "@/components/TableGrid";
import { ToggleSwitch } from "@/components/ToggleSwitch";
import { createLongPressController, LONG_PRESS_MS } from "@/lib/longPress";
import { HomeView } from "@/pages/HomePage";
import {
  type ModeHandlersArgs,
  resolveTapMode,
  toggleTapMode,
  toggleTapModeLock,
} from "@/pages/sessionModeHandlers";
import { initialSessionState } from "@/protocol/reducers";
import type { Color, SessionClientMsg, TapMode } from "@/protocol/types";

const noop = (): void => {
  // Test stub.
};

function makeHandlerArgs(overrides?: Partial<ModeHandlersArgs>) {
  const sent: SessionClientMsg[] = [];
  let localTapMode: TapMode = "green";
  let hostTapMode: TapMode = "green";
  let hostTapModeLocked = false;
  let localBeepsEnabled = false;
  let hostBeepsEnabled = false;
  let hostBeepsLocked = false;
  const args: ModeHandlersArgs = {
    state: initialSessionState,
    send: (msg) => {
      sent.push(msg);
      return true;
    },
    hostTapMode,
    hostTapModeLocked,
    setLocalTapMode: (updater) => {
      localTapMode = updater(localTapMode);
    },
    setHostTapMode: (value) => {
      hostTapMode = value;
    },
    setHostTapModeLocked: (value) => {
      hostTapModeLocked = value;
    },
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
  return { args, sent, read: () => ({ localTapMode, hostTapMode, hostTapModeLocked }) };
}

describe("table color context menu", () => {
  it("lists all three colors, marks active, and is focusable", () => {
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <TableColorMenu
          table={12}
          currentColor="orange"
          x={120}
          y={140}
          onSelect={noop}
          onClose={noop}
        />
      </MemoryRouter>,
    );
    expect(html).toContain('role="menu"');
    expect(html).toContain('tabindex="-1"');
    expect(html).toContain("Table 12");
    expect(html).toContain("Green (OK)");
    expect(html).toContain("Orange (Question)");
    expect(html).toContain("Red (Needs help)");
    expect(html).toContain('data-option="orange"');
    expect(html).toContain('aria-checked="true"');
  });

  it("clamps coordinates and applies menu CSSOM positioning and dismiss rules", () => {
    expect(clampMenuPosition(-20, -10, 800, 600)).toEqual({ left: 12, top: 12 });
    expect(clampMenuPosition(790, 590, 800, 600)).toEqual({ left: 584, top: 392 });
    const style = { left: "", top: "" };
    applyMenuPosition(style, { x: 790, y: 590 }, { width: 800, height: 600 });
    expect(style).toEqual({ left: "584px", top: "392px" });
    expect(shouldDismissOnPointer(() => true, "inside")).toBe(false);
    expect(shouldDismissOnPointer(() => false, "outside")).toBe(true);
    expect(shouldDismissOnKey("Escape")).toBe(true);
    expect(shouldDismissOnKey("Enter")).toBe(false);
    expect(resolveContextAnchor(42, 84, { left: 10, top: 20, width: 100, height: 50 })).toEqual({
      x: 42,
      y: 84,
    });
    expect(resolveContextAnchor(0, 0, { left: 10, top: 20, width: 100, height: 50 })).toEqual({
      x: 60,
      y: 45,
    });

    const listeners = new Map<string, (event: unknown) => void>();
    const fakeTarget: EventTargetLike = {
      addEventListener: (type, fn) => listeners.set(type, fn),
      removeEventListener: (type) => listeners.delete(type),
    };
    let closed = 0;
    const detach = attachMenuDismiss(
      fakeTarget,
      fakeTarget,
      (target) => target === "inside",
      () => {
        closed += 1;
      },
    );
    listeners.get("pointerdown")?.({ target: "inside" });
    listeners.get("pointerdown")?.(null);
    listeners.get("keydown")?.({ key: "Enter" });
    listeners.get("keydown")?.(null);
    expect(closed).toBe(1);
    listeners.get("pointerdown")?.({ target: "outside" });
    listeners.get("keydown")?.({ key: "Escape" });
    listeners.get("resize")?.({});
    expect(closed).toBe(4);
    detach();
    expect(listeners.size).toBe(0);

    const picks: [number, Color][] = [];
    let closedFromPick = 0;
    const recordPick = (t: number, c: Color) => {
      picks.push([t, c]);
    };
    selectMenuColor(7, "red", recordPick, () => {
      closedFromPick += 1;
    });
    expect(closedFromPick).toBe(1);
    handleTileActivate(() => true, recordPick, 8, "green");
    handleTileActivate(() => false, recordPick, 8, "green");
    expect(picks).toEqual([
      [7, "red"],
      [8, "green"],
    ]);
  });
});

describe("long-press controller", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("triggers after the hold duration and suppresses the trailing click", () => {
    vi.useFakeTimers();
    const opened: [number, number][] = [];
    const ctrl = createLongPressController((x, y) => opened.push([x, y]));
    ctrl.start({ button: 2, clientX: 50, clientY: 60 });
    vi.advanceTimersByTime(LONG_PRESS_MS + 50);
    expect(opened).toEqual([]);
    expect(ctrl.consumeSuppressedClick()).toBe(false);

    ctrl.start({ button: 0, clientX: 50, clientY: 60 });
    ctrl.move({ clientX: 54, clientY: 63 });
    vi.advanceTimersByTime(LONG_PRESS_MS);
    expect(opened).toEqual([[50, 60]]);
    expect(ctrl.consumeSuppressedClick()).toBe(true);
    expect(ctrl.consumeSuppressedClick()).toBe(false);
  });

  it("cancels on scroll movement or early release", () => {
    vi.useFakeTimers();
    const opened: [number, number][] = [];
    const ctrl = createLongPressController((x, y) => opened.push([x, y]));
    ctrl.move({ clientX: 100, clientY: 100 });
    ctrl.start({ button: 0, clientX: 50, clientY: 60 });
    ctrl.move({ clientX: 70, clientY: 60 });
    vi.advanceTimersByTime(LONG_PRESS_MS + 50);
    expect(opened).toEqual([]);

    ctrl.start({ button: 0, clientX: 50, clientY: 60 });
    ctrl.cancel();
    vi.advanceTimersByTime(LONG_PRESS_MS + 50);
    expect(opened).toEqual([]);
  });
});

describe("tap-mode handlers and switch rendering", () => {
  it("syncs host toggles and blocks locked viewers", () => {
    const host = makeHandlerArgs({
      state: { ...initialSessionState, isCreator: true },
      hostTapMode: "green",
      hostTapModeLocked: false,
    });
    toggleTapMode(host.args);
    expect(host.read().hostTapMode).toBe("cycle");
    expect(host.sent).toEqual([{ type: "set_tap_mode", tap_mode: "cycle", locked: false }]);

    const hostLock = makeHandlerArgs({
      state: { ...initialSessionState, isCreator: true },
      hostTapMode: "cycle",
      hostTapModeLocked: false,
    });
    toggleTapModeLock(hostLock.args);
    expect(hostLock.read().hostTapModeLocked).toBe(true);
    expect(hostLock.sent).toEqual([{ type: "set_tap_mode", tap_mode: "cycle", locked: true }]);

    const viewerUnlocked = makeHandlerArgs({
      state: { ...initialSessionState, isCreator: false, tapModeLocked: false },
    });
    toggleTapMode(viewerUnlocked.args);
    toggleTapModeLock(viewerUnlocked.args);
    expect(viewerUnlocked.read().localTapMode).toBe("cycle");
    expect(viewerUnlocked.sent).toEqual([]);

    const viewerLocked = makeHandlerArgs({
      state: { ...initialSessionState, isCreator: false, tapModeLocked: true },
    });
    toggleTapMode(viewerLocked.args);
    expect(viewerLocked.read().localTapMode).toBe("green");
    expect(viewerLocked.sent).toEqual([]);

    expect(
      resolveTapMode(
        { isCreator: true, tapMode: "green", tapModeLocked: false },
        { hostTapMode: "cycle", localTapMode: "green" },
      ),
    ).toBe("cycle");
    expect(
      resolveTapMode(
        { isCreator: false, tapMode: "cycle", tapModeLocked: true },
        { hostTapMode: "green", localTapMode: "green" },
      ),
    ).toBe("cycle");
    expect(
      resolveTapMode(
        { isCreator: false, tapMode: "cycle", tapModeLocked: false },
        { hostTapMode: "green", localTapMode: "green" },
      ),
    ).toBe("green");
  });

  it("renders switch checked and disabled states cleanly", () => {
    const enabled = renderToStaticMarkup(
      <ToggleSwitch
        id="tap-mode"
        label="Cycle on tap"
        hint="Tap resets to green"
        checked={false}
        onToggle={noop}
      />,
    );
    expect(enabled).toContain('role="switch"');
    expect(enabled).toContain('aria-checked="false"');
    expect(enabled).not.toContain("disabled");

    const locked = renderToStaticMarkup(
      <ToggleSwitch
        id="tap-mode-locked"
        label="Cycle on tap"
        hint="Locked by host"
        checked={true}
        disabled={true}
        onToggle={noop}
      />,
    );
    expect(locked).toContain('aria-checked="true"');
    expect(locked).toContain("disabled");
    expect(locked).toContain("Locked by host");

    expect(sessionStatusKind("open", true, false)).toBe("live");
    expect(sessionStatusKind("open", false, false)).toBe("disconnected");
    expect(sessionStatusKind("reconnecting", true, false)).toBe("reconnecting");
    expect(sessionStatusKind("open", true, true)).toBe("ended");

    const disconnectedHome = renderToStaticMarkup(
      <MemoryRouter>
        <HomeView
          state={{ sessions: [], serverInfo: null, created: null, error: null }}
          status="closed"
          ownSessionIds={new Set()}
        />
      </MemoryRouter>,
    );
    expect(disconnectedHome).toContain('data-status="disconnected"');
    expect(disconnectedHome).toContain("Disconnected");
  });
});
