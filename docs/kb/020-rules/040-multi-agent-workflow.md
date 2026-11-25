---
summary: >-
  Standing rules for multi-agent work in trafficlights_webapp, adapted from Git
  Graph Libre (`040-multi-agent-releases.md`) and `services_zamkorus`
  (`030-agent-protocol.md`): respectful coworker culture, model routing by
  complexity (Claude, GPT-OSS, headless Muse Code always on
  `muse-wonder-full-1.0`, headless Claude Code; never Gemini subagents),
  worktrees under `../.tw-wt/` (never `/tmp`), hard safety rules, small
  GPG-signed commits, rebase/merge integration (never squash-merge), branch
  cleanup with `git branch -d`, and facts-only slice reports.
always_read: true
---

# Multi-agent workflow

Adapted from Git Graph Libre (`docs/kb/020-rules/040-multi-agent-releases.md`)
and `services_zamkorus` (`docs/kb/020-rules/.../030-agent-protocol.md`) at the
maintainer's instruction (`2026-11-25`). Where this section conflicts with
another rule in this knowledge base, stop and report the conflict.

## Coworker culture

Treat every internal swarm worker and external headless agent as a respected
coworker, never as a disposable tool:

- Give clear, complete briefs with full context, ownership boundaries, and
  explicit "done" criteria so coworkers do not have to guess or churn.
- Invite judgment on edge cases within their slice, acknowledge their findings
  and reports, and credit their contributions cleanly in slice reports and
  commit trailers (`Co-Authored-By: ...`).

## Agent and model routing (`2026-11-25`)

- **Never spawn Gemini agents or subagents.** The orchestrator runs on Gemini;
  spawning Gemini subagents drains the shared quota and risks stalling the
  whole workflow.
- **Adjust model to task hardness and complexity** (cheapest capable model
  first, escalate one step only if blocked):
  - **Hard architecture, Rust concurrency/security, deep review:** internal
    `claude-opus-5-5-high` / `-medium`, external headless **Claude Code**
    (`claude -p --model opus`), or external headless **Muse Code**
    (`muse exec --yolo --model muse-wonder-full-1.0`).
  - **Standard Rust/WASM/React/TypeScript implementation and Docker/CI:**
    internal `claude-sonnet-5-5-high` / `-medium` / `-low`, external headless
    **Claude Code** (`claude -p --model sonnet`), or external headless **Muse
    Code** (`muse exec --yolo --model muse-wonder-full-1.0`).
  - **Bounded mechanical work, docs/schemas, focused helpers, or cross-model
    second-opinion audit:** internal `gpt-oss-120b-medium` or external headless
    **Claude Code** (`claude -p --model haiku`).
- **Muse Code model rule (mandatory):** whenever invoking Muse Code, always
  pass `--model muse-wonder-full-1.0` (never another Muse model).
- **Use ECC and TokenEconomizer:** leverage ECC domain review checklists and
  let TokenEconomizer (`~/.tokeneconomizer/bin/te`) track sessions and keep
  context lean.

## Workspaces and worktrees

- External headless agents work in dedicated git worktrees placed beside this
  repository under `/home/zam/Desktop/prog/_TA/trafficlights/.tw-wt/<slice>`
  (never under `/tmp`, which a reboot empties).
- Internal swarm workers that edit code run with `workspace: "share"` (their
  own working copy) and readers/reviewers run with `workspace: "inherit"`.
- A slice coworker works only inside its assigned workspace and file ownership
  list, and never touches another coworker's worktree or the main checkout.

## Hard rules (every coworker and integrator)

1. **Never push, open a pull request, or comment on an issue** unless the
   maintainer explicitly asked for that action.
2. **Never commit without a GPG signature** (`4858F11A2D1DFCBE`). Never pass
   `--no-gpg-sign`, `-c commit.gpgsign=false`, or `--no-verify`. If a sandboxed
   worker cannot reach `gpg-agent`, it leaves its verified edits cleanly in its
   worktree and hands off to the integrator to verify and create signed
   commits.
3. **Stay inside the slice's file ownership list** (plus its tests and its
   slice report).
4. **Never weaken a check to pass it:** no skipped/loosened tests, no
   `#[allow(...)]`, `biome-ignore`, `@ts-expect-error`, or `any` suppressions,
   and no relaxed compiler/linter flags.
5. **No unapproved dependencies** — every new crate or npm package must pass
   the AGPL-3.0-or-later compatibility table in Licensing.
6. **No destructive git commands** (`reset --hard`, `clean -fdx`,
   `push --force`, `branch -D`).
7. **Tests touch only temporary directories**, never real host devices or
   persistent user directories.
8. **No secrets** in code, logs, tests, fixtures, reports, or commits.
9. **Never bypass creator authorization or WebSocket `Origin` checks.**
10. **SPDX and American spelling:** every project source file starts with
    `SPDX-License-Identifier: AGPL-3.0-or-later` and uses American spelling.
11. **Report facts only:** never claim a check that was not actually run.

## Integration rules (integrator only)

- Integrate finished slices onto `main` in dependency order by **rebase +
  fast-forward** (or a single signed merge commit if too many commits conflict)
  — **never squash-merge** (`git merge --squash` is forbidden so per-commit
  history and GPG signatures survive).
- Verify every commit shows `G` in `git log --format='%h %G? %s'` (or verify
  the PGP signature header in `git cat-file -p`), verify zero conflict markers,
  and run the full `make gate` on the integrated result.
- Delete every integrated local slice branch with `git branch -d <branch>`
  (never `-D`) once its worktree is removed or recycled.
- Record every integrated slice, coworker/model, commit hash, and gate counts
  in the active execution register under Ongoing work.
