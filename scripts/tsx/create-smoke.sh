#!/bin/sh
# Build a fresh TS application against this checkout. Never starts a GUI.
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
zig=${ZIG:-zig}
"$zig" build --build-file "$repo/build.zig" cli
work=$(mktemp -d "${TMPDIR:-/tmp}/zant-tsx-create.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM
cd "$work"
"$repo/zig-out/bin/zant" create demo --lang ts --yes
cd demo
test -f src/App.tsx
test ! -f src/main.zig
test ! -f src/app_config.zig # the build generates it from app.config.json
test -f assets/icons/app-macos.png
test -f zant.d.ts
test -f tsconfig.json
"$repo/zig-out/bin/zant" sync
test ! -f src/app_config.zig
"$repo/zig-out/bin/zant" compile
"$zig" build
if test -f zig-out/bin/app || test -f zig-out/bin/app.exe; then
    echo 'TS create/sync/compile/zig build: OK (GUI not started)'
else
    echo 'Missing application executable' >&2
    exit 1
fi
# run must surface the original TSX location before it ever starts the GUI.
printf '\nconst broken = ;\n' >> src/App.tsx
if "$repo/zig-out/bin/zant" run > errors.log 2>&1; then
    echo 'Invalid TSX unexpectedly compiled' >&2
    exit 1
fi
if ! LC_ALL=C grep -Eq 'App\.tsx:[0-9]+:[0-9]+: error:' errors.log; then
    cat errors.log >&2
    exit 1
fi
echo 'TSX diagnostics through zant run: OK'
