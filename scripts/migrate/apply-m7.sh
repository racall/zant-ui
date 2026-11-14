#!/usr/bin/env bash
# Applies the staged M7 ports (see docs/rust-port/m7-cliphop.md,
# docs/rust-port/m7-stillmd.md) to the sibling app repositories.
#
#   scripts/migrate/apply-m7.sh            # dry-run: print the plan
#   scripts/migrate/apply-m7.sh --apply    # do it (needs the user's approval:
#                                          # writes outside this checkout)
#
# What it does:
#   ../cliphop-ts  (a git repo) — new branch `rust-port` from the current HEAD,
#                  the Rust app copied over it, Zig build files and native
#                  .zig sources removed, Cargo.toml path deps pointed at this
#                  checkout. The branch is left uncommitted for review.
#   ../stillmd     (not a git repo) — full tar backup next to the directory,
#                  then the Zig build/sources are replaced by the Rust port.
#
# Both ports were validated in /tmp before this script existed:
#   cliphop: 26 cargo tests, headless snapshot against the user's real DB
#   stillmd: 13 cargo tests, 11-config pixel/layout parity with the Zig app
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PARENT="$(cd "$ROOT/.." && pwd)"
CLIPHOP="$PARENT/cliphop-ts"
STILLMD="$PARENT/stillmd"
CLIPHOP_ART="/tmp/m7-cliphop-artifact/cliphop-ts-rust.tar.gz"
STILLMD_ART="/tmp/m7-stillmd-artifact/stillmd-rust.tar.gz"
CARGO_HOME_DIR="$ROOT/.cargo-home"
APPLY=0
[ "${1:-}" = "--apply" ] && APPLY=1

say() { printf '\033[36m==\033[0m %s\n' "$*"; }
ok() { printf '\033[32m✓\033[0m %s\n' "$*"; }
die() { printf '\033[31m✗\033[0m %s\n' "$*" >&2; exit 1; }

# Rewrite the absolute Zant UI path dependency into the sibling layout.
relocate_cargo_toml() { # <app dir>
    local file="$1/Cargo.toml"
    [ -f "$file" ] || return 0
    python3 - "$file" "$PARENT" <<'PY'
import pathlib, sys
toml = pathlib.Path(sys.argv[1])
parent = pathlib.Path(sys.argv[2])
text = toml.read_text()
for crate in ("zant", "zant-macros", "zant-build"):
    text = text.replace(f'path = "/Users/xiaoyi/dev/XyUI/crates/{crate}"',
                        f'path = "../XyUI/crates/{crate}"')
# a relative path is written by `zant create` as a `..`-walk from the app dir
toml.write_text(text)
PY
    grep -q 'path = "\.\./XyUI/crates/zant"' "$file" || die "$file still points outside the sibling layout"
}

plan_cliphop() {
    say "cliphop-ts: $CLIPHOP"
    say "  1. git switch -c rust-port (from the current HEAD, backup tag)"
    say "  2. copy the Rust app from the staged artifact over the tree"
    say "  3. delete build.zig, build.zig.zon, native/**/*.zig, tests/*.zig, docs/ts-port.md"
    say "  4. Cargo.toml path deps -> ../XyUI/crates/{zant,zant-macros,zant-build}"
    say "  5. cargo test (26 tests) and headless snapshot"
}

