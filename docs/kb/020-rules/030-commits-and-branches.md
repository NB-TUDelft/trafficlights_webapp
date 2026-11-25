---
summary: >-
  Standing rules: git repository initialized on `2026-11-25` on `main` (root
  commit `7fe9e60`, signed); target GitHub repository is
  `https://github.com/NB-TUDelft/trafficlights_webapp` (renamed from
  `NB-TUDelft/classroom_trafficlights`, which the maintainer will force-push
  to); linear history on `main` with short imperative GPG-signed commits (no
  type prefix, no trailer, matching `trafficlights_firmware`); signing is never
  bypassed; agents do not push or add a remote unasked.
always_read: true
---

# Commits and branches

- **The repository.** Initialized on `2026-11-25` with `git init` on `main` at
  the maintainer's request, starting with the untouched `fastapi_receiver.py`
  as root commit `7fe9e60` (signed). The target GitHub repository is
  `https://github.com/NB-TUDelft/trafficlights_webapp` (renamed from
  `NB-TUDelft/classroom_trafficlights`); the maintainer stated on `2026-11-25`
  that they will force-push this repository onto it. Adding the remote and
  pushing stay the maintainer's to run or explicitly request.
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
- **Branches and message style (confirmed `2026-11-25`).** Keep linear history
  on `main` with short imperative commit subjects (no type prefix and no
  trailer), matching `trafficlights_firmware`. Remote setup stays the
  maintainer's to ask for.
