// SPDX-License-Identifier: AGPL-3.0-or-later
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { Footer } from "@/components/Footer";
import { SessionView, type SessionViewProps } from "@/components/SessionView";
import { HomeView } from "@/pages/HomePage";
import { NewSessionView } from "@/pages/NewSessionPage";
import { initialSessionState, type SessionState } from "@/protocol/reducers";
import type { ServerBuildInfo, SessionSummary } from "@/protocol/types";

const serverInfo: ServerBuildInfo = {
  version: "1.2.3",
  commit: "abcdef123456",
  license: "AGPL-3.0-or-later",
  source_url: "https://github.com/NB-TUDelft/trafficlights_webapp",
  proto_version: 1,
  pin_required: false,
};

const summary: SessionSummary = {
  session_id: "sess-1",
  name: "Studio 3B",
  range: { start: 1, end: 60 },
  counts: { green: 58, orange: 1, red: 1 },
  receiver_connected: true,
  firmware: { kind: "new", mode: "receiver", version: "draft-1" },
  viewer_count: 3,
  created_at_ms: 1_764_086_400_000,
};

const noop = (): void => {
  // Test stub.
};

function render(node: React.ReactNode): string {
  return renderToStaticMarkup(<MemoryRouter>{node}</MemoryRouter>);
}

function baseState(overrides: Partial<SessionState>): SessionState {
  return {
    ...initialSessionState,
    sessionId: "sess-1",
    name: "Studio 3B",
    tables: [
      { table: 1, color: "green" },
      { table: 2, color: "red", red_since_ms: 1000 },
    ],
    counts: { green: 1, orange: 0, red: 1 },
    receiverConnected: true,
    viewerCount: 2,
    serverInfo,
    hasSnapshot: true,
    ...overrides,
  };
}

function viewProps(state: SessionState, overrides?: Partial<SessionViewProps>): SessionViewProps {
  return {
    state,
    socketStatus: "open",
    nowMs: 61_000,
    beepsEnabled: state.beepsEnabled,
    beepsLocked: state.beepsLocked,
    mutedTables: new Set([1]),
    tapMode: state.tapMode,
    tapModeLocked: state.tapModeLocked,
    transientError: null,
    serialConnected: true,
    serialFirmware: null,
    serialPortLabel: "micro:bit USB (0D28:0204)",
    serialSupported: true,
    serialBusy: false,
    showAllPorts: false,
    rangeStart: "1",
    rangeEnd: "2",
    renameValue: "Studio 3B",
    confirmEnd: false,
    onDismissError: noop,
    onSetTable: noop,
    onToggleMute: noop,
    onToggleBeeps: noop,
    onToggleBeepsLock: noop,
    onToggleTapMode: noop,
    onToggleTapModeLock: noop,
    onConnectSerial: noop,
    onDisconnectSerial: noop,
    onShowAllChange: noop,
    onRangeStartChange: noop,
    onRangeEndChange: noop,
    onApplyRange: noop,
    onResetAll: noop,
    onRenameChange: noop,
    onRename: noop,
    onDownloadLog: noop,
    onAskEnd: noop,
    onConfirmEnd: noop,
    onCancelEnd: noop,
    ...overrides,
  };
}

describe("homepage rendering", () => {
  it("renders icon, title, description, Add session link, and sessions", () => {
    const html = render(
      <HomeView
        state={{ sessions: [summary], serverInfo, created: null, error: null }}
        status="open"
        ownSessionIds={new Set(["sess-1"])}
      />,
    );
    expect(html).toContain("/icon.svg");
    expect(html).toContain("Classroom Traffic Lights");
    expect(html).toContain("micro:bit USB receiver");
    expect(html).toContain("Add session");
    expect(html).toContain('href="/new"');
    expect(html).toContain('href="/s/sess-1"');
    expect(html).toContain("session-card-link");
    expect(html).toContain('data-status="live"');
    expect(html).toContain("Studio 3B");
    expect(html).toContain("Your session");
    expect(html).toContain("Tables 1");
    expect(html).toContain("Receiver online");
    expect(html).toContain("New firmware (draft-1)");
  });

  it("renders an empty state with no sessions", () => {
    const html = render(
      <HomeView
        state={{ sessions: [], serverInfo, created: null, error: null }}
        status="open"
        ownSessionIds={new Set()}
      />,
    );
    expect(html).toContain("No active sessions");
  });
});

describe("add session rendering", () => {
  function newSessionHtml(pinRequired: boolean): string {
    return render(
      <NewSessionView
        serverInfo={{ ...serverInfo, pin_required: pinRequired }}
        name=""
        startValue="1"
        endValue="60"
        pin=""
        showAllPorts={false}
        serialSupported={true}
        hasPendingPort={false}
        error={null}
        submitting={false}
        onNameChange={noop}
        onStartChange={noop}
        onEndChange={noop}
        onPinChange={noop}
        onShowAllChange={noop}
        onSelectPort={noop}
        onSubmit={noop}
      />,
    );
  }

  it("shows the PIN input only when pin_required is true", () => {
    expect(newSessionHtml(false)).not.toContain('type="password"');
    expect(newSessionHtml(true)).toContain('type="password"');
    expect(newSessionHtml(true)).toContain("Session PIN");
    expect(newSessionHtml(false)).toContain("Select micro:bit receiver");
    expect(newSessionHtml(false)).toContain("Show all USB serial ports");
  });
});

