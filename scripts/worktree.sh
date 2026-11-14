#!/usr/bin/env bash
# Isolate a parallel migration WP in its own git worktree (see
# docs/rust-port/agent-brief.md).
#
#   scripts/worktree.sh new wp-3.1   # .worktrees/wp-3.1 on branch rust/wp-3.1
#   scripts/worktree.sh rm  wp-3.1   # drop the worktree (branch stays)
#
# The worktree gets symlinks to the shared cargo cache and third_party so
# builds do not re-download or re-extract anything.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cmd="${1:?usage: worktree.sh new|rm <name>}"
name="${2:?usage: worktree.sh new|rm <name>}"
wt="$root/.worktrees/$name"
case "$cmd" in
  new)
    git -C "$root" worktree add -q -b "rust/$name" "$wt" feat/rust-port
    ln -sfn "$root/.cargo-home" "$wt/.cargo-home"
    ln -sfn "$root/third_party" "$wt/third_party"
    echo "$wt"
    ;;
  rm)
    git -C "$root" worktree remove --force "$wt"
    ;;
  *)
    echo "usage: worktree.sh new|rm <name>" >&2; exit 2 ;;
esac
