---
summary: >-
  What the starting `fastapi_receiver.py` (1,303 lines, commit `7fe9e60`) does,
  read from its code on `2026-11-25` (not yet executed): runtime settings,
  `TableState`, `WebsocketManager`, `SerialReceiver` (115200 baud, auto-detect,
  wake-up newline, `T,<table>,<color>` / `T,-1,0`, three-field parser ignoring
  `RT`), HTTP/WebSocket endpoints, embedded HTML/CSS/JS teacher console, and
  defects or quirks observed in the baseline code.
always_read: true
---

# The existing FastAPI receiver

Everything here was read from `fastapi_receiver.py` (1,303 lines, root commit
`7fe9e60`) on `2026-11-25`. The application has not been executed in this
repository yet.

## Runtime and environment

`fastapi_receiver.py` is a single-file FastAPI application with an inline HTML,
CSS and JavaScript frontend (`HTML_PAGE`). It imports `fastapi`, `pydantic`,
`pyserial` (`serial` and `serial.tools.list_ports`), and `uvicorn` (in
`__main__`). No `pyproject.toml` or dependency lockfile exists yet.

| Variable | Default | Purpose |
| :--- | :--- | :--- |
| `SERIAL_PORT` | unset (auto-detect) | Explicit serial device path (e.g. `/dev/ttyACM0` or `COM5`) |
| `TABLE_RANGE` | `"1-60"` | Inclusive table ID range `"start-end"` (falls back to `1-60` on parse error) |
| `BAUD_RATE` | `115200` | Serial port baud rate |
| `API_HOST` | `"0.0.0.0"` | Bind host when run directly via `python fastapi_receiver.py` |
| `API_PORT` | `8000` | Bind port when run directly via `python fastapi_receiver.py` |

## Backend architecture

- **`TableState`** (lines 909–992): thread-safe (`threading.Lock`) in-memory
  store mapping table IDs to `"green"`, `"orange"` or `"red"` plus red start
  timestamps (`_red_started`). Builds `snapshot` and `table_update` payloads
  including color counts (`green`, `orange`, `red`), the configured `range`,
  and `redDurations` (up to 10 tables currently on red, sorted longest first in
  whole seconds).
- **`WebsocketManager`** (lines 995–1020): holds active WebSocket connections
  under an `asyncio.Lock` and broadcasts JSON messages, removing stale sockets
  that raise on `send_json`.
- **`SerialReceiver`** (lines 1022–1181): runs a daemon thread (`_worker`)
  started and stopped by FastAPI's `lifespan` handler:
  - **Port discovery (`_resolve_port`)**: uses `SERIAL_PORT` if configured,
    otherwise picks the first port from `serial.tools.list_ports.comports()`
    whose description contains `"microbit"`, `"mbed"` or `"cdc"`
    (case-insensitive). Retries every `RETRY_SECONDS = 3` seconds.
  - **Wake-up on connect**: writes `b"\n"` immediately after opening the port.
  - **Outgoing commands**:
    - `send_teacher_command(table, color)` writes `T,{table},{color_id}\n`
      (`0` green, `1` orange, `2` red).
    - `reset_all()` writes `T,-1,0\n`.
    - Both raise `RuntimeError("Serial link is not connected.")` when no port
      is open.
  - **Incoming parsing (`_parse_message`)**: reads newline-terminated lines,
    splits on `","`, ignores any line that does not have exactly 3 fields,
    ignores `role == "RT"` (repeater echo of teacher commands), parses
    `int(table)` and `int(color_index)`, maps `0`/`1`/`2` to
    `green`/`orange`/`red`, updates `TableState`, and schedules a WebSocket
    broadcast onto the FastAPI event loop via
    `asyncio.run_coroutine_threadsafe`.

## HTTP and WebSocket API

