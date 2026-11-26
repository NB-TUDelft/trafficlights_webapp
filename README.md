# trafficlights_webapp

Teacher web application for the classroom traffic lights system (hardware and
legacy serial protocol originally introduced by
[RolfHut/ClassroomIndicatorLights](https://github.com/RolfHut/ClassroomIndicatorLights)).
It connects over USB serial (via the creator's browser Web Serial API and a
lazy-loaded Rust/WASM protocol bridge) to either a legacy MakeCode micro:bit
teacher receiver or a micro:bit V2 running the replacement firmware in the
sibling repository `../trafficlights_firmware`
([NB-TUDelft/trafficlights_firmware](https://github.com/NB-TUDelft/trafficlights_firmware)),
broadcasts live classroom status over WebSockets from a Rust Axum server, tracks
how long tables have been red, and lets teachers set individual tables, reset
all tables to green, and download classroom statistics as `.jsonl` (analyzed
offline by the companion CLI in `../trafficlights_analytics`,
[NB-TUDelft/trafficlights_analytics](https://github.com/NB-TUDelft/trafficlights_analytics)).

**Status:** v1 implementation plus post-v1 polish (viewer name collection,
optional custom head script, compact creator cards, unified header, watch-mode
`make dev`) complete and verified (`make gate`, `make sonar` quality gate `OK`,
47.5 MB distroless Docker image at 9.2 MiB RSS, headless Brave E2E). Verified
live in a classroom (`2026-11-26`) on both the old MakeCode receiver and a
retrofitted `trafficlights_firmware` V2.00 receiver with ~40 legacy stations and
retrofitted station 43. Ready for remaining hardware checks (`T,-1,0` broadcast,
live `mode=station` guard, mesh-relayed stations) and Coolify production
bring-up (see Work order and History in the knowledge base).

## Knowledge base

`docs/kb/README.md` is the working record for this repository: mission, rules,
licensing, standing reference on `old/fastapi_receiver.py`, the
`trafficlights_firmware` serial interface, the statistics `.jsonl` schema,
ongoing work, and history. It is the single source of truth for maintainers and
agents (`AGENTS.md` only points there), and this README stays the short public
entry point.

## Daily loop

    nub --cwd web install  # once on a fresh clone
    make dev               # watch mode (http://127.0.0.1:5173): Vite updates the frontend on save,
                           #   the Rust server (:3000) restarts and WASM rebuilds when crates/ change
    make dev-up            # build & run the production image via Docker Compose (http://127.0.0.1:3000)
    make dev-down          # stop the local Docker Compose preview stack
    make wasm              # compile crates/wasm -> web/src/wasm
    make web-build         # build React frontend into web/dist (embedded by server)
    make sonar             # generate Vitest + Rust lcov coverage, run SonarQube scan, poll gate
    make sonar-coverage    # generate lcov coverage reports only (without uploading)
    make kb                # rewrite the index in docs/kb/README.md from section files
    make kb-check          # verify knowledge-base index and layout rules
    make kb-graph          # refresh graphify-out/ with code and knowledge base
    make gate              # full pre-commit gate (KB, WASM, Biome/TS/Vitest/Vite, Cargo)

## Layout

    Cargo.toml             Rust workspace (`crates/proto`, `crates/wasm`, `crates/server`)
    crates/proto/          I/O-free serial wire protocol, shared WS and JSONL DTOs
    crates/wasm/           wasm-bindgen browser bridge (loaded lazily by creator route)
    crates/server/         Axum 0.8 + Tokio server with rust-embed (`web/dist`)
    web/                   React 19 + Vite + TypeScript + Tailwind CSS v4 (Nub, Biome, Vitest)
    scripts/build-wasm.sh  builds crates/wasm into web/src/wasm
    scripts/dev.sh         local dev runner (`make dev`, `make dev-up`, `make dev-down`)
    scripts/sonar.sh       SonarQube coverage, scan, and quality-gate poll (`make sonar`)
    scripts/kb.sh          knowledge-base index, check, and graph runner
    sonar-project.properties  SonarQube project and coverage paths (no tokens or URLs)
    old/fastapi_receiver.py  original single-file FastAPI teacher app (reference only)
    docs/kb/               the knowledge base and its housekeeping/graphify scripts
    Makefile               gate, WASM, web-build, and knowledge-base targets
    AGENTS.md              pointer-only agent entry point (`CLAUDE.md` and `CODEX.md` link here)
    LICENSE                GNU Affero General Public License v3.0 (`AGPL-3.0-or-later`)
    NOTICE.md              copyright, contributor roster, and provenance

## License

Licensed under **AGPL-3.0-or-later** (see [LICENSE](LICENSE) and
[NOTICE.md](NOTICE.md)). Source repository:
<https://github.com/NB-TUDelft/trafficlights_webapp>.
