#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

grep -q '^version = "0.3.0"$' Cargo.toml
grep -q '^\[package.metadata.deb\]$' Cargo.toml
grep -q 'packaging/proc-lens.desktop' Cargo.toml

grep -q '^Type=Application$' packaging/proc-lens.desktop
grep -q '^Exec=proc-lens$' packaging/proc-lens.desktop
grep -q '^Icon=proc-lens$' packaging/proc-lens.desktop
grep -q '^Terminal=true$' packaging/proc-lens.desktop

test -s packaging/icons/proc-lens.svg
test -s packaging/icons/proc-lens.png
file packaging/icons/proc-lens.png | grep -q 'PNG image data, 256 x 256'

grep -q 'version=.*Cargo.toml' scripts/build-deb.sh
grep -q 'proc-lens_${version}-1_amd64.deb' scripts/build-deb.sh
grep -q 'version=.*Cargo.toml' scripts/validate-deb.sh
grep -q '${version}-1' scripts/validate-deb.sh

test -s .github/workflows/release.yml
grep -q "tags:" .github/workflows/release.yml
grep -q "v\*" .github/workflows/release.yml
grep -q 'contents: write' .github/workflows/release.yml
grep -q 'scripts/build-deb.sh' .github/workflows/release.yml
grep -q 'version=.*Cargo.toml' .github/workflows/release.yml
grep -q 'proc-lens_${version}-1_amd64.deb' .github/workflows/release.yml
grep -q 'gh release' .github/workflows/release.yml

if grep -R -n 'proc-lens_0\.2\.3-1_amd64\.deb\|Version).*0\.2\.3-1' \
  scripts .github/workflows/release.yml; then
  echo 'packaging scripts still hard-code the previous release' >&2
  exit 1
fi
