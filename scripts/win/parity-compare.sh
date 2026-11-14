#!/usr/bin/env bash
# Windows parity check: runs scripts/win/parity-capture.ps1 on the test machine
# (Zig baseline in zig-out/bin vs the Rust port in target/release), copies the
# PNG / layout artifacts back to this Mac and compares them with the same
# python diffs as scripts/parity/compare.sh (plan.md §6 / §7.2).
#
#   scripts/win/parity-compare.sh [app...]       # capture + compare
#   ZANT_WIN_SKIP_CAPTURE=1 scripts/win/parity-compare.sh   # diff what is remote
#
# Environment (same matrix knobs as compare.sh):
#   ZANT_PARITY_SIZES="600x620 900x640 1200x800"
#   ZANT_PARITY_THEMES="light dark"
#   ZANT_WIN_PARITY_DIR=target/win-parity   (remote directory, relative to the checkout)
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck disable=SC1091
[ -f "$root/scripts/win/.env" ] && . "$root/scripts/win/.env"
host="${ZANT_WIN_HOST:?set ZANT_WIN_HOST (or scripts/win/.env)}"
remote_dir="${ZANT_WIN_PARITY_DIR:-target/win-parity}"
sizes="${ZANT_PARITY_SIZES:-600x620 900x640 1200x800}"
themes="${ZANT_PARITY_THEMES:-light dark}"
python="${ZANT_PYTHON:-python3}"
out="$root/target/parity-win"

apps=("$@")
if [ ${#apps[@]} -eq 0 ]; then
    apps=(zant-demo zant-gallery zant-components zant-markdown zant-math zant-md-stream zant-pdf)
fi

if [ -z "${ZANT_WIN_SKIP_CAPTURE:-}" ]; then
    app_list="$(IFS=,; echo "${apps[*]}")"
    size_list="${sizes// /,}"
    theme_list="${themes// /,}"
    "$root/scripts/win/remote.sh" XyUI \
        "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/win/parity-capture.ps1 -Out '$remote_dir' -Apps '$app_list' -Sizes '$size_list' -Themes '$theme_list'"
fi

# pull the artifacts back (remote Windows `tar` is bsdtar; its output is a
# plain tar archive on stdout, which the paсsandbox allows over ssh)
mkdir -p "$out"
ssh ${ZANT_WIN_SSH_OPTS:-} "$host" "tar -C '$remote_dir' -cf - ." | tar -xf - -C "$out"

failures=0
declare -a results=()
for app in "${apps[@]}"; do
    app_dir="$out/$app"
    [ -d "$app_dir" ] || { results+=("MISSING $app (no artifacts in $app_dir)"); failures=$((failures + 1)); continue; }
    for zig_dir in "$app_dir"/zig-*; do
        [ -d "$zig_dir" ] || continue
        name="$(basename "$zig_dir")"
        name="${name#zig-}"
        rust_dir="$app_dir/rust-$name"
        if [ ! -d "$rust_dir" ]; then
            results+=("FAIL $app $name (no rust capture)")
            failures=$((failures + 1))
            continue
        fi
        layout_ok=1
        image_ok=1
        if [ -s "$zig_dir/layout.txt" ] || [ -s "$rust_dir/layout.txt" ]; then
            "$python" "$root/scripts/parity/layoutdiff.py" "$zig_dir/layout.txt" "$rust_dir/layout.txt" \
                >"$app_dir/layoutdiff-$name.log" 2>&1 || layout_ok=0
        fi
        "$python" "$root/scripts/parity/imgdiff.py" "$zig_dir/shot.png" "$rust_dir/shot.png" \
            --diff "$app_dir/diff-$name.png" >"$app_dir/imgdiff-$name.log" 2>&1 || image_ok=0
        if [ "$layout_ok" = 1 ] && [ "$image_ok" = 1 ]; then
            results+=("PASS $app $name")
        else
            results+=("FAIL $app $name (layout=$layout_ok image=$image_ok)")
            failures=$((failures + 1))
        fi
    done
done

echo
echo "== Windows parity =="
printf '%s\n' "${results[@]}"
if [ "$failures" -ne 0 ]; then
    echo "$failures / ${#results[@]} combinations differ (artifacts: $out)" >&2
    exit 1
fi
echo "all ${#results[@]} combinations match"
