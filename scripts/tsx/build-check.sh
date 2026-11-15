#!/bin/sh
# End-to-end check of `zant_build::tsx_app()` on this machine.
#
# Copies the fixture app (`crates/zant-build/tests/fixtures/tsx-app`, the
# same layout as `scaffold/template-ts`) into a temp directory, writes the Cargo
# package around it (the fixture is not a package of the workspace), builds it —
# which runs `build.rs` → `zant_build::tsx_app()` → TSX → `$OUT_DIR` — and then
# starts the binary headless with `ZANT_SNAPSHOT` to prove the generated app
# runs. Never opens a visible window.
#
# Usage: scripts/tsx/build-check.sh [--release]

set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cargo=${ZANT_CARGO:-cargo}
cargo_home="$repo/.cargo-home"
target="$repo/target/tsx-build-check"
release=""
if [ "${1:-}" = "--release" ]; then
    release="--release"
fi

fixture="$repo/crates/zant-build/tests/fixtures/tsx-app"
work=$(mktemp -d "${TMPDIR:-/tmp}/zant-build-check.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM

mkdir -p "$work/app"
cp -R "$fixture/." "$work/app/"

cat >"$work/app/Cargo.toml" <<EOF
[package]
name = "zant-build-fixture"
version = "0.1.0"
edition = "2024"
rust-version = "1.94"
publish = false
build = "native/build.rs"

[[bin]]
name = "zant-build-fixture"
path = "native/main.rs"

[dependencies]
zant = { path = "$repo/crates/zant" }

[build-dependencies]
zant-build = { path = "$repo/crates/zant-build" }

[profile.release]
overflow-checks = true

[workspace]
EOF

cat >"$work/app/native/build.rs" <<'EOF'
fn main() {
    zant_build::tsx_app();
}
EOF

cd "$work/app"
CARGO_HOME="$cargo_home" CARGO_TARGET_DIR="$target" "$cargo" build $release

profile=debug
if [ -n "$release" ]; then
    profile=release
fi
binary="$target/$profile/zant-build-fixture"
if [ ! -x "$binary" ]; then
    echo "missing binary: $binary" >&2
    exit 1
fi

shot="$work/shot.png"
ZANT_SNAPSHOT="$shot" ZANT_SNAPSHOT_EXIT=1 ZANT_NO_MOTION=1 "$binary"
if [ ! -s "$shot" ]; then
    echo "the app did not write a snapshot to $shot" >&2
    exit 1
fi
# PNG signature: the snapshot is a real rendering, not an empty file.
signature=$(od -An -tx1 -N8 "$shot" | tr -d ' \n')
if [ "$signature" != "89504e470d0a1a0a" ]; then
    echo "$shot is not a PNG (signature $signature)" >&2
    exit 1
fi

echo "tsx_app() end-to-end: OK ($profile build, snapshot $shot)"
