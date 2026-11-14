#!/usr/bin/env bash
# M8 cutover for the Zig -> Rust migration (`docs/rust-port/m8-cutover.md`).
#
# Moves the native C / C++ / ObjC sources under `crates/zant-sys/native/`,
# deletes the Zig toolchain-era files (all `*.zig`, `build.zig`, `build.zig.zon`,
# `tools/`, `scaffold/template/`, the Windows DLL scripts), updates every
# reference (build.rs, gen_bindings, prebuilt source lists, CI, release/package
# scripts, Rust fixtures, docs) and then verifies the Rust workspace.
#
#   scripts/migrate/cutover-m8.sh                 # dry run: print the whole plan
#   scripts/migrate/cutover-m8.sh --apply         # back up, cut over, verify
#   scripts/migrate/cutover-m8.sh --verify-only   # run the post-M8 gates only
#
# The default is a dry run: it validates the preconditions, prints every
# `git mv` / `git rm` / path rewrite it would apply and changes nothing.
#
# `--apply` refuses to run unless the tree is clean, the branch is
# `feat/rust-port`, the pre-M8 quality gates pass and the Zig parity baseline
# (`zig-out-baseline/bin/`, built by WP-0.1) is present. It first tags the
# current commit `rust-migration-pre-m8` so the whole cutover can be undone
# with `git reset --hard rust-migration-pre-m8` (plus `git clean -fd` for the
# generated files). The user must approve the cutover before it is run; see
# docs/rust-port/m8-cutover.md.
#
# `--skip-gates` is for the throwaway rehearsal copy only: it skips the pre-M8
# gates, the branch check and the post-M8 verification. Do not use it on the
# real tree.
#
# Everything this script changes is either a `git mv` / `git rm` or a literal
# text replacement in a tracked file; the replacement list lives in the Python
# program below and every entry asserts that its input text still exists, so
# the script fails loudly instead of silently skipping a rename.
#
# The Zig `zig-out-baseline/` directory (gitignored, not tracked) and the
# `.cargo-home` / `.zig-global-cache` / `third_party` symlinks are left in
# place; the script never deletes them.
set -euo pipefail

usage() {
    sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'
}

APPLY=0
SKIP_GATES=0
VERIFY_ONLY=0
for arg in "$@"; do
    case "$arg" in
        --apply) APPLY=1 ;;
        --skip-gates) SKIP_GATES=1 ;;
        --verify-only) VERIFY_ONLY=1 ;;
        --help | -h) usage; exit 0 ;;
        *)
            echo "cutover-m8.sh: unknown option '$arg' (use --help)" >&2
            exit 2
            ;;
    esac
done
if [ "$APPLY" = 1 ] && [ "$VERIFY_ONLY" = 1 ]; then
    echo "cutover-m8.sh: --apply and --verify-only are exclusive" >&2
    exit 2
fi

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
PYTHON="${ZANT_PYTHON:-python3}"
if ! command -v "$PYTHON" >/dev/null 2>&1; then
    echo "cutover-m8.sh: python3 is required (it validates and applies the edits)" >&2
    exit 2
fi
CARGO="./scripts/cargo.sh"
[ -x "$CARGO" ] || { echo "cutover-m8.sh: $CARGO is missing" >&2; exit 2; }

