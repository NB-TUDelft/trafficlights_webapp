---
summary: >-
  Standing rules: run `make gate` before committing (KB check, WASM build,
  Biome CI + strict lint + TypeScript + Vitest + Vite build, Cargo fmt +
  clippy `-D warnings` + workspace tests); fix touched-file issues; never
  weaken a check; American spelling; small commits; record the checks run and
  any known gaps; keep this knowledge base current while working.
always_read: true
---

# Documentation and Verification Rules

- Keep `README.md` as the entry point and link deeper docs from it.
- Run the gate on the finished working tree before committing: `make gate`.
  `make gate` runs `scripts/kb.sh check`, builds the browser WASM module
  (`scripts/build-wasm.sh`), runs the frontend checks (`nub --cwd web run ci`,
  `lint:strict`, `test`, `build`), and runs the Rust workspace checks
  (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`).
- Run `make sonar` (`scripts/sonar.sh scan`) after larger slices to generate
  fresh Vitest (`web/coverage/lcov.info`) and Rust (`coverage/rust-lcov.info`)
  coverage reports, upload the scan using `sonar-project.properties`, and poll
  the SonarQube quality gate until it returns `OK`. Server URL and token live
  only in `/opt/sonar-scanner/conf/sonar-scanner.properties` or environment
  variables, never in this repository.
- Develop locally with `make dev` (`scripts/dev.sh`, watch mode without
  Docker): open `http://127.0.0.1:5173`, where Vite updates the frontend on
  save; the native Rust server on `127.0.0.1:3000` restarts when `crates/`
  change (polled each second, no watcher tool needed), rebuilding the browser
  WASM module first when `crates/proto` or `crates/wasm` change. A restart
  ends in-memory sessions. Preview the production image with `make dev-up`
  (`docker compose -f compose.dev.yaml up --build` on
  `http://127.0.0.1:3000`, stopped with `make dev-down`).
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
