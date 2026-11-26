---
summary: >-
  Remaining maintainer-owned work after v1 implementation (`2026-11-25`) and
  initial live classroom receiver verification (`2026-11-26`): remaining USB Web
  Serial checks (`T,-1,0`, live `mode=station` guard, mesh-relayed stations),
  Coolify production bring-up and `.env.example` hygiene, and restoring the
  SonarQube Rust plugin on the analysis server.
always_read: true
---

# Work order

Take these in order. When an item is done, delete it here and, if it was more
than a fix, record it at the end of History.

1. **Finish live hardware verification over USB Web Serial**:
   - *Already verified live on `2026-11-26`* (see `../trafficlights_firmware`
     history record 140 and "Firmware serial interface" under Reference):
     - ran against the live classroom's old MakeCode receiver (before it was
       reflashed);
     - ran against the reflashed `../trafficlights_firmware` V2.00 classroom
       receiver (`!get` -> `!ok mode=receiver fw=draft-1`, ~40 old stations plus
       retrofitted station 43 updating cleanly, individual tile clicks changing
       station 43).
   - *Still to verify on real USB hardware:*
     - `Reset All To Green` (`T,-1,0`) against live stations (deliberately
       skipped during an active class so it would not reset the room);
     - plugging a `mode=station` board into `trafficlights_webapp` over Web
       Serial to verify the live `WrongModeBanner` and server-side disconnected
       refusal end to end;
     - end-to-end session with mesh-relayed stations once `trafficlights_firmware`
       completes multi-board mesh tests M5 and M9.
2. **Coolify production bring-up and minor repo hygiene**:
   - Verify live HTTPS Web Serial, WSS traffic, `PUBLIC_ORIGIN`, optional
     `CUSTOM_SCRIPT_SRC`, and automatic redeploy via GitHub Actions
     (`COOLIFY_WEBHOOK_URL` and `COOLIFY_TOKEN` are configured as repository
     secrets on `NB-TUDelft/trafficlights_webapp`, and `main` is pushed to
     `origin/main`).
   - Add `CUSTOM_SCRIPT_SRC` to `.env.example` (noted as follow-up in History
     record 060) and optionally clean up the four minor SonarQube smells in
     untouched files (`NewSessionPage.tsx`, `Banners.tsx`,
     `serial-bridge.test.ts`; History record 070).
3. **SonarQube Rust plugin (server-side)**:
   - Re-enable the Rust language plugin on `sonarqube.plohnensoftware.download`
     (currently running Community Build `26.11.0.134175` without a Rust plugin)
     so `make sonar` automatically imports `crates/` and
     `coverage/rust-lcov.info` alongside the already-green JS/TS/CSS/HTML scan.
