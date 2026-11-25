#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SonarQube scan for trafficlights_webapp, after Git Graph Libre and
# services_zamkorus: fresh WASM + coverage first, then the scan, then an
# explicit server-side quality-gate poll.
#
# Usage: scripts/sonar.sh [scan|local|coverage]
#
#   scan      coverage + scan + quality-gate poll (the default)
#   local     the same against a local server on http://127.0.0.1:9000
#   coverage  only (re)generate the coverage reports
#
# The server URL and token come from the global scanner configuration
# (${SONAR_SCANNER_HOME:-/opt/sonar-scanner}/conf/sonar-scanner.properties)
# or the SONAR_HOST_URL / SONAR_TOKEN environment variables. They never
# live in this repository.

set -eu
cd "$(git rev-parse --show-toplevel)"

PATH="$HOME/.cargo/bin:${SONAR_SCANNER_HOME:-/opt/sonar-scanner}/bin:$PATH"
export PATH

mode="${1:-scan}"

coverage() {
    if ! cargo llvm-cov --version >/dev/null 2>&1; then
        echo "cargo-llvm-cov is missing. Install it once:" >&2
        echo "  rustup component add llvm-tools-preview" >&2
        echo "  cargo install cargo-llvm-cov --locked" >&2
        exit 1
    fi
    # The web build imports web/src/wasm, and Rust tests embed web/dist.
    scripts/build-wasm.sh
    nub --cwd web run build
    nub --cwd web run test:coverage
    mkdir -p coverage
    cargo llvm-cov --workspace --locked --lcov --output-path coverage/rust-lcov.info
}

case "$mode" in
    coverage)
        coverage
        exit 0
        ;;
    scan | local) ;;
    *)
        echo "usage: $0 [scan|local|coverage]" >&2
        exit 2
        ;;
esac

if ! command -v sonar-scanner >/dev/null 2>&1; then
    echo "sonar-scanner is missing from PATH (checked ${SONAR_SCANNER_HOME:-/opt/sonar-scanner}/bin)." >&2
    exit 1
fi

coverage

if [ "$mode" = "local" ]; then
    sonar-scanner -Dsonar.host.url=http://127.0.0.1:9000
else
    sonar-scanner
fi

# Poll the compute-engine task the scanner just submitted, then its gate.
report=".scannerwork/report-task.txt"
ce_task_id=$(sed -n 's/^ceTaskId=//p' "$report")
server_url=$(sed -n 's/^serverUrl=//p' "$report" | sed 's:/$::')
dashboard_url=$(sed -n 's/^dashboardUrl=//p' "$report")

token="${SONAR_TOKEN:-}"
if [ -z "$token" ]; then
    conf="${SONAR_SCANNER_HOME:-/opt/sonar-scanner}/conf/sonar-scanner.properties"
    token=$(sed -n 's/^sonar.token=//p' "$conf" 2>/dev/null || true)
fi

json_field() {
    # $1 = dotted path ("task.status"); prints "" for a missing field.
    node -e '
let s = "";
process.stdin.on("data", (d) => (s += d)).on("end", () => {
  let v = JSON.parse(s);
  for (const k of process.argv[1].split(".")) v = v?.[k];
  console.log(v ?? "");
});' "$1"
}

api() {
    if [ -n "$token" ]; then
        curl -s -m 15 -u "$token:" "$server_url$1"
    else
        curl -s -m 15 "$server_url$1"
    fi
}

analysis_id=""
attempt=0
while [ "$attempt" -lt 60 ]; do
    task_json=$(api "/api/ce/task?id=$ce_task_id")
    status=$(printf '%s' "$task_json" | json_field task.status)
    case "$status" in
        SUCCESS)
            analysis_id=$(printf '%s' "$task_json" | json_field task.analysisId)
            break
            ;;
        FAILED | CANCELED)
            echo "SonarQube analysis $status. Dashboard: $dashboard_url" >&2
            exit 1
            ;;
    esac
    attempt=$((attempt + 1))
    sleep 5
done
if [ -z "$analysis_id" ]; then
    echo "Timed out waiting for the SonarQube analysis. Dashboard: $dashboard_url" >&2
    exit 1
fi

gate=$(api "/api/qualitygates/project_status?analysisId=$analysis_id" | json_field projectStatus.status)
echo "SonarQube quality gate: $gate"
echo "Dashboard: $dashboard_url"
[ "$gate" = "OK" ]
