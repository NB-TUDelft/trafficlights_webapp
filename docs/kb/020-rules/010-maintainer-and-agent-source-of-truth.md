---
summary: >-
  Names this knowledge base the single source of truth for maintainers and
  agents; `README.md` stays the public entry point; the one agent file,
  `AGENTS.md` (`CLAUDE.md` and `CODEX.md` are symlinks to it), stays
  pointer-only, with no lowercase `agents.md`; `../trafficlights_firmware` is a
  separate sibling repository read for protocol reference and not edited
  unasked.
always_read: true
---

# Maintainer and Agent Source of Truth

This knowledge base is the central maintainer and agent knowledge base for this
repository, started `2026-11-25`. Rules, standing reference, the work order and
the record of finished work belong here, not in commit messages or in anyone's
memory.

Other documents keep their own jobs, and the knowledge base links to them
rather than repeating them:

- `README.md` is the public entry point: what the web application is, and how
  to run and check it. Its developer notes stay short and point here.
- `scripts/kb.sh`, `docs/kb/housekeeping.mjs` and `docs/kb/graphify.mjs`
  explain themselves in their header comments.
- `old/` (once scaffolding starts) holds the original `fastapi_receiver.py` as
  reference-only material outside the Rust/web build and quality gate,
  following `../trafficlights_firmware`'s `old/` convention.
- `../trafficlights_firmware` is the sibling firmware repository. Read its
  knowledge base and `src/` when verifying the USB serial protocol or proposing
  new serial commands, and summarize the interface in "Firmware serial
  interface" (Reference) rather than duplicating firmware internals here. Do not
  edit files in `../trafficlights_firmware` unless the maintainer asks.

There is one agent file, `AGENTS.md`, and it only points here; `CLAUDE.md` and
`CODEX.md` are symlinks to it, so each agent finds it under the name it looks
for. Durable rules never go into it. There is deliberately no lowercase
`agents.md`: on the case-insensitive file systems macOS and Windows use by
default it would collide with `AGENTS.md`.

If anything conflicts with the knowledge base, the knowledge base wins. Raise
the conflict with the maintainer instead of guessing.
