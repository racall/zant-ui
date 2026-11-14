#!/usr/bin/env bash
# Runs the headless frame benchmark (`zant-bench`) with the Rust port and with
# the Zig baseline, and prints the per-scenario frame-time comparison with the
# release gate of docs/rust-port/plan.md §4 / §7.3:
#
#   * every scenario's average frame time must stay within 1.25x of the
#     baseline;
#   * steady-state frames must stay far below 16.7 ms (60 fps with margin;
#     the `open` scenario is a cold first frame and only gets the ratio gate).
#
# The baseline is the Zig port while a `zig-out-baseline/bin/zant-bench`
# exists (or `ZANT_ZIG_BIN` points at one). The Zig tree and toolchain were
# removed at M8, so without a baseline the Rust side runs alone and every
# scenario is compared with the Zig numbers recorded in
# docs/rust-port/status.md (RECORDED_* below; the tables are labelled
# "recorded"). `ZANT_BENCH_BASELINE=recorded` forces that mode even when a
# baseline binary is present.
#
#   scripts/parity/bench.sh [scenario...]
#
# Scenarios: open stream typing caret hover resize scroll (all when omitted).
# Both render modes run: windowless full rebuild and ZANT_BENCH_LIST=1 virtual
# list. Each side runs ZANT_BENCH_REPEAT rounds (alternating which runs first)
# and the best round per scenario is compared, so a loaded machine does not
# decide the gate. Raw outputs are kept under target/parity/bench/.
#
# Environment:
#   ZANT_ZIG_BIN          Zig baseline bin dir (default: zig-out-baseline/bin in
#                         this checkout, else in the main checkout beside a
#                         .worktrees/<wp> tree)
#   ZANT_RUST_BIN         Rust binary dir (default: target/release)
#   ZANT_BENCH_MESSAGES   chat length, forwarded to both binaries (default 30)
#   ZANT_BENCH_MODES      "default list" (default: both)
#   ZANT_BENCH_REPEAT     rounds per side, best round wins (default 3)
#   ZANT_BENCH_MAX_RATIO  ratio gate (default 1.25)
#   ZANT_BENCH_MAX_MS     absolute average-frame budget (default 16.7)
#   ZANT_PYTHON           python3
#
# Options:
#   --no-build   use the existing target/release/zant-bench instead of building
#   --build-zig  build the Zig baseline first (ReleaseFast, zig-out-baseline)
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    sed -n '2,44p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
fi

build_rust=1
build_zig=0
scenarios=()
for arg in "$@"; do
    case "$arg" in
        --no-build) build_rust=0 ;;
        --build-zig) build_zig=1 ;;
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

# ---- locate the Zig baseline (ReleaseFast, built by WP-0.1) -----------------
zig_bin="${ZANT_ZIG_BIN:-}"
if [ -z "$zig_bin" ]; then
    for candidate in \
        "$root/zig-out-baseline/bin" \
        "$root/../../zig-out-baseline/bin" \
        "$root/../../../zig-out-baseline/bin"; do
        if [ -x "$candidate/zant-bench" ]; then
            zig_bin="$(cd "$candidate" && pwd)"
            break
        fi
    done
fi

zig_src=""
if [ -f "$root/build.zig" ]; then
    zig_src="$root"
elif [ -f "$root/../../build.zig" ]; then
    zig_src="$(cd "$root/../.." && pwd)"
fi

if [ "$build_zig" = 1 ]; then
    if [ -z "$zig_src" ]; then
        echo "bench.sh: cannot find the Zig source tree to build the baseline" >&2
        exit 2
    fi
    echo "bench.sh: building the Zig baseline (ReleaseFast) in $zig_src" >&2
    (
        cd "$zig_src"
        ZIG_GLOBAL_CACHE_DIR="${ZIG_GLOBAL_CACHE_DIR:-$zig_src/.zig-global-cache}" \
            zig build -Doptimize=ReleaseFast --prefix zig-out-baseline
    )
    zig_bin="$zig_src/zig-out-baseline/bin"
fi

# ---- locate / build the Rust benchmark -------------------------------------
rust_bin="${ZANT_RUST_BIN:-$root/target/release}"
if [ "$build_rust" = 1 ]; then
    echo "bench.sh: building zant-bench (release) with cargo" >&2
    "$root/scripts/cargo.sh" build --release -p zant-examples --bin zant-bench
