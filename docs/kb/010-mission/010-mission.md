---
summary: >-
  What trafficlights_webapp is (the remote Rust-backed teacher web application
  for the classroom traffic lights system, sibling to
  `../trafficlights_firmware`), how multi-session Web Serial streaming from the
  session creator's browser replaces the local `fastapi_receiver.py` prototype,
  and how it relates to `trafficlights_firmware` and the `services_zamkorus`
  reference stack.
always_read: true
---

# Mission

trafficlights_webapp is the teacher web application for the classroom traffic
lights system. Students set a micro:bit NeoPixel light at their table to green,
orange or red to signal whether they are OK, have a question or need help; the
lights report their color over radio to a receiver micro:bit attached to the
teacher's computer over USB serial; and this web application presents table
statuses, tracks which tables have been red the longest, and lets the teacher
change individual tables or reset all tables to green. `README.md` is the short
public entry point; this knowledge base is the working record.

This repository is the web-application counterpart to the sibling firmware
repository `../trafficlights_firmware`, which replaces the old MakeCode student
light and teacher receiver programs with one MicroPython V2 firmware (station
and receiver modes, planned mesh forwarding, and USB serial control commands).
In `trafficlights_firmware`'s Requirements (R7), the maintainer noted that all
old teacher-application serial commands must keep working and that new commands
may be added because they are creating an alternative teacher application.

## Starting point and target shape

The repository started on `2026-11-25` with a single file,
`fastapi_receiver.py` (commit `7fe9e60`), a local single-session FastAPI +
WebSocket teacher console authored by the maintainer to operate on the
classroom traffic-light micro:bit hardware and legacy serial protocol
originally introduced by
[RolfHut/ClassroomIndicatorLights](https://github.com/RolfHut/ClassroomIndicatorLights).
What it currently does and the quirks observed in its code are documented in
"The existing FastAPI receiver" (Reference).

On `2026-11-25` the maintainer gave the project requirements (Requirements):
keep the same frontend look and behavior as `fastapi_receiver.py`, move the
backend onto a remote **Rust** server deployed on **Coolify** (following
`/home/zam/Desktop/prog/services_zamkorus`), list named sessions and a short
description on the homepage, stream the USB serial receiver from the session
creator's browser via an "Add session" flow, restrict serial settings and batch
operations (like "Reset All To Green") to the session creator while normal
clients use the console as before, and create a custom funny traffic-light SVG
icon.

## Relationship to the sibling firmware repository

- `../trafficlights_firmware` owns the micro:bit V2 firmware, its hex builder,
  and its own knowledge base (`../trafficlights_firmware/docs/kb/README.md`).
- The wire contract between the two repositories is the USB serial protocol at
  115200 baud (documented in "Firmware serial interface" under Reference): both
  the old comma-separated protocol (`T,<table>,<color>`, `T,-1,0`, incoming
  `<role>,<table>,<color>`) and the new firmware's `!`-prefixed serial control
  commands (`!get`, `!set`, `!clear`, `!restart`).
- Do not modify files in `../trafficlights_firmware` from a `trafficlights_webapp`
  task unless the maintainer explicitly asks.
