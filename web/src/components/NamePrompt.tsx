// SPDX-License-Identifier: AGPL-3.0-or-later

import { useEffect, useRef } from "react";
import { MAX_VIEWER_NAME_CHARS } from "@/protocol/types";

export interface NamePromptProps {
  draft: string;
  error: string | null;
  canCancel: boolean;
  onDraftChange: (value: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}

/** Asks a viewer for its name in a session that collects viewer names. */
export function NamePrompt(props: Readonly<NamePromptProps>) {
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    inputRef.current?.focus();
  }, []);
  return (
    <div className="modal-backdrop">
      <dialog open={true} className="modal-dialog" aria-modal="true" aria-labelledby="name-title">
        <form
          className="control-card session-form"
          onSubmit={(event) => {
            event.preventDefault();
            props.onSubmit();
          }}
        >
          <h2 id="name-title" className="card-label">
            Your name
          </h2>
          <p className="muted-text">
            This session records who changes each table. Enter your name to continue.
          </p>
          <label htmlFor="viewer-name">
            <span>Name</span>
            <input
              ref={inputRef}
              id="viewer-name"
              type="text"
              autoComplete="name"
              maxLength={MAX_VIEWER_NAME_CHARS}
              value={props.draft}
              aria-invalid={props.error !== null}
              aria-describedby={props.error === null ? undefined : "viewer-name-error"}
              onChange={(event) => props.onDraftChange(event.target.value)}
            />
          </label>
          {props.error === null ? null : (
            <p id="viewer-name-error" className="modal-error" role="alert">
              {props.error}
            </p>
          )}
          <span className="inline-actions">
            <button type="submit">Continue</button>
            {props.canCancel ? (
              <button type="button" className="ghost-button" onClick={props.onCancel}>
                Cancel
              </button>
            ) : null}
          </span>
        </form>
      </dialog>
    </div>
  );
}
