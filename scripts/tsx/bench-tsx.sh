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
#   ZANT_BENCH_REPEAT=5 scripts/tsx/bench-tsx.sh # more rounds
#
# Each side runs `ZANT_BENCH_REPEAT` (default 3) timed rounds, alternating which
# side goes first; the best round per side (lowest average frame time) is
# compared, like `scripts/bench.sh`, so shared-machine load does not
# decide the gate. The gate applies to every app row and every scenario row:
# ratio ≤ `ZANT_BENCH_MAX_RATIO` (default 1.2). When a row is over budget,
# rerun with `ZANT_FRAME_STATS=1` to get the runtime's per-second `[frames]` /
# `[slow-frame]` rebuild / layout / paint breakdown.
#
# Environment:
#   ZANT_BENCH_REPEAT      rounds per side, best round wins (default 3)
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
    sed -n '2,48p' "$0" | sed 's/^# \{0,1\}//'
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

rounds="${ZANT_BENCH_REPEAT:-3}"
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

echo "bench-tsx.sh: best of $rounds round(s) per side, gate <= ${gate}x hand-written Rust; raw log: $outdir/run.log" >&2
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
import re
import sys

path, gate = sys.argv[1], float(sys.argv[2])
row_re = re.compile(
    r"^(app|scenario) (\S+) frames=(\d+) tsx_avg=([0-9.]+) manual_avg=([0-9.]+) "
    r"ratio=([0-9.]+) tsx_p95=([0-9.]+) manual_p95=([0-9.]+) "
    r"tsx_max=([0-9.]+) manual_max=([0-9.]+)$"
)
rows = []
gate_line = ""
with open(path, encoding="utf-8") as log:
    for line in log:
        line = line.strip()
        if line.startswith("gate:"):
            gate_line = line
            continue
        match = row_re.match(line)
        if not match:
            continue
        rows.append(
            {
                "kind": match.group(1),
                "name": match.group(2),
                "frames": int(match.group(3)),
                "tsx_avg": float(match.group(4)),
                "manual_avg": float(match.group(5)),
                "ratio": float(match.group(6)),
                "tsx_p95": float(match.group(7)),
                "manual_p95": float(match.group(8)),
                "tsx_max": float(match.group(9)),
                "manual_max": float(match.group(10)),
            }
        )
if not rows:
    sys.exit("bench-tsx.sh: no result rows in the benchmark output")

print()
print(f"== bench-tsx: TSX-generated vs hand-written Rust frames, gate {gate:.2f}x ==")
print(
    f"{'row':<22} {'frames':>6}  {'tsx avg':>8} {'manual avg':>10} {'ratio':>6}"
    f"  {'tsx p95':>8} {'manual p95':>10} {'tsx max':>8} {'manual max':>10}  verdict"
)
failures = 0
for row in rows:
    ok = row["ratio"] <= gate
    if not ok:
        failures += 1
    # app rows are the six acceptance rows; scenario rows are indented details
    label = row["name"] if row["kind"] == "app" else "  " + row["name"]
    verdict = "PASS" if ok else f"FAIL (ratio > {gate:.2f}x)"
    print(
        f"{label:<22} {row['frames']:>6}  {row['tsx_avg']:>8.3f} {row['manual_avg']:>10.3f}"
        f" {row['ratio']:>5.2f}x  {row['tsx_p95']:>8.3f} {row['manual_p95']:>10.3f}"
        f" {row['tsx_max']:>8.3f} {row['manual_max']:>10.3f}  {verdict}"
    )

apps = sum(1 for row in rows if row["kind"] == "app")
scenarios = sum(1 for row in rows if row["kind"] == "scenario")
print()
if failures:
    print(f"gate: FAIL — {failures} / {len(rows)} rows over {gate:.2f}x hand-written Rust")
    print("profile a failing row with:")
    print("  ZANT_FRAME_STATS=1 scripts/tsx/bench-tsx.sh <row-prefix>")
    sys.exit(1)
print(f"gate: PASS — {apps} apps, {scenarios} scenarios within {gate:.2f}x hand-written Rust")
PY

echo
if [ "$table_status" -ne 0 ] || [ "$status" -ne 0 ]; then
    echo "bench-tsx.sh: frame gate FAILED; raw log: $outdir/run.log" >&2
    exit 1
fi
echo "bench-tsx.sh: frame gate PASSED; raw log: $outdir/run.log"
