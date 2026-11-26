---
summary: >-
  Names this knowledge base the single source of truth for maintainers and
  agents; `README.md` stays the public entry point; the one agent file,
  `AGENTS.md` (`CLAUDE.md` and `CODEX.md` are symlinks to it), stays
  pointer-only, with no lowercase `agents.md`; `../trafficlights_firmware` and
  `../trafficlights_analytics` are separate sibling repositories read for
  protocol/schema compatibility and not edited unasked.
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
- `old/` holds the original `fastapi_receiver.py` as reference-only material
  outside the Rust/web build and quality gate, following
  `../trafficlights_firmware`'s `old/` convention.
- `../trafficlights_firmware`
  ([NB-TUDelft/trafficlights_firmware](https://github.com/NB-TUDelft/trafficlights_firmware))
  is the sibling firmware repository. Read its knowledge base
  (`../trafficlights_firmware/docs/kb/README.md`) and `src/` when verifying the
  USB serial protocol, retrofit behavior, or proposing new serial commands, and
  summarize the contract in "Firmware serial interface" (Reference) rather than
  duplicating firmware internals here.
- `../trafficlights_analytics`
  ([NB-TUDelft/trafficlights_analytics](https://github.com/NB-TUDelft/trafficlights_analytics))
  is the sibling offline analytics and plotting repository consuming exported
  `.jsonl` logs. Coordinate any changes to `crates/proto/src/stats.rs` or
  "Classroom statistics JSONL schema" (Reference) with it.
- Do not edit files in `../trafficlights_firmware` or
  `../trafficlights_analytics` unless the maintainer explicitly asks.

There is one agent file, `AGENTS.md`, and it only points here; `CLAUDE.md` and
`CODEX.md` are symlinks to it, so each agent finds it under the name it looks
for. Durable rules never go into it. There is deliberately no lowercase
`agents.md`: on the case-insensitive file systems macOS and Windows use by
default it would collide with `AGENTS.md`.

If anything conflicts with the knowledge base, the knowledge base wins. Raise
the conflict with the maintainer instead of guessing.
