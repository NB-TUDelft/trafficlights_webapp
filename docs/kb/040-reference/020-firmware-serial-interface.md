---
summary: >-
  The USB serial contract, cooperation mechanics, and four-stage retrofit
  architecture with `../trafficlights_firmware`
  (`NB-TUDelft/trafficlights_firmware`, read at commit `caa63d3` on
  `2026-11-26`): 115200 baud micro:bit V2.00/V2.21 CDC serial, buffer and
  line-ending limits, MakeCode 32-byte string frame retrofit, legacy
  `T`/`S`/`RS`/`RT` text protocol, transparent 16-byte `0x4D` mesh bridging,
  `!get`/`!set`/`!clear`/`!restart` control commands and flash-reclaim timings,
  live classroom retrofit results, and unbuilt firmware areas.
always_read: true
---

# Firmware serial interface

Summary of the USB serial contract, cooperation model, and retrofit solutions
shared with the sibling repository `../trafficlights_firmware`
([NB-TUDelft/trafficlights_firmware](https://github.com/NB-TUDelft/trafficlights_firmware),
read from its `src/` modules and knowledge base at commit `caa63d3` on
`2026-11-26`). For full firmware internals, read
`../trafficlights_firmware/docs/kb/README.md` and its companion section "Webapp
cooperation and retrofit architecture".

## Transport and port constraints

- **Hardware:** micro:bit **V2.21** (board ID `9906`) and **V2.00** (board ID
  `9904`, discovered on the live classroom receiver on `2026-11-26` and verified
  running the firmware hex); USB ID `0d28:0204` (NXP ARM mbed DAPLink CDC),
  MicroPython `v2.1.2`.
- **Serial settings:** `115200` baud, 8N1, UTF-8 text lines. On Linux the device
  appears as `/dev/ttyACM*` (and
  `/dev/serial/by-id/usb-Arm_BBC_micro:bit_CMSIS-DAP_*`); on Windows as `COM*`.
  On Arch/CachyOS the user must be in the `uucp` group.
- **Exclusive access:** only one process may hold the port open at a time. Close
  any serial monitor or `trafficlights_firmware` helper (`make flash`,
  `make repl`, `scripts/mb.py`) before connecting the web application, and vice
  versa. When two micro:bits are plugged into the same computer, `scripts/mb.py`
  requires `MICROBIT_PORT` and refuses to guess.
- **Buffer and line limits (`src/serialio.py`, firmware test T4):**
  - The micro:bit UART receive buffer holds about **128 bytes**; lines longer
    than **`MAX_LINE = 64` bytes** or containing invalid UTF-8 are silently
    dropped.
  - Single 7–10 byte teacher commands fit comfortably; `CreatorSerialBridge`
    paces outgoing writes by `25 ms` (`WRITE_PACE_MS`) to prevent buffer
    overflow on bursts.
  - Incoming endings accepted by the firmware: `LF` (`\n`), `CR` (`\r`),
    `CR LF` (`\r\n`), or **500 ms of quiet** (`IDLE_MS = 500`). The web
    application always terminates every outgoing line with `\n` (required by the
    old MakeCode receiver's `Delimiters.NEW_LINE`).
  - Outgoing lines from the new firmware's MicroPython `print()` end in `\r\n`
    (CR LF) and are strictly printable ASCII (`oldproto.unframe` drops any frame
    containing NUL, control bytes, or non-ASCII), handled cleanly by
    `crates/proto`'s `LineAccumulator`.
  - **Never send Ctrl-C (`0x03`) during normal operation:** `serialio.read_lines`
    deliberately raises `KeyboardInterrupt` on `0x03` so firmware dev tools can
    enter the REPL, and a soft reset (`Ctrl-D`) while the radio is still on
    corrupts MicroPython memory under live traffic (firmware test M11).
    `crates/proto::format_command` guarantees `0x03` can never be emitted.

## Legacy classroom protocol (old and new receivers)

Both the old MakeCode receiver and `trafficlights_firmware` receiver mode
(`src/receiver.py`, `src/oldproto.py`) speak three comma-separated fields on
radio group `146`, with colors `0` = green, `1` = orange, `2` = red:

| Direction | Line | Meaning |
| :--- | :--- | :--- |
| Webapp -> Receiver | `T,<id>,<color>\n` | Set station `<id>` (`1..9999`) to `<color>` (`0`, `1` or `2`) |
| Webapp -> Receiver | `T,-1,0\n` | Broadcast reset: set every station to green (`0`) |
| Webapp -> Receiver | `\n` (blank line) | Wake-up line sent on connect; accepted and ignored by the new receiver |
| Receiver -> Webapp | `S,<id>,<color>` | Status broadcast from an old station, a new station, or a deduplicated mesh status frame (`src/receiver.py`) |
| Receiver -> Webapp | `RS,<id>,<color>` | `S` packet relayed once by an old MakeCode repeater |
| Receiver -> Webapp | `RT,<id>,<color>` | `T` packet relayed once by an old MakeCode repeater (echo; ignored by `crates/proto`) |

Important radio and serial details verified directly on the live classroom
network on `2026-11-26` (firmware tests M1–M4 and history records 130–140):

- **MakeCode 32-byte radio framing:** MakeCode's `radio.send_string` wraps text
  in a 32-byte frame (`01 00 01 | 02 | 4-byte LE time | 4 zero bytes | 1-byte
  len | text | zero pad`), and `radio.config(length=32)` is mandatory on group
  `146` (`length=64` drops all MakeCode frames). `trafficlights_firmware` now
  uses `oldproto.frame` / `oldproto.unframe` with `send_bytes` / `receive_bytes`,
  so the new receiver hears old stations cleanly (822 of 822 valid lines in 60 s)
  and its `T` commands are obeyed by old stations (8 of 8) and relayed by old
  repeaters.
- **Unchecked old MakeCode parser:** the old MakeCode receiver forwards any
  `\n`-terminated serial line blindly onto radio group `146`, and old MakeCode
  student lights index `colorList[colorIndex]` without bounds checks.
  `crates/proto::format_command` therefore never emits malformed `T,...` lines
  or colors outside `0..2`.
- **Non-transparent new receiver (decided `2026-11-26`):** `src/receiver.py`
  forwards only well-formed `T,<table>,<color>` lines to radio, ignores blank
  and malformed lines, and handles `!` commands locally.
- **No immediate radio ACK on `T` commands:** neither old nor new stations send
  an immediate `S` reply when commanded; confirmation arrives on the station's
  next 4–6 s heartbeat. `crates/proto::SerialSession` writes the intended color
  into its deduplication cache when `T,<table>,<color>` or `T,-1,0` is sent, so
  a lost RF packet automatically self-corrects on the dashboard at the station's
  next heartbeat.

## Transparent mesh cooperation (`src/mesh.py`, commit `caa63d3`)

The first slice of the firmware's managed-flooding mesh is implemented in
`../trafficlights_firmware/src/mesh.py` and wired into `src/station.py` and
`src/receiver.py` without requiring any change to `trafficlights_webapp`:

- **Coexistence on group `146`:** mesh frames are 16-byte binary frames with
  header `01 00 01`, type `0x4D` (`'M'`, distinct from MakeCode's string type
  `0x02`), version `1`, kind (`1` status, `2` command), origin (`0` for
  receiver, station ID otherwise), 16-bit sequence number, hop count (`3`),
  signed target (`-1` broadcast), color (`0..2`), and `via` sender ID. Verified
  harmless to live MakeCode stations at up to 10 frames/s (firmware test M12).
- **Zero-change serial bridge:** on receiving a `T,<table>,<color>\n` serial
  line, `src/receiver.py` transmits **both** the MakeCode `T` frame (for old
  stations) and a mesh `COMMAND` frame (for new stations, multi-hop). Incoming
  mesh `STATUS` frames are deduplicated in the receiver's 48-entry cache and
  printed over USB serial as standard `S,<origin>,<color>` lines. The receiver
  never relays and uses `queue=10` to handle relay bursts.

## Serial control commands (`src/control.py`, firmware R13–R14)

Every mode of the new firmware (both **receiver** and **station**) listens on
USB serial for `!`-prefixed control lines so board setup can be automated after
a hex flash (which erases settings on DAPLink whole-chip erase):

| Command | Success reply | Effect |
| :--- | :--- | :--- |
| `!get` | `!ok mode=station id=7 color=green fw=draft-1` or `!ok mode=receiver fw=draft-1` | Reports running mode, station ID and current color (station only), and firmware version. Changes nothing. |
| `!set mode=station id=12` | `!ok mode=station id=12` | All-or-nothing save to `settings.txt` and immediate mode restart. Either key may be sent alone. |
| `!set mode=receiver` | `!ok mode=receiver` | Saves receiver mode and restarts; keeps any previously stored station ID for a future return to station mode. |
| `!clear` | `!ok mode=station id=0` | Deletes `settings.txt` and restarts as an unset station (`id=0`). |
| `!restart` | `!ok restarting` | Restarts the current mode. |
| invalid line | `!err <reason>` | Changes nothing. Reasons: `empty command`, `unknown command`, `nothing to set`, `expected key=value`, `bad mode`, `bad id`, `unknown key`, `receiver has no id`. |

Key rules and measured timings for `trafficlights_webapp`:

- **Safe coexistence & probe:** `!` lines are never sent on the radio by the new
  receiver, and an old MakeCode receiver forwarding `!get` onto group `146` is
  harmless because old stations and repeaters ignore lines whose first field is
  not `T`, `RT` or `S`. Replies (`!ok ...`, `!err ...`) contain **no commas**,
  so legacy 3-field parsers ignore them. `crates/proto::parse_line` ignores
  unknown `key=value` tokens in `!ok`, keeping the webapp forward-compatible
  with future firmware fields.
- **Measured command latencies (`2026-11-26`):**
  - `!get` answered **40 of 40** within **`0.03 s`** under live classroom radio
    traffic, validating `CreatorSerialBridge`'s `1 s` probe timeout (+ 2 retries
    for boards still scrolling their 1 s start-up display).
  - `!set` and `!clear` write `settings.txt` and paused **`1.23–1.24 s`** in 3
    of 12 mode changes while MicroPython reclaimed flash pages. Any future
    webapp feature sending `!set` or `!clear` (see Future work) must wait at
    least **`3 s`** before retrying, and must tolerate a brief USB serial
    drop/reconnect observed once after `!set` on the V2.00 receiver.
- **ID rules (firmware R14):** a receiver has no ID and rejects `id=`; valid
  assigned station IDs are `1` to `9999`; ID `0` is reserved for the receiver
  and refused by `!set`; an unset station boots with `id=0` and is surfaced by
  `trafficlights_webapp` in `UnlistedBanner` (`Table 0 (unset ID)`).
- **Wrong-mode guard:** if `!get` returns `mode=station`, `WrongModeBanner`
  warns the creator and `crates/server/src/session.rs` refuses to mark the
  receiver connected.

## Retrofit solutions and compatibility matrix

The two repositories are engineered so classrooms can upgrade web software,
receiver firmware, and student stations independently in four stages:

| Stage | Student lights | Receiver | Webapp behavior & status |
| :--- | :--- | :--- | :--- |
| 1. Webapp-only retrofit | Legacy MakeCode (`old/microbit-SCRLights.hex`, incl. repeaters) | Legacy MakeCode (`old/microbit-SCRteacher.hex`) | `!get` times out in `1 s` -> seamless **Legacy MakeCode receiver** fallback; strictly validated `\n`-terminated `T` commands protect old lights. **Verified live before receiver reflash (`2026-11-26`).** |
| 2. Receiver retrofit | Legacy MakeCode stations and repeaters | `trafficlights_firmware` (`mode=receiver`) | `!get` returns `!ok mode=receiver fw=draft-1`; firmware frames MakeCode packets with `oldproto.frame`/`unframe`. **Live in classroom on V2.00 receiver (`2026-11-26`).** |
| 3. Mixed-fleet rollout | Legacy MakeCode stations + new `trafficlights_firmware` stations in batches | `trafficlights_firmware` (`mode=receiver`) | New stations send both MakeCode `S` (relayed by old repeaters) and `0x4D` mesh frames; receiver sends both `T` formats. **Station 43 retrofitted and verified live with `trafficlights_webapp` (`2026-11-26`); mesh multi-board checks pending.** |
| 4. Full mesh fleet | All `trafficlights_firmware` stations | `trafficlights_firmware` (`mode=receiver`) | 3-hop managed flooding with 48-entry cache and 2-copy suppression; future firmware option can disable duplicate MakeCode `S`/`T` frames once no legacy hardware remains. |

### Classroom rollout constraints to remember

- **Hidden legacy repeaters (stations 7 and 30):** a live survey on `2026-11-26`
  found two MakeCode repeaters carrying about a third of the classroom's tables
  from the test location. Because new stations drop the legacy `RS`/`RT`
  repeater role (firmware R6 relaxed to mesh-only), those repeaters must stay on
  MakeCode until surrounding stations run the verified mesh (or direct coverage
  from the receiver's position is confirmed).
- **Whole-chip erase on hex flash (firmware T1 / R10):** dragging
  `build/trafficlights.hex` onto `MICROBIT` erases MakeCode flash storage and
  `settings.txt`. Boards must be provisioned afterward via `scripts/mb.py
  control` (`!set`) or the logo gesture (or a future Web Serial provisioning
  flow in `trafficlights_webapp`; see Future work).

## Firmware status and unbuilt areas

As of `2026-11-26` (`trafficlights_firmware` commit `caa63d3`):

- **Implemented and verified in firmware:** MakeCode 32-byte string framing
  retrofit (`oldproto.frame`/`unframe`), `receiver` and `station` modes on group
  `146`, persistent `settings.txt`, `!get`/`!set`/`!clear`/`!restart` serial
  control commands (`fw=draft-1`), stripped single-hex builder (`make hex`,
  stripped `make flash`), and the first slice of the managed-flooding mesh
  (`src/mesh.py`, 89 host unit tests passing, 22/22 single-board smoke checks,
  46.7 KB free RAM with full cache + relay queue). Live-verified on the
  classroom V2.00 receiver and station 43 with `trafficlights_webapp`.
- **Still pending in firmware:** multi-board mesh verification (two-board,
  3+ board chain M5, 50-station load M9), live `T,-1,0` broadcast test, logo
  long-press set-up test (T7), maintainer confirmation on dropping the neighbor
  scan (R8) and choosing a project license, and any future webapp-facing serial
  extensions (coordinate with `../trafficlights_firmware` before relying on them
  here).
