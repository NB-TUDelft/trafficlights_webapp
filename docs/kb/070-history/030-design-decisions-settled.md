---
summary: >-
  Record (`2026-11-25`): completed the 13-question design interview (`/grill-me`)
  settling stack, Rust/WASM browser serial parser, WebSocket-first protocol,
  creator vs. normal permissions, optional `CREATE_SESSION_PIN`, 5-minute
  disconnect grace period, SSD-backed append-only JSONL classroom statistics
  (R15, keeping RAM minimal on the 88%-loaded host), `!get` firmware detection
  on connect, out-of-range creator notice, cartoon traffic-light SVG icon,
  Coolify distroless pipeline, and moving `fastapi_receiver.py` to `old/`.
---

# Design decisions settled (`2026-11-25`)

Walked through the full design tree with the maintainer (`/grill-me`, 13
questions) to resolve every open architectural, permission, statistics, UI, and
deployment choice before writing code.

## Decisions locked in

1. **Stack:** Rust Axum 0.8 + Tokio + `rust-embed` backend with React 19 +
   Vite + TypeScript + Tailwind CSS v4 + Nub + Biome + Vitest in `web/`,
   matching `/home/zam/Desktop/prog/services_zamkorus`.
2. **Serial processing & WASM:** browser-side **Rust/WASM** (`crates/proto`
   shared with `crates/server`, wrapped by `crates/wasm` via `wasm-bindgen`,
   lazily loaded on the creator route) parses incoming serial frames,
   deduplicates heartbeats, and formats validated outgoing commands.
3. **Transport:** WebSocket-first (`WS /ws/lobby` and `WS /ws/sessions/{id}`),
   with HTTP only for embedded SPA assets, `GET /api/health`, and streaming
   session log downloads.
4. **Permissions:** normal clients can click individual table tiles to cycle
   colors and manage local sound/per-table mute; serial port, `Reset All To
   Green`, `Table Range`, and session settings are creator-only (enforced
   server-side).
5. **Creator access:** visible "Add session" link on the homepage, unguessable
   per-session `creator_token` stored in the creator's browser, plus optional
   `CREATE_SESSION_PIN` server env var.
6. **Disconnect lifecycle:** 5-minute configurable reconnect grace period
   (`DISCONNECT_GRACE_SECONDS`) showing "Receiver disconnected" so the creator
   can refresh or reconnect, plus an explicit "End session" button.
7. **Statistics & RAM frugality (new R15):** maintainer requested documented
   classroom analytics data (lights over time, per-table timelines, average
   state, optimization metrics) and noted the production server is at **88%
   RAM**. Decided to stream append-only versioned `.jsonl` transition events
   directly to ephemeral SSD—zero event history in RAM and avoiding Turso's
   ~13.6 MiB RSS engine cost measured in `services_zamkorus`.
8. **Legacy & new firmware connect:** send wake-up `\n`, then one safe `!get`
   probe (1 s timeout falls back seamlessly to legacy MakeCode receiver; `!ok`
   records firmware version and warns if a station board was plugged in).
9. **Out-of-range tables:** keep the main grid strictly within `start..end` and
   show a subtle "Unlisted tables heard" notice on the creator page while still
   recording events in the `.jsonl` log.
10. **Funny SVG icon:** cartoon traffic light with tiny radio antenna on top
    and three expressive faces inside the lamps (red panicked, orange puzzled,
    green happy smile).
11. **Coolify deployment:** mirror `services_zamkorus` (distroless nonroot
    container with `--healthcheck`, `compose.dev.yaml`, `compose.prod.yaml`,
    GitHub Actions -> GHCR -> Coolify webhook, `PUBLIC_ORIGIN` WebSocket Origin
    check, version/commit footer) with an added `wasm` build stage.
12. **Repository policy:** move `fastapi_receiver.py` to `old/` as reference
    when scaffolding starts; keep linear `main` with short imperative signed
    commits; leave project license and remote open until deployment/release.

## Knowledge base updated

- Updated Requirements (`010-mission/020-requirements.md`) with R15 and
  confirmed statuses on R1–R15.
- Added "Architecture and design"
  (`040-reference/050-architecture-and-design.md`, *always read*).
- Updated Commits and branches, Maintainer and Agent Source of Truth, Order of
  this document, Work order, and `README.md`.
- Verified with `make kb`, `make gate`, and `make kb-graph`.
