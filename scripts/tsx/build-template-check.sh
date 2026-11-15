#!/bin/sh
# End-to-end check of the `scaffold/template-ts` application with
# `zant_build::tsx_app()`: copy the template to a temp directory,
# create the project with `zant create`, build it and start the binary headless with `ZANT_SNAPSHOT`.
# Never opens a visible window.
#
# The template is the project `zant create` writes; it exercises the whole
# stack (TSX codegen, native modules, the app layer); the failure output says
# which step failed.
#
# Usage: scripts/tsx/build-template-check.sh [--release]

set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cargo=${ZANT_CARGO:-cargo}
cargo_home="$repo/.cargo-home"
target="$repo/target/tsx-template-check"
release=""
if [ "${1:-}" = "--release" ]; then
    release="--release"
fi

template="$repo/scaffold/template-ts"
work=$(mktemp -d "${TMPDIR:-/tmp}/zant-template-check.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM

# Build the CLI, then exercise the same project creation path as users.
cd "$repo"
CARGO_HOME="$cargo_home" CARGO_TARGET_DIR="$target" "$cargo" build -p zant-cli
cd "$work"
CARGO_HOME="$cargo_home" "$target/debug/zant" create demo --yes --title "Zant Template Check" --bundle-id dev.zant.template-check
app="$work/demo"
ls -R "$app"
test ! -e "$app/build.rs"
test ! -e "$app/native/lib.rs"
test ! -e "$app/native/spec.rs"
test -z "$(find "$app/src" -name '*.rs' -print)"
cd "$app"
# `build.rs` → `zant_build::tsx_app()` compiles src/**/*.tsx and native/*.ts.
CARGO_HOME="$cargo_home" CARGO_TARGET_DIR="$target" "$cargo" build $release

profile=debug
if [ -n "$release" ]; then
    profile=release
fi
binary="$target/$profile/demo"
if [ ! -x "$binary" ]; then
    echo "missing binary: $binary" >&2
    exit 1
fi

shot="$work/demo.png"
ZANT_SNAPSHOT="$shot" ZANT_SNAPSHOT_EXIT=1 ZANT_NO_MOTION=1 "$binary"
if [ ! -s "$shot" ]; then
    echo "the app did not write a snapshot to $shot" >&2
    exit 1
fi
signature=$(od -An -tx1 -N8 "$shot" | tr -d ' \n')
if [ "$signature" != "89504e470d0a1a0a" ]; then
    echo "$shot is not a PNG (signature $signature)" >&2
    exit 1
fi

# Compile the generated hot entry as a library too.
mkdir -p "$app/native/internal"
printf '%s\n' 'pub mod internal;' > "$app/native/helper.rs"
printf '%s\n' 'pub const VALUE: u32 = 1;' > "$app/native/internal/mod.rs"
printf '%s\n' 'zant::hot_app!();'  > "$work/hot.rs"
cat >> "$app/Cargo.toml" <<EOF

[lib]
path = "$work/hot.rs"
crate-type = ["cdylib"]
EOF
CARGO_HOME="$cargo_home" CARGO_TARGET_DIR="$target" "$cargo" check --lib $release

echo "scaffold/template-ts end-to-end: OK ($profile build)"
