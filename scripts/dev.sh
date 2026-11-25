#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Start the local development preview via Docker Compose (`compose.dev.yaml`),
# building and running the production-identical distroless image on
# http://127.0.0.1:3000.
#
# Usage: scripts/dev.sh [--down | --hmr]
#
#   (default)   Run `docker compose -f compose.dev.yaml up --build` on :3000.
#   --down      Stop and remove the local compose stack.
#   --hmr       Run native Rust server (:3000) + Vite HMR (:5173) without Docker.

set -eu
cd "$(dirname "$0")/.."

mode="compose"
case "${1:-}" in
    "") ;;
    --down) mode="down" ;;
    --hmr) mode="hmr" ;;
    -h | --help)
        sed -n '3,11p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *)
        echo "usage: $0 [--down | --hmr]" >&2
        exit 2
        ;;
esac

if [ "$mode" = "down" ]; then
    exec docker compose -f compose.dev.yaml down
fi

if [ "$mode" = "compose" ]; then
    echo ""
    echo "  Preview URL (Docker Compose): http://127.0.0.1:3000"
    echo "  Web Serial works on http://127.0.0.1:3000 in Chromium/Brave/Edge."
    echo "  Press Ctrl-C to stop."
    echo ""
    exec docker compose -f compose.dev.yaml up --build
fi

# Native HMR fallback (`--hmr`).
PATH="$HOME/.cargo/bin:$PATH"
export PATH

if [ -f .env ]; then
    set -a
    # shellcheck disable=SC1091
    . ./.env
    set +a
fi

if [ ! -d web/node_modules ]; then
    echo "Installing frontend dependencies with nub..."
    nub --cwd web install
fi

echo "Building browser WASM module..."
scripts/build-wasm.sh

if [ ! -f web/dist/index.html ]; then
    echo "Building initial web/dist for rust-embed..."
    nub --cwd web run build
fi

bind="${BIND:-127.0.0.1:3000}"
vite_port="${VITE_PORT:-5173}"
server_pid=""
vite_pid=""

cleanup() {
    trap - EXIT INT TERM
    if [ -n "$vite_pid" ]; then
        kill "$vite_pid" 2>/dev/null || true
    fi
    if [ -n "$server_pid" ]; then
        kill "$server_pid" 2>/dev/null || true
    fi
    wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo ""
echo "  Frontend preview (Vite HMR): http://127.0.0.1:$vite_port"
echo "  Backend server (Axum + WS):  http://$bind"
echo "  Press Ctrl-C to stop both servers."
echo ""

BIND="$bind" LOG="${LOG:-info}" cargo run -p trafficlights-server --bin trafficlights-webapp &
server_pid=$!

VITE_BACKEND_URL="http://$bind" VITE_PORT="$vite_port" nub --cwd web run dev &
vite_pid=$!

while kill -0 "$server_pid" 2>/dev/null && kill -0 "$vite_pid" 2>/dev/null; do
    sleep 1
done

wait "$server_pid" 2>/dev/null || true
wait "$vite_pid" 2>/dev/null || true