describe("session view role split", () => {
  it("hides every creator control for normal clients", () => {
    const html = render(<SessionView {...viewProps(baseState({ isCreator: false }))} />);
    expect(html).toContain("Alert beeps");
    expect(html).toContain("Beeps Off");
    expect(html).toContain("Longest On Red");
    expect(html).toContain("red-list-wrap");
    expect(html).not.toContain("control-grid-creator");
    expect(html).toContain("Cycle on tap");
    expect(html).toContain("Tap resets to green");
    expect(html).toContain("Activate to set green. Hold or right-click for all colors.");
    expect(html).not.toContain("Lock beeps");
    expect(html).not.toContain("Lock tap mode");
    expect(html).not.toContain("Reset All To Green");
    expect(html).not.toContain("Table Range");
    expect(html).not.toContain("Serial Connection");
    expect(html).not.toContain("End session");
    expect(html).not.toContain("Download statistics");
    expect(html).not.toContain("Unlisted tables");
  });

  it("grays out the viewer tap-mode switch when locked by the host", () => {
    const state = baseState({ isCreator: false, tapMode: "cycle", tapModeLocked: true });
    const html = render(<SessionView {...viewProps(state)} />);
    expect(html).toContain('id="viewer-tap-mode"');
    expect(html).toContain('role="switch"');
    expect(html).toContain('aria-checked="true"');
    expect(html).toContain("disabled");
    expect(html).toContain("Locked by host");
    expect(html).toContain("Activate to set orange.");
  });

  it("shows creator controls, station warning, unlisted Table 0, and mute labels", () => {
    const state = baseState({
      isCreator: true,
      beepsEnabled: true,
      firmware: { kind: "new", mode: "station", version: "draft-1", station_id: 7 },
      unlistedTables: [{ table: 0, color: "red", last_seen_ms: 99 }],
    });
    const html = render(<SessionView {...viewProps(state)} />);
    expect(html).toContain("control-grid-creator");
    expect(html).toContain("session-nav-identity");
    expect(html).toContain("Alert beeps");
    expect(html).toContain("Beeps On");
    expect(html).toContain("Lock beeps");
    expect(html).toContain("Cycle on tap");
    expect(html).toContain("Lock tap mode");
    expect(html).toContain("Reset All To Green");
    expect(html).toContain("Table Range");
    expect(html).toContain("Serial Connection");
    expect(html).toContain("End session");
    expect(html).toContain("Download statistics (.jsonl)");
    expect(html).toContain("Student Station");
    expect(html).toContain("Teacher Receiver");
    expect(html).toContain("station 7");
    expect(html).toContain("Unlisted tables heard");
    expect(html).toContain("Table 0 (unset ID)");
    expect(html).toContain("Unmute beep for table 1");
    expect(html).toContain("Mute beep for table 2");
    expect(html).toContain("Table 2: 1m 00s");
  });

  it("renders the ended banner with creator download kept available", () => {
    const state = baseState({
      isCreator: true,
      ended: { reason: "grace_timeout", logAvailable: true },
    });
    const html = render(<SessionView {...viewProps(state)} />);
    expect(html).toContain("Session ended");
    expect(html).toContain("Grace timeout");
    expect(html).toContain("Download statistics (.jsonl)");
    expect(html).toContain('href="/"');
  });
});

describe("protocol mismatch and accessibility attributes", () => {
  it("warns on the homepage when the server protocol differs", () => {
    const html = render(
      <HomeView
        state={{
          sessions: [],
          serverInfo: { ...serverInfo, proto_version: 99 },
          created: null,
          error: null,
        }}
        status="open"
        ownSessionIds={new Set()}
      />,
    );
    expect(html).toContain("out of date for the server");
    expect(html).toContain("reload the page");
    expect(html).toContain('alt=""');
  });

  it("warns in the session view and exposes live and pressed states", () => {
    const state = baseState({
      isCreator: true,
      serverInfo: { ...serverInfo, proto_version: 99 },
      unlistedTables: [{ table: 0, color: "red", last_seen_ms: 99 }],
    });
    const html = render(<SessionView {...viewProps(state)} />);
    expect(html).toContain("out of date for the server");
    expect(html).toContain('aria-live="polite"');
    expect(html).toContain('aria-pressed="true"');
    expect(html).toContain('role="status"');
    expect(html).toContain('alt=""');
  });
});

describe("footer rendering", () => {
  it("renders version, short commit, and the AGPL source link", () => {
    const html = render(<Footer serverInfo={serverInfo} />);
    expect(html).toContain("Classroom Traffic Lights 1.2.3");
    expect(html).toContain("abcdef1");
    expect(html).toContain("AGPL-3.0-or-later");
    expect(html).toContain("https://github.com/NB-TUDelft/trafficlights_webapp");
    expect(html).toContain("Corresponding Source");
    expect(html).toContain("RolfHut/ClassroomIndicatorLights");
  });
});