fi

if [ ! -x "$rust_bin/zant-bench" ]; then
    echo "bench.sh: missing Rust binary: $rust_bin/zant-bench" >&2
    echo "  build it: cargo build --release -p zant-examples --bin zant-bench" >&2
    exit 2
fi

# ---- the Zig baseline, or the numbers recorded before M8 -------------------
# One row per scenario: name, frames, avg, p95, max (ms), copied from
# docs/rust-port/status.md 性能基线（WP-2.11）(the last Zig measurements
# before the Zig tree was deleted at M8).
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

recorded=0
if [ "${ZANT_BENCH_BASELINE:-}" = recorded ] ||
    [ ! -x "${zig_bin:-/nonexistent}/zant-bench" ]; then
    recorded=1
    if [ "${ZANT_BENCH_BASELINE:-}" = recorded ]; then
        echo "bench.sh: ZANT_BENCH_BASELINE=recorded: comparing with the numbers recorded in docs/rust-port/status.md" >&2
    else
        echo "bench.sh: no Zig baseline at ${zig_bin:-<not found>} (the Zig tree was removed at M8);" >&2
        echo "          the Rust side runs alone and is compared with the Zig numbers recorded in" >&2
        echo "          docs/rust-port/status.md (set ZANT_ZIG_BIN=<dir> for a direct comparison)" >&2
    fi
fi

# ---- run both implementations for each mode --------------------------------
modes=(${ZANT_BENCH_MODES:-default list})
outdir="$root/target/parity/bench"
mkdir -p "$outdir"

env_args=()
if [ -n "${ZANT_BENCH_MESSAGES:-}" ]; then
    env_args+=(ZANT_BENCH_MESSAGES="$ZANT_BENCH_MESSAGES")
fi

failures=0
tables=0
for mode in "${modes[@]}"; do
    case "$mode" in
        default) env_line=() ;;
        list) env_line=(ZANT_BENCH_LIST=1) ;;
        *)
            echo "bench.sh: ZANT_BENCH_MODES entries must be 'default' or 'list' (got '$mode')" >&2
            exit 2
            ;;
    esac
    # The benches print the header + summaries on stderr. Alternate which side
    # goes first so shared-machine load (other worktrees building) hits both
    # sides evenly; the comparison below keeps the best round per scenario.
    # bash 3.2 (macOS): an empty array under `set -u` needs the +"${a[@]}" form.
    if [ "$recorded" = 1 ]; then
        # The comparison parses zig-<mode>-<round>.log for rounds 1..N; the
        # recorded baseline is one round of rows and missing rounds are skipped.
        if [ "$mode" = default ]; then recorded_rows=recorded_default; else recorded_rows=recorded_list; fi
        rm -f "$outdir"/zig-"$mode"-*.log
        {
            "$recorded_rows" | while read -r name frames avg p95 max; do
                printf '%s frames=%4d avg=%7.2f ms p95=%7.2f ms max=%7.2f ms\n' \
                    "$name" "$frames" "$avg" "$p95" "$max"
            done
        } >"$outdir/zig-$mode-1.log"
    fi
    round=1
    while [ "$round" -le "$rounds" ]; do
        zig_log="$outdir/zig-$mode-$round.log"
        rust_log="$outdir/rust-$mode-$round.log"
        if [ $((round % 2)) -eq 1 ]; then
            if [ "$recorded" = 0 ]; then
                env "${env_line[@]+"${env_line[@]}"}" "${env_args[@]+"${env_args[@]}"}" \
                    "$zig_bin/zant-bench" "${scenarios[@]+"${scenarios[@]}"}" \
                    >/dev/null 2>"$zig_log" || true
            fi
            env "${env_line[@]+"${env_line[@]}"}" "${env_args[@]+"${env_args[@]}"}" \
                "$rust_bin/zant-bench" "${scenarios[@]+"${scenarios[@]}"}" \
                >/dev/null 2>"$rust_log" || true
        else
            env "${env_line[@]+"${env_line[@]}"}" "${env_args[@]+"${env_args[@]}"}" \
                "$rust_bin/zant-bench" "${scenarios[@]+"${scenarios[@]}"}" \
                >/dev/null 2>"$rust_log" || true
            if [ "$recorded" = 0 ]; then
                env "${env_line[@]+"${env_line[@]}"}" "${env_args[@]+"${env_args[@]}"}" \
                    "$zig_bin/zant-bench" "${scenarios[@]+"${scenarios[@]}"}" \
                    >/dev/null 2>"$zig_log" || true
            fi
        fi
        round=$((round + 1))
    done

    if ! ZANT_BENCH_RECORDED="$recorded" "$python" - "$outdir" "$mode" "$rounds" "${scenarios[@]+"${scenarios[@]}"}" <<'PY'
