---
summary: >-
  Remaining maintainer-owned work after v1 implementation completed on
  `2026-11-25`: live micro:bit V2 USB serial verification (old MakeCode and
  new firmware), Coolify public domain / webhook bring-up, and restoring the
  SonarQube Rust plugin on the analysis server.
always_read: true
---

# Work order

Take these in order. When an item is done, delete it here and, if it was more
than a fix, record it at the end of History.

1. **Live hardware verification over USB Web Serial**:
   - Connect a real micro:bit V2 receiver running the old MakeCode receiver
     (`../trafficlights_firmware/old/teacherReceiver.py`) and verify 1 s legacy
     fallback, station updates, tile clicks, and `Reset All To Green`.
   - Connect a micro:bit V2 running `../trafficlights_firmware` receiver mode
     (`!get` -> `!ok mode=receiver fw=draft-1`) and station mode (`mode=station`
     warning banner and server-side refusal to mark connected).
2. **Coolify production bring-up**:
   - Confirm the Coolify public domain (`PUBLIC_ORIGIN=https://<host>`) and
     GHCR image path (`ghcr.io/nb-tudelft/trafficlights_webapp`).
   - Configure `COOLIFY_WEBHOOK_URL` (repo variable) and `COOLIFY_TOKEN` (repo
     secret) on GitHub Actions, push `main` when ready, and verify live HTTPS
     Web Serial and WSS traffic through Coolify.
3. **SonarQube Rust plugin (server-side)**:
   - Re-enable the Rust language plugin on `sonarqube.plohnensoftware.download`
     (currently running Community Build `26.11.0.134175` without a Rust plugin)
     so `make sonar` automatically imports `crates/` and
     `coverage/rust-lcov.info` alongside the already-green JS/TS/CSS/HTML scan.
