#!/usr/bin/env bash
# Captures one Zig or Rust Zant app as PNG + layout dump for the parity check
# (docs/rust-port/plan.md §6, WP-0.4).
#
#   scripts/parity/capture.sh <zig|rust> <binary> <outdir> [light|dark] [WxH]
#
# zig  : zig-out-baseline/bin/<binary>   (ReleaseFast baseline, WP-0.1)
# rust : target/release/<binary>         (release build, no debug redraws)
#
# Environment used by both implementations:
#   ZANT_SNAPSHOT / ZANT_SNAPSHOT_EXIT / ZANT_LAYOUT_FILE / ZANT_NO_MOTION
#   ZANT_THEME=light|dark  (TSX apps) and ZANT_DARK=1 (Zig examples, dark only)
#   ZANT_WINDOW_SIZE=WxH   (automation hook added to Zig platform.createWindow)
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"

if [ $# -lt 3 ]; then
    sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
fi
side="$1"
binary="$2"
out="$3"
theme="${4:-light}"
size="${5:-}"

# ZANT_ZIG_BIN / ZANT_RUST_BIN override the binary directories (used by
# compare.sh's self-check, which compares the Zig baseline with itself).
case "$side" in
    zig) bin="${ZANT_ZIG_BIN:-$root/zig-out-baseline/bin}/$binary" ;;
    rust) bin="${ZANT_RUST_BIN:-$root/target/release}/$binary" ;;
    *)
        echo "capture.sh: side must be 'zig' or 'rust' (got '$side')" >&2
        exit 2
        ;;
esac
if [ ! -x "$bin" ]; then
    echo "capture.sh: missing binary: $bin" >&2
    echo "  zig:  zig build -Doptimize=ReleaseFast --prefix zig-out-baseline" >&2
    echo "  rust: cargo build --release -p zant-examples --bin $binary" >&2
    exit 2
fi
case "$theme" in
    light | dark) ;;
    *)
        echo "capture.sh: theme must be 'light' or 'dark' (got '$theme')" >&2
        exit 2
        ;;
esac

mkdir -p "$out"
rm -f "$out/shot.png" "$out/layout.txt" "$out/stdout.log" "$out/stderr.log"

export ZANT_NO_MOTION=1
export ZANT_SNAPSHOT="$out/shot.png"
export ZANT_SNAPSHOT_EXIT=1
export ZANT_LAYOUT_FILE="$out/layout.txt"
export ZANT_THEME="$theme"
if [ "$theme" = dark ]; then
    export ZANT_DARK=1
else
    unset ZANT_DARK
fi
if [ -n "$size" ]; then
    export ZANT_WINDOW_SIZE="$size"
else
    unset ZANT_WINDOW_SIZE
fi

# Snapshot is scheduled 600ms after the first frame; the process exits once the
# PNG is on disk. The timeout only guards a hung window/event loop.
if ! timeout "${ZANT_CAPTURE_TIMEOUT:-90}" "$bin" >"$out/stdout.log" 2>"$out/stderr.log"; then
    code=$?
    echo "capture.sh: $bin exited with $code" >&2
    tail -20 "$out/stderr.log" >&2 || true
    exit 1
fi
for wanted in shot.png layout.txt; do
    if [ ! -s "$out/$wanted" ]; then
        echo "capture.sh: $bin produced no $wanted in $out" >&2
        tail -20 "$out/stderr.log" >&2 || true
        exit 1
    fi
done
echo "captured $side $binary ($theme${size:+, $size}) -> $out"
