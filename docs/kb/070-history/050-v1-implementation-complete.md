---
summary: >-
  `2026-11-25`: completed trafficlights_webapp v1 end to end across multi-model
  coworkers (Claude Code sonnet/opus, Muse Code muse-wonder-full-1.0, Swarm
  Claude Sonnet/Opus 5.5, GPT-OSS 120B): 142 Rust tests, 62 Vitest tests,
  SonarQube quality gate OK, 47.5 MB distroless image at 9.2 MiB RSS, and live
  headless Brave E2E verification.
---

# v1 implementation complete (`2026-11-25`)

Completed the full v1 implementation of `trafficlights_webapp` covering
Requirements `R1`–`R15` under `AGPL-3.0-or-later`, orchestrated across
multi-model coworkers (`Claude Code sonnet/opus`, `Muse Code
muse-wonder-full-1.0`, Swarm `Claude Sonnet 5.5`, Swarm `Claude Opus 5.5`, and
Swarm `GPT-OSS 120B`) with GPG-signed linear commits on `main`.

## Delivered components

- **Shared protocol & browser WASM (`crates/proto`, `crates/wasm`):** I/O-free
  wire framing (`MAX_LINE_BYTES = 64`), legacy `S`/`RS`/`RT` parsing, strict
  validated `T,<table>,<color>\n` / `T,-1,0\n` / `\n` / `!get\n` formatting,
  `!ok`/`!err` firmware detection, heartbeat deduplication with RF packet-loss
  safeguard, tagged WebSocket DTOs, `schema_version: 1` `.jsonl` events, and
  lazy-loaded `wasm-bindgen 0.2.129` `SerialBridge` (`55.9 kB` `.wasm`).
- **Rust Axum 0.8 WebSocket server (`crates/server`):** live `/ws/lobby` and
  `/ws/sessions/{id}`, CSPRNG `creator_token` + constant-time `subtle`
  verification, optional `CREATE_SESSION_PIN` with global failure bucket and
  10-failure socket close, `PUBLIC_ORIGIN` CSWSH validation, `MAX_SESSIONS` /
  `MAX_CONNECTIONS` / 512-table range caps, 5-minute disconnect grace reaper,
  wrong-mode `mode=station` refusal, append-only SSD `.jsonl` logging (16 MiB
  per-session cap, `0700`/`0600` Unix permissions, O(1) RAM streaming download
  at `GET /api/sessions/{id}/log`), `rust-embed` SPA with CSP & HTTP security
  headers, `--healthcheck`, `--version` (with AGPL Section 13 source URL), and
  graceful `SIGTERM`/`SIGINT` shutdown.
- **React 19 + Vite + TypeScript frontend (`web/`):** custom funny cartoon
  traffic-light SVG icon (`web/public/icon.svg`), live homepage lobby (`/`),
  Add Session (`/new`) with conditional PIN and Web Serial picker, Creator vs.
  Normal Client console (`/s/:id`) matching `old/fastapi_receiver.py` tile
  layout, shape-based status icons, live 1 s ticking Longest On Red, Web Audio
  880/660 Hz alerts with per-table mute, lazy WASM Web Serial bridge with `!get`
  probe retries and 1 s legacy fallback, wrong-mode station & unlisted-table
  banners, `.jsonl` download, and AGPL Section 13 footer.
- **Tooling, SonarQube & Coolify deploy:** `make gate`, `make dev` (Docker
  Compose `compose.dev.yaml` on `http://127.0.0.1:3000`), `make dev-down`,
  `make dev-hmr`, `make sonar` (`sonar-project.properties` + `scripts/sonar.sh`),
  4-stage distroless `Dockerfile`, `compose.prod.yaml`, `.env.example`, and
  `.github/workflows/docker-publish.yml`.

## Verification evidence

- **`make gate` & `cargo test --workspace --locked`:** 142 Rust tests (50
  proto, 42 server unit, 5 CLI, 32 integration, 5 smoke, 8 wasm) + 62 Vitest
  tests across 9 files + Biome `lint:strict` (0 errors, 0 warnings across 50
  files) + `cargo fmt --all --check` + `cargo clippy --workspace --all-targets
  -- -D warnings` + KB check.
- **Independent reviews (`R1` Security/Container, `R2` Code/Compatibility/A11y,
  `R3` SPDX/Spelling/AGPL):** 0 CRITICAL findings; all HIGH and key MEDIUM
  findings remediated with regression tests.
- **SonarQube (`make sonar`):** quality gate **`OK`**
  (`https://sonarqube.plohnensoftware.download/dashboard?id=trafficlights_webapp`),
  0 reliability/security/blocker issues, 0 hotspots, maintainability rating A.
- **Live Docker (`compose.dev.yaml`) + headless Brave CDP E2E:** `47.5 MB`
  distroless image, `healthy` via `--healthcheck` in 5 s, `9.2 MiB` live RSS
  under `128 MiB` read-only container cap, all HTTP security/CSP headers
  verified, Lobby / Add Session / Viewer / Creator / tile click / `.jsonl`
  200+401 verified with 0 console errors and visual screenshot review, graceful
  `docker compose stop` exit code `0` in 0.5 s.

## Known gaps left for maintainer

- Real micro:bit V2 hardware testing over USB Web Serial (old MakeCode and new
  `../trafficlights_firmware` receiver/station boards).
- Coolify production domain/webhook bring-up and pushing `main`.
- Restoring the Rust plugin on `sonarqube.plohnensoftware.download` (Community
  Build `26.11.0.134175` currently loads no Rust plugin; repo already sends
  `crates/` and `coverage/rust-lcov.info`).
