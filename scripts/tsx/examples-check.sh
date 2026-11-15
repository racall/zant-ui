#!/bin/sh
# End-to-end check of the TSX example apps and the TSX compile benchmark.
#
#   scripts/tsx/examples-check.sh
#
# The examples are Cargo workspace members under `crates/zant-examples/tsx/`
# (the `scaffold/template-ts` shape: `native/build.rs` → `zant_build::tsx_app()`,
# `native/main.rs` → `zant::app!()`), so a plain workspace build covers
# them. This script then runs each app headless and asserts:
#
#   1. it builds and starts with `ZANT_SNAPSHOT` (the runtime takes the window
#      screenshot 600 ms after the first frame) writing a real PNG;
#   2. the PNG is not blank: valid PNG signature, at least 16 distinct colours,
#      and the most common colour covers < 99% of the sampled pixels (a blank
#      window is a single colour);
#   3. the window size comes from `app.config.json`: the PNG is the logical
#      window size × the display scale (between 1× and 4×);
#   4. two runs are byte-identical (PNG and layout dump): the examples are
#      static, so `ZANT_SNAPSHOT` is deterministic and a rendering change shows
#      up as a diff here;
#   5. the keyed controls of the UI are laid out with a non-empty box in the
#      `ZANT_LAYOUT_FILE` dump. Accessibility does not expose Skia text, so the
#      layout keys are the structural assertion for "the right screen";
#   6. the expected window title and a literal of the UI are in the compiled
#      binary (`app_config::title` and the TSX string literals are `&str`
#      rodata). That is the text half of the screenshot assertion; the layout
#      dump cannot see text either.
#
# The apps are counter, demo, the benchmark workload, gallery, todos, reader
# and notes.
#
# Then it runs the compile benchmark. `crates/zant-examples`' `zant-tsx-bench`
# compiles every `crates/zant-tsx/testdata/tsx/*.tsx` through `zant_tsx::compile`
# in process (10 warm-up + 100 iterations each); the script checks how many
# corpus files compiled (at least 30 of the 32; files the compiler rejects are
# reported and skipped by the benchmark).
#
# `ZANT_CARGO`, `ZANT_PYTHON` and `CARGO_TARGET_DIR` override the tools / build
# directory. Never opens a visible window; needs no network.
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cargo=${ZANT_CARGO:-cargo}
cargo_home="$repo/.cargo-home"
python=${ZANT_PYTHON:-python3}
target_dir=${CARGO_TARGET_DIR:-"$repo/target"}
corpus="$repo/crates/zant-tsx/testdata/tsx"
# The measured corpus, warm-up rounds and iterations of the compile benchmark.
warmup=10
iterations=100
# Compile time is sensitive to other load on the machine, so the benchmark runs
# several rounds. Override with `ZANT_BENCH_ROUNDS`.
rounds=${ZANT_BENCH_ROUNDS:-3}

