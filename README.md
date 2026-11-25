# trafficlights_webapp

Teacher web application for the classroom traffic lights system. It connects over USB serial
to the micro:bit V2 teacher receiver (whose firmware lives in the sibling repository
`../trafficlights_firmware`), displays each table's light status (green, orange or red),
tracks how long tables have been red, and lets the teacher set individual tables or reset all
tables to green.

**Status:** repository and knowledge base initialized around the starting
`fastapi_receiver.py` prototype. Requirements, architecture, packaging, tests and next steps
are waiting on the maintainer (see the Work order in the knowledge base).

## Knowledge base

`docs/kb/README.md` is the working record for this repository: mission, rules, licensing,
standing reference on `fastapi_receiver.py` and the `trafficlights_firmware` serial
interface, ongoing work and history. It is the single source of truth for maintainers and
agents (`AGENTS.md` only points there), and this README stays the short public entry point.

## Daily loop

    make kb        # rewrite the index in docs/kb/README.md from the section files
    make kb-check  # fail if the knowledge-base index is stale or a section breaks the layout rules
    make kb-graph  # refresh graphify-out/ with the code and the knowledge base in it
    make gate      # before a commit: currently runs kb-check (tests and checks will be wired here)

The knowledge-base tooling (`make kb*`) needs `node`; `make kb-graph` also needs `graphify`.
Application packaging (`pyproject.toml`, dependency lockfile and test commands) is not set up
yet; `fastapi_receiver.py` currently documents running via `uvicorn fastapi_receiver:app
--reload` with `fastapi`, `uvicorn`, `pydantic` and `pyserial`.

## Layout

    fastapi_receiver.py  starting single-file FastAPI + WebSocket teacher web application
    docs/kb/             the knowledge base and its housekeeping/graphify scripts
    scripts/kb.sh        knowledge-base index, check and graph runner
    Makefile             gate and knowledge-base targets
    AGENTS.md            pointer-only agent entry point (`CLAUDE.md` and `CODEX.md` link here)
