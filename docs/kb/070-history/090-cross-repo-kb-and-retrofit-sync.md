---
summary: >-
  `2026-11-26`: synchronized this knowledge base with
  `../trafficlights_firmware` (`caa63d3`), documenting mutual repository links,
  the MakeCode 32-byte framing and transparent mesh cooperation contract, the
  four-stage retrofit compatibility matrix and live classroom verification on
  the V2.00 receiver and station 43, updated the Work order, and recorded future
  firmware-cooperation directions under Future work.
---

# Cross-repository knowledge base and retrofit sync (`2026-11-26`)

Updated this knowledge base alongside `../trafficlights_firmware` (after
fast-forwarding firmware `main` to `origin/main` at `caa63d3`) so both
repositories reference each other accurately and document both their USB serial
cooperation and the four-stage retrofit strategy for MakeCode classrooms.

## What changed

- Updated "Firmware serial interface" (Reference) from firmware commit `61786da`
  to `caa63d3`: added micro:bit V2.00 (`9904`) + V2.21 (`9906`) hardware support,
  MakeCode 32-byte string framing (`length=32`), the non-transparent receiver
  decision, transparent 16-byte `0x4D` mesh bridging (`S,<origin>,<color>` on
  serial, dual TX on commands, `queue=10`), measured `!get` (`<= 0.03 s`) vs.
  `!set`/`!clear` (`<= 1.24 s` flash-reclaim) latencies, the four-stage retrofit
  compatibility matrix, and live classroom constraints (legacy repeaters 7 and
  30, DAPLink whole-chip erase).
- Updated Mission, Requirements, "Maintainer and Agent Source of Truth" (Rules),
  "Commits and branches" (Rules), "Architecture and design" (Reference),
  "Classroom statistics JSONL schema" (Reference), "Work order" (Ongoing work),
  and `README.md` to link `https://github.com/NB-TUDelft/trafficlights_firmware`
  and the new companion analytics repository `../trafficlights_analytics`
  (`https://github.com/NB-TUDelft/trafficlights_analytics`, `AGPL-3.0-or-later`),
  clarify that `source: "student_radio"` reflects a physical button press rather
  than student-only self-resolution, reflect that `origin` is pushed, credit the
  `2026-11-26` live classroom verification on both old and new receivers, and
  list remaining hardware/deploy/hygiene tasks.
- Added Future work section "Firmware cooperation and board provisioning"
  capturing unstarted cooperation ideas (`WrongModeBanner` one-click `!set
  mode=receiver`, browser Web Serial station provisioning, future mesh
  telemetry/legacy-TX toggle, and `.jsonl` archival).

## Checks run

- `make kb` (`scripts/kb.sh index`) and `make gate`: `scripts/kb.sh check`, WASM
  release build, Biome `ci` + `lint:strict` (69 files), TypeScript typecheck,
  Vitest (`99/99` across 15 test files), Vite build, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test
  --workspace` (`154/154`: 51 `proto`, 47 `server` unit, 5 `cli`, 38
  `integration`, 5 `smoke`, 8 `wasm`).
- No Rust, TypeScript, or hardware state changed; live micro:bit USB hardware
  and `make sonar` were not re-run.
