#!/usr/bin/env bash
# zant-gallery image parity: the Zig imperative gallery (src/app.zig) never
# writes ZANT_LAYOUT_FILE (that is declarative-runtime code) and its hand-built
# tree has no keys, so scripts/parity/capture.sh cannot capture a layout dump
# and scripts/parity/compare.sh stops with "produced no layout.txt".
#
# This script runs both binaries with the same environment and compares the
# screenshots with scripts/parity/imgdiff.py:
#
#   ZANT_ZIG_BIN=/path/to/zig-out-baseline/bin crates/zant-examples/parity/gallery_image_parity.sh
#
# Zig baseline binaries default to <repo>/zig-out-baseline/bin (a migration-era
# artifact; the Zig tree was removed at M8); Rust binaries to <repo>/target/release.
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
zig_bin_dir="${ZANT_ZIG_BIN:-$root/zig-out-baseline/bin}"
zig="$zig_bin_dir/zant-gallery"
rust="${ZANT_RUST_BIN:-$root/target/release}/zant-gallery"
out="${ZANT_PARITY_OUT:-$root/target/parity/zant-gallery-image}"
python="${ZANT_PYTHON:-python3}"
sizes=(${ZANT_PARITY_SIZES:-600x620 900x640 1200x800})
themes=(${ZANT_PARITY_THEMES:-light dark})
timeout_s="${ZANT_CAPTURE_TIMEOUT:-90}"

for bin in "$zig" "$rust"; do
    if [ ! -x "$bin" ]; then
        echo "gallery_image_parity.sh: missing binary: $bin" >&2
        echo "  zig:  point ZANT_ZIG_BIN at a saved zig-out-baseline (migration-era artifact)" >&2
        echo "  rust: cargo build --release -p zant-examples --bin zant-gallery" >&2
        exit 2
    fi
done

mkdir -p "$out"
failures=0
declare -a results=()
for theme in "${themes[@]}"; do
    for size in "${sizes[@]}"; do
        name="$theme-$size"
        for side in zig rust; do
            dir="$out/$side-$name"
            mkdir -p "$dir"
            rm -f "$dir/shot.png" "$dir/stdout.log" "$dir/stderr.log"
            bin="$zig"
            [ "$side" = rust ] && bin="$rust"
            ZANT_NO_MOTION=1 \
            ZANT_SNAPSHOT="$dir/shot.png" \
            ZANT_SNAPSHOT_EXIT=1 \
            ZANT_THEME="$theme" \
            ZANT_WINDOW_SIZE="$size" \
            timeout "$timeout_s" "$bin" >"$dir/stdout.log" 2>"$dir/stderr.log" || {
                echo "gallery_image_parity.sh: $side $name exited non-zero" >&2
                tail -5 "$dir/stderr.log" >&2 || true
                exit 1
            }
            if [ ! -s "$dir/shot.png" ]; then
                echo "gallery_image_parity.sh: $side $name produced no shot.png" >&2
                exit 1
            fi
        done
        if "$python" "$root/scripts/parity/imgdiff.py" \
            "$out/zig-$name/shot.png" "$out/rust-$name/shot.png" \
            --diff "$out/diff-$name.png"; then
            results+=("PASS $name")
        else
            results+=("FAIL $name")
            failures=$((failures + 1))
        fi
    done
done

echo
echo "== parity: zant-gallery (screenshot only; the Zig tree has no layout keys) =="
printf '%s\n' "${results[@]}"
if [ "$failures" -ne 0 ]; then
    echo "$failures / ${#results[@]} combinations differ (artifacts: $out)" >&2
    exit 1
fi
echo "all ${#results[@]} combinations match"
