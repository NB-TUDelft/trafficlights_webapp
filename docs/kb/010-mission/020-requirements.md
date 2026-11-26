---
summary: >-
  The maintainer's confirmed requirements for trafficlights_webapp (`2026-11-25`,
  R1–R15, quoted verbatim): port the existing frontend to a remote Rust server
  deployed on Coolify (referencing `services_zamkorus`), multi-session homepage,
  Add Session page with browser Web Serial streaming via Rust/WASM
  (`trafficlights-proto`), WebSocket-first transport, normal clients vs.
  creator-only controls, backward compatibility with the old MakeCode setup
  while extensible for new firmware, SSD-backed classroom statistics export
  respecting the host's 88% RAM utilization (R15), and a funny cartoon
  traffic-light SVG icon.
always_read: true
---

# Requirements

The maintainer gave these on `2026-11-25`, in three messages (the initial
requirements, backward compatibility R14, and classroom statistics plus the
host RAM constraint during the design interview). Their words come first
because the table after them is a reading of them: if the two differ, the
maintainer's words win and the table gets corrected. Every design branch was
settled with the maintainer in the `2026-11-25` design interview (see
"Architecture and design" under Reference).

## In the maintainer's words

> please document project requirements: goal is to have same frontend as we
> have but backend to be served from server; i would like it to be rust server
> and you can choose frontedd framework if needed in which website can be
> ported to for reference of similar project have a look
> /home/zam/Desktop/prog/services_zamkorus; on homepage there will be list of
> sessions and short description in special add session link there will be page
> to select serial device and that device will be processed and streamed to
> server by session creators browser u can decide if its better to create and
> interpret serial commands ort server and send them to bridge like browser
> part over or is it better to process it onbrowser ad just send status changes
> to server. also you can accelerat4e whole process but also creating some rust
> client wasm modules; u can choose how to set client server communication
> maybe websocets entirely; project will run on coolify like reference one i
> gave you; for normal clienrts (notr session creator) just show page like it
> looks and works now but without a settings of serial port and without batch
> operations like make all green; keep these for page of session creator;
> session can be named and it shows with that name on session list; funny
> traficlight svg icon has to be made too and artcraft apps are on this dev
> machine if needed for that

> one more thing webapp has to be backwardcompatible with old student /receiver
> setup u can see in firmware catalogue but it will be extended wioth new
> firmware functiions lateron

> lets do 1 but can we also make it to create statiscics so some kind of
> documented dataformat which will allow to later plot easily how many lights
> were turned on in given time plot fo state per table; plot of state of avg
> table etc to see if learnig can mbe optimized also u can think of additionall
> points of data and how to store it most effectivelly; you can ad turso though
> but inside ephemeral container those sqlfiles can be volitile thrugh rebuilds
> use them if u find them convinient not to make an archive we can later
> discuss that more indepth

> pls decidew betweeen 1 and 2 im tbh starting to think about 2 because my
> server is on 88% of ram currently and 2 will utilize ssd instead

## The requirements, as read and confirmed (`2026-11-25`)

Strength follows the maintainer's wording: **must** (required behavior),
**decided** (delegated item settled in the `2026-11-25` interview), **given**
(reference or operational fact).

