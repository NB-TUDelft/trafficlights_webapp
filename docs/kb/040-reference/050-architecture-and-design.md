---
summary: >-
  Locked-in architecture and design decisions (`2026-11-25` interview, updated
  as features land): Cargo workspace (`crates/proto`, `crates/wasm`,
  `crates/server`) + React/Vite/Nub frontend (`web/`), browser Rust/WASM Web
  Serial bridge with `!get` firmware probe, WebSocket-first protocol
  (`/ws/lobby`, `/ws/sessions/{id}`), creator capability token + optional
  `CREATE_SESSION_PIN`, optional per-session viewer name collection
  (`collect_names`) with `.jsonl` `actor` attribution, optional
  `CUSTOM_SCRIPT_SRC` head script injection + CSP origin extension, 5-minute
  disconnect grace period, out-of-range table notice, SSD-backed append-only
  JSONL statistics format (avoiding RAM growth and Turso's ~13.6 MiB RSS
  overhead), funny cartoon traffic-light SVG icon, and four-stage Coolify
  distroless Docker pipeline.
always_read: true
---

# Architecture and design

Settled with the maintainer during the `2026-11-25` design interview. Every
choice below traces back to Requirements (`R1`–`R15`) and the reference stack
in "Reference project and toolchain".

## Workspace and repository layout

```text
Cargo.toml                    # Rust workspace (edition 2021, rust-version 1.98)
rust-toolchain.toml           # channel = "stable", clippy, rustfmt
crates/
  proto/                      # Pure Rust protocol & domain crate (shared by server + wasm)
  wasm/                       # wasm-bindgen bridge compiled to wasm32-unknown-unknown
  server/                     # Axum 0.8 + Tokio HTTP/WS server + rust-embed (web/dist)
web/                          # React 19 + Vite + TypeScript + Tailwind CSS v4 (Nub, Biome, Vitest)
  public/icon.svg             # Custom funny cartoon traffic-light SVG icon
  src/wasm/                   # Generated wasm-bindgen JS/WASM bundle (gitignored, built by make/Docker)
old/
  fastapi_receiver.py         # Moved here as reference-only when scaffolding begins
Dockerfile                    # Four-stage build: wasm -> web -> builder -> distroless runner
compose.dev.yaml              # Local production-image harness (127.0.0.1:3000)
compose.prod.yaml             # Runtime contract for Coolify
.github/workflows/            # docker-publish.yml -> GHCR -> Coolify webhook
```

All crates declare `license = "AGPL-3.0-or-later"` and
`repository = "https://github.com/NB-TUDelft/trafficlights_webapp"`, start
every source file with `SPDX-License-Identifier: AGPL-3.0-or-later`, use
`#![forbid(unsafe_code)]`, `thiserror` error types, `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace`. The UI footer, `--version`, and Docker
`org.opencontainers.image.source` label point to
`https://github.com/NB-TUDelft/trafficlights_webapp` (AGPL Section 13; see
Licensing).

## Browser Rust/WASM Web Serial bridge (R6, R7, R8, R14)

- **`crates/proto` (`trafficlights-proto`)**: zero-I/O, shared by both
  `crates/server` and `crates/wasm`:
  - Domain types: `Color` (`Green=0`, `Orange=1`, `Red=2`), `TableId`,
    `TableRange`, `FirmwareMode`, `FirmwareInfo`, `SerialEvent`,
    `SerialCommand`.
  - Line decoder (`LineAccumulator`): handles `\r`, `\n`, `\r\n`, drops lines
    over 64 bytes (`MAX_LINE = 64`) or invalid UTF-8.
  - Legacy packet parser: parses `S,<id>,<color>` and `RS,<id>,<color>` into
    `StationStatus`, ignores `RT,<id>,<color>` echoes, parses `!ok ...` /
    `!err ...` replies from new firmware, and drops malformed lines safely.
  - Heartbeat deduplicator (in the browser WASM session instance): tracks the
    last reported color per station ID so unchanged 4–6 s radio heartbeats do
    not flood the WebSocket; forwards actual color changes immediately (and
    periodic heartbeat liveness / unseen tables as needed).
  - Strict command formatter: formats only validated `T,<table>,<color>\n`
    (`color` in `0..2`), `T,-1,0\n`, and `!get\n`—guaranteeing old MakeCode
    student lights (which index `colorList` unchecked) can never receive a
    malformed `T` packet.
