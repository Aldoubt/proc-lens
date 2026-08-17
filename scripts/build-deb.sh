#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

./scripts/verify.sh

if ! command -v cargo-deb >/dev/null 2>&1; then
  echo "cargo-deb is required." >&2
  echo "Install it with:" >&2
  echo "  cargo install cargo-deb --locked --version 3.7.0" >&2
  exit 1
fi

cargo deb

mapfile -t packages < <(
  find target/debian -maxdepth 1 -type f \
    -name 'proc-lens_0.2.3-1_amd64.deb' -print
)

if [ "${#packages[@]}" -ne 1 ]; then
  printf 'expected exactly one proc-lens_0.2.3-1_amd64.deb, found %s\n' "${#packages[@]}" >&2
  exit 1
fi

./scripts/validate-deb.sh "${packages[0]}"
printf '%s\n' "${packages[0]}"
