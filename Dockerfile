# syntax=docker/dockerfile:1.7
# SPDX-License-Identifier: AGPL-3.0-or-later

##
# trafficlights-webapp — production image.
#
# Built by GitHub Actions, pushed to ghcr.io/nb-tudelft/trafficlights_webapp,
# pulled by Coolify. Nothing is compiled on the server. Four stages: the
# browser WebAssembly module (wasm), the frontend (web), the server binary
# with the frontend baked in (builder), and a distroless runner. The image
# build is also the test gate: any failed check fails the build, so no
# broken image is ever pushed.
#
# The base pairing is load-bearing: the runner is distroless Debian 13
# (trixie, glibc), so the Rust builder is trixie too. A binary linked against
# a newer glibc than the runner's would not start.
##

# Node, Rust and Nub track their latest releases on purpose, as in the
# services_zamkorus reference image: a rebuild may pick up a new major; fix
# forward. The one exception is wasm-bindgen-cli, which must match the exact
# version pinned in Cargo.toml.

# ---------------------------------------------------------------------- wasm
FROM rust:slim-trixie AS wasm
WORKDIR /src
# The CLI first, in its own layer: it is the slowest step and changes only
# when the wasm-bindgen pin in Cargo.toml does (`wasm-bindgen = "=0.2.129"`).
# It must match that version exactly; wasm-bindgen rejects a module built
# with another one, which fails this stage loudly.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install --locked wasm-bindgen-cli --version 0.2.129
# The toolchain (and the wasm32 target) named by rust-toolchain.toml, in its
# own layer so source changes do not download it again. The explicit
# `target add` is idempotent and keeps this stage correct if the file's
# `targets` list ever changes.
COPY rust-toolchain.toml ./
RUN rustup toolchain install \
 && rustup target add wasm32-unknown-unknown
COPY Cargo.toml Cargo.lock ./
COPY crates/proto ./crates/proto
COPY crates/wasm ./crates/wasm
# The workspace names crates/server as a member, and `--locked` has to resolve
# the whole workspace, so cargo needs its manifest and a bin target to exist.
# Nothing here compiles it: a stub keeps server edits from rebuilding the
# WebAssembly module.
COPY crates/server/Cargo.toml ./crates/server/Cargo.toml
RUN mkdir -p crates/server/src \
 && echo 'fn main() {}' > crates/server/src/main.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build -p trafficlights-wasm --target wasm32-unknown-unknown --release --locked \
 && wasm-bindgen \
      --target web \
      --out-dir /out/wasm \
      --out-name trafficlights_wasm \
      target/wasm32-unknown-unknown/release/trafficlights_wasm.wasm

# ----------------------------------------------------------------------- web
FROM node:alpine AS web
# Nub's official npm package (the unscoped `nub` on npm is an unrelated
# project). Its binary comes from per-platform optional packages, which npm
# checks against the registry's integrity hashes.
RUN npm install -g @nubjs/nub@latest
WORKDIR /web
# Lockfile first, so only a dependency change busts the install layer.
COPY web/package.json web/nub.lock ./
RUN nub install --frozen-lockfile
COPY web/ ./
# The generated glue the frontend imports and type-checks against
# (web/src/wasm is gitignored and dockerignored; this is its only source).
COPY --from=wasm /out/wasm ./src/wasm
# The image build is the deployment gate: if the frontend checks or tests
# fail, Docker never produces the dist/ used by the Rust builder.
RUN nub run ci \
 && nub run lint:strict \
 && nub run test
# The commit this bundle is built from, shown in the footer. Vite inlines
# VITE_* variables at build time. Declared after the checks, so a new commit
# alone reruns only the build; unset (a local build), the bundle has no commit.
ARG TRAFFICLIGHTS_COMMIT
ENV VITE_BUILD_COMMIT=$TRAFFICLIGHTS_COMMIT
RUN nub run build

# ------------------------------------------------------------------ builder
FROM rust:slim-trixie AS builder
WORKDIR /src
# Install the toolchain (with Clippy/rustfmt) named by rust-toolchain.toml
# in its own layer, so source changes do not download it again.
COPY rust-toolchain.toml ./
RUN rustup toolchain install
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# rust-embed bakes this into the binary at compile time.
COPY --from=web /web/dist ./web/dist
# Run the Rust gate in this builder, in release profile and in one Cargo
# invocation chain, so Clippy, tests and the shipped binary share one
# target/dependency compilation instead of CI compiling the workspace in
# a separate job and Docker compiling it again. The release test compile
# is capped at two jobs: fat-LTO test binaries take several GB each, and
# Docker can expose many CPUs behind a much smaller memory limit.
#
# The build's identity: the commit (and the build time in Unix seconds),
# passed by the publish workflow. Docker hands ARGs to RUN as environment
# variables, and the server reads TRAFFICLIGHTS_COMMIT at compile time
# (option_env!) to show which build runs. Declared directly before this RUN
# so the layers above stay cached; a build without them (compose.dev.yaml) is
# a development build.
ARG TRAFFICLIGHTS_COMMIT
ARG TRAFFICLIGHTS_BUILT_AT
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo fmt --all --check \
 && cargo clippy --release --locked --workspace --all-targets -- -D warnings \
 && cargo test --jobs 2 --release --locked --workspace \
 && cargo build --release --locked -p trafficlights-server \
 && cp target/release/trafficlights-webapp /trafficlights-webapp

# ------------------------------------------------------------------- runner
# Distroless: glibc, ca-certificates, tzdata and a world-writable /tmp, where
# the server creates LOG_DIR for the session statistics `.jsonl` files at
# runtime; no shell, no package manager. Need a shell while debugging? The
# :debug-nonroot tag adds busybox.
FROM gcr.io/distroless/cc-debian13:nonroot AS runner

# The source label is the AGPL Section 13 pointer to the Corresponding Source.
LABEL org.opencontainers.image.title="trafficlights-webapp" \
      org.opencontainers.image.description="Classroom traffic lights web application (Rust Axum + WASM Web Serial)" \
      org.opencontainers.image.source="https://github.com/NB-TUDelft/trafficlights_webapp" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later"

COPY --from=builder /trafficlights-webapp /trafficlights-webapp

# BIND defaults to loopback in the binary; inside the container the proxy
# connects from the Coolify network, so listen on every interface. The rest
# are the binary's own defaults, written out so `docker inspect` shows them;
# a deployment overrides any of them (see .env.example). LOG_DIR lives in
# /tmp on purpose: classroom statistics are ephemeral, and R15 keeps them on
# disk rather than in RAM.
ENV BIND=0.0.0.0:3000 \
    LOG_DIR=/tmp/trafficlights-logs \
    DISCONNECT_GRACE_SECONDS=300 \
    LOG_RETENTION_SECONDS=600 \
    MAX_SESSIONS=32 \
    LOG=info

USER 65532:65532
EXPOSE 3000

# The binary is its own probe (no shell or curl here). Exec form is required
# for the same reason, and HEALTHCHECK does not go through ENTRYPOINT.
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/trafficlights-webapp", "--healthcheck"]

ENTRYPOINT ["/trafficlights-webapp"]
