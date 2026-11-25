---
summary: >-
  Standing rules: git repository initialized on `2026-11-25` on `main` (root
  commit `7fe9e60`, signed, no remote yet) at the maintainer's request; commits
  and tags are signed and signing is never bypassed; agents may commit in small
  steps naming checks run, and must not push or add a remote unasked; branch,
  merge and commit-message style remain open for the maintainer to settle,
  following short imperative subjects with no type prefix meanwhile.
always_read: true
---

# Commits and branches

- **The repository.** Initialized on `2026-11-25` with `git init` on `main` at
  the maintainer's request, starting with the untouched `fastapi_receiver.py`
  as root commit `7fe9e60` (signed). There is no remote. Adding a remote and
  pushing stay the maintainer's to ask for.
- **Signing.** The maintainer's global git configuration sets
  `commit.gpgsign = true`, `tag.gpgsign = true` and a signing key
  (`4858F11A2D1DFCBE`, checked `2026-11-25`). Commits and tags in this
  repository are signed too. Never pass `--no-gpg-sign` or
  `-c commit.gpgsign=false`, and never switch signing off; if signing fails,
  stop and tell the maintainer. Check with
  `git log --format='%h %G? %an %s'`.
- **When to commit.** The maintainer asked on `2026-11-25` to initialize the
  git repository and make commits. Commit in small steps, one concern each,
  run `make gate` first, and name the checks run in the commit message. Do not
  push or add a remote unless asked.
- **Open: branches, merging and message style.** As recorded in
  `trafficlights_firmware`, the maintainer's other repositories differ between
  short imperative subjects with no type prefix and Conventional Commits with a
  `Co-Authored-By` trailer, and between topic branches and linear `main`. Which
  applies here is for the maintainer to choose; record the choice in this
  section. Commits so far follow `trafficlights_firmware`: short imperative
  sentences with no type prefix and no trailer, on `main`, until the maintainer
  chooses otherwise.