| # | Requirement | Strength |
| :--- | :--- | :--- |
| R1 | Same frontend look and behavior as the existing `fastapi_receiver.py` console, ported to React 19 + Vite + TypeScript + Tailwind CSS v4 (Nub, Biome, Vitest). | must / decided |
| R2 | Backend served from a remote server written in **Rust** (Axum 0.8 + Tokio + `rust-embed`). | must / decided |
| R3 | Use `/home/zam/Desktop/prog/services_zamkorus` as a structural, tooling and deployment reference for a similar Rust + web project. | given |
| R4 | Homepage displays a live list of named sessions and a short description of the application. | must |
| R5 | A visible "Add session" link on the homepage leads to a page where the session creator names the session and selects the serial device; the named session appears on the homepage session list. | must / decided |
| R6 | The session creator's browser accesses the USB serial receiver device via the Web Serial API and streams structured updates/commands between it and the remote server. | must |
| R7 | Browser-side **Rust/WASM** (`trafficlights-proto` compiled to `wasm32-unknown-unknown`) parses and formats serial frames in the creator's browser and exchanges structured events and commands with the Rust server (which shares `trafficlights-proto`). | decided |
| R8 | Build a Rust/WASM client module (`crates/wasm`) loaded lazily on the creator route. | decided |
| R9 | **WebSocket-first** client–server communication: `/ws/lobby` (live homepage list) and `/ws/sessions/{id}` (viewer updates + creator serial events/commands), with HTTP only for embedded SPA assets, `GET /api/health`, and streaming session log downloads. | decided |
| R10 | Deployable on **Coolify**, mirroring `services_zamkorus`: multi-stage distroless `Dockerfile` (`--healthcheck`), `compose.dev.yaml`, `compose.prod.yaml`, GitHub Actions -> GHCR -> Coolify webhook, `PUBLIC_ORIGIN` WebSocket Origin validation, and version/commit footer. | must / decided |
| R11 | Normal clients (not the session creator) see the traffic-lights console, can click individual table tiles to cycle colors, and manage their own local beeps and per-table mute—**without** serial port settings, **without** batch operations ("Reset All To Green"), and **without** Table Range controls. | must / decided |
| R12 | Session creator's page retains serial port controls, batch operations ("Reset All To Green"), Table Range configuration, "Unlisted tables heard" indicator, session statistics download, and "End session" control—enforced server-side via an unguessable per-session capability token (plus optional `CREATE_SESSION_PIN` env var). | must / decided |
| R13 | Custom funny traffic-light SVG icon: cartoon traffic light with a tiny radio antenna on top and three expressive faces inside the lamps (red = panicked, orange = puzzled, green = happy smile), legible down to favicon size. | must / decided |
| R14 | Backward compatible with the old MakeCode student light and teacher receiver setup (`../trafficlights_firmware/old/`), while extensible for new `trafficlights_firmware` functions later; on serial connect send wake-up `\n` and a single safe `!get` probe (1 s timeout falls back seamlessly to legacy MakeCode receiver; `!ok` records firmware version and warns if a station board was plugged in). | must / decided |
| R15 | Record classroom statistics per session in a documented, plot-friendly append-only JSONL data format streamed directly to ephemeral SSD (no event array held in RAM and no Turso engine, respecting the host's 88% RAM usage) so active light counts over time, per-table timelines, average state, wait/resolution times, and bottleneck spikes can be plotted easily. | must / decided |

## Already established from `trafficlights_firmware`

Cross-repository facts from
`../trafficlights_firmware/docs/kb/010-mission/020-requirements.md`,
`../trafficlights_firmware/docs/kb/040-reference/070-webapp-cooperation-and-retrofit.md`,
and live classroom tests (`2026-11-24` through `2026-11-26`; details in
"Firmware serial interface" under Reference):

- **Legacy serial and radio wire rules (R14, firmware R3, R7):** 115200 baud USB
  CDC serial (`USB VID 0x0D28`), wake-up `\n` on connect, strictly `\n`-terminated
  validated `T,<table>,<color>\n` (`color` in `0..2`) and `T,-1,0\n` commands,
  accept `S` and `RS`, ignore `RT` echo, never emit malformed `T,...` lines
  (old MakeCode lights index `colorList` unchecked). On the air, MakeCode
  packets are 32-byte frames (`01 00 01 02 <time> <serial=0> <len> <text>`) and
  `radio.config(length=32)` is required on group `146`; the new firmware
  receiver is non-transparent (decided `2026-11-26`), filters non-printable
  ASCII, and drops malformed commands.
- **Transparent mesh bridging (firmware R4, R5, `src/mesh.py`):** the new
  firmware's 16-byte `0x4D` managed-flooding mesh frames coexist safely on group
  `146` beside old MakeCode stations (verified at up to 10 frames/s). The
  receiver sends every `T` command in both formats and prints deduplicated mesh
  status frames as standard `S,<origin>,<color>` lines, requiring zero wire
  change in `trafficlights_webapp`.
- **New firmware control commands (firmware R13, R14):** `!get`, `!set
  mode=... id=...`, `!clear`, `!restart` answer `!ok ...` or `!err ...` with no
  commas; receiver has no ID; station ID `0` is reserved and unset stations
  report `id=0`; valid assigned station IDs are `1` to `9999`. `!get` answers
  within `0.03 s` under live load, whereas `!set` / `!clear` can take up to
  `1.24 s` during flash-page reclamation.
- **Target hardware (firmware R11 + live retrofit `2026-11-26`):** micro:bit
  V2.21 (`9906`) and V2.00 (`9904`, found on the live classroom receiver); ~128-byte
  UART RX buffer, `MAX_LINE = 64` bytes.

## Still open (non-blocking for scaffolding)

All product, architectural, licensing (`AGPL-3.0-or-later`; see Licensing), and
source-repository (`https://github.com/NB-TUDelft/trafficlights_webapp`,
renamed from `NB-TUDelft/classroom_trafficlights`) decisions are recorded. Only
one operational detail remains for the maintainer to provide before production
deploy:

1. **Coolify deployment identifiers:** target public domain (`https://<host>:3000`)
   and GHCR image path (defaulting to
   `ghcr.io/nb-tudelft/trafficlights_webapp` unless a different registry/path
   is preferred).
