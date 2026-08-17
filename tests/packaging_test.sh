#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

grep -q '^version = "0.2.3"$' Cargo.toml
grep -q '^\[package.metadata.deb\]$' Cargo.toml
grep -q 'packaging/proc-lens.desktop' Cargo.toml

grep -q '^Type=Application$' packaging/proc-lens.desktop
grep -q '^Exec=proc-lens$' packaging/proc-lens.desktop
grep -q '^Icon=proc-lens$' packaging/proc-lens.desktop
grep -q '^Terminal=true$' packaging/proc-lens.desktop

test -s packaging/icons/proc-lens.svg
test -s packaging/icons/proc-lens.png
file packaging/icons/proc-lens.png | grep -q 'PNG image data, 256 x 256'
