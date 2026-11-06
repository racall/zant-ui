#!/usr/bin/env bash
# Runs a PowerShell command on the Windows test machine inside a project
# directory, with UTF-8 output and the pinned Zig on PATH.
#
#   scripts/win/remote.sh XyUI 'zig build test --summary all'
#   scripts/win/remote.sh stillmd 'zig build'
#
# ZANT_WIN_HOST / ZANT_WIN_DIR as in sync.sh; ZANT_WIN_ZIG (optional) is a
# Zig directory to put first on PATH.
set -euo pipefail

# shellcheck disable=SC1091
[ -f "$(dirname "$0")/.env" ] && . "$(dirname "$0")/.env"
HOST="${ZANT_WIN_HOST:?set ZANT_WIN_HOST (or scripts/win/.env)}"
DIR="${ZANT_WIN_DIR:?set ZANT_WIN_DIR (or scripts/win/.env)}"
ZIG="${ZANT_WIN_ZIG:-}"
project="$1"
shift

ssh "$HOST" "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \$OutputEncoding=[Text.Encoding]::UTF8; if ('$ZIG') { \$env:PATH='$ZIG;'+\$env:PATH }; Set-Location '$DIR/$project'; $*; exit \$LASTEXITCODE" \
  2> >(grep -vE "post-quantum|store now|pq.html" >&2)
