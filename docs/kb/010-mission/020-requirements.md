---
summary: >-
  Requirements for trafficlights_webapp: the maintainer has not yet given
  application-specific requirements (waiting on next steps as of `2026-11-25`);
  records only the cross-repository constraints already established in
  `trafficlights_firmware` and lists what remains open to decide.
always_read: true
---

# Requirements

**Status (`2026-11-25`):** The maintainer has not yet stated the requirements
for `trafficlights_webapp`. On `2026-11-25` they asked to familiarize with
`trafficlights_firmware` and its knowledge base, initialize a matching knowledge
base and git repository here, and come back for next steps. Record the
maintainer's words here verbatim first once given, followed by the structured
reading, as `trafficlights_firmware` does.

## Already established from `trafficlights_firmware`

These facts come from the maintainer's confirmed requirements in
`../trafficlights_firmware/docs/kb/010-mission/020-requirements.md` (`2026-11-24`
and `2026-11-25`) and constrain how the web application and firmware meet:

- **Old serial protocol compatibility (firmware R7).** The receiver speaks
  115200 baud USB serial with newline-delimited lines: teacher commands
  `T,<table>,<color>` (colors `0` green, `1` orange, `2` red) and `T,-1,0`
  (reset all tables to green), and incoming status/relay lines
  `<role>,<table>,<color>`. Blank lines (such as the wake-up newline
  `fastapi_receiver.py` writes on connect) are accepted and ignored by the new
  receiver.
- **Extensibility for a new teacher application (firmware R7).** New serial
  commands may be added to the firmware for the alternative application.
- **Always-listening serial control commands (firmware R13).** Both station and
  receiver modes listen on USB serial for `!get`, `!set mode=... id=...`,
  `!clear`, and `!restart`, answering `!ok ...` or `!err ...` with no commas so
  that old three-field comma parsers ignore them.
- **ID reservation (firmware R14).** The receiver has no ID; station ID `0` is
  reserved for the receiver (if the mesh later needs an address) and cannot be
  assigned to a station. Valid assigned station IDs are `1` to `9999`; an unset
  station reports `id=0`.
- **Target hardware (firmware R11).** All classroom boards are micro:bit V2.21.

## Not yet said

Everything specific to `trafficlights_webapp` is open and tracked in Work order:

- whether to evolve `fastapi_receiver.py` or restructure into a multi-file
  backend and separate frontend (and which frontend/backend stack to use);
- new features, classroom workflows, layout, accessibility, sound/alert
  behavior, and offline asset requirements (e.g. removing the Google Fonts
  network import);
- whether the web application will expose board setup/provisioning over the
  `!get`/`!set`/`!clear`/`!restart` control commands or request new firmware
  serial commands;
- how existing defects in `fastapi_receiver.py` should be handled;
- packaging, dependency management (`uv`/`pyproject.toml`, Node if needed),
  linting, static typing, and automated test expectations;
- deployment model (local single-teacher run, multi-client classroom view,
  authentication/network binding);
- license (Licensing), git remote, and branch/merge/commit conventions (Commits
  and branches).
