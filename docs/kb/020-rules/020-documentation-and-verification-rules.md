---
summary: >-
  Standing rules: run `make gate` before committing (currently the
  knowledge-base check; type checks, linting and tests will be wired in once
  the stack is chosen); fix touched-file issues; never weaken a check; American
  spelling; small commits; record the checks run and any known gaps; keep this
  knowledge base current while working.
always_read: true
---

# Documentation and Verification Rules

- Keep `README.md` as the entry point and link deeper docs from it.
- Run the gate on the finished working tree before committing: `make gate`.
  At initialization (`2026-11-25`), `make gate` runs `scripts/kb.sh check`,
  which fails while the knowledge base's index is stale or a section breaks the
  layout rules. Once Python and/or frontend tooling, type checks, linters and
  automated tests are settled (Work order), wire them into `make gate` and
  update this section.
- When changing serial-handling code or browser UI, record what was tested
  (unit/integration tests, browser checks, or a real micro:bit receiver over
  USB serial) and what was not tested. Keep in mind that only one program can
  open a micro:bit's `/dev/ttyACM*` port at a time.
- While touching a file, fix the existing issues in it that are reasonable to
  fix in the same change. Keep unrelated whole-tree cleanup in a change of its
  own.
- Never weaken a check, a test or an analysis rule to make a change pass.
- Write American spelling in code, documents and messages (`color`, `behavior`,
  `license`, `neighbor`, `recognize`, `organize`), following the maintainer's
  preference recorded in `trafficlights_firmware` on `2026-11-25`. Quotations
  of the maintainer's words stay as written, and the knowledge-base tooling
  scripts copied byte for byte stay as they came. The initial import of
  `fastapi_receiver.py` was left untouched (`7fe9e60`) and still has one
  British `colour` comment on line 537.
- Bulk edits over tracked Markdown files must skip symlinks (`CLAUDE.md` and
  `CODEX.md` are symlinks to `AGENTS.md`).
- Prefer small commits, one concern each.
- Before calling work complete, record the exact checks run and any known gaps
  in the commit message or the handoff note.
- Keep this knowledge base current while you work: update the work order as
  items start and finish, and write down what the next person would otherwise
  have to rediscover, so an interrupted task can be picked up from here.
