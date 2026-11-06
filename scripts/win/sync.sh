#!/usr/bin/env bash
# Copies this checkout (tracked + untracked, not ignored files) to the Windows
# test machine, and optionally sibling app directories, so `../XyUI` path
# dependencies keep resolving there.
#
#   scripts/win/sync.sh                 # XyUI only
#   scripts/win/sync.sh ../stillmd      # XyUI and the stillmd app
#
# ZANT_WIN_HOST (user@host) and ZANT_WIN_DIR (parent directory on the
# machine, forward slashes) come from the environment or from an untracked
# scripts/win/.env file (KEY=value lines).
set -euo pipefail
# no AppleDouble "._*" files (extended attributes) in the archives
export COPYFILE_DISABLE=1

# shellcheck disable=SC1091
[ -f "$(dirname "$0")/.env" ] && . "$(dirname "$0")/.env"
HOST="${ZANT_WIN_HOST:?set ZANT_WIN_HOST (or scripts/win/.env)}"
DIR="${ZANT_WIN_DIR:?set ZANT_WIN_DIR (or scripts/win/.env)}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

pack() { # <source dir> <archive>
  local src="$1" out="$2"
  if git -C "$src" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    (cd "$src" && git ls-files -co --exclude-standard -z | xargs -0 tar --no-xattrs -cf "$out")
  else
    tar --no-xattrs -cf "$out" -C "$src" --exclude .zig-cache --exclude zig-out --exclude .git --exclude '._*' .
  fi
}

send() { # <source dir>
  local src name archive
  src="$(cd "$1" && pwd)"
  name="$(basename "$src")"
  archive="$TMP/$name.tar"
  pack "$src" "$archive"
  scp -q "$archive" "$HOST:$DIR/cache/$name.tar"
  ssh "$HOST" "\$ErrorActionPreference='Stop'; New-Item -ItemType Directory -Force '$DIR/$name' | Out-Null; tar -xf '$DIR/cache/$name.tar' -C '$DIR/$name'; Remove-Item '$DIR/cache/$name.tar'" \
    2> >(grep -vE "post-quantum|store now|pq.html" >&2)
  echo "synced $name -> $HOST:$DIR/$name"
}

ssh "$HOST" "New-Item -ItemType Directory -Force '$DIR/cache' | Out-Null" 2> >(grep -vE "post-quantum|store now|pq.html" >&2)
send "$ROOT"
for extra in "$@"; do send "$extra"; done