| Endpoint | Request | Behavior / Response |
| :--- | :--- | :--- |
| `GET /` | — | Returns `HTML_PAGE` (`HTMLResponse`) |
| `GET /api/status` | — | Returns `table_state.snapshot()` |
| `POST /api/table/{table_id}` | `{"color": "green"\|"orange"\|"red"}` | Updates `table_state`, sends `T,{table_id},{color_id}\n` over serial (HTTP 503 if disconnected), broadcasts `table_update`, returns update |
| `POST /api/reset` | — | Resets `table_state` to green, sends `T,-1,0\n` over serial (HTTP 503 if disconnected), broadcasts `snapshot`, returns snapshot |
| `POST /api/table-range` | `{"start": int >= 1, "end": int >= start}` | Rebuilds `table_state` for `start..end` all green, broadcasts `snapshot`, returns snapshot (does not send anything over serial) |
| `GET /api/serial` | — | Returns `configured_port`, `connected_port`, `available_ports` |
| `POST /api/serial` | `{"port": str \| null}` | Sets configured port (`null` for auto-detect), closes any active connection to trigger reconnect, returns `serial_status()` |
| `WS /ws` | read-only client | Sends initial `snapshot`, then pushes `snapshot` and `table_update` JSON messages; ignores incoming client text |

## Embedded frontend (`HTML_PAGE`)

- Dark-themed responsive console loading `Space Grotesk` from Google Fonts with
  fallback to `Trebuchet MS, sans-serif`.
- Header shows live **Orange** and **Red** counts and a WebSocket status chip
  (`Connecting...`, `Live`, `Reconnecting...`) that also opens the serial
  modal on click.
- Control cards:
  - **Serial Port**: shows active/configured port and opens a `<dialog>` modal
    to pick a port or auto-detect; auto-opens on first load when disconnected.
  - **Table Range**: Start/End inputs calling `POST /api/table-range`.
  - **Quick Actions**: `Reset All To Green` (`POST /api/reset`) and global
    `Beeps On` / `Beeps Off` toggle (Web Audio API two-tone square wave at
    880 Hz then 660 Hz when a table enters `orange` or `red` from `green`).
  - **Longest On Red**: ordered list of up to 10 red tables with elapsed
    seconds.
- Table grid: one card per table showing table number, a shape-based SVG status
  icon (green checkmark, orange exclamation mark, red skull and crossbones) for
  color-blind readability, and a per-table mute button. Clicking a card cycles
  `green -> orange -> red -> green` via `POST /api/table/{id}`.

## Defects and quirks observed in the baseline

These are observations from reading `fastapi_receiver.py`, left untouched in
commit `7fe9e60`:

1. **Corrupted duplicate HTML header (lines 35–65).** `HTML_PAGE` starts with a
   truncated `<html lang=\"en\">` / `<head>` / `<style>` block containing
   literal backslash-escaped quotes and an unclosed `main {` rule, immediately
   followed by the complete `<html lang="en">` document at line 66.
2. **State mutated before serial check (lines 1241–1260).** Both
   `api_update_table` and `api_reset` update `table_state` before calling
   `serial_receiver.send_teacher_command` or `reset_all`. When the serial link
   is disconnected, the endpoint returns HTTP 503 while leaving in-memory state
   already mutated (and unbroadcast to other WebSocket clients).
3. **Pydantic v1 validator API (lines 26, 1198).** `TableRangeRequest` uses
   `@root_validator(skip_on_failure=True)`, which is deprecated in Pydantic v2
   and removed in newer releases.
4. **Broad incoming packet acceptance and dynamic out-of-range tables.** Unlike
   the Tkinter `teacherApplication.py` (which drops out-of-range table IDs),
   `TableState.update_table` inserts any received table ID into `_colors`
   (including `0` or negative values if ever received), and `_parse_message`
   accepts any 3-field role other than `RT` (not strictly `S` and `RS`).
5. **Red durations do not tick between events.** `redDurations` is only
   recomputed on the server when a snapshot or table update occurs, so displayed
   seconds go stale while classroom lights are idle.
6. **Inverted per-table mute tooltip (lines 526, 571).** `btn.title` reads
   `'Beep enabled for this table'` when `isMuted` is true and `'Beep disabled
   for this table'` when false.
7. **External font dependency and spelling.** `@import` fetches Google Fonts
   from the internet (slow or blocked on offline classroom networks), and line
   537 uses British `colour` in a JS comment.
8. **No packaging or automated tests.** There is no `pyproject.toml`, test
   suite, linter or static type check yet.
