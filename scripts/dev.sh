#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Local development runner.
#
# Usage: scripts/dev.sh [--up | --down]
#
#   (default)   Watch mode without Docker (`make dev`): Vite on :5173 updates the
#               frontend in place on save, and the Rust server on :3000
#               restarts when crates/ change (rebuilding the browser WASM
#               module first when crates/proto or crates/wasm change).
#   --up        Build and run the production-identical distroless image via
#               Docker Compose (`compose.dev.yaml`) on :3000 (`make dev-up`).
#   --down      Stop and remove the Docker Compose stack (`make dev-down`).

set -eu
cd "$(dirname "$0")/.."

mode="watch"
case "${1:-}" in
    "") ;;
    --up) mode="up" ;;
    --down) mode="down" ;;
    -h | --help)
        sed -n '3,13p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *)
        echo "usage: $0 [--up | --down]" >&2
        exit 2
        ;;
esac

if [ "$mode" = "down" ]; then
    exec docker compose -f compose.dev.yaml down
fi

if [ "$mode" = "up" ]; then
    echo ""
    echo "  Preview URL (Docker Compose): http://127.0.0.1:3000"
    echo "  Web Serial works on http://127.0.0.1:3000 in Chromium/Brave/Edge."
    echo "  Press Ctrl-C to stop."
    echo ""
    exec docker compose -f compose.dev.yaml up --build
fi

# Watch mode.
PATH="$HOME/.cargo/bin:$PATH"
export PATH

if [ -f .env ]; then
    set -a
    # shellcheck disable=SC1091
    . ./.env
    set +a
fi

bind="${BIND:-127.0.0.1:3000}"
vite_port="${VITE_PORT:-5173}"

# Fails fast when another server (often another `make dev`) holds a port:
# curl exits 7 only when nothing accepts the connection.
port_free() {
    command -v curl >/dev/null 2>&1 || return 0
    probe=0
    curl -s -o /dev/null --max-time 2 "http://$1/" 2>/dev/null || probe=$?
    [ "$probe" -eq 7 ]
}
for address in "$bind" "127.0.0.1:$vite_port"; do
    if ! port_free "$address"; then
        echo "dev: $address is already in use (is another make dev running?)." >&2
        exit 1
    fi
done

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

server_pid=""
vite_pid=""

# The page comes from Vite, so the server sees its origin on WebSocket upgrades.
case ",${PUBLIC_ORIGIN:-}," in
    ",," | *",http://127.0.0.1:$vite_port,"*) ;;
    *)
        echo "dev: PUBLIC_ORIGIN is set without http://127.0.0.1:$vite_port;" \
            "the server will refuse the dev page's WebSockets." >&2
        ;;
esac

# Rust sources are polled against this file's modification time.
stamp="target/dev-watch.stamp"
mkdir -p target

# Prints the first watched file under the given paths modified since the stamp.
changed_since_stamp() {
    find "$@" -newer "$stamp" \
        \( -name '*.rs' -o -name 'Cargo.toml' -o -name 'Cargo.lock' \) -print 2>/dev/null |
        head -n 1
}

start_server() {
    touch "$stamp"
    BIND="$bind" LOG="${LOG:-info}" cargo run -p trafficlights-server --bin trafficlights-webapp &
    server_pid=$!
}

stop_server() {
    if [ -n "$server_pid" ]; then
        kill "$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
        server_pid=""
    fi
}

cleanup() {
    trap - EXIT INT TERM
    if [ -n "$vite_pid" ]; then
        kill "$vite_pid" 2>/dev/null || true
    fi
    stop_server
    wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo ""
echo "  Open (Vite, frontend updates on save): http://127.0.0.1:$vite_port"
echo "  Backend (Axum + WS, restarts on Rust changes): http://$bind"
echo "  A server restart ends in-memory sessions; create a new one afterwards."
echo "  Press Ctrl-C to stop both."
echo ""

start_server

VITE_BACKEND_URL="http://$bind" VITE_PORT="$vite_port" nub --cwd web run dev &
vite_pid=$!

while kill -0 "$vite_pid" 2>/dev/null; do
    sleep 1
    if [ -n "$(changed_since_stamp crates Cargo.toml Cargo.lock)" ]; then
        rebuild_wasm=""
        if [ -n "$(changed_since_stamp crates/proto crates/wasm)" ]; then
            rebuild_wasm="yes"
        fi
        echo "dev: Rust sources changed; restarting the server..."
        stop_server
        if [ -n "$rebuild_wasm" ]; then
            scripts/build-wasm.sh || echo "dev: WASM build failed; fix it and save again." >&2
        fi
        start_server
    elif [ -n "$server_pid" ] && ! kill -0 "$server_pid" 2>/dev/null; then
        status=0
        wait "$server_pid" 2>/dev/null || status=$?
        server_pid=""
        echo "dev: server exited ($status); save a Rust file to rebuild and restart it." >&2
    fi
done
