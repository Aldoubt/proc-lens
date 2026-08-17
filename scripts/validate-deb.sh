#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

pkg="${1:?usage: validate-deb.sh FILE.deb}"

test -f "$pkg"
test "$(dpkg-deb -f "$pkg" Package)" = "proc-lens"
test "$(dpkg-deb -f "$pkg" Version)" = "0.2.3-1"
test "$(dpkg-deb -f "$pkg" Architecture)" = "amd64"

contents="$(dpkg-deb -c "$pkg")"
for path in \
  ./usr/bin/proc-lens \
  ./usr/share/applications/proc-lens.desktop \
  ./usr/share/icons/hicolor/scalable/apps/proc-lens.svg \
  ./usr/share/icons/hicolor/256x256/apps/proc-lens.png \
  ./usr/share/doc/proc-lens/README.md \
  ./usr/share/doc/proc-lens/LICENSE; do
  grep -Fq "$path" <<<"$contents"
done

printf 'validated %s\n' "$pkg"
