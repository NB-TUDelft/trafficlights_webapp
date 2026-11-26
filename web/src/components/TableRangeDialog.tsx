// SPDX-License-Identifier: AGPL-3.0-or-later

import { useEffect, useRef, useState } from "react";
import { attachOutsideDismiss } from "@/components/TableColorMenu";
import { MAX_TABLE_ID, MIN_TABLE_ID } from "@/protocol/types";

export interface TableRangeDialogProps {
  startValue: string;
  endValue: string;
  error: string | null;
  onStartChange: (value: string) => void;
  onEndChange: (value: string) => void;
  onApply: () => void;
  onCancel: () => void;
}

/**
 * Closes on Escape or a press outside the popup, focuses the Start input on
 * open, and gives focus back to the button that opened it.
 */
function useDialogFocusAndDismiss(
  formRef: { current: HTMLFormElement | null },
  startRef: { current: HTMLInputElement | null },
  onCancel: () => void,
): void {
  // Read during the first render: once this commit makes `<main>` inert, the
  // browser has already moved focus off the button that opened the popup.
  const [opener] = useState(() =>
    typeof document !== "undefined" && document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );
  useEffect(() => {
    startRef.current?.focus();
    startRef.current?.select();
    return () => opener?.focus();
  }, [opener, startRef]);
  useEffect(
    () =>
      attachOutsideDismiss(
        document,
        (target) => target instanceof Node && formRef.current?.contains(target) === true,
        onCancel,
      ),
    [formRef, onCancel],
  );
}

/** The creator's popup for changing which tables the session shows. */
export function TableRangeDialog(props: Readonly<TableRangeDialogProps>) {
  const formRef = useRef<HTMLFormElement>(null);
  const startRef = useRef<HTMLInputElement>(null);
  useDialogFocusAndDismiss(formRef, startRef, props.onCancel);
  return (
    <div className="modal-backdrop">
      <dialog open={true} className="modal-dialog" aria-modal="true" aria-labelledby="range-title">
        <form
          ref={formRef}
          className="control-card session-form"
          onSubmit={(event) => {
            event.preventDefault();
            props.onApply();
          }}
        >
          <h2 id="range-title" className="card-label">
            Table range
          </h2>
          <p className="muted-text">
            Tables {MIN_TABLE_ID}–{MAX_TABLE_ID}. Applying a new range resets every lit table to
            green.
          </p>
          <div className="range-inputs">
            <label htmlFor="range-start">
              <span>Start</span>
              <input
                ref={startRef}
                id="range-start"
                type="number"
                min={MIN_TABLE_ID}
                max={MAX_TABLE_ID}
                value={props.startValue}
                aria-invalid={props.error !== null}
                aria-describedby={props.error === null ? undefined : "range-error"}
                onChange={(event) => props.onStartChange(event.target.value)}
              />
            </label>
            <label htmlFor="range-end">
              <span>End</span>
              <input
                id="range-end"
                type="number"
                min={MIN_TABLE_ID}
                max={MAX_TABLE_ID}
                value={props.endValue}
                aria-invalid={props.error !== null}
                aria-describedby={props.error === null ? undefined : "range-error"}
                onChange={(event) => props.onEndChange(event.target.value)}
              />
            </label>
          </div>
          {props.error === null ? null : (
            <p id="range-error" className="modal-error" role="alert">
              {props.error}
            </p>
          )}
          <span className="inline-actions">
            <button type="submit">Apply range</button>
            <button type="button" className="ghost-button" onClick={props.onCancel}>
              Cancel
            </button>
          </span>
        </form>
      </dialog>
    </div>
  );
}
