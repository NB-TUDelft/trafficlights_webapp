// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { SessionView } from "@/components/SessionView";
import { attachOutsideDismiss, type EventTargetLike } from "@/components/TableColorMenu";
import {
  applyRangeDialog,
  closeRangeDialog,
  INVALID_RANGE_MESSAGE,
  openRangeDialog,
  type RangeDialogArgs,
} from "@/pages/tableRange";
import type { SessionClientMsg } from "@/protocol/types";
import { baseState, render, viewProps } from "./view-fixtures";

function makeRangeArgs(rangeStart: string, rangeEnd: string) {
  const sent: SessionClientMsg[] = [];
  const ui = {
    rangeStart,
    rangeEnd,
    rangeError: "stale error" as string | null,
    rangeDialogOpen: true,
  };
  const args: RangeDialogArgs = {
    range: { start: 1, end: 60 },
    rangeStart,
    rangeEnd,
    send: (msg) => {
      sent.push(msg);
      return true;
    },
    setRangeStart: (value) => {
      ui.rangeStart = value;
    },
    setRangeEnd: (value) => {
      ui.rangeEnd = value;
    },
    setRangeError: (value) => {
      ui.rangeError = value;
    },
    setRangeDialogOpen: (value) => {
      ui.rangeDialogOpen = value;
    },
  };
  return { args, sent, ui };
}

describe("table range popup handlers", () => {
  it("opens with the inputs reset to the current range", () => {
    const { args, ui } = makeRangeArgs("7", "9");
    ui.rangeDialogOpen = false;
    openRangeDialog(args);
    expect(ui).toEqual({
      rangeStart: "1",
      rangeEnd: "60",
      rangeError: null,
      rangeDialogOpen: true,
    });
  });

  it("sends a valid range and closes", () => {
    const { args, sent, ui } = makeRangeArgs("5", "30");
    applyRangeDialog(args);
    expect(sent).toEqual([{ type: "set_table_range", start: 5, end: 30 }]);
    expect(ui.rangeDialogOpen).toBe(false);
    expect(ui.rangeError).toBeNull();
  });

  it("keeps the popup open with an error for an invalid range", () => {
    for (const [start, end] of [
      ["30", "5"],
      ["0", "5"],
      ["1", "10000"],
      ["", "5"],
    ]) {
      const { args, sent, ui } = makeRangeArgs(start, end);
      applyRangeDialog(args);
      expect(sent).toEqual([]);
      expect(ui.rangeDialogOpen).toBe(true);
      expect(ui.rangeError).toBe(INVALID_RANGE_MESSAGE);
    }
  });

  it("closes and clears the error on cancel", () => {
    const { args, sent, ui } = makeRangeArgs("5", "30");
    closeRangeDialog(args);
    expect(sent).toEqual([]);
    expect(ui.rangeDialogOpen).toBe(false);
    expect(ui.rangeError).toBeNull();
  });
});

describe("table range popup dismissal", () => {
  it("closes on Escape or an outside press, but not on resize or scroll", () => {
    const listeners = new Map<string, (event: unknown) => void>();
    const target: EventTargetLike = {
      addEventListener: (type, listener) => {
        listeners.set(type, listener);
      },
      removeEventListener: (type) => {
        listeners.delete(type);
      },
    };
    let closed = 0;
    const detach = attachOutsideDismiss(
      target,
      (eventTarget) => eventTarget === "inside",
      () => {
        closed += 1;
      },
    );
    expect([...listeners.keys()].sort()).toEqual(["keydown", "pointerdown"]);
    listeners.get("pointerdown")?.({ target: "inside" });
    listeners.get("keydown")?.({ key: "Enter" });
    expect(closed).toBe(0);
    listeners.get("pointerdown")?.({ target: "outside" });
    listeners.get("keydown")?.({ key: "Escape" });
    expect(closed).toBe(2);
    detach();
    expect(listeners.size).toBe(0);
  });
});

describe("table range popup rendering", () => {
  it("renders the creator's popup over an inert page", () => {
    const state = baseState({ isCreator: true });
    const html = render(
      <SessionView
        {...viewProps(state, {
          rangeDialogOpen: true,
          rangeStart: "5",
          rangeEnd: "30",
          rangeError: INVALID_RANGE_MESSAGE,
        })}
      />,
    );
    expect(html).toContain('<main class="page" inert=""');
    expect(html).toContain('<dialog open="" class="modal-dialog"');
    expect(html).toContain('aria-labelledby="range-title"');
    expect(html).toContain('id="range-start"');
    expect(html).toContain('value="5"');
    expect(html).toContain('value="30"');
    expect(html).toContain("Apply range");
    expect(html).toContain('role="alert"');
    expect(html).toContain("Enter a valid table range");
    expect(html).toContain('aria-describedby="range-error"');
  });

  it("renders no popup while closed or for a normal client", () => {
    const closed = render(<SessionView {...viewProps(baseState({ isCreator: true }))} />);
    expect(closed).not.toContain("range-title");
    expect(closed).not.toContain('inert=""');
    const viewer = render(
      <SessionView {...viewProps(baseState({ isCreator: false }), { rangeDialogOpen: true })} />,
    );
    expect(viewer).not.toContain("range-title");
    expect(viewer).not.toContain('inert=""');
  });
});
