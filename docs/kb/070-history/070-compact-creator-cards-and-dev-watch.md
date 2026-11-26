---
summary: >-
  `2026-11-26`: the creator console's Table Range card became a Session
  Management button opening a `TableRangeDialog` popup, Quick Actions became
  two grid columns wide with its switches in a grid (creator card row about
  378 px -> 255 px tall at 1600 px), and `make dev` became a no-Docker watch
  mode (Vite on `:5173`, Rust server restart and WASM rebuild on `crates/`
  changes) while `make dev-up` runs the Docker Compose image build.
---

# Compact creator cards and dev watch mode (`2026-11-26`)

Two maintainer requests, quoted as given:

> please remove table range section and add button in session management
> instead make quick actins 2sections wide so it can be grided and all
> sections are less tall like my drawing button to change range should open
> popup.

> also can u make normal dev to be watch server autoupdate frontend for quick
> development and dev-up this whole docker build

The standing descriptions are in "Architecture and design" (Reference, creator
view) and "Documentation and Verification Rules" (Rules, local development).

## Creator console layout

- `TableRangeCard` was removed. Session Management now holds an inline rename
  form, **Download statistics (.jsonl)**, a **Table range `start`–`end`**
  button, and **End session**. The button opens `TableRangeDialog`
  (`web/src/components/TableRangeDialog.tsx`), whose open, apply and cancel
  logic lives in pure functions in `web/src/pages/tableRange.ts`. An invalid
  range keeps the popup open with an inline error instead of the page banner.
- The popup reuses the viewer name prompt's modal styling, renamed to the
  shared `modal-backdrop`, `modal-dialog` and `modal-error` classes, and the
  table color menu's Escape and outside-press listener, extracted as
  `attachOutsideDismiss` (the menu's `attachMenuDismiss` adds resize and
  scroll on top; the popup must not close on resize, which a phone keyboard
  causes). Focus goes to Start on open and returns to the opening button on
  close; the opener is read during the popup's first render, because the
  same commit makes `<main>` inert and the browser moves focus off it.
- Quick Actions spans two grid columns from 720 px and holds the viewer count
  in its header, `Reset All To Green`, and its four switches in
  `.switch-grid` (at most two columns, so each setting sits beside its lock).
  From 1080 px the four creator cards share one row, with Session Management
  1.35 columns wide so its buttons stay on one line. The grid rules moved
  from `web/src/index.css`, which is at Biome's 300-line strict limit, to
  `web/src/cards.css`.
- The unused `collectNames` prop of `SessionManagementCard` was removed after
  commit `cad7f96` dropped the note it fed.

## Dev workflow

- `make dev` (`scripts/dev.sh`) runs watch mode without Docker: Vite on
  `http://127.0.0.1:5173` updates the frontend on save, and a portable shell
  loop polls `crates/`, `Cargo.toml` and `Cargo.lock` against
  `target/dev-watch.stamp` each second, restarting the native server on
  `:3000` and first rebuilding WASM when `crates/proto` or `crates/wasm`
  changed. No watcher tool is installed on the dev machine (`cargo-watch`,
  `watchexec` and `bacon` were absent on `2026-11-26`), hence the poll. It
  warns when `PUBLIC_ORIGIN` would refuse the Vite origin.
- `make dev-up` (`scripts/dev.sh --up`) builds and runs `compose.dev.yaml`;
  `make dev-down` stops it. `make dev-hmr` was removed.

## Verification

- `make gate` (Vitest 15 files, 99 tests; 154 Rust tests) and `make sonar`
  (quality gate `OK`). The first `make sonar` run failed the gate on new-code
  coverage (76.7% against 85%; the new-code period since `2026-11-25` also
  holds the unscanned viewer-names commit `b4f2b51`). New tests brought it to
  88.7%: `web/tests/table-range.test.tsx` (popup handlers, rendering,
  dismissal) and `web/tests/session-page.test.tsx`, which server-renders
  `SessionPage` (hook bodies run, effects and so the socket do not) and
  invokes the callbacks captured from `useSessionHandlers` and
  `useViewerName` against fake arguments. No DOM test library is installed,
  so hook effects remain untested by unit tests. The four new-code Sonar
  issues left are minor smells in files this work did not touch
  (`NewSessionPage.tsx`, `Banners.tsx`, `serial-bridge.test.ts`).
- Live `make dev` with headless Brave over DevTools Protocol at 1600, 900 and
  390 px wide: creator cards 255 px tall in one row at 1600 px, no horizontal
  overflow at 390 px, the popup's focus, inline error, Escape, outside-press
  close and focus return, applying `1–20` relabeling the button and showing
  20 tiles, and no console errors. Touching `crates/proto/src/lib.rs`
  rebuilt WASM and restarted the server in about 4 s.
- Not tested: a real micro:bit over USB serial (no serial code changed), and
  `make dev-up` itself (its Docker command is unchanged; only the target name
  moved).
