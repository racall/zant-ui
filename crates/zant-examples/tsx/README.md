# TSX example apps (`crates/zant-examples/tsx/`)

WP-5.10 ports the Zig TSX examples' entry points to Rust. Each directory is a
small **Cargo workspace member** with the same layout as a `zant create` project
(`scaffold/template-ts/`), so plain `cargo build --workspace` /
`cargo test --workspace` build and check them:

```
tsx-counter/   bin zant-tsx-counter     examples/tsx/counter.tsx (1:1 port)
tsx-demo/      bin zant-tsx-demo        examples/tsx/app/App.tsx (showcase app)
tsx-bench/     bin zant-tsx-bench-app   examples/tsx/settings.tsx (frame-benchmark workload)
  Cargo.toml
  build.rs          include!("../app_config_build.rs"); main() → build_app()
  app.config.json   title / bundle id / window options
  native/           system.ts (spec) + system.rs (impl) + lib.rs (registration)
  src/App.tsx       the UI (TS + TSX only)
  src/main.rs       zant::tsx::app::run, includes the generated modules
```

`app_config_build.rs` is shared by the three `build.rs` files. It calls
`zant_build::try_tsx_app()` — the template's TSX → Rust build step — and writes
`src/app_config.rs` from the `app.config.json` that helper parsed. A generated
project gets that file from `zant sync` once; a workspace member cannot, so the
examples write it during the build instead (content-compared, so the `src/`
rerun-if-changed watch does not rerun the build script). `src/app_config.rs` and
`native/spec.rs` are generated and gitignored.

The examples are not installed or packaged; they exist to prove the TSX pipeline
end to end and to give `scripts/tsx/examples-check.sh` something to run. That
script builds them, runs each headless with `ZANT_SNAPSHOT` + `ZANT_LAYOUT_FILE`
(asserting the PNG is non-blank, the window size comes from `app.config.json`,
two runs are byte-identical, and the expected keyed controls are laid out), and
then runs the TSX compile benchmark (`zant-tsx-bench`, see
`crates/zant-examples/src/bin/zant-tsx-bench.rs`) against the Zig baseline
(`tools/tsx/compile_bench.zig`) with a ≤1.25× gate.

```bash
cargo run -p zant-tsx-counter
cargo run -p zant-tsx-demo
cargo run -p zant-tsx-bench-app
./scripts/tsx/examples-check.sh
```

Known compiler gaps the examples work around are listed under “WP-5.10 提出” in
[`docs/rust-port/status.md`](../../../../docs/rust-port/status.md) (cross-file
window components, generated-code lints, `native/spec.rs` formatting, the
`desktop`/`entities` corpus files).
