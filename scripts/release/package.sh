#!/bin/bash
# Builds the macOS release archive that install.sh downloads:
#
#   scripts/release/package.sh v0.1.0 [out-dir]
#
# Run from a clean checkout after scripts/fetch_skia.sh, with Zig on PATH
# (or $ZIG). Produces <out>/zant-macos-arm64.tar.gz and its .sha256:
#   bin/zant                 the CLI (finds zant-ui/ next to bin/)
#   zant-ui/                 the committed sources (git archive), plus
#   zant-ui/third_party/     the pinned Skia libraries and headers
#   zant-ui/.zant-version    (not VERSION: the root is an include path, and
#                            macOS would find it for C++ <version>)
set -euo pipefail

VERSION="${1:?usage: package.sh <version> [out-dir]}"
OUT="${2:-dist}"
ZIG="${ZIG:-zig}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NAME="zant-macos-arm64.tar.gz"
cd "$ROOT"

[ -f third_party/skia/skia-binaries/libskia.a ] || {
  echo "run scripts/fetch_skia.sh first" >&2
  exit 1
}
"$ZIG" build cli -Doptimize=ReleaseSafe

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/bin" "$stage/zant-ui/third_party"
cp zig-out/bin/zant "$stage/bin/zant"
git archive --format=tar HEAD | tar -x -C "$stage/zant-ui"
cp -R third_party/skia "$stage/zant-ui/third_party/skia"
rsync -a --exclude .git third_party/skia-headers/ "$stage/zant-ui/third_party/skia-headers/"
echo "$VERSION" >"$stage/zant-ui/.zant-version"

mkdir -p "$OUT"
tar -czf "$OUT/$NAME" -C "$stage" bin zant-ui
shasum -a 256 "$OUT/$NAME" | awk '{print $1}' >"$OUT/$NAME.sha256"
echo "$OUT/$NAME ($(du -h "$OUT/$NAME" | cut -f1))"
