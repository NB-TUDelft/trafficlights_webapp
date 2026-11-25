---
summary: >-
  `AGPL-3.0-or-later` for the entire project (decided `2026-11-25`, matching
  Git Graph Libre), hosted at
  `https://github.com/NB-TUDelft/trafficlights_webapp`, in `LICENSE` and
  `NOTICE.md`; no code from `RolfHut/ClassroomIndicatorLights` is incorporated
  (hardware and wire-protocol compatibility only, so no `LICENSE.apache`); SPDX
  lines in all project source files; AGPL Section 13 source link in the UI
  footer and `--version`; AGPL-compatible dependency intake rule.
always_read: true
---

# Licensing

## The project's license

**`AGPL-3.0-or-later`**, for the entire project, decided by the maintainer on
`2026-11-25` ("license it AGPL 3 as GGL", matching Git Graph Libre and
`xrdp-kwin`).

The public repository is
[NB-TUDelft/trafficlights_webapp](https://github.com/NB-TUDelft/trafficlights_webapp)
(renamed from `NB-TUDelft/classroom_trafficlights` and replaced with this clean
history). No source code from `RolfHut/ClassroomIndicatorLights` is
incorporated here—this project only operates on the same classroom traffic-light
micro:bit hardware and serial wire protocol—and the starting
`fastapi_receiver.py` prototype was authored by the maintainer and is no longer
published separately. Accordingly, there is no `LICENSE.apache` in this
repository.

- `LICENSE`: the canonical GNU Affero General Public License v3.0 text (copied
  from GNU / Git Graph Libre; never modified).
- `NOTICE.md`: the project copyright and AGPL-3.0-or-later statement,
  Corresponding Source link, contributor roster, and hardware/protocol
  compatibility note. Any future third-party source import updates `NOTICE.md`.
- **SPDX file headers:** every source file written for this project
  (`crates/**/*.rs`, `web/src/**/*.{ts,tsx}`, scripts, etc.) starts with
  `SPDX-License-Identifier: AGPL-3.0-or-later` in the file's comment syntax.
- **Package metadata:** `Cargo.toml` (`[workspace.package]`) declares
  `license = "AGPL-3.0-or-later"` and
  `repository = "https://github.com/NB-TUDelft/trafficlights_webapp"`, and
  `web/package.json` declares `"license": "AGPL-3.0-or-later"`.
- **One-way copyleft:** AGPL-compatible third-party libraries may be used with
  their notices preserved; all code written here is licensed under
  `AGPL-3.0-or-later`.

**Our own AGPL Section 13 duty.** Because `trafficlights_webapp` is a network
web application, it must prominently offer network users access to its
Corresponding Source: the UI footer (and `--version`) displays the license
(`AGPL-3.0-or-later`) and links directly to
`https://github.com/NB-TUDelft/trafficlights_webapp` (overridable via build or
runtime configuration if forked).

## What can come in (third-party dependencies)

Allow any dependency license compatible with `AGPL-3.0-or-later` (matching Git
Graph Libre and `xrdp-kwin`), with notices preserved and recorded in
`NOTICE.md` if third-party source files are ever vendored:

| License of the dependency / source | Can it come in? |
| :--- | :--- |
| MIT, Apache-2.0, X11, BSD, ISC, zlib, Unlicense, Unicode-3.0 | Yes, with notices kept. |
| LGPL (2.0, 2.1 or 3.0) | Yes, combined under the GPLv3 terms the LGPL allows. |
| GPL-2.0-or-later, GPL-3.0(-or-later), AGPL-3.0(-or-later) | Yes (GPLv3 and AGPLv3 combine under Section 13 of each). |
| GPL-2.0-only | No: incompatible with GPLv3 / AGPLv3. |
| Any license whose compatibility is unclear | Stop and ask the maintainer. |

## Inventory

| What | Where from | License |
| :--- | :--- | :--- |
| `LICENSE`, `NOTICE.md` | FSF AGPL-3.0 (via Git Graph Libre) and project notice (`2026-11-25`) | License text verbatim; `NOTICE.md` follows project license |
| `fastapi_receiver.py` (moving to `old/` on scaffold) | Maintainer's starting prototype (`7fe9e60`, SHA-256 `62495215789e935927864c235f074db069f6f27a4ad327ae0b45db627f2e0e87`, authored by Zamkorus; no longer published separately) | `AGPL-3.0-or-later` |
| `docs/kb/housekeeping.mjs`, `docs/kb/graphify.mjs`, `scripts/kb.sh` | Copied byte for byte from `../trafficlights_firmware` (Provenance of the knowledge-base tooling) | `AGPL-3.0-or-later` (`SPDX-License-Identifier` in each file) |
| `Makefile`, `README.md`, `AGENTS.md`, `.gitignore`, `docs/kb/*.md`, and all upcoming `crates/` and `web/` source | Written here from `2026-11-25` onward | `AGPL-3.0-or-later` |

Rust crate and Nub/npm dependency licenses will be verified against the
compatible-license table above as dependencies are added during scaffolding.
