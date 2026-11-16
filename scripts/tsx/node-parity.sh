#!/usr/bin/env bash
# Node parity: TSX compiled to Rust must print what Node prints.
#
#   scripts/tsx/node-parity.sh                      # every probe in crates/zant-tsx/tests/node_parity
#   scripts/tsx/node-parity.sh path/to/probe.ts ... # some probes
#
# A probe is a plain `.ts` file that exports `run(): void` and prints with
# `console.log`. The script compiles each one with zant-tsx (adding a root
# component, which Node cannot read in a .ts file), builds them into one Rust
# binary against this checkout's `zant` crate, runs `run()`, runs the same
# file with Node (built-in type stripping, Node 22.6+), and diffs the output.
# `run()` may throw; an uncaught throw prints `Uncaught …` on both sides.
#
# Exit status: 0 when every probe prints the same as Node.
set -uo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
probes=("$@")
if [ ${#probes[@]} -eq 0 ]; then
    probes=(crates/zant-tsx/tests/node_parity/*.ts)
fi
command -v node >/dev/null || { echo "node-parity: node is required"; exit 2; }

./scripts/cargo.sh build -q -p zant-tsx --bin zant-tsx || exit 2
work="$root/target/node-parity"
mkdir -p "$work/src"
cat > "$work/Cargo.toml" <<TOML
[package]
name = "node-parity"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
zant = { path = "$root/crates/zant" }

[workspace]
TOML

mods=""
calls=""
compiled=" "
for probe in "${probes[@]}"; do
    name=$(basename "${probe%.ts}")
    # zant needs a root component
    { echo 'import { View } from "zant";'; cat "$probe"; printf '\nexport default function ParityProbe() { return <View />; }\n'; } \
        > "$work/src/$name.tsx"
    if message=$("$root/target/debug/zant-tsx" "$work/src/$name.tsx" "$work/src/p_$name.rs" 2>&1); then
        mods+="#[path = \"p_$name.rs\"] mod p_$name;"$'\n'
        calls+="        \"$name\" => p_$name::run().finish(),"$'\n'
        compiled+="$name "
    else
        echo "FAIL $name: zant-tsx rejected it"
        echo "$message" | head -5
    fi
done
cat > "$work/src/main.rs" <<RUST
#![allow(dead_code, unused)]
$mods
/// \`run()\` returns \`()\`, or \`Result\` when a throw can escape it.
trait Outcome {
    fn finish(self);
}
impl Outcome for () {
    fn finish(self) {}
}
impl Outcome for Result<(), zant::tsx::Thrown> {
    fn finish(self) {
        if let Err(thrown) = self {
            zant::tsx::error::uncaught(&thrown);
        }
    }
}
fn main() {
    let name = std::env::args().nth(1).expect("probe name");
    match name.as_str() {
$calls        _ => panic!("unknown probe {name}"),
    }
}
RUST
# `scripts/cargo.sh` builds the repository workspace; this crate is its own
# workspace, so call cargo with the same CARGO_HOME
if [ -d "$root/.cargo-home" ] && [ -z "${CARGO_HOME:-}" ]; then
    export CARGO_HOME="$root/.cargo-home"
fi
if ! (cd "$work" && CARGO_TARGET_DIR="$root/target/node-parity-target" cargo build -q --manifest-path "$work/Cargo.toml" 2>&1 | grep -E "^(error|warning)" -A8 | head -60; exit "${PIPESTATUS[0]}"); then
    echo "node-parity: the generated Rust does not build"
    exit 1
fi
binary="$root/target/node-parity-target/debug/node-parity"

status=0
for probe in "${probes[@]}"; do
    name=$(basename "${probe%.ts}")
    case "$compiled" in *" $name "*) ;; *) status=1; continue ;; esac
    rust=$("$binary" "$name" 2>&1)
    js=$(cd "$(dirname "$probe")" && node --no-warnings -e "import('./$(basename "$probe")').then(m => m.run()).catch(e => console.error('Uncaught', String(e)))" 2>&1)
    if [ "$rust" = "$js" ]; then
        echo "SAME $name ($(printf '%s\n' "$rust" | wc -l | tr -d ' ') lines)"
    else
        echo "DIFF $name (< zant, > node)"
        diff <(printf '%s\n' "$rust") <(printf '%s\n' "$js") | head -20
        status=1
    fi
done
exit $status