- **`crates/wasm` (`trafficlights-wasm`)**: thin `wasm-bindgen` wrapper loaded
  **lazily** only on the Add Session / Creator route so normal viewer clients
  never download WASM bytes.
- **Web Serial lifecycle in the creator's browser**:
  - Requires a Secure Context (`https://` on Coolify or `http://localhost`) and
    a Chromium-based browser; detects missing `navigator.serial` with a clear
    message.
  - Filters `requestPort` to micro:bit `usbVendorId: 0x0D28` (with an option to
    show all ports if needed) at `115200` baud.
  - On connect: sends the legacy wake-up `\n`, then sends one `!get\n` probe
    (safe on old MakeCode lights, which ignore non-`T`/`RT`/`S` lines).
    - If `!ok ...` arrives within 1 s: records `FirmwareInfo` (`fw`, `mode`,
      `id`) and shows a prominent warning if `mode=station` was plugged in
      instead of `mode=receiver`.
    - If no `!ok` arrives within 1 s: automatically operates in **legacy
      MakeCode receiver** mode with zero error.
  - Paces outgoing writes to protect the micro:bit's ~128-byte UART RX buffer,
    requests Screen Wake Lock while streaming, and warns on tab unload.

## Client–server protocol (R9, R11, R12)

WebSocket-first architecture; HTTP serves only embedded SPA assets,
`GET /api/health` (Coolify distroless self-probe), and
`GET /api/sessions/{id}/log` (creator-authorized streaming `.jsonl` download).

- **WebSocket `Origin` validation:** when `PUBLIC_ORIGIN` is set (production),
  the server validates the HTTP `Origin` header on WebSocket upgrades to block
  cross-site WebSocket hijacking.
- **Optional head script (`CUSTOM_SCRIPT_SRC`):** when `CUSTOM_SCRIPT_SRC` is
  set to a validated absolute `http://` or `https://` URL (printable ASCII
  without `"`, `'`, `<`, `>`, `\`, or `` ` ``; plain DNS host or IPv4 with an
  optional 1–5 digit port and no user info; blank counts as unset; invalid
  values stop startup), the server injects an HTML-escaped
  `<script src="..." defer></script>` element immediately before `</head>` in
  served `index.html` responses and appends the script's lowercased origin
  (`scheme://host[:port]`) to `script-src`, `connect-src`, `img-src`, and
  `style-src` in the `Content-Security-Policy` header.
- **`WS /ws/lobby`:**
  - Server -> Client: `lobby_snapshot { sessions: [...] }` and incremental
    `lobby_update` events whenever a session is created, renamed, changes
    counts/receiver status, or ends.
  - Client -> Server: `create_session { name, start, end, pin?, collect_names? }`
    -> replies `session_created { session_id, creator_token }` (validates
    optional `CREATE_SESSION_PIN` env var when configured; optional
    `collect_names` defaults to `false` and is fixed for the session's
    lifetime).
- **`WS /ws/sessions/{id}`:**
  - Connects either as a **normal viewer** or as the **session creator** (by
    presenting `creator_token` in the initial `auth` message—never in the URL
    query string so tokens stay out of proxy logs and browser history).
  - **Server -> All clients:** `snapshot` (includes `collect_names`),
    `table_update`, `receiver_status`, `session_meta` (name, range, viewer
    count, host tap mode and lock flag, host beep mode and lock flag),
    `session_ended`, `error`.
  - **Server -> Creator only:** `serial_command` (`SetTable { table, color }`
    or `ResetAll`), `unlisted_tables` (station IDs heard outside `start..end`,
    such as unset `id=0`).
  - **Normal client -> Server:**
    - `identify { name, key? }`: required in `collect_names` sessions before
      `set_table` is accepted. Normalized by `normalize_viewer_name` (trimmed,
      1–40 Unicode scalar values, no ASCII control characters, and `"host"`
      reserved case-insensitively for the session creator). Names are kept per
      connection only, never broadcast, and must be unique within the session
      (case-insensitive), except a reconnecting tab presenting the current
      holder's matching random `key` (1–64 ASCII alphanumerics/hyphens) takes
      the name over from its stale connection. Rejected with
      `names_not_collected`, `invalid_viewer_name`, or `name_taken` without
      clearing an earlier valid name; the creator never needs `identify` and is
      always logged as `"host"`.
    - `set_table { table, color }`: validates the viewer has identified when
      `collect_names` is enabled (`name_required`), the table is within
      `start..end`, and the receiver is connected; forwards `serial_command` to
      the creator's browser and updates state only if the serial bridge is
      connected (fixing the baseline 503 state-mutation bug).
  - **Creator -> Server (server-verified `creator_token`):**
    `station_report { table, color }` (from WASM serial parser),
    `serial_status { connected, port_label?, firmware? }`, `reset_all`,
    `set_table_range { start, end }`, `rename_session { name }`,
    `set_tap_mode { tap_mode, locked }`, `set_beep_mode { enabled, locked }`,
    `end_session`.

