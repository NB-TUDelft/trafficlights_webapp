// SPDX-License-Identifier: AGPL-3.0-or-later
// The session page and its hooks, rendered on the server: hook bodies run but
// effects do not (so no socket opens), and captured callbacks are invoked
// afterwards, when React state setters are no-ops and every observable effect
// goes through the fake arguments.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SessionPage } from "@/pages/SessionPage";
import { type HandlersArgs, useSessionHandlers } from "@/pages/sessionHooks";
import { useViewerName } from "@/pages/sessionStateHooks";
import type { SessionState } from "@/protocol/reducers";
import { LAST_VIEWER_NAME_KEY, VIEWER_NAMES_KEY } from "@/protocol/storage";
import type { SessionClientMsg } from "@/protocol/types";
import { baseState } from "./view-fixtures";

afterEach(() => {
  vi.unstubAllGlobals();
});

function makeHandlerArgs(state: SessionState, overrides?: Partial<HandlersArgs>) {
  const sent: SessionClientMsg[] = [];
  const calls: string[] = [];
  const errors: (string | null)[] = [];
  const args: HandlersArgs = {
    state,
    send: (msg) => {
      sent.push(msg);
      return true;
    },
    setTransientError: (value) => {
      errors.push(value);
    },
    setMutedTables: () => {
      calls.push("muted");
    },
    beepsEnabled: true,
    hostTapMode: "green",
    hostTapModeLocked: false,
    setLocalTapMode: () => {
      calls.push("localTap");
    },
    setHostTapMode: (value) => {
      calls.push(`hostTap:${value}`);
    },
    setHostTapModeLocked: (value) => {
      calls.push(`hostTapLock:${value}`);
    },
    hostBeepsEnabled: false,
    hostBeepsLocked: false,
    setLocalBeepsEnabled: () => {
      calls.push("localBeeps");
    },
    setHostBeepsEnabled: (value) => {
      calls.push(`hostBeeps:${value}`);
    },
    setHostBeepsLocked: (value) => {
      calls.push(`hostBeepsLock:${value}`);
    },
    rangeStart: "5",
    rangeEnd: "30",
    setRangeStart: (value) => {
      calls.push(`rangeStart:${value}`);
    },
    setRangeEnd: (value) => {
      calls.push(`rangeEnd:${value}`);
    },
    setRangeError: (value) => {
      calls.push(`rangeError:${value}`);
    },
    setRangeDialogOpen: (value) => {
      calls.push(`rangeOpen:${value}`);
    },
    renameValue: "Studio 4",
    sessionId: "sess-1",
    showAllPorts: false,
    connectToPort: async () => undefined,
    bridgeRef: { current: null },
    setConfirmEnd: (value) => {
      calls.push(`confirmEnd:${value}`);
    },
    ...overrides,
  };
  return { args, sent, calls, errors };
}

type Handlers = ReturnType<typeof useSessionHandlers>;
type ViewerName = ReturnType<typeof useViewerName>;

interface CaptureBox {
  handlers?: Handlers;
  viewer?: ViewerName;
}

function HandlersProbe(props: Readonly<{ args: HandlersArgs; box: CaptureBox }>) {
  props.box.handlers = useSessionHandlers(props.args);
  return null;
}

function ViewerNameProbe(
  props: Readonly<{
    sessionId: string;
    state: SessionState;
    sent: SessionClientMsg[];
    box: CaptureBox;
  }>,
) {
  props.box.viewer = useViewerName(props.sessionId, props.state, (msg) => {
    props.sent.push(msg);
    return true;
  });
  return null;
}

function captureHandlers(args: HandlersArgs): Handlers {
  const box: CaptureBox = {};
  renderToStaticMarkup(<HandlersProbe args={args} box={box} />);
  if (box.handlers === undefined) {
    throw new Error("probe did not render");
  }
  return box.handlers;
}

function captureViewerName(
  sessionId: string,
  state: SessionState,
  sent: SessionClientMsg[],
): ViewerName {
  const box: CaptureBox = {};
  renderToStaticMarkup(
    <ViewerNameProbe sessionId={sessionId} state={state} sent={sent} box={box} />,
  );
  if (box.viewer === undefined) {
    throw new Error("probe did not render");
  }
  return box.viewer;
}

function memoryStorage(initial: Record<string, string>) {
  const items = new Map(Object.entries(initial));
  return {
    items,
    getItem: (key: string) => items.get(key) ?? null,
    setItem: (key: string, value: string) => {
      items.set(key, value);
    },
    removeItem: (key: string) => {
      items.delete(key);
    },
  };
}

