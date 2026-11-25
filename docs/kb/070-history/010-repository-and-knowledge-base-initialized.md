---
summary: >-
  Record (`2026-11-25`): git repository initialized on `main` with the
  untouched `fastapi_receiver.py` (`7fe9e60`), repository entry points and
  knowledge-base tooling copied from `../trafficlights_firmware`, the initial
  seven-group knowledge base created (including standing Reference for
  `fastapi_receiver.py` and the `trafficlights_firmware` serial interface), and
  verified with `make gate` and `make kb-graph`.
---

# Repository and knowledge base initialized (`2026-11-25`)

## Git repository

Initialized a local git repository on `main` at the maintainer's request, with
no remote configured. The pre-existing `fastapi_receiver.py` was committed
untouched as root commit `7fe9e60` (signed with the maintainer's GPG key) so
the baseline stays cleanly separated from all follow-up changes.

## Scaffolding and knowledge base

Created the same layout used in `../trafficlights_firmware`:

- `.gitignore` ignoring local virtual environments, Python bytecode,
  `node_modules/`, build outputs, `graphify-out/` and
  `/.claude/settings.local.json`.
- `README.md` as the short public entry point, pointer-only `AGENTS.md` with
  `CLAUDE.md` and `CODEX.md` symlinked to it (and no lowercase `agents.md`),
  and `Makefile` exposing `kb`, `kb-check`, `kb-graph` and `gate`.
- `scripts/kb.sh`, `docs/kb/housekeeping.mjs` and `docs/kb/graphify.mjs`
  copied byte for byte from `../trafficlights_firmware` (verified by SHA-256;
  see Provenance of the knowledge-base tooling).
- Seven-group `docs/kb/` structure (Mission, Rules, Licensing, Reference,
  Ongoing work, Future work, History) using American spelling throughout,
  recording only established facts and leaving application requirements,
  stack, licensing and git workflow style explicitly open for the maintainer.

## Checks run and left open

- `scripts/kb.sh index` (`make kb`) generated `docs/kb/README.md` (7 groups,
  10 always-read sections) and `make gate` (`scripts/kb.sh check`) passed.
- `make kb-graph` built the offline code and knowledge-base graph
  (`graphify-out/`, gitignored): 158 nodes, 287 edges (including 20 `contains`
  and 44 `mentions` edges from the knowledge base), 16 communities.
- Standing Reference sections were added for `fastapi_receiver.py` (read in
  full, 1,303 lines) and the `../trafficlights_firmware` USB serial interface
  (read at firmware commit `61786da`).
- Application code (`fastapi_receiver.py`) was read and inspected statically
  only; no Python virtual environment, server run, browser test or micro:bit
  serial test was executed yet, awaiting the maintainer's next steps.
