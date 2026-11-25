---
summary: >-
  Record (`2026-11-25`): git repository initialized on `main` with the
  untouched `fastapi_receiver.py` (`7fe9e60`), repository entry points and
  knowledge-base tooling copied from `../trafficlights_firmware`, and the
  initial seven-group knowledge base created and verified with `make gate`.
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

- `scripts/kb.sh index` (`make kb`) generated `docs/kb/README.md` and
  `make gate` (`scripts/kb.sh check`) passed.
- Application code (`fastapi_receiver.py`) was read and inspected statically
  only; no Python virtual environment, server run, browser test or micro:bit
  serial test was executed yet, awaiting the maintainer's next steps.
