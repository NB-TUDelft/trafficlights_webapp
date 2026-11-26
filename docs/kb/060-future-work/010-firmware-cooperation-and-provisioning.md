---
summary: >-
  Unstarted future directions for deeper cooperation with
  `../trafficlights_firmware` once the firmware mesh settles: one-click
  `!set mode=receiver` recovery from `WrongModeBanner`, browser-side Web Serial
  station provisioning after hex flashes (answering DAPLink chip-erase loss),
  optional mesh diagnostics and legacy-frame toggle, and long-term `.jsonl`
  archival to SQLite/Turso.
---

# Firmware cooperation and board provisioning

None of the items below has been started. They build on the existing
`!`-prefixed serial control commands in `../trafficlights_firmware/src/control.py`
and the Web Serial Rust/WASM bridge in `crates/proto` + `crates/wasm` +
`web/src/serial/`.

## 1. One-click receiver recovery in `WrongModeBanner`

Today, when a creator plugs in a micro:bit configured as `mode=station`, `!get`
detects it, `WrongModeBanner` warns them, and the server refuses to mark the
receiver connected. Adding an explicit button (for example **"Switch board to
Receiver mode"**) that sends `!set mode=receiver\n`, waits at least **3 s** for
`!ok mode=receiver` (accounting for the measured `1.24 s` MicroPython
flash-reclaim pause; see "Firmware serial interface"), and re-probes with `!get`
would let a teacher fix a misconfigured receiver directly in the browser without
leaving the session page or using the logo gesture.

## 2. Browser-side station provisioning over Web Serial

Because DAPLink's whole-chip erase wipes `settings.txt` on every `.hex` flash
(firmware test T1 / requirement R10), teachers flashing batches of student boards
must currently run `uv run python scripts/mb.py control "set mode=station id=<N>"`
or configure each board via the logo gesture. A dedicated browser provisioning
tool in `trafficlights_webapp` (using Web Serial + `!get` / `!set` / `!clear`,
waiting >= 3 s per `!set`) would let any teacher with a Chromium browser flash
and number a class set of stations with zero local Python setup.

## 3. Optional mesh telemetry and legacy-TX toggle

Once `../trafficlights_firmware` finishes multi-board mesh validation (M5, M9):

- **Legacy frame toggle:** when a classroom has replaced every MakeCode station
  and repeater, a coordinated firmware setting (configured via `!set`) could
  disable duplicate MakeCode `S`/`T` framing to cut radio airtime roughly in
  half.
- **Mesh diagnostics:** optional `!`-prefixed or extended `!ok` telemetry (such
  as hop counts, firmware version beyond `draft-1`, or relay/queue counters)
  can be surfaced to the session creator without breaking older clients because
  `crates/proto::parse_line` already ignores unknown `key=value` pairs in `!ok`.

## 4. Long-term statistics archival

Importing downloaded or rotated `schema_version: 1` `.jsonl` files into an
external SQLite/Turso archive (mapped in "Classroom statistics JSONL schema"
under Reference) remains deferred while the host RAM stays tight (R15).
