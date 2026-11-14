#!/bin/sh
# End-to-end check of the `scaffold/template-ts` application with
# `zant_build::tsx_app()` (WP-6.2): copy the template to a temp directory,
# rewrite its path placeholders to this checkout, generate `src/app_config.rs`
# with `zant sync`, build it and start the binary headless with `ZANT_SNAPSHOT`.
# Never opens a visible window.
#
# The template is the project `zant create` writes; it exercises the whole
# stack (TSX codegen, native modules, the app layer), so this script can fail on
# work packages other than 6.2 while the migration is in progress — the failure
# output says which step failed.
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

app="$work/demo"
mkdir -p "$app"
cp -R "$template/." "$app/"

# The template ships with placeholders (`zant create` substitutes them).
sed -i.bak \
    -e "s|__ZANT_PATH__|$repo|g" \
    -e 's|__APP_NAME_SNAKE__|zant_template_check|g' \
    -e 's|__APP_TITLE__|Zant Template Check|g' \
    -e 's|__BUNDLE_ID__|dev.zant.template-check|g' \
    "$app/Cargo.toml" "$app/app.config.json" "$app/src/App.tsx"
find "$app" -name '*.bak' -delete

# `zant sync` writes `src/app_config.rs` from `app.config.json` (and macOS
# bundle files); the Rust entry includes it.
cd "$repo"
CARGO_HOME="$cargo_home" CARGO_TARGET_DIR="$target" "$cargo" build -p zant-cli
cd "$app"
CARGO_HOME="$cargo_home" "$target/debug/zant" sync

# `build.rs` → `zant_build::tsx_app()` compiles src/**/*.tsx and native/*.ts.
CARGO_HOME="$cargo_home" CARGO_TARGET_DIR="$target" "$cargo" build $release

profile=debug
if [ -n "$release" ]; then
    profile=release
fi
binary="$target/$profile/zant_template_check"
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

echo "scaffold/template-ts end-to-end: OK ($profile build)"
