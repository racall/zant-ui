#!/usr/bin/env bash
# Compares a Zig baseline example with its Rust port across light/dark themes
# and three window sizes (docs/rust-port/plan.md §6, WP-0.4 / M1-M5 acceptance).
#
#   scripts/parity/compare.sh <binary> [outdir]
#
# Layout dumps must have the same keys and coordinates within 0.01pt; PNGs must
# match except for <= 0.1% of pixels whose channels differ by > 2. Mismatches
# write a diff image and exit non-zero. Override the matrix with
#   ZANT_PARITY_SIZES="600x620 900x640 1200x800" (default)
#   ZANT_PARITY_THEMES="light dark"
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
if [ $# -lt 1 ]; then
    sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
fi
binary="$1"
out="${2:-$root/target/parity/$binary}"
sizes=(${ZANT_PARITY_SIZES:-600x620 900x640 1200x800})
themes=(${ZANT_PARITY_THEMES:-light dark})
python="${ZANT_PYTHON:-python3}"

if ! command -v "$python" >/dev/null 2>&1; then
    echo "compare.sh: python3 not found (needed by imgdiff.py / layoutdiff.py)" >&2
    exit 2
fi

failures=0
declare -a results=()
for theme in "${themes[@]}"; do
    for size in "${sizes[@]}"; do
        name="$theme-$size"
        zdir="$out/zig-$name"
        rdir="$out/rust-$name"
        "$root/scripts/parity/capture.sh" zig "$binary" "$zdir" "$theme" "$size"
        "$root/scripts/parity/capture.sh" rust "$binary" "$rdir" "$theme" "$size"

        layout_ok=1
        image_ok=1
        "$python" "$root/scripts/parity/layoutdiff.py" "$zdir/layout.txt" "$rdir/layout.txt" || layout_ok=0
        "$python" "$root/scripts/parity/imgdiff.py" "$zdir/shot.png" "$rdir/shot.png" \
            --diff "$out/diff-$name.png" || image_ok=0
        if [ "$layout_ok" = 1 ] && [ "$image_ok" = 1 ]; then
            results+=("PASS $name")
        else
            results+=("FAIL $name (layout=$layout_ok image=$image_ok)")
            failures=$((failures + 1))
        fi
    done
done

echo
echo "== parity: $binary =="
printf '%s\n' "${results[@]}"
if [ "$failures" -ne 0 ]; then
    echo "$failures / ${#results[@]} combinations differ (artifacts: $out)" >&2
    exit 1
fi
echo "all ${#results[@]} combinations match"
