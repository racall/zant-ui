#!/bin/bash
# Real macOS desktop component/event/memory check. Requires Accessibility and
# Screen Recording for the terminal. No application build unless --build is set.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
case "${1:-}" in
    --build)
        [ "$#" -eq 1 ] || { echo "usage: $0 [--build]" >&2; exit 2; }
        ./scripts/cargo.sh build -p zant-examples --bin zant-components
        ;;
    "") ;;
    *) echo "usage: $0 [--build]" >&2; exit 2 ;;
esac

TASK_TMP=${TMPDIR:-/tmp}
OUT=${OUT:-$TASK_TMP/zant-checks/components}
EXE=${EXE:-$ROOT/target/debug/zant-components}
RUNNER="$TASK_TMP/zant-ui-scenario"
SOURCE="$ROOT/scripts/mac/ui-scenario.swift"
mkdir -p "$OUT/shots"
OUT=$(cd "$OUT" && pwd)
REPORT="$OUT/report.txt"
LOG="$OUT/app.log"
LAYOUT="$OUT/layout.txt"
: > "$REPORT"
: > "$LOG"
# A fresh dump prevents an old window/layout from satisfying startup checks.
rm -f "$LAYOUT"
if [ ! -x "$EXE" ]; then
    echo "FAIL executable missing: $EXE (use --build or EXE)" | tee -a "$REPORT"
    exit 1
fi
if [ ! -x "$RUNNER" ] || [ "$SOURCE" -nt "$RUNNER" ]; then
    if ! swiftc -O -module-cache-path "$TASK_TMP/zant-swift-module-cache" "$SOURCE" -o "$RUNNER"; then
        echo "FAIL compiling ui-scenario" | tee -a "$REPORT"
        exit 1
    fi
fi

APP_PID=
cleanup() {
    if [ -n "$APP_PID" ]; then
        if kill -0 "$APP_PID" 2>/dev/null; then
            kill "$APP_PID" 2>/dev/null || true
            local attempt=0
            while kill -0 "$APP_PID" 2>/dev/null && [ "$attempt" -lt 30 ]; do
                sleep 0.1
                attempt=$((attempt + 1))
            done
            if kill -0 "$APP_PID" 2>/dev/null; then kill -KILL "$APP_PID" 2>/dev/null || true; fi
        fi
        wait "$APP_PID" 2>/dev/null || true
        APP_PID=
    fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
# Keep ZANT_DARK inherited. Autoplay/snapshot mode would interfere with input.
unset ZANT_AUTOPLAY ZANT_SNAPSHOT ZANT_SNAPSHOT_EXIT
ZANT_WINDOW_SIZE=1100x820 ZANT_LAYOUT_FILE="$LAYOUT" ZANT_FRAME_STATS=1 \
    "$EXE" > "$LOG" 2>&1 &
APP_PID=$!

ready=0
for ((attempt=0; attempt<100; attempt++)); do
    if ! kill -0 "$APP_PID" 2>/dev/null; then break; fi
    if [ -s "$LAYOUT" ]; then ready=1; break; fi
    sleep 0.1
done
# Preserve exact theme events when starting in dark mode (the example tests
# presence of ZANT_DARK, including an explicitly empty value).
STEPS="${STEPS:-$ROOT/scripts/mac/components.steps}"
if [ "${ZANT_DARK+x}" = x ]; then
    awk '
        $0 == "expect [event] dark true" { print "expect [event] dark false"; next }
        $0 == "expect [event] dark false" { print "expect [event] dark true"; next }
        { print }
    ' "$STEPS" > "$OUT/components.steps"
    STEPS="$OUT/components.steps"
fi
status=0
if [ "$ready" -eq 1 ]; then
    # Do not allow a caller's DRY_RUN to turn a desktop check into a false pass.
    if DRY_RUN=0 EVENT_LOG="$LOG" APP_PID="$APP_PID" CONTENT_HEIGHT=820 \
        "$RUNNER" "$(basename "$EXE")" "$LAYOUT" "$OUT/shots" \
        "$STEPS" > "$REPORT" 2>&1; then
        :
    else
        status=$?
        echo "FAIL scenario exited $status" >> "$REPORT"
    fi
else
    echo "FAIL app did not publish a layout within 10 seconds (see app.log)" >> "$REPORT"
    status=1
fi
if kill -0 "$APP_PID" 2>/dev/null; then
    echo "PASS app still running after all interactions" >> "$REPORT"
else
    echo "FAIL app exited during interactions" >> "$REPORT"
    status=1
fi
cleanup

# First 20 rounds warm caches; compare the two identical settled cycle endpoints.
if ! awk '
    $1 == "RSS" && $2 == "start" { start = $3 }
    $1 == "RSS" && $2 == "after-20" { warm = $3 }
    $1 == "RSS" && $2 == "after-40" { final = $3 }
    END {
        if (start <= 0 || warm <= 0 || final <= 0) {
            print "FAIL RSS samples missing"; exit 1
        }
        growth = (final / warm - 1) * 100
        if (final > warm * 1.05) {
            printf "FAIL leak: RSS after-40 exceeds after-20 by %.2f%% (>5%%)\n", growth; exit 1
        }
        printf "PASS RSS growth after warm-up %.2f%% (<=5%%)\n", growth
    }
' "$REPORT" > "$OUT/memory.txt"; then status=1; fi
cat "$OUT/memory.txt" >> "$REPORT"
if ! awk '
    /^\[frames/ {
        for (i = 1; i <= NF; i++) {
            if ($i ~ /^max_frame_ms=/) {
                split($i, field, "="); value = field[2] + 0
                if (!seen || value > worst) worst = value
                seen = 1
            }
        }
    }
    END {
        if (seen) printf "worst [frames] max_frame_ms=%.2f\n", worst
        else { print "FAIL no [frames] max_frame_ms samples in app.log"; exit 1 }
    }
' "$LOG" > "$OUT/frames.txt"; then status=1; fi
cat "$OUT/frames.txt" >> "$REPORT"

passes=$(awk '$1 == "PASS" { n++ } END { print n+0 }' "$REPORT")
fails=$(awk '$1 == "FAIL" { n++ } END { print n+0 }' "$REPORT")
{
    echo "PASS $passes / FAIL $fails"
    awk '$1 == "RSS" { print }' "$REPORT"
    cat "$OUT/memory.txt" "$OUT/frames.txt"
    echo "Report: $REPORT"
    echo "Shots: $OUT/shots"
} | tee "$OUT/summary.txt"
if [ "$fails" -gt 0 ] || [ "$status" -ne 0 ]; then exit 1; fi
