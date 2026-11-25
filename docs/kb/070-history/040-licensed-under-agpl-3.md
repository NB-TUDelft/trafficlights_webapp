---
summary: >-
  Record (`2026-11-25`): licensed `trafficlights_webapp` cleanly under
  `AGPL-3.0-or-later` matching Git Graph Libre (`LICENSE` and `NOTICE.md`),
  set the public source URL to
  `https://github.com/NB-TUDelft/trafficlights_webapp` (renamed from
  `NB-TUDelft/classroom_trafficlights`), and removed `LICENSE.apache` and
  Apache-2.0 upstream lineage claims since no `RolfHut/ClassroomIndicatorLights`
  code is incorporated (hardware/protocol compatibility only) and
  `fastapi_receiver.py` is no longer published separately.
---

# Licensed under AGPL-3.0-or-later (`2026-11-25`)

At the maintainer's direction ("license it AGPL 3 as GGL"),
`trafficlights_webapp` is licensed as a whole under **`AGPL-3.0-or-later`**,
matching Git Graph Libre (`/home/zam/Desktop/prog/_GGL/git-graph-libre`).

## Files added and updated

- Added `LICENSE` (canonical GNU Affero General Public License v3.0 text) and
  `NOTICE.md` (project AGPL-3.0-or-later notice, Corresponding Source URL
  `https://github.com/NB-TUDelft/trafficlights_webapp`, contributor roster, and
  hardware/protocol compatibility note).
- Removed `LICENSE.apache` and all Apache-2.0 upstream-code lineage mentions
  after the maintainer confirmed that `fastapi_receiver.py` (authored solely by
  Zamkorus) is no longer published separately and that this repository
  incorporates no code from `RolfHut/ClassroomIndicatorLights` (only operating
  on the same micro:bit hardware and wire protocol).
- Updated Licensing (`030-licensing/010-licensing.md`) with the AGPL-3.0-or-later
  policy, `SPDX-License-Identifier: AGPL-3.0-or-later` source-header rule,
  AGPL Section 13 network source-offer duty (`https://github.com/NB-TUDelft/trafficlights_webapp`
  in the UI footer and `--version`), and AGPL-compatible dependency intake
  table.
- Updated `README.md`, Mission, Requirements, Commits and branches,
  Architecture and design, and Work order.

## Checks run

- Verified canonical `LICENSE` (661 lines), `make kb` (7 groups, 11 always-read
  sections), `make gate` (`scripts/kb.sh check`), and `make kb-graph`.
