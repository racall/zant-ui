#!/usr/bin/env bash
# Windows frame-benchmark gate: runs scripts/win/bench.ps1 on the test machine
# (Rust target\release vs Zig zig-out\bin), copies the logs back and prints the
# per-scenario table with the plan §4/§7.3 gate (avg <= 1.25x Zig, steady-state
# avg << 16.7 ms; `open` is a cold first frame and only gets the ratio gate).
#
#   scripts/win/bench-compare.sh [scenario...]
#
# Environment: ZANT_BENCH_REPEAT (default 3), ZANT_BENCH_MESSAGES (30),
# ZANT_WIN_BENCH_DIR (target/win-bench), ZANT_WIN_SKIP_RUN=1 (compare local logs).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck disable=SC1091
[ -f "$root/scripts/win/.env" ] && . "$root/scripts/win/.env"
host="${ZANT_WIN_HOST:?set ZANT_WIN_HOST (or scripts/win/.env)}"
remote_dir="${ZANT_WIN_BENCH_DIR:-target/win-bench}"
rounds="${ZANT_BENCH_REPEAT:-3}"
messages="${ZANT_BENCH_MESSAGES:-30}"
python="${ZANT_PYTHON:-python3}"
out="$root/target/bench-win"
max_ratio="${ZANT_BENCH_MAX_RATIO:-1.25}"
max_ms="${ZANT_BENCH_MAX_MS:-16.7}"

scenarios="$*"
if [ -z "${ZANT_WIN_SKIP_RUN:-}" ]; then
    "$root/scripts/win/remote.sh" XyUI \
        "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/win/bench.ps1 -Out '$remote_dir' -Repeat $rounds -Messages $messages -Scenarios '$scenarios'"
fi

rm -rf "$out"
mkdir -p "$out"
ssh ${ZANT_WIN_SSH_OPTS:-} "$host" "tar -C '$remote_dir' -cf - ." | tar -xf - -C "$out"

"$python" - "$out" "$rounds" "$scenarios" "$max_ratio" "$max_ms" <<'PY'
import re
import sys
from pathlib import Path

out = Path(sys.argv[1])
rounds = int(sys.argv[2])
scenarios = [s for s in sys.argv[3:] if s]
# the last two args are the gates
max_ratio = float(scenarios[-2]) if len(scenarios) >= 2 else 1.25
max_ms = float(scenarios[-1]) if len(scenarios) >= 2 else 16.7
scenarios = scenarios[:-2]

pattern = re.compile(
    r"^([a-z]+)\s+frames=\s*(\d+)\s+avg=\s*([0-9.]+) ms\s+p95=\s*([0-9.]+) ms\s+max=\s*([0-9.]+) ms"
)


def parse(path):
    rows = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        m = pattern.match(line.strip())
        if m:
            rows[m.group(1)] = {
                "frames": int(m.group(2)),
                "avg": float(m.group(3)),
                "p95": float(m.group(4)),
                "max": float(m.group(5)),
            }
    return rows


failures = 0
for mode in ("default", "list"):
    best = {"rust": {}, "zig": {}}
    for side in ("rust", "zig"):
        for round_no in range(1, rounds + 1):
            path = out / f"{side}-{mode}-r{round_no}.log"
            if not path.is_file():
                continue
            for name, row in parse(path).items():
                if name not in best[side] or row["avg"] < best[side][name]["avg"]:
                    best[side][name] = row
    names = sorted(set(best["rust"]) & set(best["zig"]))
    if scenarios:
        names = [n for n in names if n in scenarios]
    if not names:
        print(f"{mode}: no comparable scenarios")
        continue
    print(f"\n== Windows bench: {mode} mode ({rounds} rounds, best of each side) ==")
    print(f"{'scenario':<9} {'frames':>6}  {'avg rust':>9} {'avg zig':>8} {'ratio':>6}  gate")
    for name in names:
        r = best["rust"][name]
        z = best["zig"][name]
        ratio = r["avg"] / z["avg"] if z["avg"] > 0 else float("inf")
        ok = ratio <= max_ratio and (name == "open" or r["avg"] <= max_ms)
        if not ok:
            failures += 1
        print(
            f"{name:<9} {r['frames']:>6}  {r['avg']:>8.2f} ms {z['avg']:>7.2f} ms "
            f"{ratio:>5.2f}x  {'PASS' if ok else 'FAIL'}"
        )
if failures:
    print(f"\n{failures} scenario(s) over the gate")
    sys.exit(1)
print("\nall scenarios within the gate")
PY
