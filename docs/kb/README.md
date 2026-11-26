# Knowledge Base

This knowledge base is the working record for maintaining trafficlights_webapp:
how to work in this repository, standing reference, what is in progress, and what
was done and why. It is the single source of truth for maintainers and agents;
`AGENTS.md` (which `CLAUDE.md` and `CODEX.md` link to) only points here.
`README.md` at the repository root stays the public entry point.

**Reading it.** Read this index in full, then every section on the *always read*
list below, then go straight to the sections your task needs. Each section is one
file, small enough to read whole, and every entry links to it by path. Groups run
from most to least important (see "Order of this document" under Rules), and the
three-digit prefixes keep sections in that order on disk as well.

**Changing it.** Edit the section files, never the list below: it is generated
from each section's `# Title` and the `summary` and `always_read` in its front
matter. After adding, removing, renaming, or moving a section, run
`scripts/kb.sh index` (`make kb`) to rewrite the list. `scripts/kb.sh check`
(`make kb-check`, part of `make gate`) fails while the list is stale or a file
breaks the layout rules. `scripts/kb.sh graph` (`make kb-graph`) refreshes the
graphify graph with the knowledge base in it, so a query about code also finds
the sections that discuss it. "Order of this document" under Rules holds those
rules.

<!-- kb-index:start -->

**Always read**, in this order:

1. [Mission](010-mission/010-mission.md)
2. [Requirements](010-mission/020-requirements.md)
3. [Maintainer and Agent Source of Truth](020-rules/010-maintainer-and-agent-source-of-truth.md)
4. [Documentation and Verification Rules](020-rules/020-documentation-and-verification-rules.md)
5. [Order of this document](020-rules/020-documentation-and-verification-rules/010-order-of-this-document.md)
6. [Commits and branches](020-rules/030-commits-and-branches.md)
7. [Multi-agent workflow](020-rules/040-multi-agent-workflow.md)
8. [Licensing](030-licensing/010-licensing.md)
9. [The existing FastAPI receiver](040-reference/010-the-existing-fastapi-receiver.md)
10. [Firmware serial interface](040-reference/020-firmware-serial-interface.md)
11. [Architecture and design](040-reference/050-architecture-and-design.md)
12. [Work order](050-ongoing-work/010-work-order.md)
13. [v1 implementation register](050-ongoing-work/020-v1-implementation-register.md)

**Mission**: why the project exists, how it relates to trafficlights_firmware, and what it has to stay compatible with.

- [Mission](010-mission/010-mission.md) (*always read*) — What trafficlights_webapp is (the remote Rust-backed teacher web application for the classroom traffic lights system, sibling to `../trafficlights_firmware`), how multi-session Web Serial streaming from the session creator's browser replaces the local `fastapi_receiver.py` prototype, and how it relates to `trafficlights_firmware` and the `services_zamkorus` reference stack.
- [Requirements](010-mission/020-requirements.md) (*always read*) — The maintainer's confirmed requirements for trafficlights_webapp (`2026-11-25`, R1–R15, quoted verbatim): port the existing frontend to a remote Rust server deployed on Coolify (referencing `services_zamkorus`), multi-session homepage, Add Session page with browser Web Serial streaming via Rust/WASM (`trafficlights-proto`), WebSocket-first transport, normal clients vs. creator-only controls, backward compatibility with the old MakeCode setup while extensible for new firmware, SSD-backed classroom statistics export respecting the host's 88% RAM utilization (R15), and a funny cartoon traffic-light SVG icon.

**Rules**: how to work in this repository, verify, commit, and keep this knowledge base.