describe("session page", () => {
  it("renders the console before the first snapshot", () => {
    const html = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/s/sess-9"]}>
        <Routes>
          <Route path="/s/:id" element={<SessionPage />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(html).toContain("Reconnecting...");
    expect(html).toContain("Longest On Red");
    expect(html).not.toContain("Session Management");
    expect(html).not.toContain("range-title");
  });

  it("shows not found without a session id", () => {
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <SessionPage />
      </MemoryRouter>,
    );
    expect(html).toContain("Session not found");
  });
});

describe("session handlers", () => {
  it("only changes a table while the receiver is connected", () => {
    const offline = makeHandlerArgs(baseState({ receiverConnected: false }));
    captureHandlers(offline.args).onSetTable(1, "red");
    expect(offline.sent).toEqual([]);
    expect(offline.errors).toEqual(["Receiver is disconnected. Table was not changed."]);
    const online = makeHandlerArgs(baseState({ receiverConnected: true }));
    captureHandlers(online.args).onSetTable(1, "red");
    expect(online.sent).toEqual([{ type: "set_table", table: 1, color: "red" }]);
  });

  it("opens, applies and cancels the table-range popup", () => {
    const { args, sent, calls } = makeHandlerArgs(baseState({ isCreator: true }));
    const handlers = captureHandlers(args);
    handlers.onOpenRange();
    expect(calls).toEqual(["rangeStart:1", "rangeEnd:60", "rangeError:null", "rangeOpen:true"]);
    calls.length = 0;
    handlers.onApplyRange();
    expect(sent).toEqual([{ type: "set_table_range", start: 5, end: 30 }]);
    expect(calls).toEqual(["rangeOpen:false", "rangeError:null"]);
    calls.length = 0;
    handlers.onCancelRange();
    expect(calls).toEqual(["rangeOpen:false", "rangeError:null"]);
  });

  it("sends the creator's session actions and host modes", () => {
    const { args, sent, calls } = makeHandlerArgs(baseState({ isCreator: true }));
    const handlers = captureHandlers(args);
    handlers.onResetAll();
    handlers.onRename();
    handlers.onConfirmEnd();
    handlers.onToggleBeeps();
    handlers.onToggleBeepsLock();
    handlers.onToggleTapMode();
    handlers.onToggleTapModeLock();
    expect(sent).toEqual([
      { type: "reset_all" },
      { type: "rename_session", name: "Studio 4" },
      { type: "end_session" },
      { type: "set_beep_mode", enabled: true, locked: false },
      { type: "set_beep_mode", enabled: false, locked: true },
      { type: "set_tap_mode", tap_mode: "cycle", locked: false },
      { type: "set_tap_mode", tap_mode: "green", locked: true },
    ]);
    expect(calls).toContain("confirmEnd:false");
  });

  it("reports local failures instead of sending", async () => {
    const { args, sent, calls, errors } = makeHandlerArgs(baseState({ isCreator: true }), {
      renameValue: "x".repeat(81),
    });
    const handlers = captureHandlers(args);
    handlers.onRename();
    handlers.onDownloadLog();
    handlers.onDisconnectSerial();
    handlers.onToggleMute(3);
    handlers.onConnectSerial();
    await vi.waitFor(() => expect(errors).toContain("No receiver was selected."));
    expect(errors).toContain("Enter a session name of at most 80 characters.");
    expect(errors).toContain("Creator token is missing in this browser.");
    expect(calls).toContain("muted");
    expect(sent).toEqual([]);
  });
});

describe("viewer name hook", () => {
  const state = baseState({ collectNames: true, isCreator: false });

  it("asks for a name when none is remembered and sends nothing for an empty one", () => {
    const sent: SessionClientMsg[] = [];
    const viewer = captureViewerName("sess-1", state, sent);
    expect(viewer.name).toBeNull();
    expect(viewer.promptOpen).toBe(true);
    expect(viewer.canCancel).toBe(false);
    viewer.submit();
    viewer.change();
    viewer.cancel();
    expect(sent).toEqual([]);
  });

  it("sends a remembered name and saves it as the last one", () => {
    const storage = memoryStorage({ [VIEWER_NAMES_KEY]: JSON.stringify({ "sess-1": "Ada" }) });
    vi.stubGlobal("localStorage", storage);
    const sent: SessionClientMsg[] = [];
    const viewer = captureViewerName("sess-1", state, sent);
    expect(viewer.name).toBe("Ada");
    expect(viewer.draft).toBe("Ada");
    expect(viewer.promptOpen).toBe(false);
    viewer.submit();
    expect(sent).toEqual([expect.objectContaining({ type: "identify", name: "Ada" })]);
    expect(storage.items.get(LAST_VIEWER_NAME_KEY)).toBe("Ada");
  });
});
