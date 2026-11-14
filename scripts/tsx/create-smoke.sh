#!/bin/sh
# Build a fresh Rust/TSX application from `zant create` against this checkout.
# Never starts a GUI (headless `cargo build` + `cargo test` + a compile error).
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cargo_home=${CARGO_HOME:-$repo/.cargo-home}
zant="$repo/target/debug/zant"
if [ ! -x "$zant" ]; then
    ( cd "$repo" && CARGO_HOME="$cargo_home" cargo build -p zant-cli )
fi
work=$(mktemp -d "${TMPDIR:-/tmp}/zant-tsx-create.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM
cd "$work"
"$zant" create demo --lang ts --yes
cd demo
for f in Cargo.toml build.rs src/main.rs src/App.tsx rust-toolchain.toml \
    zant.d.ts tsconfig.json app.config.json native/lib.rs native/system.ts \
    native/system.rs assets/icons/app-macos.png; do
    test -f "$f" || { echo "missing $f" >&2; exit 1; }
done
test ! -f src/main.zig || { echo "the Zig entry must not exist" >&2; exit 1; }
test ! -f native/system.zig || { echo "the Zig native module must not exist" >&2; exit 1; }
grep -q 'crates/zant' Cargo.toml || { echo "Cargo.toml lacks the zant path dependency" >&2; exit 1; }
# the CLI generates src/app_config.rs from app.config.json
test -f src/app_config.rs || { echo "zant create must generate src/app_config.rs" >&2; exit 1; }
"$zant" sync >/dev/null
test -f src/app_config.rs
CARGO_HOME="$cargo_home" cargo build
if [ -x target/debug/demo ] || [ -x target/debug/demo.exe ]; then :; else
    echo "missing application executable" >&2; exit 1
fi
CARGO_HOME="$cargo_home" cargo test
# a broken TSX must surface the original location before any GUI starts
printf '\nconst broken = ;\n' >> src/App.tsx
if "$zant" compile > errors.log 2>&1; then
    echo "invalid TSX unexpectedly compiled" >&2; cat errors.log >&2; exit 1
fi
if ! LC_ALL=C grep -Eq 'App\.tsx:[0-9]+:[0-9]+: error:' errors.log; then
    cat errors.log >&2; exit 1
fi
echo 'TSX create/sync/compile/cargo build/cargo test: OK (GUI not started)'