- [Maintainer and Agent Source of Truth](020-rules/010-maintainer-and-agent-source-of-truth.md) (*always read*) — Names this knowledge base the single source of truth for maintainers and agents; `README.md` stays the public entry point; the one agent file, `AGENTS.md` (`CLAUDE.md` and `CODEX.md` are symlinks to it), stays pointer-only, with no lowercase `agents.md`; `../trafficlights_firmware` is a separate sibling repository read for protocol reference and not edited unasked.
- [Documentation and Verification Rules](020-rules/020-documentation-and-verification-rules.md) (*always read*) — Standing rules: run `make gate` before committing (KB check, WASM build, Biome CI + strict lint + TypeScript + Vitest + Vite build, Cargo fmt + clippy `-D warnings` + workspace tests); fix touched-file issues; never weaken a check; American spelling; small commits; record the checks run and any known gaps; keep this knowledge base current while working.
  - [Order of this document](020-rules/020-documentation-and-verification-rules/010-order-of-this-document.md) (*always read*) — Standing rule: the knowledge base is one file per section under `docs/kb/`, in groups (Mission, Rules, Licensing, Reference, Ongoing work, Future work, History oldest first) ordered by three-digit prefixes; how to place new and finished work; the index generated by `docs/kb/housekeeping.mjs` through `scripts/kb.sh` from each section's title and front matter, and checked by `make gate`.
- [Commits and branches](020-rules/030-commits-and-branches.md) (*always read*) — Standing rules: git repository initialized on `2026-11-25` on `main` (root commit `7fe9e60`, signed); target GitHub repository is `https://github.com/NB-TUDelft/trafficlights_webapp` (renamed from `NB-TUDelft/classroom_trafficlights`, which the maintainer will force-push to); linear history on `main` with short imperative GPG-signed commits (no type prefix; optional `Co-Authored-By:` trailer for multi-agent coworker credit); signing is never bypassed; agents do not push or add a remote unasked.
- [Multi-agent workflow](020-rules/040-multi-agent-workflow.md) (*always read*) — Standing rules for multi-agent work in trafficlights_webapp, adapted from Git Graph Libre (`040-multi-agent-releases.md`) and `services_zamkorus` (`030-agent-protocol.md`): respectful coworker culture, model routing by complexity (Claude, GPT-OSS, headless Muse Code always on `muse-wonder-full-1.0`, headless Claude Code; never Gemini subagents), worktrees under `../.tw-wt/` (never `/tmp`), hard safety rules, small GPG-signed commits, rebase/merge integration (never squash-merge), branch cleanup with `git branch -d`, and facts-only slice reports.

**Licensing**: what licenses apply to what is here, where it came from, and what is still undecided.

- [Licensing](030-licensing/010-licensing.md) (*always read*) — `AGPL-3.0-or-later` for the entire project (decided `2026-11-25`, matching Git Graph Libre), hosted at `https://github.com/NB-TUDelft/trafficlights_webapp`, in `LICENSE` and `NOTICE.md`; no code from `RolfHut/ClassroomIndicatorLights` is incorporated (hardware and wire-protocol compatibility only, so no `LICENSE.apache`); SPDX lines in all project source files; AGPL Section 13 source link in the UI footer and `--version`; AGPL-compatible dependency intake rule.
  - [Provenance of the knowledge-base tooling](030-licensing/010-licensing/010-provenance-of-the-knowledge-base-tooling.md) — `docs/kb/housekeeping.mjs`, `docs/kb/graphify.mjs` and `scripts/kb.sh` were copied byte for byte from `../trafficlights_firmware` on `2026-11-25` at the maintainer's request; they are the maintainer's own AGPL-3.0-or-later tooling originally from Git Graph Libre, xrdp-kwin and ImProc.

**Reference**: standing guidance to read when a task touches its area.

