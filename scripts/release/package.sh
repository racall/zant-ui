#!/bin/bash
# Builds the macOS release archive that install.sh downloads:
#
#   scripts/release/package.sh v0.1.0 [out-dir]
#   ZANT_DRY_RUN=1 scripts/release/package.sh dev dist    # list what it would package
#
# Run from a clean checkout after scripts/fetch_skia.sh, with Rust on PATH
# (or $CARGO): rustup installs the toolchain pinned in rust-toolchain.toml, so
# the build machine needs no Zig any more.
# Produces <out>/zant-macos-arm64.tar.gz and its .sha256:
#   bin/zant                  the CLI (finds zant-ui/ next to bin/)
#   zant-ui/                  the committed Rust workspace (git archive): Cargo.toml,
#                             Cargo.lock, rust-toolchain.toml, crates/ and the native
#                             sources it compiles (src/renderer/skia, src/platform)
#   zant-ui/third_party/      the pinned Skia libraries and headers
#   zant-ui/.zant-version     (not VERSION: the root is an include path, and
#                             macOS would find it for C++ <version>)
set -euo pipefail

VERSION="${1:?usage: package.sh <version> [out-dir]}"
OUT="${2:-dist}"
CARGO="${CARGO:-cargo}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NAME="zant-macos-arm64.tar.gz"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
cd "$ROOT"

die() {
  echo "$*" >&2
  exit 1
}

[ -f third_party/skia/skia-binaries/libskia.a ] ||
  die "run scripts/fetch_skia.sh first"
[ -f third_party/skia-headers/include/core/SkSurface.h ] ||
  die "run scripts/fetch_skia.sh first"

# Offline self-test: check the inputs the archive is built from and list what
# would go into it, without compiling or writing anything. Used by CI (and by
# hand) to catch a release layout that drifted from the Rust workspace.
if [ "${ZANT_DRY_RUN:-0}" = 1 ]; then
  tracked="$(git ls-tree -r --name-only HEAD)"
  total="$(printf '%s\n' "$tracked" | wc -l | tr -d ' ')"
  crates="$(printf '%s\n' "$tracked" | grep -c '^crates/' || true)"
  top="$(git ls-tree --name-only HEAD | tr '\n' ' ')"
  skia_files="$(find third_party/skia -type f | wc -l | tr -d ' ')"
  headers="$(find third_party/skia-headers -type f ! -path '*/.git/*' | wc -l | tr -d ' ')"
  skia_libs="$(cd third_party/skia/skia-binaries && ls ./*.a | sed 's|^./||' | tr '\n' ' ')"
  channel="$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml | head -1)"
  # The archive must contain the Rust workspace and the native sources /
  # headers crates/zant-sys compiles. A file that is not committed would be
  # silently missing from `git archive`, so fail instead.
  for want in Cargo.toml Cargo.lock rust-toolchain.toml \
    crates/zant/Cargo.toml crates/zant-sys/Cargo.toml crates/zant-cli/Cargo.toml \
    src/platform/platform.h src/renderer/skia/skia_bridge.h \
    src/renderer/skia/skia_bridge.cpp scripts/fetch_skia.sh; do
    printf '%s\n' "$tracked" | grep -Fqx "$want" ||
      die "the archive would miss $want (is it committed?)"
  done
  grep -Eq '^[[:space:]]*name[[:space:]]*=[[:space:]]*"zant"' crates/zant-cli/Cargo.toml ||
    die "crates/zant-cli/Cargo.toml has no [[bin]] name = \"zant\""
  if command -v "$CARGO" >/dev/null 2>&1; then
    cargo_state="$(command -v "$CARGO")"
  else
    cargo_state="NOT FOUND — set \$CARGO (e.g. CARGO=./scripts/cargo.sh)"
  fi
  if [ -x "$TARGET_DIR/release/zant" ]; then
    bin_state="already built ($(du -h "$TARGET_DIR/release/zant" | cut -f1))"
  else
    bin_state="not built yet: the real run starts with \`$CARGO build --release --locked -p zant-cli --bin zant\`"
  fi
  echo "dry run: $VERSION -> $OUT/$NAME"
  echo "  cargo                          $cargo_state"
  echo "  rust-toolchain.toml channel    ${channel:-<none>}"
  echo "  bin/zant                       target/release/zant — $bin_state"
  echo "  zant-ui/                       git archive HEAD: $total files ($top)"
  echo "  zant-ui/crates/                $crates files"
  echo "  zant-ui/third_party/skia/      $skia_files files: $skia_libs"
  echo "  zant-ui/third_party/skia-headers/  $headers files (.git excluded)"
  echo "  zant-ui/.zant-version          $VERSION"
  echo "  $OUT/$NAME.sha256              sha256 of the archive"
  echo "  (nothing was built or written)"
  exit 0
fi

# Rust targets the baseline of the architecture by default, so the machine's
# own CPU features would not run everywhere; Zig needed -Dcpu=baseline here.
# --locked keeps the release on the committed Cargo.lock.
"$CARGO" build --release --locked -p zant-cli --bin zant

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/bin" "$stage/zant-ui/third_party"
cp "$TARGET_DIR/release/zant" "$stage/bin/zant"
git archive --format=tar HEAD | tar -x -C "$stage/zant-ui"
cp -R third_party/skia "$stage/zant-ui/third_party/skia"
rsync -a --exclude .git third_party/skia-headers/ "$stage/zant-ui/third_party/skia-headers/"
echo "$VERSION" >"$stage/zant-ui/.zant-version"

mkdir -p "$OUT"
tar -czf "$OUT/$NAME" -C "$stage" bin zant-ui
shasum -a 256 "$OUT/$NAME" | awk '{print $1}' >"$OUT/$NAME.sha256"
echo "$OUT/$NAME ($(du -h "$OUT/$NAME" | cut -f1))"
