#!/usr/bin/env bash
# Frame benchmark gate: runs the headless frame benchmark (`zant-bench`) and
# compares every scenario's frame time with the recorded baseline below.
#
#   * every scenario's average frame time must stay within 1.25x of the
#     recorded baseline;
#   * steady-state frames must stay far below 16.7 ms (60 fps with margin;
#     the `open` scenario is a cold first frame and only gets the ratio gate).
#
#   scripts/bench.sh [scenario...]
#
# Scenarios: open stream typing caret hover resize scroll (all when omitted).
# Both render modes run: windowless full rebuild and ZANT_BENCH_LIST=1 virtual
# list. The benchmark runs ZANT_BENCH_REPEAT rounds and the best round per
# scenario is compared, so a loaded machine does not decide the gate. Raw
# outputs are kept under target/bench/.
#
# The recorded baseline (recorded_default / recorded_list below) was measured
# on macOS with the default chat length; update the rows when a deliberate
# change moves the numbers.
#
# Environment:
#   ZANT_RUST_BIN         directory with the binary (default: target/release)
#   ZANT_BENCH_MESSAGES   chat length, forwarded to the binary (default 30;
#                         the recorded baseline assumes the default)
#   ZANT_BENCH_MODES      "default list" (default: both)
#   ZANT_BENCH_REPEAT     rounds, best round wins (default 3)
#   ZANT_BENCH_MAX_RATIO  ratio gate (default 1.25)
#   ZANT_BENCH_MAX_MS     absolute average-frame budget (default 16.7)
#   ZANT_PYTHON           python3
#
# Options:
#   --no-build   use the existing target/release/zant-bench instead of building
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    sed -n '2,33p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
fi

build=1
scenarios=()
for arg in "$@"; do
    case "$arg" in
        --no-build) build=0 ;;
        -*)
            echo "bench.sh: unknown option '$arg' (use --help)" >&2
            exit 2
            ;;
        *) scenarios+=("$arg") ;;
    esac
done

python="${ZANT_PYTHON:-python3}"
if ! command -v "$python" >/dev/null 2>&1; then
    echo "bench.sh: python3 not found (needed to parse / compare the numbers)" >&2
    exit 2
fi

rounds="${ZANT_BENCH_REPEAT:-3}"
case "$rounds" in
    ''|*[!0-9]*)
        echo "bench.sh: ZANT_BENCH_REPEAT must be a positive integer (got '$rounds')" >&2
        exit 2
        ;;
esac
if [ "$rounds" -lt 1 ]; then
    rounds=1
fi

# ---- locate / build the benchmark ------------------------------------------
bin_dir="${ZANT_RUST_BIN:-$root/target/release}"
if [ "$build" = 1 ]; then
    echo "bench.sh: building zant-bench (release) with cargo" >&2
    "$root/scripts/cargo.sh" build --release -p zant-examples --bin zant-bench
fi

if [ ! -x "$bin_dir/zant-bench" ]; then
    echo "bench.sh: missing binary: $bin_dir/zant-bench" >&2
    echo "  build it: cargo build --release -p zant-examples --bin zant-bench" >&2
    exit 2
fi

# ---- the recorded baseline --------------------------------------------------
# One row per scenario: name, frames, avg, p95, max (ms).
recorded_default() {
    cat <<'ROWS'
open       5 120.88 143.71 143.71
stream   186   2.24   2.82   3.21
typing   200   0.84   1.04   1.12
caret    100   0.67   0.79   0.83
hover    300   0.35   0.43   0.54
resize   120   4.69   4.69  82.64
scroll   300   0.75   1.01   1.29
ROWS
}
recorded_list() {
    cat <<'ROWS'
open       5  20.98  41.48  41.48
stream   186   0.85   1.09   4.50
typing   200   0.69   0.90   1.06
caret    100   0.64   0.77   0.88
hover    300   0.35   0.39   0.55
resize   120   0.72   0.93   3.98
scroll   300   0.62   0.74   0.90
ROWS
}

# ---- run the benchmark for each mode ----------------------------------------
modes=(${ZANT_BENCH_MODES:-default list})
outdir="$root/target/bench"
mkdir -p "$outdir"

env_args=()
if [ -n "${ZANT_BENCH_MESSAGES:-}" ]; then
    env_args+=(ZANT_BENCH_MESSAGES="$ZANT_BENCH_MESSAGES")
fi

