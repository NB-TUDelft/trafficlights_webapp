---
summary: >-
  The work order (`2026-11-25`): waiting on the maintainer's requirements and
  next steps after initializing the repository and knowledge base; then settle
  architecture, packaging, quality gate and repository choices, and build in
  small verified slices.
always_read: true
---

# Work order

Take these in order. When an item is done, delete it here and, if it was more
than a fix, record it at the end of History.

1. **Receive requirements and next steps from the maintainer.** Record the
   maintainer's words and structured requirements in Requirements, and any
   standing process choices in Rules:
   - *Direction and architecture.* Whether to evolve `fastapi_receiver.py` or
     restructure/split the backend and frontend, what framework/stack to use,
     and what features or UI changes are wanted.
   - *Firmware integration.* Whether the web application stays on the old
     comma-separated serial protocol alone, also uses the firmware's `!get` /
     `!set` / `!clear` / `!restart` control commands (for board setup), or
     introduces new serial commands coordinated with `../trafficlights_firmware`.
   - *Baseline defects.* Which quirks observed in "The existing FastAPI
     receiver" (Reference) to fix right away (broken duplicate HTML header on
     lines 35–65, state mutated before serial write on 503, Pydantic v1
     `root_validator`, red-duration live refresh, mute tooltip wording,
     out-of-range table handling, external Google Fonts dependency).
   - *Tooling and gate.* Packaging (`uv` / `pyproject.toml` and any frontend
     toolchain), type checker, linter/formatter, and automated test suite to
     wire into `make gate`.
   - *Repository.* Project license (Licensing), git remote, and branch, merge
     and commit-message style (Commits and branches).
2. **Set up the development environment and verification gate** once step 1
   settles the stack, so every code change is backed by automated checks in
   `make gate`.
3. **Implement in small, verified slices**, keeping this knowledge base updated
   at each step and recording exact checks run (and any hardware or browser
   gaps) in commits and History.