- [The existing FastAPI receiver](040-reference/010-the-existing-fastapi-receiver.md) (*always read*) — What the starting `fastapi_receiver.py` (1,303 lines, commit `7fe9e60`) does, read from its code on `2026-11-25` (not yet executed): runtime settings, `TableState`, `WebsocketManager`, `SerialReceiver` (115200 baud, auto-detect, wake-up newline, `T,<table>,<color>` / `T,-1,0`, three-field parser ignoring `RT`), HTTP/WebSocket endpoints, embedded HTML/CSS/JS teacher console, and defects or quirks observed in the baseline code.
- [Firmware serial interface](040-reference/020-firmware-serial-interface.md) (*always read*) — The USB serial contract with `../trafficlights_firmware` (read at firmware commit `61786da` on `2026-11-25`): 115200 baud micro:bit V2 CDC serial, buffer and line-ending limits, the legacy `T`/`S`/`RS`/`RT` text protocol, the always-listening `!get`/`!set`/`!clear`/`!restart` control commands, compatibility guarantees, and what the firmware has not built yet.
- [Graphify Map](040-reference/030-graphify-map.md) — How `make kb-graph` (`scripts/kb.sh graph`) builds the graphify code and knowledge-base graph: `graphify update .` (offline), the `contains` and `mentions` edges `docs/kb/housekeeping.mjs --graph` adds, re-clustering, the tree; why `graphify-out/` is gitignored; what it indexes here.
- [Reference project and toolchain](040-reference/040-reference-project-and-toolchain.md) — Facts gathered on `2026-11-25` from the reference repository `/home/zam/Desktop/prog/services_zamkorus` (Rust 1.98 + Axum 0.8 + Tokio + `rust-embed`, React 19 + Vite + TypeScript + Tailwind CSS v4 + Nub + Biome + Vitest, three-stage distroless Dockerfile for Coolify on `zamkorland`) and from inspecting installed tools on this development machine (Rust, Nub, Docker/Podman, Inkscape/GIMP/Blender, and absence of WASM CLI tooling).
- [Architecture and design](040-reference/050-architecture-and-design.md) (*always read*) — Locked-in architecture and design decisions (`2026-11-25` interview, updated as features land): Cargo workspace (`crates/proto`, `crates/wasm`, `crates/server`) + React/Vite/Nub frontend (`web/`), browser Rust/WASM Web Serial bridge with `!get` firmware probe, WebSocket-first protocol (`/ws/lobby`, `/ws/sessions/{id}`), creator capability token + optional `CREATE_SESSION_PIN`, optional per-session viewer name collection (`collect_names`) with `.jsonl` `actor` attribution, optional `CUSTOM_SCRIPT_SRC` head script injection + CSP origin extension, 5-minute disconnect grace period, out-of-range table notice, SSD-backed append-only JSONL statistics format (avoiding RAM growth and Turso's ~13.6 MiB RSS overhead), funny cartoon traffic-light SVG icon, and four-stage Coolify distroless Docker pipeline.
- [Agent tooling](040-reference/060-agent-tooling.md) — Available agent tooling on this dev machine (`2026-11-25`): ECC plugin (`2.2.3`) review and TDD skills, TokenEconomizer (`~/.tokeneconomizer/bin/te`), internal swarm models (Claude Opus/Sonnet 5.5, GPT-OSS 120B; never Gemini), external headless Muse Code (`muse exec --yolo --model muse-wonder-full-1.0`) and Claude Code (`claude -p`), Inkscape SVG rendering, and headless Brave browser verification over DevTools Protocol.
- [Classroom statistics JSONL schema](040-reference/070-statistics-jsonl-schema.md) — Classroom statistics append-only JSONL schema (schema_version 1) streamed directly to ephemeral SSD (`LOG_DIR`, default `/tmp/trafficlights-logs`): file lifecycle, event specifications (including optional `collect_names` and `actor` attribution), server invariants, Python plotting recipes (pandas + matplotlib), and future SQLite/Turso archival mapping.

**Ongoing work**: what is in progress, in the order to take it up.

- [Work order](050-ongoing-work/010-work-order.md) (*always read*) — Remaining maintainer-owned work after v1 implementation completed on `2026-11-25`: live micro:bit V2 USB serial verification (old MakeCode and new firmware), Coolify public domain / webhook bring-up, and restoring the SonarQube Rust plugin on the analysis server.
- [v1 implementation register](050-ongoing-work/020-v1-implementation-register.md) (*always read*) — Execution register for trafficlights_webapp v1 (`2026-11-25`): complete record of slices S1–S7, independent reviews R1–R3, remediation, SonarQube quality gate OK, and live Docker + headless Brave verification.

**Future work**: recorded directions nobody has started.


**History**: dated records of finished work, oldest first.

- [Repository and knowledge base initialized (`2026-11-25`)](070-history/010-repository-and-knowledge-base-initialized.md) — Record (`2026-11-25`): git repository initialized on `main` with the untouched `fastapi_receiver.py` (`7fe9e60`), repository entry points and knowledge-base tooling copied from `../trafficlights_firmware`, the initial seven-group knowledge base created (including standing Reference for `fastapi_receiver.py` and the `trafficlights_firmware` serial interface), and verified with `make gate` and `make kb-graph`.
- [Requirements recorded (`2026-11-25`)](070-history/020-requirements-recorded.md) — Record (`2026-11-25`): the maintainer's requirements were recorded verbatim and structured as R1–R14 in Requirements (including backward compatibility with the old MakeCode student/receiver setup while extensible for new firmware functions later); `/home/zam/Desktop/prog/services_zamkorus`, `../trafficlights_firmware/old/`, and the local dev machine toolchain were inspected and documented in Reference; proposed architecture choices and open questions were captured in Requirements and Work order.
- [Design decisions settled (`2026-11-25`)](070-history/030-design-decisions-settled.md) — Record (`2026-11-25`): completed the 13-question design interview (`/grill-me`) settling stack, Rust/WASM browser serial parser, WebSocket-first protocol, creator vs. normal permissions, optional `CREATE_SESSION_PIN`, 5-minute disconnect grace period, SSD-backed append-only JSONL classroom statistics (R15, keeping RAM minimal on the 88%-loaded host), `!get` firmware detection on connect, out-of-range creator notice, cartoon traffic-light SVG icon, Coolify distroless pipeline, and moving `fastapi_receiver.py` to `old/`.
- [Licensed under AGPL-3.0-or-later (`2026-11-25`)](070-history/040-licensed-under-agpl-3.md) — Record (`2026-11-25`): licensed `trafficlights_webapp` cleanly under `AGPL-3.0-or-later` matching Git Graph Libre (`LICENSE` and `NOTICE.md`), set the public source URL to `https://github.com/NB-TUDelft/trafficlights_webapp` (renamed from `NB-TUDelft/classroom_trafficlights`), and removed `LICENSE.apache` and Apache-2.0 upstream lineage claims since no `RolfHut/ClassroomIndicatorLights` code is incorporated (hardware/protocol compatibility only) and `fastapi_receiver.py` is no longer published separately.
- [v1 implementation complete (`2026-11-25`)](070-history/050-v1-implementation-complete.md) — `2026-11-25`: completed trafficlights_webapp v1 end to end across multi-model coworkers (Claude Code sonnet/opus, Muse Code muse-wonder-full-1.0, Swarm Claude Sonnet/Opus 5.5, GPT-OSS 120B): 142 Rust tests, 62 Vitest tests, SonarQube quality gate OK, 47.5 MB distroless image at 9.2 MiB RSS, and live headless Brave E2E verification.
- [Viewer name collection and custom head script (`2026-11-26`)](070-history/060-viewer-names-and-custom-script.md) — `2026-11-26`: commit `fe069e4` added optional per-session viewer name collection (`collect_names` + `identify` + `.jsonl` `actor` attribution) and optional `CUSTOM_SCRIPT_SRC` deferred head-script injection with matching CSP origin rules; documented in Reference ("Architecture and design" and "Classroom statistics JSONL schema").
- [Compact creator cards and dev watch mode (`2026-11-26`)](070-history/070-compact-creator-cards-and-dev-watch.md) — `2026-11-26`: the creator console's Table Range card became a Session Management button opening a `TableRangeDialog` popup, Quick Actions became two grid columns wide with its switches in a grid (creator card row about 378 px -> 255 px tall at 1600 px), and `make dev` became a no-Docker watch mode (Vite on `:5173`, Rust server restart and WASM rebuild on `crates/` changes) while `make dev-up` runs the Docker Compose image build.
- [Unified page header (`2026-11-26`)](070-history/080-unified-page-header.md) — `2026-11-26`: Home, Add session and the session page share one header, styled in `web/src/header.css`, with the same height (117.2 px), width and position on every page at every width from 320 to 1920 px, measured in headless Brave; `make dev` now refuses to start when `:3000` or `:5173` is taken.

<!-- kb-index:end -->