failures=0
tables=0
for mode in "${modes[@]}"; do
    case "$mode" in
        default)
            env_line=()
            recorded_rows=recorded_default
            ;;
        list)
            env_line=(ZANT_BENCH_LIST=1)
            recorded_rows=recorded_list
            ;;
        *)
            echo "bench.sh: ZANT_BENCH_MODES entries must be 'default' or 'list' (got '$mode')" >&2
            exit 2
            ;;
    esac
    # The comparison reads the baseline in the benchmark's own output format.
    "$recorded_rows" | while read -r name frames avg p95 max; do
        printf '%s frames=%4d avg=%7.2f ms p95=%7.2f ms max=%7.2f ms\n' \
            "$name" "$frames" "$avg" "$p95" "$max"
    done >"$outdir/baseline-$mode.log"

    # The benchmark prints the header + summaries on stderr. The comparison
    # below keeps the best round per scenario.
    # bash 3.2 (macOS): an empty array under `set -u` needs the +"${a[@]}" form.
    rm -f "$outdir"/run-"$mode"-*.log
    round=1
    while [ "$round" -le "$rounds" ]; do
        env "${env_line[@]+"${env_line[@]}"}" "${env_args[@]+"${env_args[@]}"}" \
            "$bin_dir/zant-bench" "${scenarios[@]+"${scenarios[@]}"}" \
            >/dev/null 2>"$outdir/run-$mode-$round.log" || true
        round=$((round + 1))
    done

    if ! "$python" - "$outdir" "$mode" "$rounds" "${scenarios[@]+"${scenarios[@]}"}" <<'PY'
import os
import re
import sys

outdir, mode, rounds = sys.argv[1], sys.argv[2], int(sys.argv[3])
wanted = sys.argv[4:]
canonical = ["open", "stream", "typing", "caret", "hover", "resize", "scroll"]
line_re = re.compile(
    r"^([a-z]+)\s+frames=\s*(\d+)\s+avg=\s*([0-9.]+) ms\s+p95=\s*([0-9.]+) ms\s+max=\s*([0-9.]+) ms"
)


def parse(paths):
    """Best row per scenario: the one with the lowest average frame time."""
    best = {}
    for path in paths:
        if not os.path.exists(path):
            continue
        with open(path, encoding="utf-8", errors="replace") as handle:
            for line in handle:
                match = line_re.match(line.strip())
                if not match:
                    continue
                name = match.group(1)
                row = {
                    "frames": int(match.group(2)),
                    "avg": float(match.group(3)),
                    "p95": float(match.group(4)),
                    "max": float(match.group(5)),
                }
                if name not in best or row["avg"] < best[name]["avg"]:
                    best[name] = row
    return best


base = parse([os.path.join(outdir, f"baseline-{mode}.log")])
run = parse(
    [os.path.join(outdir, f"run-{mode}-{index}.log") for index in range(1, rounds + 1)]
)
if not run:
    print(f"bench.sh: no benchmark output under {outdir} for mode {mode}", file=sys.stderr)
    sys.exit(2)

order = wanted if wanted else [name for name in canonical if name in base]
order = [name for name in order if name in base and name in run]
if wanted:
    missing = [name for name in wanted if name not in base or name not in run]
    if missing:
        print(f"bench.sh: no result for: {', '.join(missing)}", file=sys.stderr)
        sys.exit(2)

max_ratio = float(os.environ.get("ZANT_BENCH_MAX_RATIO", "1.25"))
max_ms = float(os.environ.get("ZANT_BENCH_MAX_MS", "16.7"))

label = "default (full rebuild)" if mode == "default" else "virtual list (ZANT_BENCH_LIST=1)"
print()
print(f"== zant-bench: {label}; best of {rounds} round(s); recorded baseline ==")
print(
    f"{'scenario':<9} {'frames':>6}  {'avg':>9} {'avg base':>8} {'ratio':>6}"
    f"  {'p95':>8} {'max':>8} {'p95 base':>8} {'max base':>8}  verdict"
)
failures = 0
for name in order:
    b, r = base[name], run[name]
    ratio = r["avg"] / b["avg"] if b["avg"] > 0 else float("inf")
    ratio_ok = ratio <= max_ratio
    # The absolute 60 fps budget only applies to steady-state scenarios: the
    # baseline cold `open` frame is itself over 16.7 ms, so only the ratio
    # counts there.
    steady = b["avg"] <= max_ms
    abs_ok = r["avg"] <= max_ms
    reasons = []
    if not ratio_ok:
        reasons.append(f"ratio {ratio:.2f}x > {max_ratio:.2f}x")
    if steady and not abs_ok:
        reasons.append(f"avg {r['avg']:.2f} ms > {max_ms:.1f} ms")
    if reasons:
        failures += 1
        verdict = "FAIL: " + ", ".join(reasons)
    elif steady:
        verdict = "PASS"
    else:
        verdict = "PASS* (ratio only: cold start, baseline itself > 16.7 ms)"
    print(
        f"{name:<9} {r['frames']:>6}  {r['avg']:>9.2f} {b['avg']:>8.2f} {ratio:>5.2f}x"
        f"  {r['p95']:>8.2f} {r['max']:>8.2f} {b['p95']:>8.2f} {b['max']:>8.2f}  {verdict}"
    )

score = len(order) - failures
if failures:
    print(f"gate: FAIL — {failures} / {len(order)} scenarios over budget")
    sys.exit(1)
print(f"gate: PASS — {score} / {len(order)} scenarios within {max_ratio:.2f}x of the baseline")
PY
    then
        failures=$((failures + 1))
    fi
    tables=$((tables + 1))
done

echo
if [ "$failures" -ne 0 ]; then
    echo "bench.sh: performance gate FAILED ($failures mode(s)); raw logs: $outdir" >&2
    exit 1
fi
echo "bench.sh: performance gate PASSED ($tables mode(s)); raw logs: $outdir"
