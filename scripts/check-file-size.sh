#!/usr/bin/env bash
# Keeps source files at about 500 lines (CLAUDE.md): a file over the limit
# fails unless it is in the baseline, and a baseline file may only shrink.
#
#   scripts/check-file-size.sh            # check
#   scripts/check-file-size.sh --update   # rewrite the baseline (only to record files that shrank or were split)
#
# Generated code (the TSX tag table, bindgen output) and test data are not
# counted. Split a baseline file into modules, then run --update so its entry
# disappears.
set -euo pipefail
cd "$(dirname "$0")/.."
LIMIT=500
BASELINE=scripts/file-size-baseline.txt

current() {
  git ls-files -co --exclude-standard -- '*.rs' '*.m' '*.mm' '*.c' '*.cpp' '*.h' |
    grep -v -E '^crates/zant-sys/src/(skia|platform|pdf)\.rs$|/testdata/|/snapshots/|/fixtures/|^third_party/' |
    while IFS= read -r file; do
      [ -f "$file" ] || continue
      lines=$(wc -l < "$file" | tr -d ' ')
      if [ "$lines" -gt "$LIMIT" ]; then printf '%s %s\n' "$lines" "$file"; fi
    done | sort -k2
}

if [ "${1:-}" = "--update" ]; then
  {
    echo "# Files over $LIMIT lines that predate scripts/check-file-size.sh, with their"
    echo "# line count when recorded. They may only shrink; split them into modules."
    current
  } > "$BASELINE"
  echo "check-file-size: wrote $BASELINE ($(grep -vc '^#' "$BASELINE") files)"
  exit 0
fi

status=0
while read -r lines file; do
  allowed=$(awk -v f="$file" '$2 == f { print $1 }' "$BASELINE" 2>/dev/null || true)
  if [ -z "$allowed" ]; then
    echo "check-file-size: $file has $lines lines (limit $LIMIT): split it into modules"
    status=1
  elif [ "$lines" -gt "$allowed" ]; then
    echo "check-file-size: $file grew to $lines lines (baseline $allowed): split it instead of growing it"
    status=1
  fi
done < <(current)
[ $status -eq 0 ] && echo "check-file-size: ok ($(grep -vc '^#' "$BASELINE") files left to split)"
exit $status
