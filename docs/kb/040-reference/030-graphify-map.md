---
summary: >-
  How `make kb-graph` (`scripts/kb.sh graph`) builds the graphify code and
  knowledge-base graph: `graphify update .` (offline), the `contains` and
  `mentions` edges `docs/kb/housekeeping.mjs --graph` adds, re-clustering, the
  tree; why `graphify-out/` is gitignored; what it indexes here.
---

# Graphify Map

## Overview

This repository supports the same **graphify + knowledge base** workflow as
`../trafficlights_firmware`, Git Graph Libre, xrdp-kwin and ImProc, through
`scripts/kb.sh` and `docs/kb/graphify.mjs`. `make kb-graph` runs
`scripts/kb.sh graph`, which does four things:

1. `graphify update .` parses the code and Markdown (offline, no model call)
   into `graphify-out/graph.json`.
2. `node docs/kb/housekeeping.mjs --graph` adds the knowledge base's own edges,
   each marked `"_origin": "kb"`: `contains` edges from the index to each
   group, from each group to its sections and from each section to its
   subsections, and `mentions` edges from a section to repository files named
   in inline backticks (matching exactly one file in the graph) or to
   code-shaped names matching one to three symbols.
3. `GRAPHIFY_VIZ_NODE_LIMIT=8000 graphify cluster-only .` re-clusters the graph
   with those edges included.
4. `graphify tree` refreshes the text tree summary.

To query it: `graphify query "<question>"`, or `graphify explain "<node>"` and
`graphify path "<A>" "<B>"`, from the repository root.

## What it indexes here

- Currently indexes `README.md`, `fastapi_receiver.py`, `scripts/kb.sh`, and
  `docs/kb/`. Ignored directories (`.venv/`, `node_modules/`, `dist/`,
  `build/`, `graphify-out/`) are skipped via `.gitignore`.
- Run `make kb-graph` after section files are in place so `graphify update .`
  sees every file before step 2 links them.

## Why `graphify-out/` is gitignored

It is regenerable local tool output and can grow very large in bigger
repositories. `.gitignore` lists `graphify-out/` so it never reaches a commit.
`graphify` itself is installed outside the repository
(`~/.local/bin/graphify`).
