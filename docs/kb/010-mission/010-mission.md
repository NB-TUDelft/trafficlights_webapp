---
summary: >-
  What trafficlights_webapp is (the teacher web application for the classroom
  traffic lights system, sibling to `../trafficlights_firmware`), what the
  starting `fastapi_receiver.py` prototype does, and how it fits alongside the
  old MakeCode receiver and the replacement MicroPython firmware.
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

## Starting point

The repository started on `2026-11-25` with a single file,
`fastapi_receiver.py` (commit `7fe9e60`), a FastAPI + WebSocket teacher console
in the lineage of `NB-TUDelft/classroom_trafficlights` (a fork of
`RolfHut/ClassroomIndicatorLights`). What it currently does and the quirks
observed in its code are documented in "The existing FastAPI receiver"
(Reference). Specific requirements and direction for `trafficlights_webapp` are
waiting on the maintainer (Requirements, Work order).

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