import os
import re
import sys

outdir, mode, rounds = sys.argv[1], sys.argv[2], int(sys.argv[3])
wanted = sys.argv[4:]
canonical = ["open", "stream", "typing", "caret", "hover", "resize", "scroll"]
line_re = re.compile(
    r"^([a-z]+)\s+frames=\s*(\d+)\s+avg=\s*([0-9.]+) ms\s+p95=\s*([0-9.]+) ms\s+max=\s*([0-9.]+) ms"
)


def parse(prefix):
    """Best round per scenario: the one with the lowest average frame time."""
    best = {}
    for round_index in range(1, rounds + 1):
        path = os.path.join(outdir, f"{prefix}-{mode}-{round_index}.log")
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
                    "round": round_index,
                }
                if name not in best or row["avg"] < best[name]["avg"]:
                    best[name] = row
    return best


zig = parse("zig")
rust = parse("rust")
if not zig:
    print(f"bench.sh: no Zig output under {outdir} for mode {mode}", file=sys.stderr)
    sys.exit(2)
if not rust:
    print(f"bench.sh: no Rust output under {outdir} for mode {mode}", file=sys.stderr)
    sys.exit(2)

order = wanted if wanted else [name for name in canonical if name in zig]
order = [name for name in order if name in zig and name in rust]
if wanted:
    missing = [name for name in wanted if name not in zig or name not in rust]
    if missing:
        print(f"bench.sh: no result for: {', '.join(missing)}", file=sys.stderr)
        sys.exit(2)

max_ratio = float(os.environ.get("ZANT_BENCH_MAX_RATIO", "1.25"))
max_ms = float(os.environ.get("ZANT_BENCH_MAX_MS", "16.7"))

label = "default (full rebuild)" if mode == "default" else "virtual list (ZANT_BENCH_LIST=1)"
baseline = "recorded Zig baseline (docs/rust-port/status.md)"
if os.environ.get("ZANT_BENCH_RECORDED") == "0":
    baseline = "live Zig baseline"
print()
print(f"== zant-bench: {label}; best of {rounds} round(s) per side; baseline: {baseline} ==")
print(
    f"{'scenario':<9} {'frames':>6}  {'avg rust':>9} {'avg zig':>8} {'ratio':>6}"
    f"  {'p95 rust':>8} {'max rust':>8} {'p95 zig':>8} {'max zig':>8}  verdict"
)
failures = 0
for name in order:
    z, r = zig[name], rust[name]
    ratio = r["avg"] / z["avg"] if z["avg"] > 0 else float("inf")
    ratio_ok = ratio <= max_ratio
    # The absolute 60 fps budget only applies to steady-state scenarios: the
    # Zig cold `open` frame is itself over 16.7 ms, so only the ratio counts
    # there (documented in docs/rust-port/status.md).
    steady = z["avg"] <= max_ms
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
        verdict = "PASS* (ratio only: cold start, Zig itself > 16.7 ms)"
    print(
        f"{name:<9} {r['frames']:>6}  {r['avg']:>9.2f} {z['avg']:>8.2f} {ratio:>5.2f}x"
        f"  {r['p95']:>8.2f} {r['max']:>8.2f} {z['p95']:>8.2f} {z['max']:>8.2f}  {verdict}"
    )

score = len(order) - failures
if failures:
    print(f"gate: FAIL — {failures} / {len(order)} scenarios over budget")
    sys.exit(1)
print(f"gate: PASS — {score} / {len(order)} scenarios within {max_ratio:.2f}x Zig")
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
