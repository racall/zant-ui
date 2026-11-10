#!/bin/bash
# Robustness check of zant-tsx against SWC's parser fixtures (TypeScript, TSX
# and JSX inputs; https://github.com/swc-project/swc, Apache-2.0). The
# fixtures are not part of this repository: clone SWC somewhere first.
#
#   git clone --depth 1 https://github.com/swc-project/swc.git /tmp/swc
#   scripts/tsx/swc-corpus.sh /tmp/swc
#
# Every input must either compile or be rejected with a positioned error:
# a crash (signal / exit code > 1) or a hang (5 s) is a bug. Prints the
# rejection messages grouped by kind: the most common ones are the TS / React
# syntax worth supporting next.
set -u
swc=${1:?usage: swc-corpus.sh <swc clone>}
tests="$swc/crates/swc_ecma_parser/tests"
tool="$(cd "$(dirname "$0")/../.." && pwd)/zig-out/bin/zant-tsx"
[ -x "$tool" ] || { echo "build zant-tsx first: zig build install"; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(cd "$tests" && { find . \( -name '*.tsx' -o -name 'input.ts' \) -not -path '*flow*'; find . -path '*jsx*' -name 'input.js'; }) > "$work/files"
compiled=0 rejected=0 crashed=0 hung=0
while read -r f; do
  timeout 5 "$tool" "$tests/$f" "$work/out.zig" > /dev/null 2> "$work/err"
  code=$?
  case $code in
    0) compiled=$((compiled + 1)) ;;
    1) rejected=$((rejected + 1)); head -1 "$work/err" | sed -E 's/^[^:]+:[0-9]+:[0-9]+: error: //' >> "$work/messages" ;;
    124) hung=$((hung + 1)); echo "hang: $f" ;;
    *) crashed=$((crashed + 1)); echo "crash ($code): $f" ;;
  esac
done < "$work/files"
echo "inputs=$(wc -l < "$work/files") compiled=$compiled rejected=$rejected crashed=$crashed hung=$hung"
echo "most common rejections:"
sed -E 's/`[^`]*`/`…`/g; s/<[A-Za-z.]+>/<…>/g; s/"[^"]*"/"…"/g' "$work/messages" | sort | uniq -c | sort -rn | head -25
[ $crashed -eq 0 ] && [ $hung -eq 0 ]
