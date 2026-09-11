#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

if ! command -v cargo >/dev/null 2>&1; then
  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
  fi
fi

command -v cargo >/dev/null 2>&1 || {
  echo "ERROR: cargo not found in PATH" >&2
  exit 1
}

timestamp=$(date -u +%Y%m%d-%H%M%S)
artifact_dir="$ROOT_DIR/artifacts/runtime-smoke/$timestamp"
mkdir -p "$artifact_dir"

echo "runtime smoke artifacts: $artifact_dir"
cargo build --release 2>&1 | tee "$artifact_dir/build.log"
./target/release/proc-lens snapshot --json > "$artifact_dir/runtime.json"
python3 scripts/check-runtime-json.py "$artifact_dir/runtime.json" \
  --report-json "$artifact_dir/report.json" \
  --report-text "$artifact_dir/summary.txt"
echo "PASS: runtime smoke test"