log() { printf '\n\033[36m== %s\033[0m\n' "$*"; }
ok() { printf '  \033[32m✓\033[0m %s\n' "$*"; }
die() { printf '\n\033[31m✗ %s\033[0m\n\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Preconditions
# ---------------------------------------------------------------------------

# The Zig baseline the parity scripts compare against (WP-0.1). It lives in the
# main checkout; the resolution mirrors scripts/parity/bench.sh so a worktree
# copy finds it too. `ZANT_ZIG_BIN` overrides it.
find_zig_baseline() {
    local candidate
    if [ -n "${ZANT_ZIG_BIN:-}" ]; then
        printf '%s\n' "$ZANT_ZIG_BIN"
        return 0
    fi
    for candidate in \
        "$ROOT/zig-out-baseline/bin" \
        "$ROOT/../../zig-out-baseline/bin" \
        "$ROOT/../../../zig-out-baseline/bin"; do
        if [ -x "$candidate/zant-bench" ]; then
            printf '%s\n' "$(cd "$candidate" && pwd)"
            return 0
        fi
    done
    return 1
}

precondition_gates() {
    log "pre-M8 quality gates (skipped by --skip-gates)"
    "$CARGO" fmt --all -- --check || die "cargo fmt --all -- --check is not clean"
    "$CARGO" clippy --workspace --all-targets -- -D warnings ||
        die "clippy is not clean"
    "$CARGO" test --workspace || die "cargo test --workspace is not green"
    ok "fmt / clippy / cargo test --workspace are green"
}

preconditions() {
    log "preconditions"
    [ -f build.zig ] && [ -f Cargo.toml ] ||
        die "this does not look like the pre-M8 tree (build.zig + Cargo.toml)"
    [ -f src/renderer/skia/skia_bridge.h ] ||
        die "src/renderer/skia/skia_bridge.h is missing (already cut over?)"
    [ -f src/platform/platform.h ] ||
        die "src/platform/platform.h is missing (already cut over?)"
    [ -f tools/tsx/testdata/counter.tsx ] ||
        die "tools/tsx/testdata/ is missing (already cut over?)"
    [ -d scaffold/template ] || die "scaffold/template/ is missing (already cut over?)"
    [ -f crates/zant-tsx/testdata/counter.rs ] ||
        die "crates/zant-tsx/testdata/ is missing"
    ok "pre-M8 layout present"

    if [ "$APPLY" = 1 ] && [ "$SKIP_GATES" = 0 ]; then
        local branch
        branch="$(git rev-parse --abbrev-ref HEAD)"
        [ "$branch" = "feat/rust-port" ] ||
            die "on branch '$branch': the cutover runs on 'feat/rust-port'"
        if [ -n "$(git status --porcelain)" ]; then
            git status --short >&2
            die "the tree is not clean: commit or stash the changes first"
        fi
        ok "branch feat/rust-port, clean tree"

        local zig_bin
        zig_bin="$(find_zig_baseline || true)"
        [ -n "$zig_bin" ] ||
            die "no Zig parity baseline: build zig-out-baseline first (WP-0.1, 'zig build -Doptimize=ReleaseFast --prefix zig-out-baseline') or set ZANT_ZIG_BIN"
        for binary in zant-demo zant-components zant-gallery zant-markdown \
            zant-math zant-pdf zant-bench; do
            [ -x "$zig_bin/$binary" ] ||
                die "the Zig baseline $zig_bin has no $binary (WP-0.1 baseline)"
        done
        [ -f tests/fixtures/widgets.pdf ] || die "tests/fixtures/widgets.pdf is missing"
        ok "Zig parity baseline present: $zig_bin"

        precondition_gates
    elif [ "$SKIP_GATES" = 1 ]; then
        printf '  \033[33m!\033[0m --skip-gates: branch, clean tree, Zig baseline and pre-M8 gates are not checked (rehearsal only)\n'
    fi

    if [ "$APPLY" = 0 ] && [ "$VERIFY_ONLY" = 0 ]; then
        # A dry run is useful in a dirty worktree, but say what --apply needs.
        if [ -n "$(git status --porcelain)" ]; then
            printf '  \033[33m!\033[0m the tree is not clean; --apply will require a clean feat/rust-port tree\n'
        fi
        local zig_bin
        if ! zig_bin="$(find_zig_baseline)"; then
            printf '  \033[33m!\033[0m no Zig parity baseline found; --apply requires one (or set ZANT_ZIG_BIN)\n'
        fi
    fi
}

# ---------------------------------------------------------------------------
# Post-M8 verification
# ---------------------------------------------------------------------------

verify() {
    log "post-M8 verification"
    local failures=0
    gate() { # <label> <command...>
        local label="$1"
        shift
        printf '\n  → %s\n' "$label"
        if "$@"; then
            ok "$label"
        else
            printf '  \033[31m✗ %s\033[0m\n' "$label" >&2
            failures=$((failures + 1))
        fi
    }

    gate "cargo build --workspace" "$CARGO" build --workspace
    gate "cargo test --workspace" "$CARGO" test --workspace
    gate "clippy --workspace --all-targets -- -D warnings" \
        "$CARGO" clippy --workspace --all-targets -- -D warnings
    gate "cargo fmt --all -- --check" "$CARGO" fmt --all -- --check
    gate "scripts/tsx/build-template-check.sh" ./scripts/tsx/build-template-check.sh
    gate "scripts/tsx/examples-check.sh" ./scripts/tsx/examples-check.sh
    # Best-of-N rounds per side, like the WP-2.11 measurements in status.md:
    # sub-millisecond scenarios are otherwise dominated by whatever else runs
    # on the machine (the cutover may run on a shared/loaded developer box).
    # 12 rounds keep the gate stable on a machine with load average > 10.
    # `ZANT_M8_BENCH_ADVISORY=1` downgrades a bench failure to a warning for a
    # knowingly loaded machine; re-run scripts/parity/bench.sh on a quiet
    # machine before releasing.
    if ! ZANT_BENCH_REPEAT="${ZANT_BENCH_REPEAT:-12}" ./scripts/parity/bench.sh; then
        if [ "${ZANT_M8_BENCH_ADVISORY:-0}" = 1 ]; then
            printf '  \033[33m!\033[0m bench gate failed but ZANT_M8_BENCH_ADVISORY=1: not failing the cutover\n' >&2
            printf '    re-run scripts/parity/bench.sh on a quiet machine before releasing\n' >&2
        else
            printf '  \033[31m✗ %s\033[0m\n' "scripts/parity/bench.sh" >&2
            failures=$((failures + 1))
        fi
    else
        ok "scripts/parity/bench.sh (Rust side; recorded Zig baseline)"
    fi

    printf '\n  → no Zig sources remain\n'
    local zig_files
    zig_files="$(git ls-files '*.zig')"
    if [ -n "$zig_files" ]; then
        printf '%s\n' "$zig_files" >&2
        printf '  \033[31m✗ tracked *.zig files remain\033[0m\n' >&2
        failures=$((failures + 1))
    elif [ -e build.zig ] || [ -e build.zig.zon ] || [ -e tools ] ||
        [ -e src ] || [ -e examples ] || [ -e scaffold/template ]; then
        printf '  \033[31m✗ a Zig-era directory/file is still present\033[0m\n' >&2
        failures=$((failures + 1))
    else
        ok "no Zig sources, no tools/, src/, examples/, scaffold/template/"
    fi

    if [ "$failures" -ne 0 ]; then
        printf '\n\033[31m✗ post-M8 verification: %d gate(s) failed\033[0m\n' "$failures" >&2
        printf '  Inspect the state, fix the script/code, or undo with:\n' >&2
        printf '    git reset --hard rust-migration-pre-m8 && git clean -fd\n' >&2
        exit 1
    fi
    ok "all post-M8 gates passed"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

if [ "$VERIFY_ONLY" = 1 ]; then
    [ -z "$(git ls-files '*.zig')" ] ||
        die "--verify-only is meant for a cut-over tree, but tracked *.zig files remain"
    verify
    exit 0
fi

preconditions

if [ "$APPLY" = 1 ]; then
    log "backup tag"
    git tag -f rust-migration-pre-m8 >/dev/null
    ok "tagged rust-migration-pre-m8 at $(git rev-parse --short HEAD)"
else
    printf '\n  (dry run) would create the backup tag rust-migration-pre-m8 at %s\n' \
        "$(git rev-parse --short HEAD)"
fi

PY_FILE="$(mktemp "${TMPDIR:-/tmp}/zant-m8-cutover.XXXXXX.py")"
trap 'rm -f "$PY_FILE"' EXIT HUP INT TERM
cat >"$PY_FILE" <<'PY'
# ---------------------------------------------------------------------------
# The M8 cutover plan: moves, deletions and literal text replacements.
# ---------------------------------------------------------------------------
# `mode` is "dry-run" (validate + print, change nothing) or "apply". Every
# replacement asserts that its input text exists, so a drifted tree fails
# loudly instead of being half-edited.
import os
import subprocess
import sys
from pathlib import Path

MODE = sys.argv[1]
ROOT = Path(sys.argv[2]).resolve()
APPLY = MODE == "apply"

ERRORS = []
MOVED = 0
DELETED = 0
EDITED = 0
NEW_FILES = 0


def fail(message):
    ERRORS.append(message)


def git(*args):
    return subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, check=False
    )


def tracked(*pathspecs):
    result = git("ls-files", "-z", "--", *pathspecs)
    if result.returncode != 0:
        fail(f"git ls-files failed: {result.stderr.strip()}")
        return []
    return [entry for entry in result.stdout.split("\0") if entry]


# Post-move prefix -> pre-move prefix, so a dry run can read the source of a
# file that only exists at the new path in the applied tree.
MOVED_PREFIXES = [
    ("crates/zant-sys/native/skia/", "src/renderer/skia/"),
    ("crates/zant-sys/native/platform/", "src/platform/"),
    ("crates/zant-tsx/testdata/tsx/", "tools/tsx/testdata/"),
    ("crates/zant-examples/tsx/corpus/", "examples/tsx/"),
    ("crates/zant-examples/assets/", "examples/assets/"),
    ("crates/zant/src/math/mhchem_cases.json", "src/math/mhchem_cases.json"),
    ("scaffold/template-ts/DESIGN.md", "scaffold/template/DESIGN.md"),
    ("scripts/package/", "tools/zant/package/"),
]


def source_of(path):
    """(path on disk to read, text) of `path`, also for a not-yet-moved file."""
    direct = ROOT / path
    if direct.is_file():
        return direct, direct.read_text(encoding="utf-8")
    for new, old in MOVED_PREFIXES:
        if path == new or path.startswith(new):
            pre = old + path[len(new):]
            candidate = ROOT / pre
            if candidate.is_file():
                return candidate, candidate.read_text(encoding="utf-8")
    fail(f"cannot read {path} (neither at the new nor at the old location)")
    return None, ""


def move(src, dst):
    global MOVED
    MOVED += 1
    print(f"  git mv {src} -> {dst}")
    if not APPLY:
        return
    if not (ROOT / src).exists():
        fail(f"git mv source is missing: {src}")
        return
    (ROOT / dst).parent.mkdir(parents=True, exist_ok=True)
    result = git("mv", src, dst)
    if result.returncode != 0:
        fail(f"git mv {src} {dst} failed: {result.stderr.strip()}")


def move_files(files, dest_dir):
    for file in files:
        move(file, f"{dest_dir}/{Path(file).name}")


def move_tree(src_dir, dst_dir):
    files = tracked(src_dir)
    if not files:
        fail(f"cannot move {src_dir}: no tracked files")
        return
    move(src_dir, dst_dir)


def pre_of(path):
    """The pre-move path of a (to-be-)moved file, for dry-run bookkeeping."""
    for new, old in MOVED_PREFIXES:
        if path == new or path.startswith(new):
            return old + path[len(new):]
    return path


def delete(path, label=None):
    global DELETED
    # A path may be a tracked file, a tracked directory, an empty directory left
    # behind by earlier deletions, or already gone (a directory whose files all
    # moved/deleted). Only the first two need `git rm`.
    if not (ROOT / path).exists() and not tracked(pre_of(path)):
        print(f"  (already gone) {path}")
        return
    files = tracked(path) or tracked(pre_of(path))
    if not files:
        if (ROOT / path).is_dir():
            print(f"  (empty directory, removed by cleanup) {path}")
            return
        fail(f"cannot delete {path}: not tracked")
        return
    is_dir = (ROOT / path).is_dir()
    command = f"git rm -r {path}" if is_dir else f"git rm {path}"
    print(f"  {command} ({len(files)} files)")
    DELETED += len(files)
    if APPLY:
        result = git("rm", "-rf", "--quiet", "--", path)
        if result.returncode != 0:
            fail(f"{command} failed: {result.stderr.strip()}")


def delete_glob(pattern):
    global DELETED
    files = tracked(pattern)
    if not files:
        fail(f"no tracked files match {pattern}")
        return
    print(f"  git rm {pattern} ({len(files)} files)")
    DELETED += len(files)
    if APPLY:
        for start in range(0, len(files), 100):
            result = git("rm", "--quiet", "--", *files[start:start + 100])
            if result.returncode != 0:
                fail(f"git rm {pattern} failed: {result.stderr.strip()}")


def edit(path, pairs):
    """Apply literal replacements to one file (all pairs must match)."""
    global EDITED
    disk, text = source_of(path)
    if disk is None:
        return
    new = text
    missing = []
    for old, replacement in pairs:
        count = new.count(old)
        if count == 0:
            missing.append(old)
        new = new.replace(old, replacement)
    if missing:
        for old in missing:
            fail(f"{path}: replacement text not found:\n---\n{old}\n---")
        return
    if new == text:
        fail(f"{path}: replacements did not change the file")
        return
    EDITED += 1
    print(f"  edit {path} ({len(pairs)} replacement(s))")
    if APPLY:
        disk.write_text(new, encoding="utf-8")


def edit_many(paths, pairs):
    for path in paths:
        edit(path, pairs)


def write_new(path, content):
    global NEW_FILES
    NEW_FILES += 1
    print(f"  write {path} (new content)")
    if APPLY:
        (ROOT / path).parent.mkdir(parents=True, exist_ok=True)
        (ROOT / path).write_text(content, encoding="utf-8")


# ---------------------------------------------------------------------------
# 1. Pre-move layout
# ---------------------------------------------------------------------------
for required in [
    "build.zig",
    "src/renderer/skia/skia_bridge.h",
    "src/platform/platform.h",
    "crates/zant-sys/build.rs",
    "crates/zant-sys/prebuilt/sources-macos.txt",
    "crates/zant-sys/prebuilt/sources-windows.txt",
    "tools/tsx/testdata/counter.tsx",
    "tools/zant/package/macos.sh",
    "scaffold/template/DESIGN.md",
    "examples/assets/zant.png",
    "src/math/mhchem_cases.json",
    "crates/zant-tsx/testdata/counter.rs",
]:
    if not (ROOT / required).exists():
        fail(f"required pre-M8 file is missing: {required}")

# ---------------------------------------------------------------------------
# 2. Moves (git mv): native sources, fixtures, runtime assets
# ---------------------------------------------------------------------------
print("\n-- moves")
move_tree("src/renderer/skia", "crates/zant-sys/native/skia")
move("src/platform/platform.h", "crates/zant-sys/native/platform/platform.h")
move("src/platform/pdf.h", "crates/zant-sys/native/platform/pdf.h")
move_tree("src/platform/macos", "crates/zant-sys/native/platform/macos")
move_tree("src/platform/windows", "crates/zant-sys/native/platform/windows")

# TSX corpus: `crates/zant-tsx/tests/golden.rs`, `tests/codegen_app.rs` and
# `src/bin/zant-tsx-bench.rs` read `tools/tsx/testdata/*.tsx`.
tsx_corpus = tracked("tools/tsx/testdata/*.tsx")
move_files(tsx_corpus, "crates/zant-tsx/testdata/tsx")
# The TSX example sources stay useful as parser test corpus; the Rust ports
# live in `crates/zant-examples/tsx/`.
example_tsx = tracked("examples/tsx/*.tsx", "examples/tsx/**/*.tsx")
seen = set()
for file in example_tsx:
    if file in seen:
        continue
    seen.add(file)
    rel = file[len("examples/tsx/"):]
    move(file, f"crates/zant-examples/tsx/corpus/{rel}")
move("examples/assets/zant.png", "crates/zant-examples/assets/zant.png")
move("src/math/mhchem_cases.json", "crates/zant/src/math/mhchem_cases.json")
move("scaffold/template/DESIGN.md", "scaffold/template-ts/DESIGN.md")
move_tree("tools/zant/package", "scripts/package")

# ---------------------------------------------------------------------------
# 3. Deletions: the Zig toolchain and its sources
# ---------------------------------------------------------------------------
print("\n-- deletions")
delete_glob("*.zig")  # 348 files: src/, tools/, tests/, examples/, build.zig
delete("build.zig.zon")
delete("tools", "tools/ (Zig TSX compiler + CLI; fixtures/package moved out)")
delete("src", "src/ (native sources moved to crates/zant-sys/native/)")
delete("examples", "examples/ (Zig examples; TSX corpus + zant.png moved out)")
delete("tests/tsx", "tests/tsx/ (Zig e2e fixtures; Rust mirrors live in crates/zant-tsx/tests/fixtures)")
delete("scaffold/template", "scaffold/template/ (pure-Zig template)")
delete("scripts/win/build_bridge.ps1")
delete("scripts/win/gen_def.sh")
delete("scripts/tsx/create-smoke.sh")
delete("scripts/tsx/swc-corpus.sh")
delete("crates/zant-sys/native/skia/zant_skia.def")
delete("crates/zant-sys/native/skia/zant_skia_resources.h")
delete("crates/zant-sys/native/platform/windows/winsqlite3.def")

# ---------------------------------------------------------------------------
# 4. Native sources: include paths and the old DLL resource branch
# ---------------------------------------------------------------------------
print("\n-- native sources")
edit("crates/zant-sys/native/platform/platform.h", [(
    "/* Zant UI platform C ABI, implemented by src/platform/macos/window.m\n"
    " * (AppKit) and src/platform/windows/window.c (Win32). An app has a main",
    "/* Zant UI platform C ABI, implemented by platform/macos/window.m\n"
    " * (AppKit) and platform/windows/window.c (Win32). An app has a main",
)])
edit("crates/zant-sys/native/platform/macos/window.h", [(
    '#include "src/platform/platform.h"',
    '#include "platform/platform.h"',
)])
edit("crates/zant-sys/native/platform/macos/window.m", [
    ('#include "src/platform/macos/window.h"',
     '#include "platform/macos/window.h"'),
    ('#include "src/platform/platform.h"',
     '#include "platform/platform.h"'),
    ('#include "src/renderer/skia/skia_bridge.h"',
     '#include "skia/skia_bridge.h"'),
])
edit("crates/zant-sys/native/platform/windows/pdf_win.cpp", [
    ("// Windows PDF backend for ui.pdfPreview (src/platform/pdf.h), built into",
     "// Windows PDF backend for ui.pdfPreview (platform/pdf.h), built into"),
    ('#include "src/platform/pdf.h"', '#include "platform/pdf.h"'),
])
edit("crates/zant-sys/native/platform/windows/window.c", [
    ("/* Win32 backend of src/platform/platform.h: one HWND, Skia via zant_skia.dll",
     "/* Win32 backend of platform/platform.h: one HWND, Skia linked into the exe"),
    ('#include "src/platform/platform.h"', '#include "platform/platform.h"'),
    ('#include "src/renderer/skia/skia_bridge.h"', '#include "skia/skia_bridge.h"'),
])
write_new(
    "crates/zant-sys/native/platform/windows/README.md",
    """# Windows native backend

Windows 10/11 x64 implementation of `platform/platform.h`, compiled into the
executable by `crates/zant-sys/build.rs` (MSVC, C11):

| File | Purpose |
|---|---|
| `window.c` | Win32 window: message loop, per-monitor DPI v2, mouse / keyboard / `WM_CHAR` (surrogate pairs), IMM32 composition and candidate placement, cursors, clipboard, app-drawn title bar (`WM_NCCALCSIZE` / `WM_NCHITTEST`, `HTMAXBUTTON` for snap layouts), DWM dark title bar, close-request veto, `IFileOpenDialog` / `IFileSaveDialog`, `TaskDialogIndirect`, `ShellExecuteW`, snapshots |
| `pdf_win.cpp` | PDF backend: PDFium (loaded at runtime), pages rendered to memory pixels |
| `zant.manifest` | Common-Controls v6 manifest embedded by `zant-build` |
| `platform.h` / `pdf.h` (in `..`) | The C ABI both platforms implement |

Rendering lives in `skia/skia_bridge_win.cpp` (DirectWrite fonts, embedded ICU
data and STIX Two Math, Direct3D 12 Ganesh swap chain). The native code is
linked statically; there is no `zant_skia.dll`, `.def` or `dlltool` import
library any more.
""",
)
edit("crates/zant-sys/native/skia/skia_bridge_win.cpp", [
    (
        "// Windows part of the Skia bridge: DirectWrite font manager, ICU data (an\n"
        "// RCDATA resource of zant_skia.dll) and a Direct3D 12 (Ganesh) flip-model\n"
        "// swap chain with vsync'd present.",
        "// Windows part of the Skia bridge: DirectWrite font manager, ICU data (embedded\n"
        "// in the executable by zant-sys) and a Direct3D 12 (Ganesh) flip-model swap\n"
        "// chain with vsync'd present.",
    ),
    ('#include "zant_skia_resources.h"\n\n', ""),
    (
        "#include <malloc.h>\n",
        "",
    ),
    (
        "#include <cstring>\n",
        "",
    ),
    (
        "#ifdef ZANT_STATIC_DATA\n"
        "/* Static-link builds (the Rust port) compile this file into the executable\n"
        "   instead of a DLL, so there are no RCDATA resources to read: zant-sys\n"
        "   embeds icudtl.dat and STIX Two Math in the binary and hands them over with\n"
        "   these two setters before zant_skia_init(). The pointers must stay valid\n"
        "   for the life of the process (zant-sys passes `include_bytes!` statics; the\n"
        "   ICU block is 16-byte aligned there). The Zig build does not define\n"
        "   ZANT_STATIC_DATA, so its DLL path below is unchanged. */\n"
        "static const void *g_static_icu = nullptr;\n"
        "static const void *g_static_math_font = nullptr;\n"
        "static size_t g_static_math_font_size = 0;\n"
        "\n"
        "extern \"C\" void zant_set_icu_data(const void *data, size_t size) {\n"
        "    (void)size;\n"
        "    g_static_icu = data;\n"
        "}\n"
        "\n"
        "extern \"C\" void zant_set_math_font(const void *data, size_t size) {\n"
        "    g_static_math_font = data;\n"
        "    g_static_math_font_size = size;\n"
        "}\n"
        "#endif\n",
        "/* The native layer is linked into the executable, so there are no RCDATA\n"
        "   resources to read: zant-sys embeds icudtl.dat and STIX Two Math in the\n"
        "   binary and hands them over with these two setters before\n"
        "   zant_skia_init(). The pointers must stay valid for the life of the process\n"
        "   (zant-sys passes `include_bytes!` statics; the ICU block is 16-byte\n"
        "   aligned there). */\n"
        "static const void *g_static_icu = nullptr;\n"
        "static const void *g_static_math_font = nullptr;\n"
        "static size_t g_static_math_font_size = 0;\n"
        "\n"
        "extern \"C\" void zant_set_icu_data(const void *data, size_t size) {\n"
        "    (void)size;\n"
        "    g_static_icu = data;\n"
        "}\n"
        "\n"
        "extern \"C\" void zant_set_math_font(const void *data, size_t size) {\n"
        "    g_static_math_font = data;\n"
        "    g_static_math_font_size = size;\n"
        "}\n",
    ),
    (
        "HMODULE this_module() {\n"
        "    HMODULE module = nullptr;\n"
        "    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |\n"
        "                               GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,\n"
        "                       reinterpret_cast<LPCWSTR>(&this_module), &module);\n"
        "    return module;\n"
        "}\n"
        "\n"
        "/* An RCDATA resource of this DLL (bytes live as long as the module). */\n"
        "const void *resource_bytes(int id, DWORD *size) {\n"
        "    HMODULE module = this_module();\n"
        "    HRSRC info = FindResourceW(module, MAKEINTRESOURCEW(id), MAKEINTRESOURCEW(10) /* RT_RCDATA */);\n"
        "    if (!info) return nullptr;\n"
        "    HGLOBAL handle = LoadResource(module, info);\n"
        "    *size = SizeofResource(module, info);\n"
        "    return handle ? LockResource(handle) : nullptr;\n"
        "}\n"
        "\n"
        "/* ICU wants its data 16-byte aligned; copy the resource once. */\n"
        "void load_icu_data() {\n"
        "    static bool done = false;\n"
        "    if (done) return;\n"
        "    done = true;\n"
        "#ifdef ZANT_STATIC_DATA\n"
        "    /* embedded by zant-sys: already 16-byte aligned, lives for the process */\n"
        "    if (g_static_icu) {\n"
        "        C_SetICU(g_static_icu);\n"
        "        return;\n"
        "    }\n"
        "#endif\n"
        "    DWORD size = 0;\n"
        "    const void *bytes = resource_bytes(ZANT_ICU_DATA, &size);\n"
        "    if (!bytes || size == 0) {\n"
        "        fprintf(stderr, \"[zant] ICU data resource missing; text shaping may fail\\n\");\n"
        "        return;\n"
        "    }\n"
        "    void *copy = _aligned_malloc(size, 16);\n"
        "    if (!copy) return;\n"
        "    memcpy(copy, bytes, size);\n"
        "    C_SetICU(copy); /* lives for the whole process */\n"
        "}\n",
        "/* ICU data is embedded by zant-sys: already 16-byte aligned and lives for\n"
        "   the process. */\n"
        "void load_icu_data() {\n"
        "    static bool done = false;\n"
        "    if (done) return;\n"
        "    done = true;\n"
        "    if (g_static_icu) {\n"
        "        C_SetICU(g_static_icu);\n"
        "    } else {\n"
        "        fprintf(stderr, \"[zant] ICU data is missing; text shaping may fail\\n\");\n"
        "    }\n"
        "}\n",
    ),
    (
        "#ifdef ZANT_STATIC_DATA\n"
        "        if (g_static_math_font && g_static_math_font_size) {\n"
        "            static sk_sp<SkFontMgr> static_mgr = SkFontMgr_New_DirectWrite();\n"
        "            if (static_mgr) {\n"
        "                math = static_mgr->makeFromData(\n"
        "                        SkData::MakeWithoutCopy(g_static_math_font, g_static_math_font_size));\n"
        "            }\n"
        "            return math;\n"
        "        }\n"
        "#endif\n"
        "        DWORD size = 0;\n"
        "        const void *bytes = resource_bytes(ZANT_MATH_FONT, &size);\n"
        "        if (bytes && size) {\n"
        "            static sk_sp<SkFontMgr> mgr = SkFontMgr_New_DirectWrite();\n"
        "            if (mgr) math = mgr->makeFromData(SkData::MakeWithoutCopy(bytes, size));\n"
        "        }\n",
        "        if (g_static_math_font && g_static_math_font_size) {\n"
        "            static sk_sp<SkFontMgr> static_mgr = SkFontMgr_New_DirectWrite();\n"
        "            if (static_mgr) {\n"
        "                math = static_mgr->makeFromData(\n"
        "                        SkData::MakeWithoutCopy(g_static_math_font, g_static_math_font_size));\n"
        "            }\n"
        "        }\n",
    ),
])
edit("crates/zant-sys/native/skia/skia_bridge.h", [(
    "#ifdef ZANT_STATIC_DATA\n"
    "/* Static-link builds (the Rust port): hand the ICU common data and the STIX\n"
    " * Two Math font embedded in the executable to the bridge before\n"
    " * zant_skia_init(). Windows-only implementation (skia_bridge_win.cpp); the\n"
    " * pointers must live for the whole process and the ICU block must be 16-byte\n"
    " * aligned. The Zig build does not define ZANT_STATIC_DATA. */\n"
    "void zant_set_icu_data(const void *data, size_t size);\n"
    "void zant_set_math_font(const void *data, size_t size);\n"
    "#endif\n",
    "/* Hand the ICU common data and the STIX Two Math font embedded in the\n"
    " * executable to the bridge before zant_skia_init(). Windows-only\n"
    " * implementation (skia_bridge_win.cpp); the pointers must live for the whole\n"
    " * process and the ICU block must be 16-byte aligned. */\n"
    "void zant_set_icu_data(const void *data, size_t size);\n"
    "void zant_set_math_font(const void *data, size_t size);\n",
)])

# ---------------------------------------------------------------------------
# 5. crates/zant-sys: build.rs, prebuilt list + docs, bindgen comments
# ---------------------------------------------------------------------------
print("\n-- crates/zant-sys")
edit("crates/zant-sys/build.rs", [
    (
        "//! This mirrors the native parts of the Zig `build.zig` (macOS branch,\n"
        "//! `build.zig:12-53`) and `scripts/win/build_bridge.ps1` (Windows branch).\n"
        "//! The native sources stay in place (`src/renderer/skia/`,\n"
        "//! `src/platform/`) during the port; M8 moves them under this crate.\n",
        "//! The native sources live under this crate (`native/skia/`,\n"
        "//! `native/platform/`); during the port they mirrored the native parts of the\n"
        "//! old Zig `build.zig` and `scripts/win/build_bridge.ps1`.\n",
    ),
    (
        "//!          `include_bytes!` (ZANT_STATIC_DATA, see skia_bridge_win.cpp).\n",
        "//!          `include_bytes!` (see skia_bridge_win.cpp).\n",
    ),
    (
        "/// Include paths shared by both platforms (the Zig build passes the repo root\n"
        "/// so native sources can include \"src/platform/platform.h\" style paths).\n"
        "fn add_includes(build: &mut cc::Build, root: &Path) {\n"
        "    build\n"
        "        .include(root)\n",
        "/// Include paths shared by both platforms: the native sources include\n"
        "/// \"platform/platform.h\" / \"skia/skia_bridge.h\" style paths from\n"
        "/// `crates/zant-sys/native/`, and Skia's headers come from\n"
        "/// `third_party/skia-headers`.\n"
        "fn add_includes(build: &mut cc::Build, root: &Path) {\n"
        "    build\n"
        "        .include(root.join(\"crates/zant-sys/native\"))\n",
    ),
    (
        "    // C++: portable bridge + Direct3D 12 backend + PDFium backend. Flags are\n"
        "    // the ones scripts/win/build_bridge.ps1 passes to cl.exe, plus\n"
        "    // ZANT_STATIC_DATA (no DLL resources: data is embedded in the exe).\n",
        "    // C++: portable bridge + Direct3D 12 backend + PDFium backend. Flags are\n"
        "    // the ones the old Windows `build_bridge.ps1` passed to cl.exe (that DLL\n"
        "    // build was removed at M8); no DLL resources: data is embedded in the exe.\n",
    ),
    (
        '        .define("_CRT_SECURE_NO_WARNINGS", None)\n'
        '        .define("ZANT_STATIC_DATA", None);\n',
        '        .define("_CRT_SECURE_NO_WARNINGS", None);\n',
    ),
    (
        "    // Static Skia for MSVC (same archives as build_bridge.ps1).\n",
        "    // Static Skia for MSVC (same archives the old DLL build used).\n",
    ),
    (
        "        // part; pdf.mm keeps the compiler default (matches build.zig).\n",
        "        // part; pdf.mm keeps the compiler default.\n",
    ),
    (
        "    // Static Skia (same archives and order as build.zig).\n",
        "    // Static Skia (same archives and order as the old Zig build).\n",
    ),
    (
        "//! The Zig build puts both into `zant_skia.dll` as RCDATA resources (ids 101 /\n"
        "//! 102). A statically linked Rust executable has no such resources, so the\n"
        "//! bytes are compiled in and handed to the bridge with `zant_set_icu_data` /\n"
        "//! `zant_set_math_font` before `zant_skia_init()`.\n",
        "//! A statically linked executable has no RCDATA resources, so the bytes are\n"
        "//! compiled in and handed to the bridge with `zant_set_icu_data` /\n"
        "//! `zant_set_math_font` before `zant_skia_init()`.\n",
    ),
])
edit("crates/zant-sys/prebuilt.rs", [(
    "//! (`src/renderer/skia/*.cpp`, `src/platform/*`, `skia_bridge_mac.mm`, …) on\n",
    "//! (`crates/zant-sys/native/skia/*.cpp`, `native/platform/*`, …) on\n",
)])
edit("crates/zant-sys/src/lib.rs", [(
    "//! (bindgen) from `src/renderer/skia/skia_bridge.h`, `src/platform/platform.h`\n"
    "//! and `src/platform/pdf.h`; the results are checked in, so users need no\n",
    "//! (bindgen) from `crates/zant-sys/native/skia/skia_bridge.h`,\n"
    "//! `native/platform/platform.h` and `native/platform/pdf.h`; the results are\n"
    "//! checked in, so users need no\n",
)])
edit("crates/zant-sys/src/skia.rs", [(
    "from src/renderer/skia/skia_bridge.h",
    "from crates/zant-sys/native/skia/skia_bridge.h",
)])
edit("crates/zant-sys/src/platform.rs", [(
    "from src/platform/platform.h",
    "from crates/zant-sys/native/platform/platform.h",
)])
edit("crates/zant-sys/src/pdf.rs", [(
    "from src/platform/pdf.h",
    "from crates/zant-sys/native/platform/pdf.h",
)])
write_new(
    "crates/zant-sys/prebuilt/sources-macos.txt",
    """# Native sources compiled by crates/zant-sys/build.rs on macOS.
#
# User-facing description: WP-6.6 in docs/rust-port/plan.md.
#
# This file is canonical: build.rs compiles the `.cpp` / `.mm` / `.m` entries in
# this order, and the prebuilt native library's `source_hash` (metadata.txt) is
# the SHA-256 over this whole list, headers included, in this order. Adding,
# removing, reordering or editing any entry invalidates a prebuilt archive, and
# build.rs falls back to compiling from source (with a warning).
crates/zant-sys/native/skia/skia_bridge.cpp
crates/zant-sys/native/skia/skia_bridge.h
crates/zant-sys/native/skia/skia_bridge_internal.h
crates/zant-sys/native/skia/skia_bridge_mac.mm
crates/zant-sys/native/platform/macos/window.m
crates/zant-sys/native/platform/macos/window.h
crates/zant-sys/native/platform/macos/pdf.mm
crates/zant-sys/native/platform/platform.h
crates/zant-sys/native/platform/pdf.h
""",
)
write_new(
    "crates/zant-sys/prebuilt/sources-windows.txt",
    """# Native sources compiled by crates/zant-sys/build.rs on Windows.
#
# User-facing description: WP-6.6 in docs/rust-port/plan.md.
#
# This file is canonical: build.rs compiles the `.cpp` / `.c` entries in this
# order, and the prebuilt native library's `source_hash` (metadata.txt) is the
# SHA-256 over this whole list, headers included, in this order. Adding,
# removing, reordering or editing any entry invalidates a prebuilt archive, and
# build.rs falls back to compiling from source (with a warning).
crates/zant-sys/native/skia/skia_bridge.cpp
crates/zant-sys/native/skia/skia_bridge.h
crates/zant-sys/native/skia/skia_bridge_internal.h
crates/zant-sys/native/skia/skia_bridge_win.cpp
crates/zant-sys/native/platform/windows/pdf_win.cpp
crates/zant-sys/native/platform/windows/window.c
crates/zant-sys/native/platform/platform.h
crates/zant-sys/native/platform/pdf.h
""",
)
edit("crates/zant-sys/prebuilt/README.md", [
    (
        "`crates/zant-sys/build.rs` normally compiles Zant UI's native layer — the Skia\n"
        "bridge (`src/renderer/skia/*.cpp`, `skia_bridge_mac.mm` / `skia_bridge_win.cpp`),\n"
        "the AppKit / Win32 window backend and the PDF backend — on every clean app\n"
        "build.",
        "`crates/zant-sys/build.rs` normally compiles Zant UI's native layer — the Skia\n"
        "bridge (`crates/zant-sys/native/skia/*.cpp`, `skia_bridge_mac.mm` /\n"
        "`skia_bridge_win.cpp`), the AppKit / Win32 window backend and the PDF backend —\n"
        "on every clean app build.",
    ),
    (
        "The design mirrors the Zig `third_party/zant-skia-prebuilt` +\n"
        "`hasPrebuiltBridge()` mechanism (`build.zig`), with one difference the Rust\n"
        "port needs: the archive only contains the objects build.rs would compile, so\n"
        "the build must prove it matches the tree — not just check that a file exists.",
        "The design follows the old Zig `third_party/zant-skia-prebuilt` /\n"
        "`hasPrebuiltBridge()` mechanism, with one difference: the archive only\n"
        "contains the objects build.rs would compile, so the build must prove it\n"
        "matches the tree — not just check that a file exists.",
    ),
    (
        "- C++ ABI: the archive is linked into the same executable as the Rust code, so\n"
        "  on Windows it must be built with MSVC's STL and `/MD` like the Skia archives\n"
        "  it links against (`build_bridge.ps1` uses the same flags).\n",
        "- C++ ABI: the archive is linked into the same executable as the Rust code, so\n"
        "  on Windows it must be built with MSVC's STL and `/MD` like the Skia archives\n"
        "  it links against (the flags live in `crates/zant-sys/build.rs`).\n",
    ),
    (
        "- macOS: the manifest/flags match `build.zig`'s native compile; the archive is\n"
        "  built with the same source list, so a prebuilt and a from-source build are\n"
        "  the same program.\n",
        "- macOS: the flags match `crates/zant-sys/build.rs`'s native compile; the\n"
        "  archive is built with the same source list, so a prebuilt and a\n"
        "  from-source build are the same program.\n",
    ),
])

# ---------------------------------------------------------------------------
# 6. Rust references to moved fixtures, assets and template files
# ---------------------------------------------------------------------------
print("\n-- rust references")
edit("crates/zant/src/math/mhchem/mod.rs", [(
    'include_str!("../../../../../src/math/mhchem_cases.json")',
    'include_str!("../mhchem_cases.json")',
)])
edit("crates/zant/src/math/mhchem/tests.rs", [(
    'from_str(FIXTURE_JSON).expect("src/math/mhchem_cases.json")',
    'from_str(FIXTURE_JSON).expect("crates/zant/src/math/mhchem_cases.json")',
)])
edit("crates/zant/tests/mhchem_render_test.rs", [(
    'from_str(mhchem::FIXTURE_JSON).expect("src/math/mhchem_cases.json")',
    'from_str(mhchem::FIXTURE_JSON).expect("crates/zant/src/math/mhchem_cases.json")',
)])
edit("crates/zant-build/src/lib.rs", [(
    "//! Common-Controls manifest (`src/platform/windows/zant.manifest`) and the GUI\n",
    "//! Common-Controls manifest (`crates/zant-sys/native/platform/windows/zant.manifest`)\n"
    "//! and the GUI\n",
)])
edit("crates/zant-build/src/windows.rs", [(
    'crate::zant_ui_root().join("src/platform/windows/zant.manifest")',
    'crate::zant_ui_root().join("crates/zant-sys/native/platform/windows/zant.manifest")',
)])

edit("crates/zant-tsx/tests/golden.rs", [
    (
        "//! WP-5.9 golden cases: the 31 Rust outputs of `tools/tsx/testdata/*.tsx`, and\n",
        "//! WP-5.9 golden cases: the 31 Rust outputs of the TSX corpus, and\n",
    ),
    (
        "//!   (`tools/tsx/testdata/entities.tsx` has `&#xD800;`; the Zig compiler used\n",
        "//!   (`crates/zant-tsx/testdata/tsx/entities.tsx` has `&#xD800;`; the Zig\n"
        "//!   compiler used\n",
    ),
    (
        "/// The 32 `tools/tsx/testdata/*.tsx` names in the Zig golden order\n"
        "/// (`tools/tsx/golden_test.zig`); the generated ones are below.\n",
        "/// The 32 corpus names in the old Zig golden order; the generated ones are\n"
        "/// below. The corpus moved from `tools/tsx/testdata/` at M8.\n",
    ),
    (
        "/// `tools/tsx/testdata/<name>.tsx` that has no `.rs` golden: it is rejected\n"
        "/// by the Rust compiler for the reason in the module doc (`entities`: lone\n"
        "/// surrogate in a `zant::Str`).\n",
        "/// Corpus files that have no `.rs` golden: rejected by the Rust compiler for\n"
        "/// the reason in the module doc (`entities`: lone surrogate in a `zant::Str`).\n",
    ),
    (
        '    manifest_dir().join("../../tools/tsx/testdata")\n',
        '    manifest_dir().join("testdata/tsx")\n',
    ),
])
edit("crates/zant-tsx/tests/codegen_app.rs", [(
    'let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/tsx/testdata");',
    'let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/tsx");',
)])
edit("crates/zant-tsx/src/parser.rs", [
    (
        "        let mut paths: Vec<std::path::PathBuf> = Vec::new();\n"
        "        let testdata = root.join(\"tools/tsx/testdata\");\n"
        "        // The Zig sources (and their fixtures) are deleted at M8; the test is\n"
        "        // a no-op once they are gone.\n"
        "        if !testdata.is_dir() {\n"
        "            return;\n"
        "        }\n",
        "        let mut paths: Vec<std::path::PathBuf> = Vec::new();\n"
        "        let testdata = root.join(\"crates/zant-tsx/testdata/tsx\");\n"
        "        if !testdata.is_dir() {\n"
        "            return;\n"
        "        }\n",
    ),
    (
        '        let examples = root.join("examples/tsx");\n',
        '        let examples = root.join("crates/zant-examples/tsx/corpus");\n',
    ),
])
edit("crates/zant-examples/src/bin/zant-tsx-bench.rs", [
    (
        "//! The TSX compile benchmark (WP-5.10): compile every `tools/tsx/testdata/*.tsx`\n"
        "//! through the in-process compiler, `warmup + iterations` times each, and report\n"
        "//! the same statistics line as the Zig frame benchmark\n"
        "//! (`examples/tsx/bench_harness.zig`).\n"
        "//!\n"
        "//! The shipped Zig binary (`zig build bench-tsx` → `zant-tsx-bench`) measures\n"
        "//! *frames* (clicks, typing, …), not compile time, so the Zig side of this\n"
        "//! comparison is `tools/tsx/compile_bench.zig` (the same `tools/tsx/compile.zig`\n"
        "//! the Zig compiler uses, in process). `scripts/tsx/examples-check.sh` builds\n"
        "//! both, runs both on the same corpus and gates the ratio at 1.25×.\n",
        "//! The TSX compile benchmark (WP-5.10): compile every corpus `.tsx`\n"
        "//! (`crates/zant-tsx/testdata/tsx/`) through the in-process compiler,\n"
        "//! `warmup + iterations` times each, and report the same statistics line as the\n"
        "//! old Zig frame benchmark.\n"
        "//!\n"
        "//! During the migration the comparison baseline was the Zig compiler in process\n"
        "//! (`tools/tsx/compile_bench.zig`, gate ≤1.25×); the Zig toolchain and sources\n"
        "//! were removed at M8, so `scripts/tsx/examples-check.sh` now runs the Rust side\n"
        "//! alone and checks the corpus count.\n",
    ),
    (
        '    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/tsx/testdata")\n',
        '    Path::new(env!("CARGO_MANIFEST_DIR")).join("../zant-tsx/testdata/tsx")\n',
    ),
    (
        '            "expected the 32 tools/tsx/testdata files, found {}",\n',
        '            "expected the 32 crates/zant-tsx/testdata/tsx files, found {}",\n',
    ),
])
edit("crates/zant-examples/src/bin/zant-demo.rs", [
    (
        'const IMAGE_SRC: &str = "examples/assets/zant.png";',
        'const IMAGE_SRC: &str = "crates/zant-examples/assets/zant.png";',
    ),
    (
        "    /// `State::init` normally decodes `examples/assets/zant.png` relative to\n",
        "    /// `State::init` normally decodes `crates/zant-examples/assets/zant.png`\n"
        "    /// relative to\n",
    ),
    (
        '        let path = concat!(\n'
        '            env!("CARGO_MANIFEST_DIR"),\n'
        '            "/../../examples/assets/zant.png"\n'
        '        );\n',
        '        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/zant.png");\n',
    ),
])
edit("crates/zant-examples/src/components_app/view.rs", [(
    'src: "examples/assets/zant.png".into(),',
    'src: "crates/zant-examples/assets/zant.png".into(),',
)])

# ---------------------------------------------------------------------------
# 7. CLI: the pure-Zig template and `--lang zig` are gone
# ---------------------------------------------------------------------------
print("\n-- zant-cli")
write_new(
    "crates/zant-cli/src/project.rs",
    """//! Project language conventions shared by create, sync and the CLI.
//!
//! The pure-Zig template (`zant create --lang zig`) was removed together with
//! the Zig toolchain at M8 (docs/rust-port/m8-cutover.md); TSX is the only
//! application language.

use std::path::Path;

/// Whether `root` is a TSX application, i.e. has `src/App.tsx`. Such a project
/// needs the `build.rs` that calls `zant_build::tsx_app()`.
pub fn is_tsx_app(root: &str) -> bool {
    Path::new(root).join("src").join("App.tsx").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_tsx_app_by_its_app_tsx() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target/zant-cli-test/project-detect");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("src dir");
        let root_str = root.to_str().expect("utf8 path");
        assert!(!is_tsx_app(root_str));
        std::fs::write(root.join("src/App.tsx"), "").expect("write App.tsx");
        assert!(is_tsx_app(root_str));
    }
}
""",
)
edit("crates/zant-cli/src/main.rs", [
    (
        "  create <项目名>       生成项目（--lang ts|zig，默认 TS）\n",
        "  create <项目名>       生成项目（TSX + Rust）\n",
    ),
    (
        "    if project::detect(root)? == project::Language::Ts\n"
        "        && !Path::new(root).join(\"build.rs\").is_file()\n",
        "    if project::is_tsx_app(root) && !Path::new(root).join(\"build.rs\").is_file()\n",
    ),
    (
        "            \"{root} 没有 Cargo.toml：zant run/compile/package 用 cargo build 构建 Cargo 应用；`--lang zig` 的旧模板只保留到 M8（没有 Cargo.toml，不能构建）\"\n",
        "            \"{root} 没有 Cargo.toml：zant run/compile/package 用 cargo build 构建 Cargo 应用；请用 zant create 生成 TSX 项目\"\n",
    ),
    (
        "        /// 语言：ts（默认）或 zig（旧模板）\n"
        "        #[arg(long, value_name = \"ts|zig\")]\n"
        "        lang: Option<String>,\n",
        "",
    ),
    (
        "            name,\n"
        "            yes,\n"
        "            force,\n"
        "            lang,\n"
        "            title,\n"
        "            style,\n"
        "            bundle_id,\n"
        "            sandbox,\n"
        "            sign,\n"
        "        } => create::cmd_create(create::CreateOptions {\n"
        "            name,\n"
        "            yes,\n"
        "            force,\n"
        "            lang,\n",
        "            name,\n"
        "            yes,\n"
        "            force,\n"
        "            title,\n"
        "            style,\n"
        "            bundle_id,\n"
        "            sandbox,\n"
        "            sign,\n"
        "        } => create::cmd_create(create::CreateOptions {\n"
        "            name,\n"
        "            yes,\n"
        "            force,\n",
    ),
])
edit("crates/zant-cli/src/create.rs", [
    ("use crate::project::Language;\n", ""),
    (
        "    pub force: bool,\n"
        "    pub lang: Option<String>,\n"
        "    pub title: Option<String>,\n",
        "    pub force: bool,\n"
        "    pub title: Option<String>,\n",
    ),
    (
        "pub fn cmd_create(options: CreateOptions) -> std::io::Result<()> {\n"
        "    let language_arg = options\n"
        "        .lang\n"
        "        .as_deref()\n"
        "        .map(|value| Language::parse(value).unwrap_or_else(|| fail(\"语言必须是 zig 或 ts\")));\n"
        "    let name = options.name.as_str();\n",
        "pub fn cmd_create(options: CreateOptions) -> std::io::Result<()> {\n"
        "    let name = options.name.as_str();\n",
    ),
    (
        "    let mut language = language_arg.unwrap_or(Language::Ts);\n"
        "    let mut title = options.title.clone().unwrap_or(default_title);\n",
        "    let mut title = options.title.clone().unwrap_or(default_title);\n",
    ),
    (
        "    if !options.yes {\n"
        "        eprintln!(\"\\nZant UI 项目生成器\\n\");\n"
        "        if language_arg.is_none() {\n"
        "            eprintln!(\n"
        "                \"语言：\\n  1) TypeScript + TSX（Rust 应用，默认）\\n  2) Zig（旧模板，仅保留到 M8）\"\n"
        "            );\n"
        "            loop {\n"
        "                let value = prompt(\"选择\", \"1\");\n"
        "                let choice = match value.as_str() {\n"
        "                    \"1\" => \"ts\",\n"
        "                    \"2\" => \"zig\",\n"
        "                    other => other,\n"
        "                };\n"
        "                if let Some(chosen) = Language::parse(choice) {\n"
        "                    language = chosen;\n"
        "                    break;\n"
        "                }\n"
        "                eprintln!(\"请输入 1（TypeScript + TSX）或 2（Zig）\");\n"
        "            }\n"
        "        }\n"
        "        if options.title.is_none() {\n",
        "    if !options.yes {\n"
        "        eprintln!(\"\\nZant UI 项目生成器\\n\");\n"
        "        if options.title.is_none() {\n",
    ),
    (
        "        target: target.to_string_lossy().into_owned(),\n"
        "        language,\n"
        "        title: title.clone(),\n",
        "        target: target.to_string_lossy().into_owned(),\n"
        "        title: title.clone(),\n",
    ),
])
edit("crates/zant-cli/src/scaffold.rs", [
    (
        "use crate::project::Language;\n"
        "use std::path::Path;\n",
        "use std::path::Path;\n",
    ),
    (
        "/// The legacy pure-Zig template (`--lang zig`). It is only kept so the old\n"
        "/// language stays selectable until M8 (plan.md): the CLI builds apps with\n"
        "/// `cargo build` since WP-6.1, so a Zig-template project does not compile\n"
        "/// any more. WP-6.3 does not delete it because `scripts/tsx/create-smoke.sh`\n"
        "/// and the Windows scaffold check still reference it; removing it is M8\n"
        "/// cleanup.\n"
        "pub const TEMPLATE_ZIG: &str = \"scaffold/template\";\n"
        "\n"
        "/// The TSX template: a Rust application (`Cargo.toml` + `build.rs` +\n"
        "/// `rust-toolchain.toml`). `src/` holds only TS/TSX, `native/` the Rust native\n"
        "/// modules, and `zant_build::tsx_app()` (WP-6.2) compiles the TSX in `build.rs`.\n"
        "/// This is the default (and future only) `zant create` template.\n"
        "pub const TEMPLATE_TS: &str = \"scaffold/template-ts\";\n"
        "\n"
        "/// The template directory of a language (the constant a later WP updates).\n"
        "pub fn template_dir(language: Language) -> &'static str {\n"
        "    match language {\n"
        "        Language::Zig => TEMPLATE_ZIG,\n"
        "        Language::Ts => TEMPLATE_TS,\n"
        "    }\n"
        "}\n",
        "/// The TSX template: a Rust application (`Cargo.toml` + `build.rs` +\n"
        "/// `rust-toolchain.toml`). `src/` holds only TS/TSX, `native/` the Rust native\n"
        "/// modules, and `zant_build::tsx_app()` (WP-6.2) compiles the TSX in `build.rs`.\n"
        "/// It is the only `zant create` template since M8 (the pure-Zig template was\n"
        "/// deleted with the Zig toolchain).\n"
        "pub const TEMPLATE_TS: &str = \"scaffold/template-ts\";\n",
    ),
    (
        "    pub target: String,\n"
        "    pub language: Language,\n"
        "    pub title: String,\n",
        "    pub target: String,\n"
        "    pub title: String,\n",
    ),
    (
        "pub fn write(options: &Options) -> std::io::Result<()> {\n"
        "    let source = Path::new(&options.framework).join(template_dir(options.language));\n"
        "    copy_tree(&source, Path::new(&options.target), options, true)?;\n"
        "    if options.language == Language::Ts {\n"
        "        // The TSX template ships its own icons (`assets/`), and both\n"
        "        // languages show the same design rules, so only DESIGN.md is shared\n"
        "        // with the Zig template. The editor type declarations of the `zant`\n"
        "        // module are the same file the TSX compiler uses.\n"
        "        let design = Path::new(&options.framework).join(\"scaffold/template/DESIGN.md\");\n"
        "        let dest = Path::new(&options.target).join(\"DESIGN.md\");\n"
        "        let data = crate::read_file(&design)?;\n"
        "        crate::write_file(&dest, &data)?;\n"
        "        let types = Path::new(&options.framework).join(\"tools/tsx/zant.d.ts\");\n"
        "        let dest_types = Path::new(&options.target).join(\"zant.d.ts\");\n"
        "        let declarations = crate::read_file(&types)?;\n"
        "        crate::write_file(&dest_types, &declarations)?;\n"
        "    } else {\n"
        "        let ignore = Path::new(&options.target).join(\".gitignore\");\n"
        "        crate::write_file(\n"
        "            &ignore,\n"
        "            b\"# build output and caches\\nzig-out*/\\ntarget/\\n.zig-cache/\\n# Zant UI link for projects on another drive (Windows)\\n.zant/\\n# generated by `zant` on macOS from app.config.json\\nInfo.plist\\napp.entitlements\\n\",\n"
        "        )?;\n"
        "    }\n"
        "    Ok(())\n"
        "}\n",
        "pub fn write(options: &Options) -> std::io::Result<()> {\n"
        "    let source = Path::new(&options.framework).join(TEMPLATE_TS);\n"
        "    copy_tree(&source, Path::new(&options.target), options, true)?;\n"
        "    // `zant.d.ts` is generated from the Rust tag table (`zant-tsx-dts`), so\n"
        "    // it is copied from the compiler crate instead of being kept in the\n"
        "    // template (a template copy would go stale).\n"
        "    let types = Path::new(&options.framework).join(\"crates/zant-tsx/zant.d.ts\");\n"
        "    let dest_types = Path::new(&options.target).join(\"zant.d.ts\");\n"
        "    let declarations = crate::read_file(&types)?;\n"
        "    crate::write_file(&dest_types, &declarations)?;\n"
        "    Ok(())\n"
        "}\n",
    ),
    (
        "    fn options(target: &std::path::Path, language: Language) -> Options {\n"
        "        Options {\n"
        "            framework: repo_root(),\n"
        "            target: target.to_string_lossy().into_owned(),\n"
        "            language,\n"
        "            title: \"My \\\"App\\\"\".to_string(),\n",
        "    fn options(target: &std::path::Path) -> Options {\n"
        "        Options {\n"
        "            framework: repo_root(),\n"
        "            target: target.to_string_lossy().into_owned(),\n"
        "            title: \"My \\\"App\\\"\".to_string(),\n",
    ),
    (
        "    #[test]\n"
        "    fn template_directories_are_constants() {\n"
        "        assert_eq!(template_dir(Language::Zig), TEMPLATE_ZIG);\n"
        "        assert_eq!(template_dir(Language::Ts), TEMPLATE_TS);\n"
        "        assert!(Path::new(&repo_root()).join(TEMPLATE_ZIG).is_dir());\n"
        "        assert!(Path::new(&repo_root()).join(TEMPLATE_TS).is_dir());\n"
        "    }\n"
        "\n"
        "    #[test]\n"
        "    fn copies_the_zig_template_and_substitutes_metadata() {\n"
        "        let dir = temp_dir(\"scaffold-zig\");\n"
        "        let target = dir.join(\"my-app\");\n"
        "        write(&options(&target, Language::Zig)).expect(\"scaffold\");\n"
        "        assert!(target.join(\"src/main.zig\").is_file());\n"
        "        assert!(target.join(\"src/components/dashboard.zig\").is_file());\n"
        "        assert!(target.join(\"build.zig\").is_file());\n"
        "        assert!(target.join(\"assets/icons/app-macos.png\").is_file());\n"
        "        // app.config.json is written by create, not copied from the template\n"
        "        assert!(!target.join(\"app.config.json\").exists());\n"
        "        assert!(target.join(\".gitignore\").is_file());\n"
        "        let zon = std::fs::read_to_string(target.join(\"build.zig.zon\")).expect(\"zon\");\n"
        "        assert!(zon.contains(\".name = .my_app\"), \"{zon}\");\n"
        "        assert!(zon.contains(\".path = \\\"../zant-ui\\\"\"), \"{zon}\");\n"
        "        assert!(zon.contains(\"0x\"), \"{zon}\");\n"
        "        let readme = std::fs::read_to_string(target.join(\"README.md\")).expect(\"readme\");\n"
        "        assert!(readme.contains(\"com.acme.myapp\"), \"{readme}\");\n"
        "        assert!(readme.contains(\"My \\\"App\\\"\"), \"{readme}\");\n"
        "        let main = std::fs::read_to_string(target.join(\"src/main.zig\")).expect(\"main\");\n"
        "        assert!(main.contains(\".top = 56\"), \"{main}\");\n"
        "        // Only substitutes in text files, not in the binary icon assets.\n"
        "        let icon = std::fs::read(target.join(\"assets/icons/app-macos.png\")).expect(\"icon\");\n"
        "        let original = std::fs::read(\n"
        "            Path::new(&repo_root()).join(\"scaffold/template/assets/icons/app-macos.png\"),\n"
        "        )\n"
        "        .expect(\"original icon\");\n"
        "        assert_eq!(icon, original);\n"
        "    }\n"
        "\n"
        "    #[test]\n"
        "    fn copies_the_rust_ts_template_and_substitutes_metadata() {\n"
        "        let dir = temp_dir(\"scaffold-ts\");\n"
        "        let target = dir.join(\"my-ts-app\");\n"
        "        write(&options(&target, Language::Ts)).expect(\"scaffold\");\n",
        "    #[test]\n"
        "    fn template_directory_is_the_tsx_template() {\n"
        "        assert!(Path::new(&repo_root()).join(TEMPLATE_TS).is_dir());\n"
        "        assert!(!Path::new(&repo_root())\n"
        "            .join(\"scaffold/template\")\n"
        "            .exists());\n"
        "    }\n"
        "\n"
        "    #[test]\n"
        "    fn copies_the_rust_ts_template_and_substitutes_metadata() {\n"
        "        let dir = temp_dir(\"scaffold-ts\");\n"
        "        let target = dir.join(\"my-ts-app\");\n"
        "        write(&options(&target)).expect(\"scaffold\");\n",
    ),
    (
        "        // The Zig build files are gone from the TS template.\n"
        "        for file in [\n"
        "            \"build.zig\",\n"
        "            \"build.zig.zon\",\n"
        "            \"src/main.zig\",\n"
        "            \"native/lib.zig\",\n"
        "            \"native/system.zig\",\n"
        "        ] {\n"
        "            assert!(!target.join(file).exists(), \"{file} should not be copied\");\n"
        "        }\n",
        "        // No Zig files exist in the template any more.\n"
        "        assert!(!target.join(\"src/main.zig\").exists());\n",
    ),
])
edit("crates/zant-cli/src/distribute.rs", [(
    '        .join("tools/zant/package")\n',
    '        .join("scripts/package")\n',
)])
edit("crates/zant-cli/src/distribute/macos.rs", [(
    '.join("tools/zant/package/dmg-background.png")',
    '.join("scripts/package/dmg-background.png")',
)])
edit("crates/zant-cli/tests/cli.rs", [(
    "#[test]\n"
    "fn create_lang_zig_still_copies_the_legacy_template() {\n"
    "    // Kept selectable until M8 deletes `scaffold/template/`; the CLI cannot\n"
    "    // build it any more (`cargo build` needs Cargo.toml), so only the file\n"
    "    // layout is checked here.\n"
    "    let dir = temp_dir(\"create-zig\");\n"
    "    let output = run(zant()\n"
    "        .current_dir(&dir)\n"
    "        .args([\"create\", \"demo-zig\", \"--yes\", \"--lang\", \"zig\"]));\n"
    "    assert!(output.status.success(), \"{}\", stderr(&output));\n"
    "    let project = dir.join(\"demo-zig\");\n"
    "    for file in [\n"
    "        \"build.zig\",\n"
    "        \"build.zig.zon\",\n"
    "        \"src/main.zig\",\n"
    "        \"src/app_config.rs\",\n"
    "    ] {\n"
    "        assert!(project.join(file).is_file(), \"{file} missing\");\n"
    "    }\n"
    "    assert!(!project.join(\"src/App.tsx\").exists());\n"
    "}\n"
    "\n",
    "",
)])

# ---------------------------------------------------------------------------
# 8. Scripts: bindings, package/release, parity, TSX checks, Windows helpers
# ---------------------------------------------------------------------------
print("\n-- scripts")
edit("scripts/gen_bindings.sh", [
    (
        "#   src/renderer/skia/skia_bridge.h -> crates/zant-sys/src/skia.rs\n"
        "#   src/platform/platform.h         -> crates/zant-sys/src/platform.rs\n"
        "#   src/platform/pdf.h              -> crates/zant-sys/src/pdf.rs\n",
        "#   crates/zant-sys/native/skia/skia_bridge.h -> crates/zant-sys/src/skia.rs\n"
        "#   crates/zant-sys/native/platform/platform.h -> crates/zant-sys/src/platform.rs\n"
        "#   crates/zant-sys/native/platform/pdf.h      -> crates/zant-sys/src/pdf.rs\n",
    ),
    (
        "# --no-layout-tests: the same generated file is compiled on macOS and Windows,\n"
        "# and layout assertions are host-specific.\n"
        "# -DZANT_STATIC_DATA: keeps the Windows static-link setters in the bindings on\n"
        "# every host (they are only called under cfg(windows)).\n",
        "# --no-layout-tests: the same generated file is compiled on macOS and Windows,\n"
        "# and layout assertions are host-specific.\n",
    ),
    (
        "        -- -DZANT_STATIC_DATA -I.\n",
        "        -- -I crates/zant-sys/native\n",
    ),
    (
        "generate src/renderer/skia/skia_bridge.h crates/zant-sys/src/skia.rs\n"
        "generate src/platform/platform.h crates/zant-sys/src/platform.rs\n"
        "generate src/platform/pdf.h crates/zant-sys/src/pdf.rs\n",
        "generate crates/zant-sys/native/skia/skia_bridge.h crates/zant-sys/src/skia.rs\n"
        "generate crates/zant-sys/native/platform/platform.h crates/zant-sys/src/platform.rs\n"
        "generate crates/zant-sys/native/platform/pdf.h crates/zant-sys/src/pdf.rs\n",
    ),
])
edit("scripts/release/package.sh", [
    (
        "#   zant-ui/                  the committed Rust workspace (git archive): Cargo.toml,\n"
        "#                             Cargo.lock, rust-toolchain.toml, crates/ and the native\n"
        "#                             sources it compiles (src/renderer/skia, src/platform)\n",
        "#   zant-ui/                  the committed Rust workspace (git archive): Cargo.toml,\n"
        "#                             Cargo.lock, rust-toolchain.toml, crates/ and the native\n"
        "#                             sources it compiles (crates/zant-sys/native/skia and\n"
        "#                             crates/zant-sys/native/platform)\n",
    ),
    (
        "    src/platform/platform.h src/renderer/skia/skia_bridge.h \\\n"
        "    src/renderer/skia/skia_bridge.cpp scripts/fetch_skia.sh \\\n",
        "    crates/zant-sys/native/platform/platform.h \\\n"
        "    crates/zant-sys/native/skia/skia_bridge.h \\\n"
        "    crates/zant-sys/native/skia/skia_bridge.cpp scripts/fetch_skia.sh \\\n",
    ),
])
edit("scripts/release/package.ps1", [
    (
        "#                                        crates/ and the native sources it compiles\n"
        "#                                        (src/renderer/skia, src/platform)\n",
        "#                                        crates/ and the native sources it compiles\n"
        "#                                        (crates/zant-sys/native/skia, platform)\n",
    ),
    (
        "# The Skia bridge is linked into the app statically in the Rust port, so there\n"
        "# is no zant_skia.dll / zant-skia-prebuilt/ in the archive (Zig: both existed).\n",
        "# The Skia bridge is linked into the app statically, so there is no\n"
        "# zant_skia.dll or zant-skia-prebuilt/ in the archive.\n",
    ),
])
edit("scripts/release/prebuilt.sh", [(
    "# compiling C++/ObjC (WP-6.6, docs/rust-port/plan.md §5). Same idea as the Zig\n"
    "# build's third_party/zant-skia-prebuilt: the release workflow compiles the\n"
    "# native layer once per platform and ships the archive inside\n",
    "# compiling C++/ObjC (WP-6.6, docs/rust-port/plan.md §5). The release workflow\n"
    "# compiles the native layer once per platform and ships the archive inside\n",
)])
edit("scripts/release/prebuilt.ps1", [(
    "# compiling C++/C (WP-6.6, docs/rust-port/plan.md §5). Same idea as the Zig\n"
    "# build's third_party/zant-skia-prebuilt: the release workflow compiles the\n"
    "# native layer once per platform and ships the archive inside\n",
    "# compiling C++/C (WP-6.6, docs/rust-port/plan.md §5). The release workflow\n"
    "# compiles the native layer once per platform and ships the archive inside\n",
)])

# ---------------------------------------------------------------------------
# 9. CI: no Zig setup / Zig build any more
# ---------------------------------------------------------------------------
print("\n-- CI")
edit(".github/workflows/rust.yml", [
    (
        "# Rust port CI: formatting, clippy, tests and the Zig parity build.\n"
        "#\n"
        "# Mirrors plan.md §3 rule 7 on both platforms:\n"
        "#   cargo fmt --all -- --check\n"
        "#   cargo clippy --workspace --all-targets -- -D warnings\n"
        "#   cargo test -p <crate> / cargo test --workspace\n"
        "#   zig build test          (the Zig version stays buildable during the port)\n",
        "# Rust CI: formatting, clippy and the test suite on both platforms.\n"
        "#\n"
        "#   cargo fmt --all -- --check\n"
        "#   cargo clippy --workspace --all-targets -- -D warnings\n"
        "#   cargo test --workspace\n",
    ),
    (
        "env:\n"
        "  ZIG_VERSION: 0.17.1\n"
        "  CARGO_TERM_COLOR: always\n",
        "env:\n"
        "  CARGO_TERM_COLOR: always\n",
    ),
    (
        "      - name: Zig\n"
        "        uses: mlugg/setup-zig@v2\n"
        "        with:\n"
        "          version: ${{ env.ZIG_VERSION }}\n"
        "\n",
        "",
    ),
    (
        "\n"
        "      # The Zig framework must keep building: it is the behavior reference.\n"
        "      - name: zig build test\n"
        "        run: zig build test\n",
        "",
    ),
])
edit(".github/workflows/release.yml", [(
    "#\n"
    "# The Rust port builds the CLI with cargo, so no Zig is downloaded any more:\n"
    "# the runners' rustup installs the toolchain pinned in rust-toolchain.toml\n"
    "# (channel and components), and the scripts still fetch the pinned Skia first.\n",
    "#\n"
    "# The runners' rustup installs the toolchain pinned in rust-toolchain.toml\n"
    "# (channel and components), and the scripts fetch the pinned Skia first.\n",
)])

# ---------------------------------------------------------------------------
# 10. Scripts that move from Zig to Cargo / post-M8 behaviour
# ---------------------------------------------------------------------------
print("\n-- post-M8 scripts")
# `scripts/parity/bench.sh`: documented Rust-only mode when the Zig baseline is
# gone, comparing against the pre-M8 numbers recorded in status.md.
edit("scripts/parity/bench.sh", [
    (
        "#   * every scenario's average frame time must stay within 1.25x of Zig;\n"
        "#   * steady-state frames must stay far below 16.7 ms (60 fps with margin;\n"
        "#     the `open` scenario is a cold first frame and only gets the ratio gate).\n",
        "#   * every scenario's average frame time must stay within 1.25x of the\n"
        "#     baseline;\n"
        "#   * steady-state frames must stay far below 16.7 ms (60 fps with margin;\n"
        "#     the `open` scenario is a cold first frame and only gets the ratio gate).\n"
        "#\n"
        "# The baseline is the Zig port while a `zig-out-baseline/bin/zant-bench`\n"
        "# exists (or `ZANT_ZIG_BIN` points at one). The Zig tree and toolchain were\n"
        "# removed at M8, so without a baseline the Rust side runs alone and every\n"
        "# scenario is compared with the Zig numbers recorded in\n"
        "# docs/rust-port/status.md (RECORDED_* below; the tables are labelled\n"
        "# \"recorded\"). `ZANT_BENCH_BASELINE=recorded` forces that mode even when a\n"
        "# baseline binary is present.\n",
    ),
    (
        "missing=0\n"
        "if [ ! -x \"$zig_bin/zant-bench\" ]; then\n"
        "    echo \"bench.sh: missing Zig baseline: ${zig_bin:-<not found>}/zant-bench\" >&2\n"
        "    echo \"  build it in the main Zig checkout: zig build -Doptimize=ReleaseFast --prefix zig-out-baseline\" >&2\n"
        "    echo \"  or point ZANT_ZIG_BIN at a directory that contains it (e.g. /path/to/XyUI/zig-out-baseline/bin)\" >&2\n"
        "    missing=1\n"
        "fi\n"
        "if [ ! -x \"$rust_bin/zant-bench\" ]; then\n"
        "    echo \"bench.sh: missing Rust binary: $rust_bin/zant-bench\" >&2\n"
        "    echo \"  build it: cargo build --release -p zant-examples --bin zant-bench\" >&2\n"
        "    missing=1\n"
        "fi\n"
        "if [ \"$missing\" = 1 ]; then\n"
        "    exit 2\n"
        "fi\n",
        "if [ ! -x \"$rust_bin/zant-bench\" ]; then\n"
        "    echo \"bench.sh: missing Rust binary: $rust_bin/zant-bench\" >&2\n"
        "    echo \"  build it: cargo build --release -p zant-examples --bin zant-bench\" >&2\n"
        "    exit 2\n"
        "fi\n"
        "\n"
        "# ---- the Zig baseline, or the numbers recorded before M8 -------------------\n"
        "# One row per scenario: name, frames, avg, p95, max (ms), copied from\n"
        "# docs/rust-port/status.md 性能基线（WP-2.11）(the last Zig measurements\n"
        "# before the Zig tree was deleted at M8).\n"
        "recorded_default() {\n"
        "    cat <<'ROWS'\n"
        "open       5 120.88 143.71 143.71\n"
        "stream   186   2.24   2.82   3.21\n"
        "typing   200   0.84   1.04   1.12\n"
        "caret    100   0.67   0.79   0.83\n"
        "hover    300   0.35   0.43   0.54\n"
        "resize   120   4.69   4.69  82.64\n"
        "scroll   300   0.75   1.01   1.29\n"
        "ROWS\n"
        "}\n"
        "recorded_list() {\n"
        "    cat <<'ROWS'\n"
        "open       5  20.98  41.48  41.48\n"
        "stream   186   0.85   1.09   4.50\n"
        "typing   200   0.69   0.90   1.06\n"
        "caret    100   0.64   0.77   0.88\n"
        "hover    300   0.35   0.39   0.55\n"
        "resize   120   0.72   0.93   3.98\n"
        "scroll   300   0.62   0.74   0.90\n"
        "ROWS\n"
        "}\n"
        "\n"
        "recorded=0\n"
        "if [ \"${ZANT_BENCH_BASELINE:-}\" = recorded ] ||\n"
        "    [ ! -x \"${zig_bin:-/nonexistent}/zant-bench\" ]; then\n"
        "    recorded=1\n"
        "    if [ \"${ZANT_BENCH_BASELINE:-}\" = recorded ]; then\n"
        "        echo \"bench.sh: ZANT_BENCH_BASELINE=recorded: comparing with the numbers recorded in docs/rust-port/status.md\" >&2\n"
        "    else\n"
        "        echo \"bench.sh: no Zig baseline at ${zig_bin:-<not found>} (the Zig tree was removed at M8);\" >&2\n"
        "        echo \"          the Rust side runs alone and is compared with the Zig numbers recorded in\" >&2\n"
        "        echo \"          docs/rust-port/status.md (set ZANT_ZIG_BIN=<dir> for a direct comparison)\" >&2\n"
        "    fi\n"
        "fi\n",
    ),
    (
        "    round=1\n"
        "    while [ \"$round\" -le \"$rounds\" ]; do\n"
        "        zig_log=\"$outdir/zig-$mode-$round.log\"\n"
        "        rust_log=\"$outdir/rust-$mode-$round.log\"\n"
        "        if [ $((round % 2)) -eq 1 ]; then\n"
        "            env \"${env_line[@]+\"${env_line[@]}\"}\" \"${env_args[@]+\"${env_args[@]}\"}\" \\\n"
        "                \"$zig_bin/zant-bench\" \"${scenarios[@]+\"${scenarios[@]}\"}\" \\\n"
        "                >/dev/null 2>\"$zig_log\" || true\n"
        "            env \"${env_line[@]+\"${env_line[@]}\"}\" \"${env_args[@]+\"${env_args[@]}\"}\" \\\n"
        "                \"$rust_bin/zant-bench\" \"${scenarios[@]+\"${scenarios[@]}\"}\" \\\n"
        "                >/dev/null 2>\"$rust_log\" || true\n"
        "        else\n"
        "            env \"${env_line[@]+\"${env_line[@]}\"}\" \"${env_args[@]+\"${env_args[@]}\"}\" \\\n"
        "                \"$rust_bin/zant-bench\" \"${scenarios[@]+\"${scenarios[@]}\"}\" \\\n"
        "                >/dev/null 2>\"$rust_log\" || true\n"
        "            env \"${env_line[@]+\"${env_line[@]}\"}\" \"${env_args[@]+\"${env_args[@]}\"}\" \\\n"
        "                \"$zig_bin/zant-bench\" \"${scenarios[@]+\"${scenarios[@]}\"}\" \\\n"
        "                >/dev/null 2>\"$zig_log\" || true\n"
        "        fi\n"
        "        round=$((round + 1))\n"
        "    done\n",
        "    if [ \"$recorded\" = 1 ]; then\n"
        "        # The comparison parses zig-<mode>-<round>.log for rounds 1..N; the\n"
        "        # recorded baseline is one round of rows and missing rounds are skipped.\n"
        "        if [ \"$mode\" = default ]; then recorded_rows=recorded_default; else recorded_rows=recorded_list; fi\n"
        "        rm -f \"$outdir\"/zig-\"$mode\"-*.log\n"
        "        {\n"
        "            \"$recorded_rows\" | while read -r name frames avg p95 max; do\n"
        "                printf '%s frames=%4d avg=%7.2f ms p95=%7.2f ms max=%7.2f ms\\n' \\\n"
        "                    \"$name\" \"$frames\" \"$avg\" \"$p95\" \"$max\"\n"
        "            done\n"
        "        } >\"$outdir/zig-$mode-1.log\"\n"
        "    fi\n"
        "    round=1\n"
        "    while [ \"$round\" -le \"$rounds\" ]; do\n"
        "        zig_log=\"$outdir/zig-$mode-$round.log\"\n"
        "        rust_log=\"$outdir/rust-$mode-$round.log\"\n"
        "        if [ $((round % 2)) -eq 1 ]; then\n"
        "            if [ \"$recorded\" = 0 ]; then\n"
        "                env \"${env_line[@]+\"${env_line[@]}\"}\" \"${env_args[@]+\"${env_args[@]}\"}\" \\\n"
        "                    \"$zig_bin/zant-bench\" \"${scenarios[@]+\"${scenarios[@]}\"}\" \\\n"
        "                    >/dev/null 2>\"$zig_log\" || true\n"
        "            fi\n"
        "            env \"${env_line[@]+\"${env_line[@]}\"}\" \"${env_args[@]+\"${env_args[@]}\"}\" \\\n"
        "                \"$rust_bin/zant-bench\" \"${scenarios[@]+\"${scenarios[@]}\"}\" \\\n"
        "                >/dev/null 2>\"$rust_log\" || true\n"
        "        else\n"
        "            env \"${env_line[@]+\"${env_line[@]}\"}\" \"${env_args[@]+\"${env_args[@]}\"}\" \\\n"
        "                \"$rust_bin/zant-bench\" \"${scenarios[@]+\"${scenarios[@]}\"}\" \\\n"
        "                >/dev/null 2>\"$rust_log\" || true\n"
        "            if [ \"$recorded\" = 0 ]; then\n"
        "                env \"${env_line[@]+\"${env_line[@]}\"}\" \"${env_args[@]+\"${env_args[@]}\"}\" \\\n"
        "                    \"$zig_bin/zant-bench\" \"${scenarios[@]+\"${scenarios[@]}\"}\" \\\n"
        "                    >/dev/null 2>\"$zig_log\" || true\n"
        "            fi\n"
        "        fi\n"
        "        round=$((round + 1))\n"
        "    done\n",
    ),
    (
        "    if ! \"$python\" - \"$outdir\" \"$mode\" \"$rounds\" \"${scenarios[@]+\"${scenarios[@]}\"}\" <<'PY'\n",
        "    if ! ZANT_BENCH_RECORDED=\"$recorded\" \"$python\" - \"$outdir\" \"$mode\" \"$rounds\" \"${scenarios[@]+\"${scenarios[@]}\"}\" <<'PY'\n",
    ),
    (
        "label = \"default (full rebuild)\" if mode == \"default\" else \"virtual list (ZANT_BENCH_LIST=1)\"\n"
        "print()\n"
        "print(f\"== zant-bench: {label}; best of {rounds} round(s) per side ==\")\n",
        "label = \"default (full rebuild)\" if mode == \"default\" else \"virtual list (ZANT_BENCH_LIST=1)\"\n"
        "baseline = \"recorded Zig baseline (docs/rust-port/status.md)\"\n"
        "if os.environ.get(\"ZANT_BENCH_RECORDED\") == \"0\":\n"
        "    baseline = \"live Zig baseline\"\n"
        "print()\n"
        "print(f\"== zant-bench: {label}; best of {rounds} round(s) per side; baseline: {baseline} ==\")\n",
    ),
])

# `scripts/tsx/examples-check.sh`: corpus moved and the Zig compile baseline is
# gone; keep the direct comparison only while the Zig sources still exist.
edit("scripts/tsx/examples-check.sh", [
    (
        "# Then it runs the compile benchmark. `crates/zant-examples`' `zant-tsx-bench`\n"
        "# compiles every `tools/tsx/testdata/*.tsx` through `zant_tsx::compile` in\n"
        "# process (10 warm-up + 100 iterations each), and this script compares it with\n"
        "# the Zig baseline `tools/tsx/compile_bench.zig` (the same corpus through\n"
        "# `tools/tsx/compile.zig` in process) and gates the average compile time at\n"
        "# \u2264 1.25\u00d7 Zig (best of `ZANT_BENCH_ROUNDS`, default 3, alternating rounds, so\n"
        "# other load on the machine does not decide the gate). The shipped Zig frame\n"
        "# benchmark binary\n"
        "# (`zig-out-baseline/bin/zant-tsx-bench`) measures frames, not compile time, so\n"
        "# it is not the baseline here (see docs/rust-port/status.md WP-5.10).\n",
        "# Then it runs the compile benchmark. `crates/zant-examples`' `zant-tsx-bench`\n"
        "# compiles every `crates/zant-tsx/testdata/tsx/*.tsx` through `zant_tsx::compile`\n"
        "# in process (10 warm-up + 100 iterations each). While the Zig tree existed\n"
        "# this compared against the Zig compiler in process\n"
        "# (`tools/tsx/compile_bench.zig`, gate \u2264 1.25\u00d7, see docs/rust-port/status.md\n"
        "# WP-5.10). The Zig toolchain and sources were removed at M8, so the script\n"
        "# now runs the Rust benchmark alone and checks that every corpus file compiled\n"
        "# (the recorded pre-M8 ratio was 1.08-1.10\u00d7).\n",
    ),
    (
        'corpus="$repo/tools/tsx/testdata"\n',
        'corpus="$repo/crates/zant-tsx/testdata/tsx"\n',
    ),
    (
        'echo "== TSX compile benchmark: zant-tsx-bench against the Zig baseline"\n',
        'echo "== TSX compile benchmark: zant-tsx-bench"\n',
    ),
    (
        'if [ "$zig_available" -eq 1 ]; then\n',
        'if [ "$zig_available" -eq 1 ] && [ -f "$repo/tools/tsx/compile_bench.zig" ]; then\n',
    ),
    (
        "else\n"
        "    echo \"   note: \\`$zig\\` not found; skipping the Zig compile-time comparison\" >&2\n"
        "fi\n",
        "else\n"
        "    echo \"   Zig toolchain/sources are gone (M8): Rust-only compile benchmark\" >&2\n"
        "    round=1\n"
        "    while [ \"$round\" -le \"$rounds\" ]; do\n"
        "        if ! \"$rust_bench\" \"$corpus\" \"$warmup\" \"$iterations\" \\\n"
        "            >\"$work/bench/rust-$round.log\" 2>&1; then\n"
        "            echo \"zant-tsx-bench failed in round $round:\" >&2\n"
        "            tail -30 \"$work/bench/rust-$round.log\" >&2 || true\n"
        "            exit 1\n"
        "        fi\n"
        "        round=$((round + 1))\n"
        "    done\n"
        "    compiled=$(grep -c 'frames=' \"$work/bench/rust-1.log\" || true)\n"
        "    if [ \"$compiled\" -lt 30 ]; then\n"
        "        echo \"zant-tsx-bench compiled only $compiled of the 32 corpus files:\" >&2\n"
        "        tail -30 \"$work/bench/rust-1.log\" >&2 || true\n"
        "        exit 1\n"
        "    fi\n"
        "    echo \"   Rust-only: $compiled corpus files compiled (recorded pre-M8 ratio 1.08-1.10x Zig)\"\n"
        "fi\n",
    ),
])

# Zig-era script references: Windows desktop scripts build with cargo now, the
# worktree helper needs no Zig cache, parity is a migration-time tool.
edit("scripts/win/components-check.ps1", [(
    'if (!$Exe) { $Exe = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "zig-out\\bin\\zant-components.exe") }\n',
    'if (!$Exe) { $Exe = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "target\\debug\\zant-components.exe") }\n',
)])
edit("scripts/win/pdf-check.ps1", [(
    'if (!$App) { $App = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "zig-out\\bin\\zant-pdf.exe") }\n',
    'if (!$App) { $App = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "target\\debug\\zant-pdf.exe") }\n',
)])
edit("scripts/win/resize-check.ps1", [(
    'if (!$Exe) { $Exe = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "zig-out\\bin\\zant-components.exe") }\n',
    'if (!$Exe) { $Exe = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "target\\debug\\zant-components.exe") }\n',
)])
edit("scripts/win/stream-resize-check.ps1", [(
    'if (!$Exe) { $Exe = (Join-Path $WorkDir "zig-out\\bin\\zant-md-stream.exe") }\n',
    'if (!$Exe) { $Exe = (Join-Path $WorkDir "target\\debug\\zant-md-stream.exe") }\n',
)])
edit("scripts/win/run-desktop.ps1", [(
    "#   powershell -File scripts/win/run-desktop.ps1 -Exe zig-out\\bin\\zant-demo.exe\n",
    "#   powershell -File scripts/win/run-desktop.ps1 -Exe target\\debug\\zant-demo.exe\n",
)])
edit("scripts/win/ui-auto.ps1", [(
    '#   $app = Start-ZantApp "<repo>\\zig-out\\bin\\zant-components.exe" -Out $out\n',
    '#   $app = Start-ZantApp "<repo>\\target\\debug\\zant-components.exe" -Out $out\n',
)])
edit("scripts/win/smoke-stillmd.ps1", [(
    '     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\\scripts\\win\\smoke-stillmd.ps1 -App <stillmd>\\zig-out\\bin\\app.exe" -Wait 120\n',
    '     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\\scripts\\win\\smoke-stillmd.ps1 -App <stillmd>\\target\\debug\\app.exe" -Wait 120\n',
)])
edit("scripts/win/scaffold-check.ps1", [
    (
        "param(\n"
        "    [string]$Zant = \"\",\n"
        "    [string]$Out = \"\",\n"
        "    # the language answer: 1 = Zig, 2 = TypeScript + TSX\n"
        "    [ValidateSet(\"1\", \"2\")][string]$Lang = \"1\"\n"
        ")\n"
        "if (!$Zant) { $Zant = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) \"zig-out\\bin\\zant.exe\") }\n",
        "param(\n"
        "    [string]$Zant = \"\",\n"
        "    [string]$Out = \"\"\n"
        ")\n"
        "if (!$Zant) { $Zant = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) \"target\\debug\\zant.exe\") }\n",
    ),
    (
        "Start-Sleep -Milliseconds 300\n"
        "Shot $h \"01-prompt\"\n"
        "# language first\n"
        "[ZantKeys]::Type($Lang); [ZantKeys]::Enter(); Start-Sleep -Milliseconds 800\n"
        "[ZantKeys]::Type($title); Start-Sleep -Milliseconds 300\n",
        "Start-Sleep -Milliseconds 300\n"
        "Shot $h \"01-prompt\"\n"
        "[ZantKeys]::Type($title); Start-Sleep -Milliseconds 300\n",
    ),
    (
        "$app = Join-Path $work \"demo-app\"\n"
        "if ($Lang -eq \"2\") {\n"
        "    $ts = (Test-Path \"$app\\src\\App.tsx\") -and !(Test-Path \"$app\\src\\main.zig\") -and (Test-Path \"$app\\zant.d.ts\")\n"
        "    Result $ts \"TS project: src\\App.tsx and zant.d.ts, no src\\main.zig\"\n"
        "} else {\n"
        "    Result ((Test-Path \"$app\\src\\main.zig\") -and !(Test-Path \"$app\\src\\App.tsx\")) \"Zig project: src\\main.zig\"\n"
        "}\n",
        "$app = Join-Path $work \"demo-app\"\n"
        "$ts = (Test-Path \"$app\\src\\App.tsx\") -and (Test-Path \"$app\\Cargo.toml\") -and (Test-Path \"$app\\zant.d.ts\") -and !(Test-Path \"$app\\src\\main.zig\")\n"
        "Result $ts \"TSX project: src\\App.tsx, Cargo.toml and zant.d.ts\"\n",
    ),
])
edit("scripts/win/sync.sh", [(
    "    tar --no-xattrs -cf \"$out\" -C \"$src\" --exclude .zig-cache --exclude zig-out --exclude .git --exclude '._*' .\n",
    "    tar --no-xattrs -cf \"$out\" -C \"$src\" --exclude target --exclude .git --exclude '._*' .\n",
)])
edit("scripts/win/remote.sh", [
    (
        "# Runs a PowerShell command on the Windows test machine inside a project\n"
        "# directory, with UTF-8 output and the pinned Zig on PATH.\n"
        "#\n"
        "#   scripts/win/remote.sh XyUI 'zig build test --summary all'\n"
        "#   scripts/win/remote.sh stillmd 'zig build'\n"
        "#\n"
        "# ZANT_WIN_HOST / ZANT_WIN_DIR as in sync.sh; ZANT_WIN_ZIG (optional) is a\n"
        "# Zig directory to put first on PATH.\n",
        "# Runs a PowerShell command on the Windows test machine inside a project\n"
        "# directory, with UTF-8 output.\n"
        "#\n"
        "#   scripts/win/remote.sh XyUI 'cargo test --workspace'\n"
        "#   scripts/win/remote.sh stillmd 'cargo build'\n"
        "#\n"
        "# ZANT_WIN_HOST / ZANT_WIN_DIR as in sync.sh; ZANT_WIN_PATH (optional) is a\n"
        "# directory to put first on PATH.\n",
    ),
    (
        "ZIG=\"${ZANT_WIN_ZIG:-}\"\n",
        "PATH_PREFIX=\"${ZANT_WIN_PATH:-}\"\n",
    ),
    (
        "ssh \"$HOST\" \"[Console]::OutputEncoding=[Text.Encoding]::UTF8; \\$OutputEncoding=[Text.Encoding]::UTF8; if ('$ZIG') { \\$env:PATH='$ZIG;'+\\$env:PATH }; Set-Location '$DIR/$project'; $*; exit \\$LASTEXITCODE\" \\\n",
        "ssh \"$HOST\" \"[Console]::OutputEncoding=[Text.Encoding]::UTF8; \\$OutputEncoding=[Text.Encoding]::UTF8; if ('$PATH_PREFIX') { \\$env:PATH='$PATH_PREFIX;'+\\$env:PATH }; Set-Location '$DIR/$project'; $*; exit \\$LASTEXITCODE\" \\\n",
    ),
])
edit("scripts/worktree.sh", [
    (
        "# The worktree gets symlinks to the shared cargo / zig caches and third_party\n"
        "# so builds do not re-download or re-extract anything.\n",
        "# The worktree gets symlinks to the shared cargo cache and third_party so\n"
        "# builds do not re-download or re-extract anything.\n",
    ),
    (
        '    ln -sfn "$root/.cargo-home" "$wt/.cargo-home"\n'
        '    ln -sfn "$root/.zig-global-cache" "$wt/.zig-global-cache"\n',
        '    ln -sfn "$root/.cargo-home" "$wt/.cargo-home"\n',
    ),
])
edit("scripts/mac/ui-scenario.swift", [(
    "//   ZANT_LAYOUT_FILE=/tmp/layout.txt ./zig-out/bin/zant-tsx-counter &\n",
    "//   ZANT_LAYOUT_FILE=/tmp/layout.txt ./target/debug/zant-tsx-counter &\n",
)])
# scripts/package/*: the scripts moved out of tools/, and their header comments
# still described the Zig output layout.
edit("scripts/package/macos.sh", [(
    "# macOS release steps run by `zant build` after it has built and signed\n"
    "# target/<title>.app from target/release/<name> (Zig: zig-out/<title>.app).\n",
    "# macOS release steps run by `zant build` after it has built and signed\n"
    "# target/<title>.app from target/release/<name>.\n",
)])
edit("scripts/package/windows.ps1", [(
    "# zant_skia.dll any more (Zig: zig-out/<title>/ with zant_skia.dll).\n",
    "# zant_skia.dll any more.\n",
)])

# ---------------------------------------------------------------------------
# 11. Docs: remove migration notes, update moved paths
# ---------------------------------------------------------------------------
print("\n-- docs")
edit("AGENTS.md", [
    (
        "这个仓库既包含框架，也包含生成项目的模板、TSX 编译器和 CLI。这里只写**最终状态**；\n"
        "迁移进度、未合入的 WP 与「迁移中」的接口见 [docs/rust-port/status.md](docs/rust-port/status.md)。\n",
        "这个仓库既包含框架，也包含生成项目的模板、TSX 编译器和 CLI。这里只写**最终状态**；\n"
        "Zig → Rust 迁移的历史记录与验证数字见 [docs/rust-port/status.md](docs/rust-port/status.md)。\n",
    ),
    (
        "\n"
        "> 迁移中：`zant run/compile/package` 对 TSX 应用的编译接口\n"
        "> `zant_build::tsx_app()`（WP-6.2）与 TSX 应用层（WP-5.6 / 5.8–5.10）尚未合入时，\n"
        "> 命令会报「接口尚未实现」；以 `docs/rust-port/status.md` 为准，不要沿用旧文档的 Zig 构建命令。\n",
        "",
    ),
    (
        "scaffold/template-ts/   生成项目的模板（Cargo.toml + build.rs + src/App.tsx + native/）\n"
        "scaffold/template/      旧纯 Zig 模板（只保留到 M8）\n"
        "scripts/                Skia 依赖、cargo 包装、parity 对照、发布打包、Windows 测试机脚本\n",
        "scaffold/template-ts/   生成项目的模板（Cargo.toml + build.rs + src/App.tsx + native/）\n"
        "scripts/                Skia 依赖、cargo 包装、parity 对照、发布打包、Windows 测试机脚本\n",
    ),
    (
        "- 测试：`cargo test --workspace` 全绿（macOS 与 Windows 都要过）；新增行为应补测试。\n"
        "  测试数量不得少于迁移前的 Zig 基线（remend 522 条夹具、mhchem 823 条夹具包含在内）。\n",
        "- 测试：`cargo test --workspace` 全绿（macOS 与 Windows 都要过）；新增行为应补测试。\n"
        "  测试数量不得少于迁移完成时的数量（含 remend 522 条、mhchem 823 条夹具；见\n"
        "  `docs/rust-port/status.md` 最终验证矩阵）。\n",
    ),
    (
        "- **性能闸门**：迁移/优化必须与 Zig 基线对照，全部示例（`zant-demo`、`zant-gallery`、\n"
        "  `zant-components`、`zant-markdown`、`zant-math`、`zant-pdf`）先跑\n"
        "  `ZANT_ZIG_BIN=… scripts/parity/compare.sh <名字>`（layout 差 ≤ 0.01、像素差 ≤ 0.1%）；\n"
        "  帧基准 `zant-bench` 各场景（open / stream / typing / caret / hover / resize / scroll，\n"
        "  另跑 `ZANT_BENCH_LIST=1`）不超过 Zig 基线 **1.25 倍**；`zant-md-stream` 流式帧率达标。\n"
        "  没有可测量收益、或过不了闸门的改动不要合入。\n",
        "- **性能闸门**：`scripts/parity/bench.sh` 跑 `zant-bench` 各场景（open / stream / typing /\n"
        "  caret / hover / resize / scroll，另跑 `ZANT_BENCH_LIST=1`），每个场景不超过记录基线\n"
        "  **1.25 倍**（基线数字记录在 `docs/rust-port/status.md`，脚本内置；旧的\n"
        "  `zig-out-baseline` 目录还在时脚本直接用它对照）；`zant-md-stream` 流式帧率达标。\n"
        "  布局 / 像素对照（`scripts/parity/compare.sh`）是迁移期工具，需要旧的 Zig 基线产物。\n"
        "  没有可测量收益、或过不了闸门的改动不要合入。\n",
    ),
    (
        "- Skia 通过 `fetch_skia.sh` / `fetch_skia_windows.ps1` 固定版本；原生代码统一由\n"
        "  `crates/zant-sys/build.rs` 编译、链接（旧脚本 `scripts/win/build_bridge.ps1` / `gen_def.sh`\n"
        "  对应已删除的 `zant_skia.dll`，只保留到 M8）。\n",
        "- Skia 通过 `fetch_skia.sh` / `fetch_skia_windows.ps1` 固定版本；原生代码统一由\n"
        "  `crates/zant-sys/build.rs` 编译、链接（源文件在 `crates/zant-sys/native/`）。\n",
    ),
    (
        "- 平台差异只放在 `crates/zant/src/platform/` 与原生源码（`src/renderer/skia/`、`src/platform/`，\n"
        "  M8 起移入 `crates/zant-sys/native/`；`skia_bridge_{mac.mm,win.cpp}`）；组件和应用代码不写\n",
        "- 平台差异只放在 `crates/zant/src/platform/` 与原生源码（`crates/zant-sys/native/skia/`、\n"
        "  `crates/zant-sys/native/platform/`；`skia_bridge_{mac.mm,win.cpp}`）；组件和应用代码不写\n",
    ),
    (
        "  远程执行，`scripts/win/run-desktop.ps1` 在可见桌面运行（SSH 会话看不到窗口）；\n"
        "  `remote.sh` / `run-desktop.ps1` 里的工具链与产物路径还在从 Zig 迁移到 Cargo（「迁移中」）。\n",
        "  远程执行，`scripts/win/run-desktop.ps1` 在可见桌面运行（SSH 会话看不到窗口）。\n",
    ),
    (
        "- 无障碍 API 未暴露。\n"
        "- TSX 编译器与应用层（`zant-tsx` CLI、`zant_build::tsx_app()`、native 模块 trait 生成、\n"
        "  TSX 示例与黄金用例）仍在迁移中，见 `docs/rust-port/status.md`。\n",
        "- 无障碍 API 未暴露。\n",
    ),
])
edit("CLAUDE.md", [
    (
        "  示例与编译器应用层（WP-5.6 / 5.8–5.10、WP-6.2）在**迁移中**，以 `docs/rust-port/status.md` 为准。\n",
        "",
    ),
    (
        "- 性能基准：`zant-bench`（打开长对话、流式、打字、光标、悬停、缩放、滚动；`ZANT_BENCH_LIST=1`\n"
        "  用虚拟列表；**迁移中**：WP-2.11）。优化前先建基线（Release），两边平台对比。\n",
        "- 性能基准：`zant-bench`（打开长对话、流式、打字、光标、悬停、缩放、滚动；`ZANT_BENCH_LIST=1`\n"
        "  用虚拟列表；`scripts/parity/bench.sh` 对照记录基线）。优化前先建基线（Release），两边平台对比。\n",
    ),
])
edit("README.md", [
    (
        "\n> **迁移中（WP-6.2）**：`zant build/run/compile/package` 调用的编译接口\n"
        "> `zant_build::tsx_app()` 与 `zant-tsx` 的完整应用层（WP-5.6 / 5.8–5.10）还在合入。\n"
        "> 本 worktree 里 `zant create`、`zant sync` 可用；TSX 应用的编译与运行以\n"
        "> [`docs/rust-port/status.md`](docs/rust-port/status.md) 的进度为准，暂时不要按旧文档的 Zig 构建命令编译。\n",
        "",
    ),
    (
        "| TSX 编译器（`.tsx` → Rust，编译期完成，无 JS 运行时）与 Rust 脚手架 `zant` | ✅（应用层接口迁移中） |\n",
        "| TSX 编译器（`.tsx` → Rust，编译期完成，无 JS 运行时）与 Rust 脚手架 `zant` | ✅ |\n",
    ),
    (
        "公式块的 UI 包装（`math::block`）随 WP-3.11 的偏差登记在 `docs/rust-port/status.md`，\n"
        "当前由 `zant-math` 示例内部实现，属于**迁移中**。\n",
        "公式块的 UI 包装（`math::block`）当前由 `zant-math` 示例内部实现（偏差记录见\n"
        "`docs/rust-port/status.md`）。\n",
    ),
    (
        "迁移状态（哪些 WP 已并入、哪些是「迁移中」）见 [`docs/rust-port/status.md`](docs/rust-port/status.md)；\n"
        "Zig 版在 M8 之前保留作行为对照，迁移完成后删除。\n",
        "Zig → Rust 迁移的历史记录与验证数字见 [`docs/rust-port/status.md`](docs/rust-port/status.md)。\n",
    ),
    (
        "- 平台层是一套 C ABI（`src/platform/platform.h`，M8 起移入 `crates/zant-sys/native/`）：macOS 用 `NSWindow` / `NSView`，Windows 用 Win32；Skia 桥接层是可移植 C++（`src/renderer/skia/`），平台部分（CoreText + Metal、DirectWrite + Direct3D 12）分文件实现。\n",
        "- 平台层是一套 C ABI（`crates/zant-sys/native/platform/platform.h`）：macOS 用 `NSWindow` / `NSView`，Windows 用 Win32；Skia 桥接层是可移植 C++（`crates/zant-sys/native/skia/`），平台部分（CoreText + Metal、DirectWrite + Direct3D 12）分文件实现。\n",
    ),
    (
        "scaffold/template-ts/       生成项目的模板（Cargo.toml + build.rs + src/App.tsx + native/）\n"
        "scaffold/template/          旧纯 Zig 模板（只保留到 M8）\n",
        "scaffold/template-ts/       生成项目的模板（Cargo.toml + build.rs + src/App.tsx + native/）\n",
    ),
])
edit("docs/scaffold.md", [
    (
        "`cargo build -p zant-cli`，再把 `target/debug/zant` 放到 PATH（沙箱里用 `./scripts/cargo.sh`）。\n"
        "Rust 版与 Zig 版并存期间，`scaffold/template/`（纯 Zig 模板）还保留作行为对照，M8 删除。\n",
        "`cargo build -p zant-cli`，再把 `target/debug/zant` 放到 PATH（沙箱里用 `./scripts/cargo.sh`）。\n",
    ),
    (
        "\n> **迁移中**：过渡期 `zant create` 仍接受 `--lang ts|zig`（默认 `ts`）；纯 Zig 模板只保留到 M8，\n"
        "> 最终只生成 TSX 项目。\n",
        "",
    ),
    (
        "\n> **迁移中（WP-6.2）**：TSX 应用的编译接口 `zant_build::tsx_app()` 合入前，`zant run` / `zant compile` /\n"
        "> `zant package` 会以「接口尚未实现」报错停止；`zant create` / `zant sync` 已可用。\n"
        "> 进度与合入顺序见 [`docs/rust-port/status.md`](rust-port/status.md)。\n",
        "",
    ),
])
edit("docs/tsx.md", [
    (
        "\n> **迁移中（WP-5.6 / 5.8–5.10、WP-6.2）**：编译器的词法 / 语法 / hooks / codegen 已落在\n"
        "> `crates/zant-tsx/`，但应用层（imports、native trait、入口、CLI）、`zant_build::tsx_app()`\n"
        "> 与仓库内的 TSX 示例（`examples/tsx/` 目前仍是 Zig 入口）还没全部合入。本文件写最终用法；\n"
        "> 未合入处以 [`docs/rust-port/status.md`](rust-port/status.md) 为准。\n",
        "",
    ),
    (
        "`bench-tsx`（Zig 版既有基准；Rust 侧仍是既有 Zig 入口）用同样的输入对比 TSX 生成代码与手写 Rust\n"
        "的帧耗时；每次改代码生成都要复测，差距超过 5% 视为回归。回归门槛：TSX 生成代码不慢于手写 Rust\n"
        "对照组 **1.2 倍**。\n"
        "\n"
        "Rust 侧的编译基准已经落地（WP-5.10）：`zant-tsx-bench`（`crates/zant-examples/src/bin/`）把\n"
        "`tools/tsx/testdata/*.tsx` 逐个在进程内编译（预热 + 迭代），对照 Zig 编译器进程内基线\n"
        "（`tools/tsx/compile_bench.zig`），由 `scripts/tsx/examples-check.sh` 跑并判 ≤1.25×；本机平均\n"
        "**1.08×**。编译耗时不是帧耗时——帧对比基准（TSX 生成代码 vs 手写 Rust）仍未移植。\n"
        "\n"
        "> **示例**：仓库内的 TSX 示例现在有 Rust 入口（WP-5.10）：`crates/zant-examples/tsx/` 下\n"
        "> `tsx-counter`（`examples/tsx/counter.tsx`）、`tsx-demo`（`examples/tsx/app/App.tsx`）、\n"
        "> `tsx-bench`（`examples/tsx/settings.tsx`，帧基准的重负载页）三个 Cargo workspace 成员，形态与\n"
        "> `scaffold/template-ts/` 相同（`build.rs` → `zant_build::tsx_app()`）；`scripts/tsx/examples-check.sh`\n"
        "> 构建、无窗口截图（`ZANT_SNAPSHOT` + `ZANT_LAYOUT_FILE`）并断言。`todos.tsx`、`gallery.tsx`、\n"
        "> `notes/`、`reader.tsx` 与帧基准的场景文件仍是 Zig 入口。\n",
        "`bench-tsx` 用同样的输入对比 TSX 生成代码与手写 Rust 的帧耗时；每次改代码生成都要复测，\n"
        "差距超过 5% 视为回归。回归门槛：TSX 生成代码不慢于手写 Rust 对照组 **1.2 倍**。\n"
        "\n"
        "编译基准：`zant-tsx-bench`（`crates/zant-examples/src/bin/`）把\n"
        "`crates/zant-tsx/testdata/tsx/*.tsx` 逐个在进程内编译（预热 + 迭代）。迁移期间它与 Zig 编译器\n"
        "进程内基线对照（≤1.25×，本机平均 **1.08×**，见 `docs/rust-port/status.md` WP-5.10）；Zig\n"
        "工具链删除后 `scripts/tsx/examples-check.sh` 只跑 Rust 侧并核对语料数量。编译耗时不是帧耗时。\n"
        "\n"
        "> **示例**：仓库内的 TSX 示例有 Rust 入口（WP-5.10）：`crates/zant-examples/tsx/` 下\n"
        "> `tsx-counter`（`crates/zant-examples/tsx/corpus/counter.tsx`）、`tsx-demo`\n"
        "> （`corpus/app/App.tsx`）、`tsx-bench`（`corpus/settings.tsx`，帧基准的重负载页）三个 Cargo\n"
        "> workspace 成员，形态与 `scaffold/template-ts/` 相同（`build.rs` → `zant_build::tsx_app()`）；\n"
        "> `scripts/tsx/examples-check.sh` 构建、无窗口截图（`ZANT_SNAPSHOT` + `ZANT_LAYOUT_FILE`）并断言。\n",
    ),
    (
        "- 迁移中：帧对比基准 `bench-tsx`（TSX 生成代码 vs 手写 Rust 的 clicks/typing/todos/list/markdown/local\n"
        "  场景，需要把 `examples/tsx/*_manual.zig` 对照组与 `bench_harness` 移植到 Rust）。逐项进度见\n"
        "  [`docs/rust-port/status.md`](rust-port/status.md)。\n",
        "- 计划中：帧对比基准 `bench-tsx`（TSX 生成代码 vs 手写 Rust 的 clicks/typing/todos/list/markdown/local\n"
        "  场景，需要把旧的 Zig 对照组与 `bench_harness` 逻辑移植到 Rust）。\n",
    ),
])
edit("docs/development.md", [
    (
        "- 迁移期间同时用 **parity 对照**验证 Rust 版与 Zig 基线行为一致：\n"
        "  `ZANT_ZIG_BIN=/path/to/zig-out-baseline/bin scripts/parity/compare.sh zant-demo`\n"
        "  （layout 每个 key 差值 ≤ 0.01；像素通道差 > 2 的像素占比 ≤ 0.1%）。\n",
        "- 布局 / 像素对照（迁移期的工具，需要旧的 Zig 基线产物）：\n"
        "  `ZANT_ZIG_BIN=/path/to/zig-out-baseline/bin scripts/parity/compare.sh zant-demo`\n"
        "  （layout 每个 key 差值 ≤ 0.01；像素通道差 > 2 的像素占比 ≤ 0.1%）。\n",
    ),
    (
        "> **迁移中（WP-2.11）**：`zant-bench` 的 Rust 版尚未合入。闸门要求（见 AGENTS.md「性能规则」）：\n"
        "> 各场景不超过 Zig 基线 **1.25 倍**；在合入前用 Zig 版 `zig-out-baseline/bin/zant-bench` 与\n"
        "> `zant-md-stream` 的帧统计做参照。\n",
        "> 闸门要求见 AGENTS.md「性能规则」：各场景不超过记录基线 **1.25 倍**\n"
        "> （`scripts/parity/bench.sh` 内置 `docs/rust-port/status.md` 记录的 Zig 基线数字；\n"
        "> 有旧的 `zig-out-baseline` 时直接用它对照）。\n",
    ),
    (
        "\n> `scripts/win/remote.sh` / `run-desktop.ps1` 里的工具链（`ZANT_WIN_ZIG`）与产物路径（`zig-out\\`）\n"
        "> 还在从 Zig 迁移到 Cargo（**迁移中**），在 Windows 脚本更新前按脚本当前写法传参。\n",
        "",
    ),
    (
        "\n> **迁移中**：`zant run/compile/package` 对 TSX 应用的编译接口（WP-6.2）与 TSX 应用层\n"
        "> （WP-5.6 / 5.8–5.10）尚未合入，当前只对已有 `Cargo.toml` 的 Rust 应用生效；\n"
        "> 进度见 [`docs/rust-port/status.md`](rust-port/status.md)。\n",
        "",
    ),
    (
        "- `crates/zant/src/platform/`：平台 ABI 的 Rust 包装；原生实现仍在 `src/platform/`（M8 移入 `zant-sys`）。\n",
        "- `crates/zant/src/platform/`：平台 ABI 的 Rust 包装；原生实现在 `crates/zant-sys/native/platform/`。\n",
    ),
    (
        "- `scripts/parity/`：Rust 与 Zig 基线的布局 / 像素对照。\n",
        "- `scripts/parity/`：布局 / 像素 / 帧基准对照（Zig 基线数字记录在 `docs/rust-port/status.md`）。\n",
    ),
])
edit("docs/architecture.md", [
    (
        "        └── src/platform/platform.h  平台 C ABI（M8 起移入 zant-sys/native），两份实现：\n",
        "        └── crates/zant-sys/native/platform/platform.h  平台 C ABI，两份实现：\n",
    ),
    (
        "macOS 13+（Apple Silicon，预编译 Skia 为 arm64）与 Windows 10/11 x64。平台差异只在 `src/platform/` 与 Skia 桥接的平台文件中；\n",
        "macOS 13+（Apple Silicon，预编译 Skia 为 arm64）与 Windows 10/11 x64。平台差异只在 `crates/zant-sys/native/platform/` 与 Skia 桥接的平台文件中；\n",
    ),
])
edit("docs/components/pdf.md", [
    (
        "\n> **迁移中**：框架侧的 `ui::pdf_viewer` / TSX 标签生成（待决问题见 `docs/rust-port/status.md`）；\n"
        "> 当前在 `crates/zant-examples/src/components_app/pdf_viewer.rs` 内实现同名组件，示例已用它通过 parity。\n",
        "",
    ),
    (
        "- 原生桥（`src/platform/pdf.h`，M8 起移入 `crates/zant-sys/native/`）：文档句柄、页面尺寸、分块渲染到内存像素、\n",
        "- 原生桥（`crates/zant-sys/native/platform/pdf.h`）：文档句柄、页面尺寸、分块渲染到内存像素、\n",
    ),
])
edit("docs/markdown.md", [(
    "\n> **迁移中**：`ui::markdown` 的 `ui::*` re-export 与 TSX 标签生成预计随组件收尾合入；\n"
    "> 当前实现位于 `crates/zant/src/components/widgets.rs`（`zant::components::widgets::markdown`），\n"
    "> 已由 `zant-markdown` / `zant-components` 示例验证。\n",
    "",
)])
edit("docs/components/markdown.md", [(
    "（TSX 的 `Markdown` 就是它；\n"
    "`ui::*` 的 re-export 迁移中，当前在 `zant::components::widgets`）。编辑器的其余 Rust 选项：\n",
    "（TSX 的 `Markdown` 就是它；\n"
    "实现位于 `zant::components::widgets`）。编辑器的其余 Rust 选项：\n",
)])
edit("docs/components/pointer-area.md", [(
    "TSX 应用暂时用 `onResize` / 内置组件组合实现（迁移中，见 `docs/rust-port/status.md`）。\n",
    "TSX 应用用 `onResize` / 内置组件组合实现。\n",
)])
edit("docs/tsx-native.md", [(
    "\n> **迁移中（WP-5.8）**：从 `.ts` 规范生成 Rust 类型与 trait 的代码生成还在合入；模板里的\n"
    "> `native/system.rs` 已经按最终形态写成 `impl`（生成物就位前先用手写类型），生成的静态调用路径\n"
    "> 不需要运行时注册表。进度见 [`docs/rust-port/status.md`](rust-port/status.md)。\n",
    "",
)])
edit("tests/fixtures/README.md", [(
    "`tests/remend_test.zig` runs them against the Zig port.\n",
    "`crates/zant/tests/remend_test.rs` runs them against the Rust port.\n",
)])
edit("THIRD_PARTY_NOTICES.md", [(
    "`crates/zant/src/math/mhchem.rs` is a Rust port of the mhchem extension for KaTeX\n"
    "(`katex/contrib/mhchem/mhchem.js`), and `src/math/mhchem_cases.json` was\n",
    "`crates/zant/src/math/mhchem/mod.rs` is a Rust port of the mhchem extension for\n"
    "KaTeX (`katex/contrib/mhchem/mhchem.js`), and\n"
    "`crates/zant/src/math/mhchem_cases.json` was\n",
)])
edit("crates/zant-examples/tsx/README.md", [
    (
        "tsx-counter/   bin zant-tsx-counter     examples/tsx/counter.tsx (1:1 port)\n"
        "tsx-demo/      bin zant-tsx-demo        examples/tsx/app/App.tsx (showcase app)\n"
        "tsx-bench/     bin zant-tsx-bench-app   examples/tsx/settings.tsx (frame-benchmark workload)\n",
        "tsx-counter/   bin zant-tsx-counter     corpus/counter.tsx (1:1 port)\n"
        "tsx-demo/      bin zant-tsx-demo        corpus/app/App.tsx (showcase app)\n"
        "tsx-bench/     bin zant-tsx-bench-app   corpus/settings.tsx (frame-benchmark workload)\n",
    ),
    (
        "then runs the TSX compile benchmark (`zant-tsx-bench`, see\n"
        "`crates/zant-examples/src/bin/zant-tsx-bench.rs`) against the Zig baseline\n"
        "(`tools/tsx/compile_bench.zig`) with a ≤1.25× gate.\n",
        "then runs the TSX compile benchmark (`zant-tsx-bench`, see\n"
        "`crates/zant-examples/src/bin/zant-tsx-bench.rs`): the Rust side only since the\n"
        "Zig toolchain was removed at M8 (the recorded pre-M8 ratio was 1.08–1.10×).\n",
    ),
])
edit("scripts/parity/README.md", [
    (
        "# Zig ↔ Rust 对照工具（WP-0.4）\n"
        "\n"
        "Zig 版是行为基准，Rust 版必须逐像素、逐坐标一致。两边使用同一套 Skia 桥，\n"
        "所以布局应当完全相同、像素几乎相同。\n",
        "# Zig ↔ Rust 对照工具（迁移期，WP-0.4）\n"
        "\n"
        "Zig 版曾是行为基准，Rust 版必须逐像素、逐坐标一致（迁移记录见\n"
        "`docs/rust-port/status.md`）。Zig 工具链在 M8 已删除；这套脚本需要旧的\n"
        "Zig 基线产物（`zig-out-baseline/bin/`，用 `ZANT_ZIG_BIN` 指向它），否则只能\n"
        "抓 Rust 一侧的快照。帧基准 `bench.sh` 在没有 Zig 基线时改用\n"
        "`docs/rust-port/status.md` 记录的基线数字（见脚本头部说明）。\n",
    ),
    (
        "- `capture.sh` 设置 `ZANT_SNAPSHOT` / `ZANT_SNAPSHOT_EXIT` / `ZANT_LAYOUT_FILE` /\n"
        "  `ZANT_NO_MOTION` / `ZANT_THEME`（`ZANT_DARK=1` 用于 Zig 示例）/ `ZANT_WINDOW_SIZE`，\n"
        "  Zig 二进制取自 `zig-out-baseline/bin/`，Rust 取自 `target/release/`。\n",
        "- `capture.sh` 设置 `ZANT_SNAPSHOT` / `ZANT_SNAPSHOT_EXIT` / `ZANT_LAYOUT_FILE` /\n"
        "  `ZANT_NO_MOTION` / `ZANT_THEME`（`ZANT_DARK=1` 用于旧的 Zig 示例）/ `ZANT_WINDOW_SIZE`，\n"
        "  Zig 二进制取自 `zig-out-baseline/bin/`（迁移期产物），Rust 取自 `target/release/`。\n",
    ),
    (
        "注意：`ZANT_WINDOW_SIZE` 是迁移期间给 Zig `platform.createWindow` 加的自动化钩子\n"
        "（`docs/rust-port/status.md` 的 Zig 变更日志有登记），只影响窗口初始尺寸。\n",
        "注意：`ZANT_WINDOW_SIZE` 是迁移期间给 Zig `platform.createWindow` 加的自动化钩子\n"
        "（Rust 版照搬；`docs/rust-port/status.md` 的 Zig 变更日志有登记），只影响窗口初始尺寸。\n",
    ),
])
edit("crates/zant-examples/README.md", [(
    "Both scripts take the Zig baseline directory from `ZANT_ZIG_BIN` (default\n"
    "`<repo>/zig-out-baseline/bin`).\n",
    "Both scripts take the Zig baseline directory from `ZANT_ZIG_BIN` (default\n"
    "`<repo>/zig-out-baseline/bin`, a migration-era artifact; the Zig toolchain was\n"
    "removed at M8, see `docs/rust-port/m8-cutover.md`).\n",
)])

# Leftover references in files whose main edit is elsewhere.
edit("crates/zant-examples/Cargo.toml", [
    (
        "# `zant-tsx-bench` (WP-5.10) drives the TSX compiler in process; the Zig frame\n"
        "# benchmark's compile-side baseline is `tools/tsx/compile_bench.zig`.\n",
        "# `zant-tsx-bench` (WP-5.10) drives the TSX compiler in process; the Zig\n"
        "# compiler baseline was removed at M8 (see docs/rust-port/status.md).\n",
    ),
    (
        "# TSX compile benchmark (port of the compile-time half of `examples/tsx/\n"
        "# bench.zig`, WP-5.10): every `tools/tsx/testdata/*.tsx` through\n"
        "# `zant_tsx::compile`, warm-up + iterations, in the Zig frame benchmark's\n"
        "# statistics format. `scripts/tsx/examples-check.sh` runs it against the Zig\n"
        "# baseline (`tools/tsx/compile_bench.zig`) and gates the ratio.\n",
        "# TSX compile benchmark (port of the compile-time half of the old Zig\n"
        "# `examples/tsx/bench.zig`, WP-5.10): every `crates/zant-tsx/testdata/tsx/*.tsx`\n"
        "# through `zant_tsx::compile`, warm-up + iterations, in the old Zig frame\n"
        "# benchmark's statistics format. `scripts/tsx/examples-check.sh` runs it\n"
        "# Rust-only since the Zig compiler was removed at M8 (recorded pre-M8 ratio\n"
        "# 1.08-1.10x, docs/rust-port/status.md WP-5.10).\n",
    ),
])
edit("crates/zant-cli/src/distribute.rs", [(
    "//! steps to the scripts in `tools/zant/package/` (macos.sh: DMG, notarization,\n",
    "//! steps to the scripts in `scripts/package/` (macos.sh: DMG, notarization,\n",
)])
edit("crates/zant-sys/native/platform/windows/zant.manifest", [(
    "<!-- Application manifest for Zant UI apps on Windows (set by\n"
    "     zant_ui.configureApp in build.zig): per-monitor DPI v2, the UTF-8\n",
    "<!-- Application manifest for Zant UI apps on Windows (embedded by\n"
    "     zant-build; formerly zant_ui.configureApp in build.zig): per-monitor DPI v2, the UTF-8\n",
)])
edit("crates/zant-sys/native/platform/macos/window.m", [
    (
        "/* Unbundled runs (`zant run`, zig-out/bin/app) have no AppIcon.icns: show\n",
        "/* Unbundled runs (`zant run`, target/debug/app) have no AppIcon.icns: show\n",
    ),
    (
        "    /* <project>/zig-out/bin/app -> <project>/assets/icons/app-macos.png */\n",
        "    /* <project>/target/debug/app -> <project>/assets/icons/app-macos.png */\n",
    ),
])
edit("crates/zant-cli/src/sources.rs", [
    (
        "/// Config files that affect the app build (Zig: app.config.json, build.zig,\n"
        "/// build.zig.zon).\n",
        "/// Config files that affect the app build (`app.config.json`, `build.rs`,\n"
        "/// `Cargo.toml`).\n",
    ),
    (
        "/// Source files under the watched directories (Zig: .zig / .tsx / .ts).\n"
        "pub const SOURCE_EXTENSIONS: [&str; 4] = [\"rs\", \"tsx\", \"ts\", \"zig\"];\n",
        "/// Source files under the watched directories (`.rs` / `.tsx` / `.ts`).\n"
        "pub const SOURCE_EXTENSIONS: [&str; 3] = [\"rs\", \"tsx\", \"ts\"];\n",
    ),
])
edit("crates/zant-examples/parity/components_parity.sh", [(
    "# Zig baseline binaries default to <repo>/zig-out-baseline/bin; Rust binaries\n"
    "# to <repo>/target/release (same as scripts/parity/capture.sh).\n",
    "# Zig baseline binaries default to <repo>/zig-out-baseline/bin (a migration-era\n"
    "# artifact; the Zig tree was removed at M8, docs/rust-port/m8-cutover.md); Rust\n"
    "# binaries to <repo>/target/release (same as scripts/parity/capture.sh).\n",
)])
edit("crates/zant-examples/parity/gallery_image_parity.sh", [
    (
        "# Zig baseline binaries default to <repo>/zig-out-baseline/bin; Rust binaries\n"
        "# to <repo>/target/release.\n",
        "# Zig baseline binaries default to <repo>/zig-out-baseline/bin (a migration-era\n"
        "# artifact; the Zig tree was removed at M8); Rust binaries to <repo>/target/release.\n",
    ),
    (
        '        echo "  zig:  zig build -Doptimize=ReleaseFast --prefix zig-out-baseline" >&2\n',
        '        echo "  zig:  point ZANT_ZIG_BIN at a saved zig-out-baseline (migration-era artifact)" >&2\n',
    ),
])

# ---------------------------------------------------------------------------
# 12. Empty directories left behind by `git rm`
# ---------------------------------------------------------------------------
ZIG_ERA_DIRS = ["src", "examples", "tools", "scaffold/template"]


def remove_empty_dirs():
    for relative in ZIG_ERA_DIRS:
        start = ROOT / relative
        if not start.is_dir():
            continue
        # `git rm` leaves the emptied directories behind; try every directory
        # bottom-up (topdown=False) and keep the ones that are not empty.
        for current, _dirs, _files in os.walk(start, topdown=False):
            try:
                Path(current).rmdir()
            except OSError:
                pass


# ---------------------------------------------------------------------------
# 13. Final structure checks (apply only)
# ---------------------------------------------------------------------------
def final_checks():
    leftovers = tracked("*.zig")
    if leftovers:
        fail(f"{len(leftovers)} tracked *.zig files remain (first: {leftovers[0]})")
    for path in ["build.zig", "build.zig.zon", "tools", "src", "examples", "scaffold/template"]:
        if (ROOT / path).exists():
            fail(f"{path} still exists after the cutover")
    for required in [
        "crates/zant-sys/native/skia/skia_bridge.cpp",
        "crates/zant-sys/native/skia/skia_bridge_win.cpp",
        "crates/zant-sys/native/platform/platform.h",
        "crates/zant-sys/native/platform/pdf.h",
        "crates/zant-sys/native/platform/macos/window.m",
        "crates/zant-sys/native/platform/windows/window.c",
        "crates/zant-sys/native/platform/windows/zant.manifest",
        "crates/zant-tsx/testdata/tsx/counter.tsx",
        "crates/zant-examples/tsx/corpus/counter.tsx",
        "crates/zant-examples/assets/zant.png",
        "crates/zant/src/math/mhchem_cases.json",
        "scaffold/template-ts/DESIGN.md",
        "scripts/package/macos.sh",
    ]:
        if not (ROOT / required).exists():
            fail(f"cutover is missing {required}")
    for stale in [
        "crates/zant-sys/native/skia/zant_skia.def",
        "crates/zant-sys/native/skia/zant_skia_resources.h",
        "crates/zant-sys/native/platform/windows/winsqlite3.def",
    ]:
        if (ROOT / stale).exists():
            fail(f"{stale} should have been deleted")


print()
if APPLY:
    remove_empty_dirs()
    final_checks()

print(
    f"cutover plan: {MOVED} moves, {DELETED} deletions, {EDITED} edits, "
    f"{NEW_FILES} rewritten file(s)"
)
if ERRORS:
    print(f"\n{len(ERRORS)} problem(s):", file=sys.stderr)
    for error in ERRORS:
        print(f"  - {error}", file=sys.stderr)
    sys.exit(1)
if APPLY:
    print("cutover applied")
else:
    print("dry run: nothing was changed")
PY

if [ "$APPLY" = 1 ]; then
    log "applying the cutover"
    "$PYTHON" "$PY_FILE" apply "$ROOT"
    ok "native sources moved, Zig removed and references updated"
    # The text replacements can push Rust lines over rustfmt's width; the
    # pre-M8 tree is already formatted, so formatting here only touches the
    # lines the cutover rewrote.
    log "formatting (cargo fmt --all)"
    "$CARGO" fmt --all || die "cargo fmt --all failed (is the rustfmt component installed?)"
    ok "cargo fmt --all"
    if [ "$SKIP_GATES" = 0 ]; then
        verify
    else
        printf '  \033[33m!\033[0m --skip-gates: skipping the post-M8 verification (run --verify-only afterwards)\n'
    fi
    log "done"
    printf '  Roll back with: git reset --hard rust-migration-pre-m8 && git clean -fd\n'
    printf '  Merge to main and tag only after the user approves the release (docs/rust-port/m8-cutover.md).\n'
else
    log "planned cutover"
    "$PYTHON" "$PY_FILE" dry-run "$ROOT"
    ok "dry run finished: nothing was changed"
    printf '\n  Apply with: scripts/migrate/cutover-m8.sh --apply\n'
fi