## Role split, UI parity, and out-of-range handling (R1, R4, R5, R11, R12)

- **Homepage (`/`):** displays the funny traffic-light SVG icon, a short
  description of the classroom traffic lights system, the live list of active
  sessions (name, Orange/Red counts, receiver online/offline badge, viewer
  count), and a visible **"Add session"** button.
- **Add session (`/new`):** session name input, initial Table Range (`1`–`60`),
  optional PIN field (only shown when the server reports PIN-gated creation),
  optional **"Collect viewer names"** toggle switch (default off), and
  micro:bit Web Serial picker button; creates the session, saves
  `creator_token` in browser storage, and opens the creator console.
- **Session page (`/s/{id}`):**
  - Preserves the exact visual identity of `fastapi_receiver.py` (dark
    `#030712` / `#10182b` palette, green `#57d86b`, orange `#ffa534`, red
    `#ff4d4d`, cyan `#29b6f6`, shape-based SVG status icons, Web Audio 880/660
    Hz alert beeps, per-table mute, Longest On Red top-10 list).
  - Fixes all baseline defects: no broken duplicate HTML header, live
    client-side ticking red durations anchored to server UTC `red_since_ms`
    timestamps, no external Google Fonts dependency (self-hosted or clean
    system font stack), accurate mute button tooltips, and atomic error
    handling when the receiver is offline.
  - **Normal client view:** counters, live/receiver status chip, Alert beeps
    switch (defaulting to off; locked and grayed out when the host enables beep
    lock), Cycle-on-tap switch (defaulting to immediate reset-to-green on tap,
    with long-hold or right-click opening a color picker for all colors; locked
    and grayed out when the host enables tap-mode lock), Longest On Red, and
    clickable table grid strictly within `start..end` (with smart per-table
    mute icons that all show off while global beeps are off and preserve manual
    mutes when beeps are turned back on). In sessions with `collect_names`, a
    modal name prompt (`NamePrompt`, marking `<main>` `inert`) asks the viewer
    for a name before interacting—remembered per session in `localStorage`
    (`trafficlights:viewerNames`), suggested from `trafficlights:lastViewerName`,
    auto-resent on reconnect with a per-tab random key in `sessionStorage`
    (`trafficlights:viewerKey`), displaying server name errors inline, and
    showing `Your name` plus a **Change name** button in the info card. No
    serial controls, no Table Range, no Reset All To Green.
  - **Creator view** (automatically enabled when browser holds matching
    `creator_token` for `{id}`): adds Serial Connection panel (connect,
    reconnect, firmware badge, wrong-mode station warning), Table Range card,
    `Reset All To Green`, Alert-beeps + Lock-beeps and Cycle-on-tap +
    Lock-tap-mode switches in Quick Actions, session rename, subtle **"Unlisted
    tables heard"** banner (e.g. catching unset ID `0` or misnumbered
    stations), **Download statistics (.jsonl)** button (with a note when viewer
    names are collected and the creator is logged as `"host"`), and **End
    session** button.

## Session lifecycle and disconnect grace period

- Active sessions keep current table states in memory (<1 KB per 60-table
  session).
- If the creator's WebSocket or serial device disconnects, the session stays
  listed with status **"Receiver disconnected"** for `DISCONNECT_GRACE_SECONDS`
  (default `300` seconds = 5 minutes). If the creator refreshes or reconnects
  with their stored `creator_token`, streaming resumes seamlessly.
- Clicking **"End session"** (or grace-period expiry) closes the session,
  notifies viewers, finalizes the `.jsonl` log with a `session_end` record, and
  cleans up the temporary log file after a short download window (so the
  creator can still grab the final statistics file right after ending class).

## Classroom statistics format and RAM frugality (R15)