work=$(mktemp -d "${TMPDIR:-/tmp}/zant-tsx-examples.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM

cd "$repo"
echo "== building the TSX example apps (workspace members)"
CARGO_HOME="$cargo_home" "$cargo" build -p zant-tsx-counter -p zant-tsx-demo -p zant-tsx-bench-app \
    -p zant-tsx-gallery -p zant-tsx-todos -p zant-tsx-reader -p zant-tsx-notes
bin_dir="$target_dir/debug"

# PNG checks (2., 3.) of one snapshot. `scripts/lib/imgdiff.py` provides the
# pure-stdlib PNG decoder.
check_png() {
    "$python" - "$1" "$2" "$3" "$repo/scripts/lib/imgdiff.py" <<'PY'
import collections
import importlib.util
import sys

path, logical_w, logical_h, imgdiff_path = sys.argv[1:5]
logical_w, logical_h = int(logical_w), int(logical_h)
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("imgdiff", imgdiff_path)
imgdiff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(imgdiff)
try:
    width, height, rgba = imgdiff.read_png(path)
except imgdiff.PngError as error:
    sys.exit(f"{error}")
if width <= 0 or height <= 0:
    sys.exit(f"{path}: empty image {width}x{height}")
# The snapshot is the window at the display scale (2x on a Retina display,
# 1x elsewhere): it must be at least the configured logical size and not absurd.
if (
    width < logical_w
    or height < logical_h
    or width > 4 * logical_w
    or height > 4 * logical_h
):
    sys.exit(
        f"{path}: {width}x{height} is not the {logical_w}x{logical_h} window "
        "at a 1-4x display scale"
    )
counts = collections.Counter()
pixels = width * height
step = max(1, pixels // 20000)
for index in range(0, pixels, step):
    counts[bytes(rgba[index * 4 : index * 4 + 4])] += 1
sampled = sum(counts.values())
top = counts.most_common(1)[0][1] / sampled
if len(counts) < 16 or top > 0.99:
    sys.exit(
        f"{path}: looks blank ({len(counts)} distinct colours sampled, the most "
        f"common covers {top:.1%} of the pixels)"
    )
print(
    f"   png: {width}x{height} ({width // logical_w}x scale), "
    f"{len(counts)} distinct colours sampled, background {top:.1%}"
)
PY
}

# One example (checks 1.-6.). Arguments: name, binary, window title, UI literal,
# logical width, logical height, then the layout keys that must be laid out.
run_app() {
    name=$1
    binary=$2
    title=$3
    literal=$4
    logical_w=$5
    logical_h=$6
    shift 6
    path="$bin_dir/$binary"
    if [ ! -x "$path" ]; then
        echo "missing example binary: $path" >&2
        exit 1
    fi
    # 6.: the title from app.config.json and a literal of the UI are `&str`
    # rodata of the compiled binary.
    if ! grep -a -q -F -- "$title" "$path"; then
        echo "$binary: the window title \`$title\` is not in the binary" >&2
        exit 1
    fi
    if ! grep -a -q -F -- "$literal" "$path"; then
        echo "$binary: the UI literal \`$literal\` is not in the binary" >&2
        exit 1
    fi
    # 1. + 4.: two deterministic headless runs.
    for run in 1 2; do
        shot="$work/$name-$run.png"
        layout="$work/$name-$run.layout"
        log="$work/$name-$run.log"
        if ! ZANT_THEME=light ZANT_NO_MOTION=1 ZANT_SNAPSHOT="$shot" \
            ZANT_SNAPSHOT_EXIT=1 ZANT_LAYOUT_FILE="$layout" \
            timeout 120 "$path" >"$log" 2>&1; then
            echo "$binary: did not run to a snapshot" >&2
            tail -20 "$log" >&2 || true
            exit 1
        fi
        if [ ! -s "$shot" ]; then
            echo "$binary: wrote no snapshot to $shot" >&2
            exit 1
        fi
        if [ ! -s "$layout" ]; then
            echo "$binary: wrote no layout dump to $layout" >&2
            exit 1
        fi
    done
    if ! cmp -s "$work/$name-1.png" "$work/$name-2.png"; then
        echo "$binary: two snapshot runs differ (the example is not deterministic)" >&2
        exit 1
    fi
    if ! cmp -s "$work/$name-1.layout" "$work/$name-2.layout"; then
        echo "$binary: two layout dumps differ (the example is not deterministic)" >&2
        exit 1
    fi
    # 2. + 3.
    check_png "$work/$name-1.png" "$logical_w" "$logical_h"
    # 5.: every expected control is laid out with a non-empty box.
    for key in "$@"; do
        if ! awk -v key="$key" '$1 == key && $4 > 0 && $5 > 0 { found = 1 } END { exit !found }' \
            "$work/$name-1.layout"; then
            echo "$binary: the layout dump has no box for the keyed control \`$key\`" >&2
            exit 1
        fi
    done
    echo "   $binary: $# keyed controls laid out, two runs byte-identical"
}

echo "== running the TSX example apps headless"
run_app counter zant-tsx-counter "TSX Counter" "Notifications are on" 520 480 \
    title count name inc reset step large notify note
run_app demo zant-tsx-demo "TSX App" "Hello from TSX" 480 360 \
    label-Apples label-Pears minus-Apples plus-Apples total toggle-panel copy note
run_app bench zant-tsx-bench-app "TSX Bench" "Settings" 640 900 \
    changes row-0 sw-0 row-39 sw-39
run_app gallery zant-tsx-gallery "TSX Gallery" "TSX component gallery" 960 760 \
    page inputs name agree notify plan select size display progress advance open \
    refresh saved nesting rich save-icon copy-icon clickable-card inner tabs
run_app todos zant-tsx-todos "TSX Todos" "Nothing to do yet." 560 640 \
    new add filter empty left clear
# The reader opens the repo's small PDF fixture (`tests/fixtures/widgets.pdf`),
# so the script runs from the repo root.
run_app reader zant-tsx-reader "TSX Reader" "Document reader" 1120 760 \
    append finish reply-scroll reply path open reader
run_app notes zant-tsx-notes "TSX Notes" "No notes yet." 520 560 \
    new add search empty

echo
echo "== TSX compile benchmark: zant-tsx-bench"
CARGO_HOME="$cargo_home" "$cargo" build --release -p zant-examples --bin zant-tsx-bench
rust_bench="$target_dir/release/zant-tsx-bench"
mkdir -p "$work/bench"
echo "   $rounds rounds"
round=1
while [ "$round" -le "$rounds" ]; do
    if ! "$rust_bench" "$corpus" "$warmup" "$iterations" \
        >"$work/bench/rust-$round.log" 2>&1; then
        echo "zant-tsx-bench failed in round $round:" >&2
        tail -30 "$work/bench/rust-$round.log" >&2 || true
        exit 1
    fi
    round=$((round + 1))
done
compiled=$(grep -c 'frames=' "$work/bench/rust-1.log" || true)
if [ "$compiled" -lt 30 ]; then
    echo "zant-tsx-bench compiled only $compiled of the 32 corpus files:" >&2
    tail -30 "$work/bench/rust-1.log" >&2 || true
    exit 1
fi
echo "   $compiled corpus files compiled"

echo
echo "TSX examples and compile benchmark: OK"
