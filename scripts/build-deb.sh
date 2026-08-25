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

version="$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml | head -n1)"
if [ -z "$version" ]; then
  echo "failed to read package version from Cargo.toml" >&2
  exit 1
fi
expected="proc-lens_${version}-1_amd64.deb"

cargo deb

mapfile -t packages < <(
  find target/debian -maxdepth 1 -type f -name "$expected" -print
)

if [ "${#packages[@]}" -ne 1 ]; then
  printf 'expected exactly one %s, found %s\n' "$expected" "${#packages[@]}" >&2
  exit 1
fi

./scripts/validate-deb.sh "${packages[0]}"
printf '%s\n' "${packages[0]}"