The production Coolify host currently runs at **88% RAM**. Rather than
buffering event arrays in RAM or embedding Turso/SQLite (measured at **+13.6
MiB RSS** and exclusive file locks in `services_zamkorus`), each session
streams transition events directly to an **append-only `.jsonl` file on
ephemeral SSD** (`LOG_DIR`, default `/tmp/trafficlights-logs/<session_id>.jsonl`,
flushed per event, streamed on download in O(1) RAM).

Every line is a self-contained JSON object (`schema_version: 1`) with UTC
millisecond timestamp `ts_ms` and elapsed session milliseconds `elapsed_ms`:

| `event` | Fields | Analytical use |
| :--- | :--- | :--- |
| `session_start` | `schema_version`, `session_id`, `name`, `ts_ms`, `range: {start, end}`, `collect_names?` | Session metadata header (`collect_names: true` written only when enabled) |
| `receiver_status` | `ts_ms`, `elapsed_ms`, `connected`, `firmware?: {kind, mode?, version?}` | Masks offline gaps in plots; tracks legacy vs. new firmware |
| `state_change` | `ts_ms`, `elapsed_ms`, `table`, `prev`, `next`, `source`, `in_range`, `actor?` | Reconstructs exact active color counts over time, per-table timelines, average state, wait/resolution times, student self-resolution vs. TA/teacher resolution (`source`: `student_radio`, `teacher_click`, `teacher_reset_all`, `range_reset`), and optional per-actor attribution (`actor`: viewer name or `"host"` for web-app changes in `collect_names` sessions) |
| `range_change` | `ts_ms`, `elapsed_ms`, `prev_range`, `next_range` | Explains denominator changes in average-state plots |
| `viewer_count` | `ts_ms`, `elapsed_ms`, `viewers` | Correlates TA/helper staffing with red-light wait times |
| `session_end` | `ts_ms`, `elapsed_ms`, `reason` (`manual` \| `grace_timeout` \| `shutdown`) | Clean end marker for duration calculations |

Because it is standard newline-delimited JSON, `pandas.read_json(path, lines=True)`,
DuckDB, R, or a future database importer can load it directly without custom
parsers.

## Funny traffic-light SVG icon (R13)

`web/public/icon.svg` (used as favicon, homepage logo, header badge, and PWA
icon): a cartoon rounded traffic-light body with a playful little radio antenna
on top (nod to the micro:bit radio) and three expressive faces inside the
lamps—**red** panicked/sweating ("HELP!"), **orange** puzzled/squinting
("hmm?"), and **green** chill happy smile—crafted with bold strokes so it stays
crisp and funny even at 16×16 favicon size. Rendered and visually inspected at
multiple sizes via Inkscape during creation.

## Build, container, and Coolify pipeline (R3, R10)

Mirrors `services_zamkorus` with one extra builder stage for the Rust/WASM
module:

1. **`wasm` stage (`rust:slim-trixie`)**: installs `wasm32-unknown-unknown` +
   `wasm-bindgen-cli`, compiles `crates/wasm` in release mode, and emits the
   JS/WASM bindings.
2. **`web` stage (`node:alpine` + `@nubjs/nub@latest`)**: copies `web/` and the
   built WASM output, runs `nub install --frozen-lockfile`, `nub run ci`,
   `nub run lint:strict`, `nub run test`, and `nub run build` (embedding
   `VITE_BUILD_COMMIT`).
3. **`builder` stage (`rust:slim-trixie`)**: copies `web/dist` for
   `rust-embed`, runs `cargo fmt --all --check`, `cargo clippy --release
   --locked --workspace --all-targets -- -D warnings`, `cargo test --jobs 2
   --release --locked --workspace`, and builds the release server binary
   (stamping build commit and timestamp via `option_env!`).
4. **`runner` stage (`gcr.io/distroless/cc-debian13:nonroot`, uid `65532`)**:
   contains only the single compiled binary (no shell, no runtime Node, no
   database lock), `BIND=0.0.0.0:3000`, writable `/tmp` for ephemeral session
   `.jsonl` logs, and `HEALTHCHECK CMD ["/trafficlights-webapp", "--healthcheck"]`.

Supported by `compose.dev.yaml`, `compose.prod.yaml` (both passing through
optional `CUSTOM_SCRIPT_SRC` for runtime head-script injection without
rebuilding the image), and `.github/workflows/docker-publish.yml` (push to GHCR
+ Coolify webhook with `force=true`). Because there is no SQLite/Turso
exclusive lock, Coolify does not require stop-then-start locking (though rolling
restarts drop in-memory sessions, so deployments happen outside active classes).
