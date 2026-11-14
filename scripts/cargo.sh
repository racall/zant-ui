#!/usr/bin/env bash
# Cargo wrapper for sandboxed sessions.
#
# The default `~/.cargo` registry cache is outside the writable sandbox used
# by the agent tools, so builds fail with "Operation not permitted". Redirect
# CARGO_HOME into the repo (gitignored) and forward all arguments, e.g.
#
#   ./scripts/cargo.sh fmt --all
#   ./scripts/cargo.sh test -p zant geometry
#
# Normal (non-sandboxed) use can keep calling `cargo` directly.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME="$PWD/.cargo-home"
exec cargo "$@"
