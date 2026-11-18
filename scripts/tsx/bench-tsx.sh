#!/usr/bin/env bash
# TSX frame benchmark gate:
#
#   TSX 生成代码不慢于手写 Rust 对照组 1.2 倍
#
# `zant-tsx-frame-bench` (`crates/zant-examples/src/bin/`) runs each TSX
# benchmark page against a hand-written `ui::App` control with the same UI
# tree, through the same headless frame loop:
#
#   counter   counter.tsx        vs hand-written counter control    clicks + typing
#   todos     todos.tsx          vs hand-written todos control      add + toggle + filter
#   list      list_bench.tsx*    vs hand-written list control       scroll + row clicks
#   local     local_bench.tsx    vs hand-written local-state rows   mount + counter clicks
#   markdown  markdown_bench.tsx vs hand-held Streamdown cache      streaming frames
#   settings  settings.tsx       vs hand-written settings control   2000 switch clicks
#
# `*` list rows are built by the page itself because Rust root state is private;
# see the file header of `tsx_views/ListBench.tsx` and `docs/tsx.md`.
#
# The TSX side is not a stub: this script builds the Release binary, whose
# `build.rs` compiles the real `crates/zant-examples/tsx/corpus/*.tsx` inputs with
# `zant_build::tsx_app_in` (the same step a Zant app's build script runs) and
# includes the generated modules; the hand-written controls live in
# `src/bin/zant_tsx_frame_bench/controls/`. One binary measures both sides with
# the same raster canvas size, fake clock, warm-up frame and Release profile, so
# the comparison is apples to apples.
#
#   scripts/tsx/bench-tsx.sh                     # all six apps
#   scripts/tsx/bench-tsx.sh counter list        # some of them
#   ZANT_BENCH_REPEAT=7 scripts/tsx/bench-tsx.sh # more rounds
#
# Each paired round compares median frame times, alternating which side goes
# first. The gate uses the median of those ratios. App rows pool scenarios'
# frames within each round. Independent manual/manual pairs give the A/A
# ratio and noise (maximum absolute deviation from 1). Noise above 0.10 is
# marked for reference. Rerun with ZANT_FRAME_STATS=1 for a phase breakdown.
#
# Environment:
#   ZANT_BENCH_REPEAT      paired rounds (default 5)
#   ZANT_BENCH_SLOW_TSX    multiply TSX frame time by busy-waiting (testing only)
#   ZANT_BENCH_MAX_RATIO   ratio gate (default 1.20)
#   ZANT_FRAME_STATS=1     forward the runtime's frame-phase statistics (stderr)
#   ZANT_RUST_BIN          directory with the binary (default target/release)
#   CARGO_TARGET_DIR       build directory (default <repo>/target)
#
# Options:
#   --no-build   use the existing Release binary instead of building it
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    sed -n '1d; /^#/!q; p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
fi

build=1
scenarios=()
for arg in "$@"; do
    case "$arg" in
        --no-build) build=0 ;;
        -*) echo "bench-tsx.sh: unknown option '$arg' (use --help)" >&2; exit 2 ;;
        *) scenarios+=("$arg") ;;
    esac
done

python="${ZANT_PYTHON:-python3}"
if ! command -v "$python" >/dev/null 2>&1; then
    echo "bench-tsx.sh: python3 not found (needed to format / gate the numbers)" >&2
    exit 2
fi

rounds="${ZANT_BENCH_REPEAT:-5}"
case "$rounds" in
    ''|*[!0-9]*) echo "bench-tsx.sh: ZANT_BENCH_REPEAT must be a positive integer (got '$rounds')" >&2; exit 2 ;;
esac
if [ "$rounds" -lt 1 ]; then rounds=1; fi
gate="${ZANT_BENCH_MAX_RATIO:-1.2}"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
bin_dir="${ZANT_RUST_BIN:-$target_dir/release}"
outdir="$target_dir/tsx/bench-tsx"
mkdir -p "$outdir"

