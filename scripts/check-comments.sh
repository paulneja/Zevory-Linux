#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ "$#" -gt 0 ]; then
  FILES=("$@")
elif git rev-parse --git-dir >/dev/null 2>&1; then
  mapfile -t FILES < <(git ls-files '*.rs' '*.sh' '*.toml')
else
  mapfile -t FILES < <(find . \
    \( -path ./sources -o -path ./build -o -path ./zevinit/target -o -path ./.git \) -prune -o \
    \( -name '*.rs' -o -name '*.sh' -o -name '*.toml' \) -print)
fi

if [ "${#FILES[@]}" -eq 0 ]; then
  echo "check-comments: found nothing to check, refusing to report success" >&2
  exit 1
fi

found=0

for f in "${FILES[@]}"; do
  [ -f "$f" ] || continue
  case "$f" in
    *.rs) marker='//' ;;
    *)    marker='#'  ;;
  esac

  while IFS=: read -r line text; do
    [ -n "$line" ] || continue
    case "$text" in
      '#!'*) continue ;;
      *SPDX-License-Identifier*) continue ;;
    esac
    printf '%s:%s: %s\n' "$f" "$line" "$(echo "$text" | sed 's/^[[:space:]]*//')"
    found=1
  done < <(grep -nE "^[[:space:]]*${marker}" "$f")

  if [ "$marker" = '//' ]; then
    while IFS=: read -r line text; do
      [ -n "$line" ] || continue
      printf '%s:%s: %s\n' "$f" "$line" "$(echo "$text" | sed 's/^[[:space:]]*//')"
      found=1
    done < <(grep -nE '[^:[:space:]][[:space:]]+//' "$f" | grep -v '://')
  fi
done

if [ "$found" -ne 0 ]; then
  echo ""
  echo "comments are not allowed in this repo. remove the lines above."
  echo "only shebangs and SPDX headers are exempt."
  exit 1
fi
exit 0
