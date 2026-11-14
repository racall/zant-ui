#!/usr/bin/env bash
# zant-components parity that scripts/parity/compare.sh cannot report on its
# own (see crates/zant-examples/README.md):
#
#   * the Zig `ui.pdfViewer` writes the layout key `pdf-2002` twice, which
#     scripts/parity/layoutdiff.py rejects as a duplicate key, and
#   * the PDF viewer is not ported yet (WP-3.9), so its inner keys exist only
#     on the Zig side; the Rust example renders a placeholder in its slot.
#
# This script compares light/dark x 600x620 / 900x640 / 1200x800 with the
# screenshot unchanged (imgdiff.py) and the layout dump with duplicate keys
# and `pdf-2002*` filtered out (filter_layout.py + layoutdiff.py).
#
#   ZANT_ZIG_BIN=/path/to/zig-out-baseline/bin crates/zant-examples/parity/components_parity.sh
#
# Zig baseline binaries default to <repo>/zig-out-baseline/bin (a migration-era
# artifact; the Zig tree was removed at M8, docs/rust-port/m8-cutover.md); Rust
# binaries to <repo>/target/release (same as scripts/parity/capture.sh).
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
out="${ZANT_PARITY_OUT:-$root/target/parity/zant-components-filtered}"
python="${ZANT_PYTHON:-python3}"
sizes=(${ZANT_PARITY_SIZES:-600x620 900x640 1200x800})
themes=(${ZANT_PARITY_THEMES:-light dark})

mkdir -p "$out"
failures=0
declare -a results=()
for theme in "${themes[@]}"; do
    for size in "${sizes[@]}"; do
        name="$theme-$size"
        zdir="$out/zig-$name"
        rdir="$out/rust-$name"
        "$root/scripts/parity/capture.sh" zig zant-components "$zdir" "$theme" "$size"
        "$root/scripts/parity/capture.sh" rust zant-components "$rdir" "$theme" "$size"

        image_ok=1
        layout_ok=1
        "$python" "$root/scripts/parity/imgdiff.py" "$zdir/shot.png" "$rdir/shot.png" \
            --diff "$out/diff-$name.png" || image_ok=0
        "$python" "$root/crates/zant-examples/parity/filter_layout.py" \
            "$zdir/layout.txt" "$out/zig-$name-layout.txt" --drop-prefix pdf- >/dev/null
        "$python" "$root/crates/zant-examples/parity/filter_layout.py" \
            "$rdir/layout.txt" "$out/rust-$name-layout.txt" --drop-prefix pdf- >/dev/null
        "$python" "$root/scripts/parity/layoutdiff.py" \
            "$out/zig-$name-layout.txt" "$out/rust-$name-layout.txt" || layout_ok=0

        if [ "$layout_ok" = 1 ] && [ "$image_ok" = 1 ]; then
            results+=("PASS $name")
        else
            results+=("FAIL $name (layout=$layout_ok image=$image_ok)")
            failures=$((failures + 1))
        fi
    done
done

echo
echo "== parity: zant-components (filtered: screenshot + non-pdf layout keys) =="
printf '%s\n' "${results[@]}"
if [ "$failures" -ne 0 ]; then
    echo "$failures / ${#results[@]} combinations differ (artifacts: $out)" >&2
    exit 1
fi
echo "all ${#results[@]} combinations match"
