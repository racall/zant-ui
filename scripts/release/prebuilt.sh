#!/bin/bash
# Builds the prebuilt native library that released apps link instead of
# compiling C++/ObjC (WP-6.6, docs/rust-port/plan.md §5). Same idea as the Zig
# build's third_party/zant-skia-prebuilt: the release workflow compiles the
# native layer once per platform and ships the archive inside
# zant-ui/third_party/zant-native-prebuilt/.
#
#   scripts/release/prebuilt.sh [out-dir]
#
# Writes <out-dir>/libzant_native.a + metadata.txt (default
# target/prebuilt/macos; the out dir may be relative to the repo root).
# scripts/release/package.sh copies it into the release archive.
#
# Needs the pinned Skia (scripts/fetch_skia.sh) and the Xcode command line
# tools. Rust as usual: $CARGO, e.g. CARGO=./scripts/cargo.sh in the agent
# sandbox (where ~/.cargo is not writable).
#
# The archive itself is produced by crates/zant-sys/build.rs when
# ZANT_NATIVE_PREBUILT_OUT is set, so the source hash in the metadata is
# computed by the same code that verifies it on the user's machine.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT_ARG="${1:-target/prebuilt/macos}"
case "$OUT_ARG" in
/*) OUT="$OUT_ARG" ;;
*) OUT="$ROOT/$OUT_ARG" ;;
esac
CARGO="${CARGO:-cargo}"
case "$CARGO" in
*/*) CARGO="$(cd "$(dirname "$CARGO")" && pwd)/$(basename "$CARGO")" ;;
esac
cd "$ROOT"

die() {
  echo "prebuilt.sh: $*" >&2
  exit 1
}

[ "$(uname -s)" = "Darwin" ] ||
  die "this script builds the macOS archive; on Windows use scripts/release/prebuilt.ps1"
[ "$(uname -m)" = "arm64" ] ||
  die "Zant UI's macOS build is arm64 only (the pinned Skia is arm64)"
[ -f third_party/skia/skia-binaries/libskia.a ] ||
  die "Skia is missing; run ./scripts/fetch_skia.sh first"
[ -f third_party/skia-headers/include/core/SkSurface.h ] ||
  die "Skia headers are missing; run ./scripts/fetch_skia.sh first"

# The Rust aarch64-apple-darwin target links with a minimum of macOS 11.0;
# clang would otherwise record the build machine's macOS version in the
# objects, and an app built on an older machine can refuse objects "built for
# a newer macOS". The release package requires macOS 13+, so pinning the floor
# is safe and keeps the archive usable on every supported Mac.
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

# Build in release: the archive is linked into release *and* debug app builds,
# and the C++ side should not be unoptimised in either.
CARGO_FLAGS=(build --release --locked -p zant-sys)
if ! "$CARGO" --version >/dev/null 2>&1; then
  die "cargo not found; set \$CARGO (the agent sandbox uses CARGO=./scripts/cargo.sh)"
fi

commit="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
toolchain="$(clang --version 2>/dev/null | head -1 || echo unknown)"
toolchain="$toolchain; MACOSX_DEPLOYMENT_TARGET=$MACOSX_DEPLOYMENT_TARGET"

rm -rf "$OUT"
mkdir -p "$OUT"

echo "prebuilt.sh: building the native layer (${CARGO_FLAGS[*]})"
ZANT_NATIVE_PREBUILT_OUT="$OUT" \
  ZANT_NATIVE_PREBUILT_COMMIT="$commit" \
  ZANT_NATIVE_PREBUILT_TOOLCHAIN="$toolchain" \
  "$CARGO" "${CARGO_FLAGS[@]}" ||
  die "cargo ${CARGO_FLAGS[*]} failed"

# MSVC's create_archive also leaves libzant_native.a next to the .lib on
# Windows; on macOS only the .a exists. Keep exactly one archive.
[ -f "$OUT/libzant_native.a" ] || die "build.rs did not write $OUT/libzant_native.a"
[ -f "$OUT/metadata.txt" ] || die "build.rs did not write $OUT/metadata.txt"

echo
echo "prebuilt native library: $OUT"
ls -l "$OUT"
echo
source_hash="$(sed -n 's/^source_hash=//p' "$OUT/metadata.txt")"
echo "source_hash: $source_hash"
echo "commit:      $commit"
echo "toolchain:   $toolchain"
echo
echo "next: ./scripts/release/package.sh <version>  (or point build.rs at it with"
echo "      ZANT_NATIVE_PREBUILT=$OUT for a local app build)"
