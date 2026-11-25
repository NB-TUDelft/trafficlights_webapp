---
summary: >-
  Facts gathered on `2026-11-25` from the reference repository
  `/home/zam/Desktop/prog/services_zamkorus` (Rust 1.98 + Axum 0.8 + Tokio +
  `rust-embed`, React 19 + Vite + TypeScript + Tailwind CSS v4 + Nub + Biome +
  Vitest, three-stage distroless Dockerfile for Coolify on `zamkorland`) and
  from inspecting installed tools on this development machine (Rust, Nub,
  Docker/Podman, Inkscape/GIMP/Blender, and absence of WASM CLI tooling).
---

# Reference project and toolchain

Gathered on `2026-11-25` at the maintainer's pointer to
`/home/zam/Desktop/prog/services_zamkorus` as a reference for a similar Rust +
web project deployed on Coolify.

## What `services_zamkorus` uses

Read from `services_zamkorus`'s `README.md`, `Cargo.toml`, `crates/app/Cargo.toml`,
`rust-toolchain.toml`, `Dockerfile`, `compose.dev.yaml`, `compose.prod.yaml`,
`web/package.json`, `web/vite.config.ts`, and `docs/kb/` (`2026-11-25`):

- **Rust backend:**
  - Single Cargo workspace at the repository root (`resolver = "2"`,
    `edition = "2021"`, `rust-version = "1.98"`), `rust-toolchain.toml` tracking
    `channel = "stable"` with `clippy` and `rustfmt`.
  - Release profile: `lto = true`, `codegen-units = 1`, `strip = true`.
  - Core crates: `axum = "0.8"`, `tokio = "1"`, `tower` / `tower-http`,
    `rust-embed = "8"` (bakes `web/dist` into the binary at compile time),
    `serde` / `serde_json`, `thiserror = "2"`, `tracing` /
    `tracing-subscriber`, `uuid = "1"` (`v7`).
  - Conventions: `#![forbid(unsafe_code)]`, `cargo fmt --all --check`,
    `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`,
    graceful shutdown on `SIGTERM` and `SIGINT`, and a built-in `--healthcheck`
    CLI probe because the runtime container is distroless.
  - Resource discipline: one process, one container, no runtime Node, target
    idle RAM well under 50 MB.
- **Web frontend (`web/`):**
  - **React 19 + Vite 7 + TypeScript + Tailwind CSS v4** (`@tailwindcss/vite`,
    `react-router`, `lucide-react`).
  - **Package manager:** **Nub** (`@nubjs/nub`, pnpm-compatible, lockfile
    `web/nub.lock`).
  - **Quality gate:** **Biome** (`biome.json` + `biome.strict.jsonc`),
    `tsc --noEmit`, and **Vitest** (`vitest run`, testing server-rendered
    markup and pure TypeScript modules without a heavy DOM library).
  - Dev server: Vite on port `5173` proxying `/api` (and here `/ws`) to the
    Rust server on `127.0.0.1:3000`.
- **Docker and Coolify deployment (`zamkorland`):**
  - **Three-stage `Dockerfile`**:
    1. `web` (`node:alpine` + `npm install -g @nubjs/nub@latest`): runs
       `nub install --frozen-lockfile`, `nub run ci`, `nub run lint:strict`,
       `nub run test`, and `nub run build`.
    2. `builder` (`rust:slim-trixie`, matching Debian 13 glibc): copies
       `web/dist` for `rust-embed`, runs `cargo fmt --all --check`,
       `cargo clippy --release --locked --workspace --all-targets -- -D warnings`,
       `cargo test --jobs 2 --release --locked --workspace`, and
       `cargo build --release --locked`.
    3. `runner` (`gcr.io/distroless/cc-debian13:nonroot`, uid `65532`): copies
       only the compiled binary, sets `BIND=0.0.0.0:3000`, exposes `3000`, and
       defines `HEALTHCHECK CMD ["/<binary>", "--healthcheck"]`.
  - **Local harnesses:** `compose.dev.yaml` (builds locally on
    `127.0.0.1:3000`) and `compose.prod.yaml` (documents and validates the
    production runtime contract).
  - **Pipeline:** GitHub Actions runs `scripts/kb.sh check`, builds the Docker
    image (which itself is the test/lint gate), pushes to GHCR, and triggers
    the Coolify deploy webhook (`force=true`).
  - **Coolify specifics on `zamkorland`:** Docker Image build pack from GHCR,
    Caddy proxy with the container port included in the Domain
    (`https://<host>:3000`), and Coolify's own health check turned **off** so
    Docker's built-in `HEALTHCHECK` is used instead (distroless lacks `curl`
    and `wget`).

## Where `trafficlights_webapp` differs from `services_zamkorus`

- `services_zamkorus` hosts multiple authenticated personal modules backed by a
  persistent Turso/SQLite database (`services.db`, requiring stop-then-start
  deploys because of Turso's exclusive file lock) and an FFI C library
  (`libimproc.so`).
- `trafficlights_webapp` is a focused real-time classroom application: if
  sessions stay ephemeral in memory (see Open questions in Requirements), it
  needs no database, no persistent volume, and no exclusive file lock, while
  adding WebSockets (`axum` `ws` feature) and browser **Web Serial API**
  streaming from the session creator's browser.

## Dev machine tooling verified (`2026-11-25`)

Checked on this host before choosing any toolchain:

| Category | Installed | Not installed |
| :--- | :--- | :--- |
| Rust | `rustc 1.98.1`, `cargo 1.98.1`, `~/.cargo/bin/wasm-bindgen` (`0.2.129`), `cargo-llvm-cov` | `wasm32-unknown-unknown` target (installed via `rustup target add` in Slice 1), `wasm-pack`, `wasm-opt`, `trunk` |
| JavaScript / Node | `node v26.13.1`, `nub v0.11.2`, `pnpm`, `npm`, `bun` | — |
| Containers / Browser | `docker`, `podman`, `/usr/bin/brave` | `google-chrome`, `chromium` |
| Graphics / SVG | `inkscape`, `gimp`, `blender` | `krita` |
| KB / Graph / Agents | `graphify`, `muse` (`muse-wonder-full-1.0`), `claude` (`2.1.331`), `~/.tokeneconomizer/bin/te` | — |
