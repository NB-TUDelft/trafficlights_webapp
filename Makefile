.PHONY: gate kb kb-check kb-graph

# The gate before a commit. Starts with the knowledge-base layout and index check;
# wire type checks, linting and tests here once the application stack is settled.
gate: kb-check

# Knowledge base (docs/kb): rewrite its index, check it, refresh the graphify graph.
kb:
	scripts/kb.sh index

kb-check:
	scripts/kb.sh check

kb-graph:
	scripts/kb.sh graph
