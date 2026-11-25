#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Start the local development preview: builds WASM, runs the Rust Axum server
# on BIND (default 127.0.0.1:3000), and starts Vite HMR on http://localhost:5173
# proxying /api and /ws to the backend.
#
# Usage: scripts/dev.sh [--embedded]
#
#   (default)   Run Rust server (:3000) + Vite HMR frontend (:5173) together.
#   --embedded  Build WASM + production web/dist and run only the Rust server
#               on http://127.0.0.1:3000 (mirrors the container setup).

set -eu
cd "$(dirname "$0")/.."

PATH="$HOME/.cargo/bin:$PATH"
export PATH

mode="hmr"
case "${1:-}" in
    "") ;;
    --embedded) mode="embedded" ;;
    -h | --help)
        sed -n '3,11p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *)
        echo "usage: $0 [--embedded]" >&2
        exit 2
        ;;
esac

# Load local .env if present (never committed).
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

if [ "$mode" = "embedded" ]; then
    echo "Building production web bundle..."
    nub --cwd web run build
    bind="${BIND:-127.0.0.1:3000}"
    echo ""
    echo "  Preview URL (embedded SPA): http://$bind"
    echo "  Press Ctrl-C to stop."
    echo ""
    exec env BIND="$bind" LOG="${LOG:-info}" cargo run -p trafficlights-server --bin trafficlights-webapp
fi

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
echo "  Web Serial works on http://127.0.0.1:$vite_port in Chromium/Brave/Edge."
echo "  Press Ctrl-C to stop both servers."
echo ""

BIND="$bind" LOG="${LOG:-info}" cargo run -p trafficlights-server --bin trafficlights-webapp &
server_pid=$!

VITE_BACKEND_URL="http://$bind" VITE_PORT="$vite_port" nub --cwd web run dev &
vite_pid=$!

# Stop both if either exits.
while kill -0 "$server_pid" 2>/dev/null && kill -0 "$vite_pid" 2>/dev/null; do
    sleep 1
done

wait "$server_pid" 2>/dev/null || true
wait "$vite_pid" 2>/dev/null || true
