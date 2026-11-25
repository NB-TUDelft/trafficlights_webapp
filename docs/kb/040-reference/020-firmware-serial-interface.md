---
summary: >-
  The USB serial contract with `../trafficlights_firmware` (read at firmware
  commit `61786da` on `2026-11-25`): 115200 baud micro:bit V2 CDC serial,
  buffer and line-ending limits, the legacy `T`/`S`/`RS`/`RT` text protocol,
  the always-listening `!get`/`!set`/`!clear`/`!restart` control commands,
  compatibility guarantees, and what the firmware has not built yet.
always_read: true
---

# Firmware serial interface

Summary of the USB serial contract established in the sibling repository
`../trafficlights_firmware` (read from its `src/` modules and knowledge base at
commit `61786da` on `2026-11-25`). For full firmware internals, read
`../trafficlights_firmware/docs/kb/README.md`.

## Transport and port constraints

- **Hardware:** micro:bit V2.21 (`USB VID 0x0D28`, PID `0x0204`, NXP ARM mbed
  DAPLink CDC), running MicroPython `v2.1.2`.
- **Serial settings:** `115200` baud, 8N1, UTF-8 text lines. On Linux the
  device appears as `/dev/ttyACM*` (and
  `/dev/serial/by-id/usb-Arm_BBC_micro:bit_CMSIS-DAP_*`); on Arch/CachyOS the
  user must be in the `uucp` group.
- **Exclusive access:** only one process may hold the port open at a time.
  Close any serial monitor or `trafficlights_firmware` helper (`make flash`,
  `make repl`, `scripts/mb.py`) before connecting the web application, and vice
  versa.
- **Buffer and line limits (`src/serialio.py`, firmware test T4):**
  - The micro:bit UART receive buffer holds about **128 bytes**; lines longer
    than **`MAX_LINE = 64` bytes** or containing invalid UTF-8 are silently
    dropped.
  - Single 7–10 byte teacher commands fit comfortably, while bursts of lines
    can overflow the receive buffer unless paced or acknowledged per line.
  - Line endings accepted: `LF` (`\n`), `CR` (`\r`), `CR LF` (`\r\n`), or
    **500 ms of quiet** (`IDLE_MS = 500`). The web application should always
    terminate every outgoing line with `\n`.
  - **Never send Ctrl-C (`0x03`) during normal operation:** `serialio.read_lines`
    deliberately raises `KeyboardInterrupt` on `0x03` so firmware dev tools can
    stop `main.py` and enter the REPL.

## Legacy classroom protocol (old and new receivers)

Both the old MakeCode receiver and `trafficlights_firmware` receiver mode
(`src/receiver.py`, `src/oldproto.py`) speak three comma-separated fields on
radio group `146`, with colors `0` = green, `1` = orange, `2` = red:

| Direction | Line | Meaning |
| :--- | :--- | :--- |
| Webapp -> Receiver | `T,<id>,<color>\n` | Set station `<id>` to `<color>` (`0`, `1` or `2`) |
| Webapp -> Receiver | `T,-1,0\n` | Broadcast reset: set every station to green (`0`) |
| Webapp -> Receiver | `\n` (blank line) | Sent by `fastapi_receiver.py` on connect; accepted and ignored by the new receiver |
| Receiver -> Webapp | `S,<id>,<color>` | Status broadcast from a station (on button change and every 4–6 s) |
| Receiver -> Webapp | `RS,<id>,<color>` | `S` packet relayed once by an old MakeCode repeater |
| Receiver -> Webapp | `RT,<id>,<color>` | `T` packet relayed once by an old MakeCode repeater (echo; ignored by teacher apps) |

Important differences in the new firmware draft (`src/receiver.py`):

- The old MakeCode receiver forwarded any serial line blindly onto the radio.
  The new draft forwards only well-formed `T,<table>,<color>` commands so
  garbage cannot crash old MakeCode stations (which parse without bounds
  checks).
- Neither old nor new stations send an immediate `S` acknowledgement when a
  `T` command changes their color; confirmation arrives on the station's next
  4–6 s heartbeat.

## Serial control commands (`src/control.py`, firmware R13)

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

Key rules for the web application:

- **Safe coexistence:** `!` lines are handled locally by the board and never
  transmitted over radio. Replies (`!ok ...`, `!err ...`) contain **no commas**,
  so a legacy 3-field comma parser (`fastapi_receiver.py`'s `_parse_message`)
  drops them harmlessly.
- **ID rules (firmware R14):** a receiver has no ID and rejects `id=`; valid
  station IDs are `1` to `9999`; ID `0` is reserved for the receiver and
  refused by `!set`; an unset station reports `id=0`.
- **Command timing:** a board does not read UART while scrolling its ID at
  start-up (~1 s), waiting out the 100 ms `A+B` button window, or inside the
  logo setup UI. Automated callers should wait for `!ok`/`!err` with a timeout
  and retry if no reply arrives.
- **USB only, no radio:** control commands act only on the locally attached
  board, never remotely over radio.

## Firmware status and unbuilt areas

As of `2026-11-25` (`trafficlights_firmware` commit `61786da`):

- Implemented in firmware draft: `receiver` and `station` modes on the legacy
  text protocol (group 146), persistent `settings.txt`, logo setup UI, and the
  `!get`/`!set`/`!clear`/`!restart` serial control commands (`fw=draft-1`).
- Not yet implemented in firmware: mesh forwarding and loop prevention, binary
  radio framing, initial neighbor scan, and any additional webapp-specific
  serial commands. Coordinate any new serial commands with
  `../trafficlights_firmware` before relying on them here.
