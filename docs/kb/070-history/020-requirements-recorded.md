---
summary: >-
  Record (`2026-11-25`): the maintainer's requirements were recorded verbatim
  and structured as R1–R14 in Requirements (including backward compatibility
  with the old MakeCode student/receiver setup while extensible for new
  firmware functions later); `/home/zam/Desktop/prog/services_zamkorus`,
  `../trafficlights_firmware/old/`, and the local dev machine toolchain were
  inspected and documented in Reference; proposed architecture choices and open
  questions were captured in Requirements and Work order.
---

# Requirements recorded (`2026-11-25`)

The maintainer gave the requirements for `trafficlights_webapp` in two
messages on `2026-11-25` (the second adding backward compatibility with the old
MakeCode student/receiver setup alongside later extension for new firmware
functions, recorded as R14) and pointed to
`/home/zam/Desktop/prog/services_zamkorus` as a reference for a similar Rust +
web project deployed on Coolify.

## What was inspected

- **`../trafficlights_firmware/old/`:** read `teacherReceiver.py` and
  `studentLightMicrobit.py` directly to verify exact legacy wire constraints
  (`Delimiters.NEW_LINE`, blind radio forwarding by the old receiver, unchecked
  color indexing on `T`/`RT` in the old student light, and harmless ignoring of
  non-`T`/`RT`/`S` lines).

- **`/home/zam/Desktop/prog/services_zamkorus`:** read `README.md`,
  `Cargo.toml`, `crates/app/Cargo.toml`, `rust-toolchain.toml`, `Dockerfile`,
  `compose.dev.yaml`, `compose.prod.yaml`, `web/package.json`,
  `web/vite.config.ts`, `web/public/icon.svg`, and the key KB sections
  (`README.md`, `020-minimal-and-resource-frugal.md`, `020-rust-conventions.md`,
  `030-frontend-conventions.md`, `080-build-test-and-deploy.md`).
- **Dev machine tools:** verified `rustc 1.98.1`, `cargo 1.98.1`, `node
  v26.13.1`, `nub v0.11.2`, `pnpm`, `npm`, `bun`, `docker`, `podman`,
  `inkscape`, `gimp`, and `blender` are installed; `wasm32-unknown-unknown`,
  `wasm-pack`, `wasm-bindgen`, `wasm-opt`, and `trunk` are not installed.

## Changes to the knowledge base

- Updated Mission (`010-mission/010-mission.md`) and `README.md` to reflect the
  remote Rust server, multi-session homepage, creator Web Serial streaming,
  role split, and custom SVG icon.
- Rewrote Requirements (`010-mission/020-requirements.md`) with the
  maintainer's verbatim messages, structured requirements `R1`–`R14`, technical
  consequences (Web Serial API secure-context and browser constraints),
  proposed answers to the delegated decisions (stack matching
  `services_zamkorus`, server-authoritative Rust protocol crate with thin
  browser Web Serial line bridge and no WASM in v1, WebSocket-first transport),
  and open questions to confirm before scaffolding.
- Added "Reference project and toolchain"
  (`040-reference/040-reference-project-and-toolchain.md`).
- Rewrote Work order (`050-ongoing-work/010-work-order.md`) around confirming
  the requirements and open questions first, then designing, scaffolding, and
  implementing in slices.

## Checks run and left open

- `make kb` rewrote `docs/kb/README.md`, `make gate` (`scripts/kb.sh check`)
  passed, and `make kb-graph` refreshed `graphify-out/`.
- No application code or scaffolding was created yet; waiting on the
  maintainer to confirm the requirements reading, proposed choices, and open
  questions.