if [ "$build" = 1 ]; then
    echo "bench-tsx.sh: building the Release benchmark (TSX pages are compiled by build.rs)" >&2
    # Both sides live in this binary: `build.rs` compiles
    # `tsx/corpus/*.tsx` through zant-build, the controls are hand-written Rust.
    "$root/scripts/cargo.sh" build --release -p zant-examples --bin zant-tsx-frame-bench
    # The TSX side really was generated (not a stale / missing build script run):
    # one generated `main.rs` per benchmark page under $OUT_DIR.
    generated=0
    for main_rs in "$target_dir"/release/build/zant-examples-*/out/tsx-frame-bench/cases/*/main.rs; do
        [ -f "$main_rs" ] && generated=$((generated + 1))
    done
    if [ "$generated" -lt 6 ]; then
        echo "bench-tsx.sh: expected 6 generated TSX apps under target/release/build, found $generated" >&2
        echo "  (the TSX → Rust step of crates/zant-examples/build.rs did not produce them)" >&2
        exit 2
    fi
    echo "bench-tsx.sh: $generated generated TSX apps included in the benchmark" >&2
fi

binary="$bin_dir/zant-tsx-frame-bench"
if [ ! -x "$binary" ]; then
    echo "bench-tsx.sh: missing Release binary: $binary" >&2
    echo "  build it: scripts/cargo.sh build --release -p zant-examples --bin zant-tsx-frame-bench" >&2
    exit 2
fi

echo "bench-tsx.sh: median of $rounds paired round(s), gate <= ${gate}x hand-written Rust; raw log: $outdir/run.log" >&2
if env | grep -q '^ZANT_FRAME_STATS=1'; then
    echo "bench-tsx.sh: ZANT_FRAME_STATS=1 — frame-phase lines follow on stderr" >&2
fi

status=0
ZANT_BENCH_REPEAT="$rounds" ZANT_BENCH_MAX_RATIO="$gate" \
    "$binary" ${scenarios[@]+"${scenarios[@]}"} \
    >"$outdir/run.log" 2>"$outdir/frame-stats.log" || status=$?
# `ZANT_FRAME_STATS=1` writes the runtime's per-second phase lines on stderr;
# show them (a plain file redirect: process substitution is not available in
# every sandbox).
if [ -s "$outdir/frame-stats.log" ]; then
    cat "$outdir/frame-stats.log" >&2
fi

if [ ! -s "$outdir/run.log" ]; then
    echo "bench-tsx.sh: the benchmark produced no output (exit $status)" >&2
    exit 2
fi

table_status=0
"$python" - "$outdir/run.log" "$gate" <<'PY' || table_status=$?
import sys

path, gate = sys.argv[1], float(sys.argv[2])
rows = []
over_budget = set()
gate_line = ""
with open(path, encoding="utf-8") as log:
    for line in log:
        fields = line.split()
        if not fields:
            continue
        if fields[0] in ("app", "scenario"):
            kind, name, *values = fields
            stats = dict(value.split("=", 1) for value in values if "=" in value)
            rows.append((kind, name, stats, "noisy" in values))
        elif fields[0] == "over-budget":
            over_budget.add(tuple(fields[1:3]))
        elif fields[0] == "gate:":
            gate_line = line.strip()
if not rows or not gate_line:
    sys.exit("bench-tsx.sh: incomplete benchmark output")

print()
print(f"== bench-tsx: TSX-generated vs hand-written Rust frames, gate {gate:.2f}x ==")
print(
    f"{'row':<22} {'frames':>6}  {'tsx median':>10} {'manual median':>13}"
    f" {'paired ratio':>12} {'A/A ratio':>9} {'noise':>7}"
    f"  {'tsx avg':>8} {'manual avg':>10} {'tsx p95':>8} {'manual p95':>10} {'tsx max':>8} {'manual max':>10}  verdict"
)
for kind, name, stats, noisy in rows:
    label = name if kind == "app" else "  " + name
    # The binary compares the unrounded ratio.
    verdict = "FAIL" if (kind, name) in over_budget else "PASS"
    marker = " noisy" if noisy else ""
    print(
        f"{label:<22} {stats['frames']:>6}  {stats['tsx_median']:>10} {stats['manual_median']:>13}"
        f" {stats['ratio']:>11}x {stats['aa_ratio']:>8}x {stats['noise']:>7}"
        f"  {stats['tsx_avg']:>8} {stats['manual_avg']:>10} {stats['tsx_p95']:>8} {stats['manual_p95']:>10} {stats['tsx_max']:>8}"
        f" {stats['manual_max']:>10}  {verdict}{marker}"
    )

print()
print(gate_line)
if over_budget:
    print("profile a failing row with:")
    print("  ZANT_FRAME_STATS=1 scripts/tsx/bench-tsx.sh <row-prefix>")
    sys.exit(1)
PY

echo
if [ "$table_status" -ne 0 ] || [ "$status" -ne 0 ]; then
    echo "bench-tsx.sh: frame gate FAILED; raw log: $outdir/run.log" >&2
    exit 1
fi
echo "bench-tsx.sh: frame gate PASSED; raw log: $outdir/run.log"
