.PHONY: gate wasm web-build kb kb-check kb-graph

# The gate before a commit, in this order: the knowledge-base layout and index
# check; the wasm module (the frontend imports its generated glue); the frontend
# (Biome ci with type check, strict lint, tests, production build, which
# `trafficlights-server` embeds); then the Rust workspace (format, clippy with
# warnings denied, tests). Needs `nub --cwd web install` once on a fresh clone.
gate:
	scripts/kb.sh check
	scripts/build-wasm.sh
	nub --cwd web run ci
	nub --cwd web run lint:strict
	nub --cwd web run test
	nub --cwd web run build
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace

# Browser WebAssembly module: build trafficlights-wasm and generate web/src/wasm.
wasm:
	scripts/build-wasm.sh

# Frontend production build into web/dist (embedded by trafficlights-server).
web-build:
	nub --cwd web run build

# Knowledge base (docs/kb): rewrite its index, check it, refresh the graphify graph.
kb:
	scripts/kb.sh index

kb-check:
	scripts/kb.sh check

kb-graph:
	scripts/kb.sh graph
