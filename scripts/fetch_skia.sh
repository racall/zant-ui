#!/usr/bin/env bash
set -euo pipefail

# Fetches the exact prebuilt Skia archives and matching public headers used by
# Zant UI. Safe to re-run.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
mkdir -p third_party

RELEASE="0.157.0"
ASSET="skia-binaries-30669978c3f39a73e8a7-aarch64-apple-darwin-ganesh-gl-jpegd-jpege-metal-pdf-textlayout.tar.gz"
URL="https://github.com/rust-skia/skia-binaries/releases/download/${RELEASE}/${ASSET}"
SHA256="b783a68efe4fdeb1cf5e012aabad14def14f64ea6b75234bd36ecbf1a5e58399"
HEADERS_REF="m157.2"
HEADERS_COMMIT="6a8fe80eadfac737c12e64229a1b9530bc78f3d1"

# third_party/skia/.zant-release records the archive in place, so a newer
# pin (after `zant update` pulls) downloads again. A tree from before the
# stamp existed is taken as the current pin.
STAMP="third_party/skia/.zant-release"
if [[ -f "third_party/skia/skia-binaries/libskia.a" && ! -f "$STAMP" ]]; then
  echo "${RELEASE} ${SHA256}" >"$STAMP"
fi
if [[ -f "$STAMP" && "$(cat "$STAMP")" != "${RELEASE} ${SHA256}" ]]; then
  echo "Skia pin changed: replacing third_party/skia"
  rm -rf third_party/skia
fi

if [[ ! -f "third_party/skia/skia-binaries/libskia.a" ]]; then
  echo "Downloading Skia ${RELEASE} (macOS arm64)..."
  curl -L --fail --retry 3 --max-time 600 "$URL" -o "third_party/${ASSET}"
  ACTUAL="$(shasum -a 256 "third_party/${ASSET}" | awk '{print $1}')"
  if [[ "$ACTUAL" != "$SHA256" ]]; then
    echo "Skia archive checksum mismatch: $ACTUAL" >&2
    exit 1
  fi
  mkdir -p third_party/skia
  tar -xzf "third_party/${ASSET}" -C third_party/skia
  rm -f "third_party/${ASSET}"
  echo "${RELEASE} ${SHA256}" >"$STAMP"
else
  echo "Skia libraries already present."
fi

if [[ -d third_party/skia-headers/.git && "$(git -C third_party/skia-headers rev-parse HEAD 2>/dev/null)" != "$HEADERS_COMMIT" ]]; then
  echo "Skia headers pin changed: fetching again"
  rm -rf third_party/skia-headers
fi
if [[ ! -f "third_party/skia-headers/include/core/SkSurface.h" ]]; then
  echo "Fetching Skia ${HEADERS_REF} public headers..."
  rm -rf third_party/skia-headers
  git clone --depth 1 --branch "$HEADERS_REF" --filter=blob:none --sparse \
    https://github.com/rust-skia/skia.git third_party/skia-headers
  (
    cd third_party/skia-headers
    git sparse-checkout set \
      include \
      modules/skparagraph/include \
      modules/skshaper/include \
      modules/skunicode/include \
      modules/skcms \
      src/core \
      src/utils
    ACTUAL_COMMIT="$(git rev-parse HEAD)"
    if [[ "$ACTUAL_COMMIT" != "$HEADERS_COMMIT" ]]; then
      echo "Warning: expected header commit $HEADERS_COMMIT, got $ACTUAL_COMMIT" >&2
    fi
  )
else
  echo "Skia headers already present."
fi

echo "Skia is ready at third_party/skia and third_party/skia-headers."
