---
summary: >-
  `2026-11-26`: commit `fe069e4` added optional per-session viewer name
  collection (`collect_names` + `identify` + `.jsonl` `actor` attribution) and
  optional `CUSTOM_SCRIPT_SRC` deferred head-script injection with matching CSP
  origin rules; documented in Reference ("Architecture and design" and
  "Classroom statistics JSONL schema").
---

# Viewer name collection and custom head script (`2026-11-26`)

Commit `fe069e4` landed two post-v1 features across `crates/proto`,
`crates/server`, `web/`, `compose.dev.yaml`, and `compose.prod.yaml`. The
standing contracts are documented in "Architecture and design" and "Classroom
statistics JSONL schema" under Reference.

## What was added

- **Optional viewer name collection (`collect_names`):**
  - **Protocol (`crates/proto`, `PROTO_VERSION = 1`):** added
    `MAX_VIEWER_NAME_CHARS = 40`, `HOST_ACTOR = "host"`,
    `ProtoError::InvalidViewerName`, `normalize_viewer_name`, optional
    `collect_names` on `LobbyClientMsg::CreateSession` and
    `SessionServerMsg::Snapshot`, `SessionClientMsg::Identify { name, key? }`,
    and backward-compatible optional `collect_names` (`session_start`) and
    `actor` (`state_change`) fields on `schema_version: 1` `.jsonl` events.
  - **Server (`crates/server`):** enforces viewer identification before
    `set_table` in `collect_names` sessions (`name_required`), case-insensitive
    uniqueness within a session (`name_taken`) with same-tab reconnect takeover
    via an optional `key` (1–64 ASCII alphanumerics/hyphens), reservation of
    `"host"` for the creator (`invalid_viewer_name`), and `names_not_collected`
    in anonymous sessions. Names stay connection-local (never broadcast) and
    are written as `actor` on web-app transitions (`teacher_click`,
    `teacher_reset_all`, `range_reset`; creator always logged as `"host"`).
  - **Frontend (`web/`):** added the **"Collect viewer names"** switch on
    `/new` (`NewSessionPage.tsx`), the modal `NamePrompt` dialog (`inert` on
    `<main>`), per-session and last-used name persistence in `localStorage`
    (`trafficlights:viewerNames`, `trafficlights:lastViewerName`), per-tab
    reconnect key in `sessionStorage` (`trafficlights:viewerKey`) auto-resent
    on reconnect, `Your name` + **Change name** in `AudioInfoCard`, inline name
    errors, and the creator note in `SessionManagementCard`.
- **Optional custom head script (`CUSTOM_SCRIPT_SRC`):**
  - **Config & compose (`crates/server/src/config.rs`, `compose.dev.yaml`,
    `compose.prod.yaml`):** added `CUSTOM_SCRIPT_SRC` (blank/unset disables;
    validated absolute `http://` or `https://` URL with plain DNS host or IPv4
    and optional 1–5 digit port, no quotes/angle brackets/backslashes/backticks
    or user info; invalid values fail startup).
  - **HTML & CSP (`crates/server/src/lib.rs`):** injects an HTML-escaped
    `<script src="..." defer></script>` before `</head>` in served `index.html`
    responses and appends the script's lowercased origin to `script-src`,
    `connect-src`, `img-src`, and `style-src` in `Content-Security-Policy`.
  - **Follow-up note:** `.env.example` does not list `CUSTOM_SCRIPT_SRC` yet.

## Verification

- `make kb` rewrote `docs/kb/README.md` (`7 groups, 13 always-read sections`).
- `make gate` passed: `scripts/kb.sh check`, WASM release build, Biome `ci` +
  `lint:strict` (63 files checked), TypeScript typecheck, Vitest (`85/85`
  passed across 13 test files), Vite build, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace` (`154/154` passed: 51 `proto`, 47 `server` unit, 5
  `cli`, 38 `integration`, 5 `smoke`, 8 `wasm`).
- `make kb-graph` refreshed `graphify-out/` with the updated knowledge base.
- Not re-run here: `make sonar`, Docker image build, or live micro:bit USB
  hardware verification.
