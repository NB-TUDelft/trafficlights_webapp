#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Knowledge-base housekeeping (docs/kb). See "Order of this document" and
# "Graphify Map" in the knowledge base for what each step does.
#
# Usage: scripts/kb.sh index | check | graph
#
#   index  rewrite the index in docs/kb/README.md from the section files
#   check  fail if the index is stale or a section breaks the layout rules
#   graph  refresh graphify-out/ with the code and the knowledge base in it

set -e
# Run from the repository root (the parent of scripts/), with or without a git repository.
cd "$(dirname "$0")/.."

case "$1" in
    index)
        node docs/kb/housekeeping.mjs
        ;;
    check)
        node docs/kb/housekeeping.mjs --check
        ;;
    graph)
        graphify update .
        node docs/kb/housekeeping.mjs --graph
        # The graph is past graphify's default 5,000-node limit for graph.html.
        GRAPHIFY_VIZ_NODE_LIMIT=8000 graphify cluster-only .
        graphify tree
        ;;
    *)
        echo "usage: $0 index | check | graph" >&2
        exit 2
        ;;
esac
