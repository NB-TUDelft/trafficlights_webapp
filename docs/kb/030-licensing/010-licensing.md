---
summary: >-
  No project license is chosen or recorded yet (the maintainer's to decide);
  `fastapi_receiver.py` descends from the Apache-2.0
  `NB-TUDelft/classroom_trafficlights` / `RolfHut/ClassroomIndicatorLights`
  lineage, which bears on that choice; the knowledge-base tooling is
  AGPL-3.0-or-later.
always_read: true
---

# Licensing

## Project licensing status

**No license has been chosen for this repository.** There is no `LICENSE` file
and no package metadata license field yet. The choice is the maintainer's (Work
order). Until it is made, do not add or enforce project-level license claims.

Two things bear on the choice:

- **The starting file's lineage.** `fastapi_receiver.py` (`7fe9e60`) matches
  the role and design of `software/teacherApplication/fastapi_receiver.py` in
  [NB-TUDelft/classroom_trafficlights](https://github.com/NB-TUDelft/classroom_trafficlights),
  a fork of
  [RolfHut/ClassroomIndicatorLights](https://github.com/RolfHut/ClassroomIndicatorLights).
  As recorded in `trafficlights_firmware`'s Licensing section (checked
  `2026-11-24`), both upstream repositories carry an Apache-2.0 `LICENSE` at
  their root. Whether this copy of `fastapi_receiver.py` carries local
  modifications beyond the upstream checkout was not compared byte for byte
  from this workspace (the upstream clone sits outside it). Whether the final
  web application modifies `fastapi_receiver.py` or replaces it independently
  decides what attribution and license terms apply; that is for the maintainer
  to settle.
- **The knowledge-base tooling is AGPL-3.0-or-later** (below). It is
  development tooling and is not shipped as part of the web application.

## Inventory

| What | Where from | License |
| :--- | :--- | :--- |
| `fastapi_receiver.py` | Starting file present in this folder on `2026-11-25` (SHA-256 `62495215789e935927864c235f074db069f6f27a4ad327ae0b45db627f2e0e87`), lineage from `classroom_trafficlights` | Upstream repository carries Apache-2.0; confirm provenance and exact terms with the maintainer |
| `docs/kb/housekeeping.mjs`, `docs/kb/graphify.mjs`, `scripts/kb.sh` | Copied byte for byte from `../trafficlights_firmware` (Provenance of the knowledge-base tooling) | AGPL-3.0-or-later, kept in each file's `SPDX-License-Identifier` line |
| `Makefile`, `README.md`, `AGENTS.md`, `.gitignore`, `docs/kb/*.md` | Written here on `2026-11-25` | Follows the project license, once chosen |

Runtime and development dependencies (`fastapi`, `uvicorn`, `pydantic`,
`pyserial`, and any frontend libraries) have not been packaged or inventoried
yet.
