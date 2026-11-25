#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Build the browser WebAssembly module and generate its JavaScript glue.
#
# Usage: scripts/build-wasm.sh
#
# Compiles `trafficlights-wasm` for wasm32-unknown-unknown in release mode, then
# runs wasm-bindgen (`--target web`) so the frontend can import
# `web/src/wasm/trafficlights_wasm.js`. The output directory is gitignored.
#
# The wasm-bindgen CLI must match the `wasm-bindgen` crate version pinned in
# Cargo.toml (`cargo install wasm-bindgen-cli --version <version>`).

set -eu
# Run from the repository root (the parent of scripts/).
cd "$(dirname "$0")/.."

# `cargo install` puts wasm-bindgen here, which is not always on a
# non-interactive shell's PATH (make, IDEs, CI).
PATH="$HOME/.cargo/bin:$PATH"
export PATH

cargo build -p trafficlights-wasm --target wasm32-unknown-unknown --release

wasm-bindgen \
    --target web \
    --out-dir web/src/wasm \
    --out-name trafficlights_wasm \
    target/wasm32-unknown-unknown/release/trafficlights_wasm.wasm