apply_cliphop() {
    [ -d "$CLIPHOP" ] || die "missing $CLIPHOP"
    [ -f "$CLIPHOP_ART" ] || die "missing $CLIPHOP_ART (staged port)"
    git -C "$CLIPHOP" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
        || die "$CLIPHOP is not a git repository"
    [ -z "$(git -C "$CLIPHOP" status --porcelain)" ] \
        || die "$CLIPHOP has uncommitted changes; commit/stash them first"
    git -C "$CLIPHOP" rev-parse --verify rust-port >/dev/null 2>&1 \
        && die "$CLIPHOP already has a rust-port branch; delete it first"
    git -C "$CLIPHOP" tag -f m7-zig-baseline HEAD
    git -C "$CLIPHOP" switch -c rust-port
    local tmp
    tmp="$(mktemp -d)"
    tar xzf "$CLIPHOP_ART" -C "$tmp"
    # the archive keeps the port at its top level (m7-cliphop-clean/ = repo)
    local src
    src="$(find "$tmp" -maxdepth 2 -name Cargo.toml -printf '%h\n' | head -1)"
    [ -n "$src" ] || die "the cliphop artifact has no Cargo.toml"
    (cd "$src" && tar cf - --exclude=zant-fix.patch .) | (cd "$CLIPHOP" && tar xf -)
    git -C "$CLIPHOP" rm -q -r --cached build.zig build.zig.zon 2>/dev/null || true
    rm -f "$CLIPHOP/build.zig" "$CLIPHOP/build.zig.zon"
    (cd "$CLIPHOP" && rm -f native/*.zig native/lib.zig tests/*.zig docs/ts-port.md
     rmdir tests 2>/dev/null || true; find native -name '*.zig' -delete)
    relocate_cargo_toml "$CLIPHOP"
    rm -rf "$tmp"
}

plan_stillmd() {
    say "stillmd: $STILLMD"
    say "  1. tar backup: $PARENT/stillmd-zig-backup-<date>.tar.gz"
    say "  2. delete build.zig, build.zig.zon, zig-out/, zig-out-baseline/, src/*.zig, src/components/"
    say "  3. copy Cargo.toml/Cargo.lock/.gitignore/README.md/src/*.rs/src/components/*.rs"
    say "  4. Cargo.toml path deps -> ../XyUI/crates/zant"
    say "  5. cargo test (13 tests) and headless snapshot"
}

apply_stillmd() {
    [ -d "$STILLMD" ] || die "missing $STILLMD"
    [ -f "$STILLMD_ART" ] || die "missing $STILLMD_ART (staged port)"
    local backup="$PARENT/stillmd-zig-backup-$(date +%Y%m%d-%H%M%S).tar.gz"
    say "backup -> $backup"
    tar czf "$backup" -C "$PARENT" --exclude=zig-out --exclude=zig-out-baseline \
        --exclude=.zig-cache stillmd
    ok "backup written"
    (cd "$STILLMD" && rm -rf build.zig build.zig.zon zig-out zig-out-baseline
     .zig-cache src/main.zig src/state.zig src/history.zig src/palette.zig \
     src/shortcuts.zig src/app_config.zig src/components)
    local tmp
    tmp="$(mktemp -d)"
    tar xzf "$STILLMD_ART" -C "$tmp"
    (cd "$tmp/stillmd-rust" && tar cf - .) | (cd "$STILLMD" && tar xf -)
    rm -rf "$tmp"
    relocate_cargo_toml "$STILLMD"
    touch "$STILLMD/.gitignore" 2>/dev/null || true
    grep -q '^/target/' "$STILLMD/.gitignore" 2>/dev/null \
        || printf '/target/\n' >> "$STILLMD/.gitignore"
}

verify() { # <dir> <expected tests>
    local dir="$1" tests="$2"
    say "verify $dir"
    CARGO_HOME="$CARGO_HOME_DIR" cargo test --manifest-path "$dir/Cargo.toml" 2>&1 |
        grep -E "test result: ok\. ${tests} passed" \
        || die "$dir: cargo test did not report ${tests} passing tests"
    ok "$dir: ${tests} tests pass"
}

if [ "$APPLY" = 0 ]; then
    say "dry-run — pass --apply to perform the migration (needs user approval)"
    plan_cliphop
    echo
    plan_stillmd
    exit 0
fi

say "applying the M7 ports"
apply_cliphop
ok "cliphop-ts migrated (branch rust-port, Zig baseline tag m7-zig-baseline)"
apply_stillmd
ok "stillmd migrated (Zig tree backed up next to it)"
verify "$CLIPHOP" 26
verify "$STILLMD" 13
say "done. Review the trees, then commit/merge:"
say "  git -C $CLIPHOP status; git -C $CLIPHOP add -A && git -C $CLIPHOP commit"
say "  (stillmd is not a git repo: keep the backup tarball or init one)"
