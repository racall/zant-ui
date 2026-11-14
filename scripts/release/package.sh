#!/bin/bash
# Builds the macOS release archive that install.sh downloads:
#
#   scripts/release/package.sh v0.1.0 [out-dir]
#   ZANT_DRY_RUN=1 scripts/release/package.sh dev dist    # list what it would package
#
# Run from a clean checkout after scripts/fetch_skia.sh, with Rust on PATH
# (or $CARGO): rustup installs the toolchain pinned in rust-toolchain.toml.
# Apps in the archive link the
# prebuilt native layer instead of compiling C++/ObjC, so this needs
# scripts/release/prebuilt.sh to have run (or $ZANT_NATIVE_PREBUILT set to a
# directory made by it); `ZANT_ALLOW_NO_PREBUILT=1` packages without it.
# Produces <out>/zant-macos-arm64.tar.gz and its .sha256:
#   bin/zant                  the CLI (finds zant-ui/ next to bin/)
#   zant-ui/                  the committed Rust workspace (git archive): Cargo.toml,
#                             Cargo.lock, rust-toolchain.toml, crates/ and the native
#                             sources it compiles (crates/zant-sys/native/skia and
#                             crates/zant-sys/native/platform)
#   zant-ui/third_party/      the pinned Skia libraries and headers
#   zant-ui/third_party/zant-native-prebuilt/
#                             libzant_native.a + metadata.txt: app builds
#                             link this instead of compiling the native layer;
#                             build.rs falls back to compiling if it is stale
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

# The prebuilt native library the archive ships. $ZANT_NATIVE_PREBUILT
# is the same variable build.rs reads, so setting it here also makes the
# packaging build skip the C++ compile. Otherwise use the directory
# scripts/release/prebuilt.sh wrote by default.
prebuilt=""
for candidate in "${ZANT_NATIVE_PREBUILT:-}" "$TARGET_DIR/prebuilt/macos"; do
  [ -n "$candidate" ] || continue
  if [ -f "$candidate/libzant_native.a" ]; then
    prebuilt="$candidate"
    break
  fi
done
if [ -z "$prebuilt" ]; then
  if [ "${ZANT_ALLOW_NO_PREBUILT:-0}" = 1 ]; then
    echo "warning: no prebuilt native library: the archive will need a native C++/ObjC compile" >&2
  else
    die "no prebuilt native library: run scripts/release/prebuilt.sh first (or set
  ZANT_NATIVE_PREBUILT=<dir>, or ZANT_ALLOW_NO_PREBUILT=1 to ship without it)"
  fi
fi
if [ -n "$prebuilt" ]; then
  [ -f "$prebuilt/metadata.txt" ] ||
    die "$prebuilt/metadata.txt is missing (not a directory made by scripts/release/prebuilt.sh)"
  grep -qx 'platform=macos' "$prebuilt/metadata.txt" ||
    die "$prebuilt/metadata.txt is not a macOS archive"
  grep -q '^source_hash=.\+' "$prebuilt/metadata.txt" ||
    die "$prebuilt/metadata.txt has no source_hash"
  # build.rs reads this variable too: the packaging build below links it.
  export ZANT_NATIVE_PREBUILT="$prebuilt"
fi

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
    crates/zant-sys/native/platform/platform.h \
    crates/zant-sys/native/skia/skia_bridge.h \
    crates/zant-sys/native/skia/skia_bridge.cpp scripts/fetch_skia.sh \
    crates/zant-sys/prebuilt/sources-macos.txt crates/zant-sys/prebuilt.rs; do
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
  if [ -n "$prebuilt" ]; then
    prebuilt_state="$prebuilt: $(du -h "$prebuilt/libzant_native.a" | cut -f1), $(sed -n 's/^source_hash=//p' "$prebuilt/metadata.txt")"
  else
    prebuilt_state="NONE (ZANT_ALLOW_NO_PREBUILT=1)"
  fi
  echo "dry run: $VERSION -> $OUT/$NAME"
  echo "  cargo                          $cargo_state"
  echo "  rust-toolchain.toml channel    ${channel:-<none>}"
  echo "  bin/zant                       target/release/zant — $bin_state"
  echo "  zant-ui/                       git archive HEAD: $total files ($top)"
  echo "  zant-ui/crates/                $crates files"
  echo "  zant-ui/third_party/skia/      $skia_files files: $skia_libs"
  echo "  zant-ui/third_party/skia-headers/  $headers files (.git excluded)"
  echo "  zant-ui/third_party/zant-native-prebuilt/  $prebuilt_state"
  echo "  zant-ui/.zant-version          $VERSION"
  echo "  $OUT/$NAME.sha256              sha256 of the archive"
  echo "  (nothing was built or written)"
  exit 0
fi

# Rust targets the baseline of the architecture by default, so the binary does
# not depend on the build machine's own CPU features.
# --locked keeps the release on the committed Cargo.lock.
"$CARGO" build --release --locked -p zant-cli --bin zant

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/bin" "$stage/zant-ui/third_party"
cp "$TARGET_DIR/release/zant" "$stage/bin/zant"
git archive --format=tar HEAD | tar -x -C "$stage/zant-ui"
cp -R third_party/skia "$stage/zant-ui/third_party/skia"
rsync -a --exclude .git third_party/skia-headers/ "$stage/zant-ui/third_party/skia-headers/"
if [ -n "$prebuilt" ]; then
  mkdir -p "$stage/zant-ui/third_party/zant-native-prebuilt"
  cp "$prebuilt/libzant_native.a" "$prebuilt/metadata.txt" \
    "$stage/zant-ui/third_party/zant-native-prebuilt/"
fi
echo "$VERSION" >"$stage/zant-ui/.zant-version"

mkdir -p "$OUT"
tar -czf "$OUT/$NAME" -C "$stage" bin zant-ui
shasum -a 256 "$OUT/$NAME" | awk '{print $1}' >"$OUT/$NAME.sha256"
echo "$OUT/$NAME ($(du -h "$OUT/$NAME" | cut -f1))"
