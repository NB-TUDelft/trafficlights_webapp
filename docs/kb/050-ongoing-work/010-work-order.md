---
summary: >-
  The work order (`2026-11-25`): requirements (R1–R15) and architecture
  decisions are settled; ready to implement in verified slices starting with
  workspace/web scaffolding (moving `fastapi_receiver.py` to `old/`), then
  `crates/proto` + `crates/wasm`, then the Axum WebSocket server + SSD JSONL
  logger, then the React frontend + funny SVG icon, then Coolify container and
  hardware verification.
always_read: true
---

# Work order

Take these in order. When an item is done, delete it here and, if it was more
than a fix, record it at the end of History.

1. **Scaffold the Rust workspace, `web/` frontend, and quality gate**:
   - `git mv fastapi_receiver.py old/fastapi_receiver.py` (reference-only).
   - Create `Cargo.toml` workspace (`crates/proto`, `crates/wasm`,
     `crates/server`, `license = "AGPL-3.0-or-later"`), `rust-toolchain.toml`,
     and `web/` (React 19 + Vite + TypeScript + Tailwind CSS v4 + Nub + Biome +
     Vitest, `"license": "AGPL-3.0-or-later"`, matching `services_zamkorus`),
     with `SPDX-License-Identifier: AGPL-3.0-or-later` headers on all project
     source files.
   - Install `wasm32-unknown-unknown` target and `wasm-bindgen-cli` locally if
     not yet present; wire `wasm` build, `cargo fmt`, `cargo clippy -D
     warnings`, `cargo test`, `nub run ci`, `nub run lint:strict`, `nub run
     test`, and `scripts/kb.sh check` into `make gate`.
2. **Implement `crates/proto` and `crates/wasm` (TDD)**:
   - Shared domain types, line framing (`\r`, `\n`, `\r\n`, `MAX_LINE = 64`),
     legacy `S`/`RS`/`RT` parsing, strict `T,<table>,<color>\n` and `T,-1,0\n`
     formatting, `!get` probe & `!ok`/`!err` parsing, and heartbeat
     deduplication.
   - `crates/wasm` `wasm-bindgen` bindings and unit tests (Rust host tests +
     Vitest wrapper tests).
3. **Implement `crates/server` (Axum WebSocket server + SSD JSONL statistics)**:
   - In-memory session registry, capability token creation, optional
     `CREATE_SESSION_PIN`, `PUBLIC_ORIGIN` WebSocket Origin validation,
     5-minute configurable `DISCONNECT_GRACE_SECONDS` cleanup task, and
     out-of-range unlisted table tracking.
   - `WS /ws/lobby` and `WS /ws/sessions/{id}` handlers with strict server-side
     creator authorization.
   - Append-only SSD `.jsonl` statistics writer (`schema_version: 1`) and
     creator-authorized streaming download `GET /api/sessions/{id}/log`.
   - `rust-embed` static SPA serving, `GET /api/health`, `--healthcheck`
     self-probe, `--version` / build commit metadata, and graceful
     `SIGTERM`/`SIGINT` shutdown.
   - Integration tests covering lobby, viewer vs. creator permissions, grace
     period, PIN gating, Origin check, and JSONL output.
4. **Create the funny traffic-light SVG icon (`web/public/icon.svg`)**:
   - Design the cartoon traffic light with tiny radio antenna and expressive
     red/orange/green faces; render at favicon (16–64 px) and large sizes via
     Inkscape to visually verify crispness.
5. **Implement the `web/` React frontend**:
   - Homepage (`/`) with funny icon, short description, live session list
     (`/ws/lobby`), and visible "Add session" link.
   - Add session (`/new`) and Creator console (`/s/{id}` with stored
     `creator_token`): lazy-loaded `trafficlights-wasm` + Web Serial API
     bridge (`0x0D28`, 115200 baud, wake-up `\n`, `!get` probe, station-mode
     warning, Wake Lock, disconnect/reconnect), Table Range, `Reset All To
     Green`, unlisted-tables banner, `.jsonl` download, and `End session`.
   - Normal client console (`/s/{id}`): visual parity with
     `fastapi_receiver.py` (counters, live ticking red timer, shape status
     icons, Web Audio beeps, per-table mute, clickable tiles) without serial
     controls, Table Range, or batch operations, plus build version/commit and
     AGPL-3.0-or-later source footer (Section 13).
   - Vitest unit and markup tests.
6. **Add Coolify containerization and CI workflow**:
   - Four-stage distroless `Dockerfile`, `.dockerignore`, `compose.dev.yaml`,
     `compose.prod.yaml`, `.env.example`, and
     `.github/workflows/docker-publish.yml`.
   - Verify full `make gate`, local Docker build + `compose.dev.yaml` run
     (checking image size, `--healthcheck`, idle RSS memory, graceful
     shutdown), browser E2E check, and live micro:bit V2 Web Serial check.
7. **Remaining maintainer decision (before production deploy)**:
   - Coolify public domain (`https://<host>:3000`) and confirmation of the GHCR
     image path (`ghcr.io/nb-tudelft/trafficlights_webapp`).
