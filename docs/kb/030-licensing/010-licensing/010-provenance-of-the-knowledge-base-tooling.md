---
summary: >-
  `docs/kb/housekeeping.mjs`, `docs/kb/graphify.mjs` and `scripts/kb.sh` were
  copied byte for byte from `../trafficlights_firmware` on `2026-11-25` at the
  maintainer's request; they are the maintainer's own AGPL-3.0-or-later tooling
  originally from Git Graph Libre, xrdp-kwin and ImProc.
---

# Provenance of the knowledge-base tooling

## Origin

- `docs/kb/housekeeping.mjs`, `docs/kb/graphify.mjs` and `scripts/kb.sh` were
  copied byte for byte from `../trafficlights_firmware` on `2026-11-25`
  (verified by SHA-256) when initializing a matching knowledge base here.
- `trafficlights_firmware`'s provenance record notes that those three files
  came from ImProc (`/home/zam/Desktop/prog/ImProc`), authored solely by the
  maintainer in Git Graph Libre and adapted in xrdp-kwin under
  `SPDX-License-Identifier: AGPL-3.0-or-later` to standardize active
  repositories on one knowledge-base layout and one graphify edge linker.
- The section layout, rules and pointer-only `AGENTS.md` (with `CLAUDE.md` and
  `CODEX.md` symlinked to it, and no lowercase `agents.md`) follow
  `trafficlights_firmware` and those repositories.

## Differences from ImProc

- `scripts/kb.sh` carries `trafficlights_firmware`'s one change: `cd
  "$(dirname "$0")/.."` instead of `git rev-parse --show-toplevel`, which
  resolves the repository root with or without git.
- `docs/kb/graphify.mjs` keeps ImProc's `CODE_PATH`, which lists extensions
  beyond what this repository uses (`zig`, `c`, `h`, etc.) alongside `py`,
  `js`, `ts`, `html`, `css`, `json`, `toml`, `sh` and `md`; they are harmless
  and kept so the file remains identical.

Keep these three files in step with `../trafficlights_firmware` and the
maintainer's other repositories rather than making one-off changes here.
