---
summary: >-
  Available agent tooling on this dev machine (`2026-11-25`): ECC plugin
  (`2.2.3`) review and TDD skills, TokenEconomizer (`~/.tokeneconomizer/bin/te`),
  internal swarm models (Claude Opus/Sonnet 5.5, GPT-OSS 120B; never Gemini),
  external headless Muse Code (`muse exec --yolo --model muse-wonder-full-1.0`)
  and Claude Code (`claude -p`), Inkscape SVG rendering, and headless Brave
  browser verification over DevTools Protocol.
---

# Agent tooling

## Internal swarm models (non-Gemini only)

Available via `swarm` (`action="add"`):

| Model ID | Display name | Best fit |
| :--- | :--- | :--- |
| `claude-opus-5-5-high` / `-medium` | Claude Opus 5.5 | Hard backend concurrency, security review, protocol architecture |
| `claude-sonnet-5-5-high` / `-medium` / `-low` | Claude Sonnet 5.5 | Standard Rust/WASM/React slices, SVG design, UI/integration tests, Docker/CI |
| `gpt-oss-120b-medium` | GPT-OSS 120B | Bounded docs/schemas, focused helpers, cross-model audits |

Gemini models are intentionally excluded from worker dispatch to preserve the
orchestrator's quota (see Multi-agent workflow).

## External headless coding agents

- **Muse Code** (`/usr/bin/muse`): always run with `--model muse-wonder-full-1.0`:
  ```sh
  muse exec --yolo --model muse-wonder-full-1.0 --workspace <worktree> --prompt-file <prompt>
  ```
- **Claude Code** (`/usr/bin/claude` -> `/opt/claude-code/bin/claude`, `2.1.331`):
  ```sh
  claude -p --dangerously-skip-permissions --model <haiku|sonnet|opus> < <prompt>
  ```
  Both run outside the sandbox so they can access provider APIs and `gpg-agent`
  for signed commits.

## ECC and TokenEconomizer

- **ECC (`2.2.3`)** is installed globally under `~/.gemini/config/plugins/ecc`
  and `~/.claude/settings.json`. Applicable practices here: Rust patterns and
  testing (`rust-patterns`, `rust-testing`), React/Vite/frontend patterns and
  accessibility (`react-patterns`, `frontend-patterns`, `vite-patterns`,
  `make-interfaces-feel-better`), security review (`security-review` —
  mandatory on WebSocket Origin, capability tokens, PIN auth, and SSD log
  paths), and E2E/browser verification.
- **TokenEconomizer** (`~/.tokeneconomizer/bin/te`) is installed for both
  `claude` and `antigravity` (`microskills=envfacts+distiller`,
  `toolprune=auto`, `loopguard=nudge`). Check status with
  `~/.tokeneconomizer/bin/te --host claude status`.

## Visual and browser verification

- **SVG rendering:** `inkscape --export-type=png --export-width=<px> -o <out.png> web/public/icon.svg`
  verifies the funny traffic-light SVG icon at favicon (16–64 px) and display
  (256 px) sizes.
- **Headless browser:** following `services_zamkorus`, drive Brave headlessly
  (`brave --headless=new --remote-debugging-port=<port> --user-data-dir=<tmp>`)
  over Chrome DevTools Protocol using Node 26's built-in `WebSocket` (or
  Playwright if installed) to test live WebSocket flows and capture
  screenshots without adding browser npm dependencies.
