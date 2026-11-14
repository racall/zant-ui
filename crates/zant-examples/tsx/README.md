# TSX example apps (`crates/zant-examples/tsx/`)

Small TSX apps that exercise the TSX pipeline end to end. Each directory is a
small **Cargo workspace member** with the same layout as a `zant create` project
(`scaffold/template-ts/`), so plain `cargo build --workspace` /
`cargo test --workspace` build and check them:

```
tsx-counter/   bin zant-tsx-counter     counter (state, handlers, inputs)
tsx-demo/      bin zant-tsx-demo        showcase app (store, tray, native module, components/)
tsx-bench/     bin zant-tsx-bench-app   settings page (frame-benchmark workload)
tsx-gallery/   bin zant-tsx-gallery     component gallery
tsx-todos/     bin zant-tsx-todos       todo app
tsx-reader/    bin zant-tsx-reader      Markdown + PDF reader
tsx-notes/     bin zant-tsx-notes       two files + native module
  Cargo.toml
  build.rs          include!("../app_config_build.rs"); main() → build_app()
  app.config.json   title / bundle id / window options
  assets/icons/     statusbar / tray icons (copy of the template's, small PNGs)
  native/           system.ts (spec) + system.rs (impl) + lib.rs (registration)
  src/App.tsx       the UI (TS + TSX only)
  src/main.rs       zant::tsx::app::run, includes the generated modules
```

Notes on individual apps:

- `tsx-notes` keeps its `plural` helper in a native module:
  `native/format.ts` (spec) + `native/format.rs` (`impl FormatSpec for Native`).
  `src/App.tsx` imports it from `"native/format"` — compiled to a direct
  `format::plural` call, checked by the generated trait at the `impl`.
- `tsx-gallery` declares its `Modal` footer as `footer={open ? (<Row>…) :
  null}`: the codegen types an element for an `Option<Node>` prop as a `Node`
  and only wraps a null-union in `Some(..)`. `open` is always true in the
  `open && (…Modal)` branch, so the rendered tree is the same.
- `tsx-reader` opens the repo's small PDF fixture by a relative path
  (`tests/fixtures/widgets.pdf`), so run it from the repository root
  (`cargo run -p zant-tsx-reader`). The fixture is shared, not copied.

`corpus/` holds standalone `.tsx` inputs: the pages `zant-tsx-frame-bench`
compiles in `crates/zant-examples/build.rs`, plus extra files for the
`zant-tsx` parser test. They are not apps of their own.

`app_config_build.rs` is shared by the seven `build.rs` files. It calls
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
`crates/zant-examples/src/bin/zant-tsx-bench.rs`) and checks how many corpus
files compiled.

```bash
cargo run -p zant-tsx-counter
cargo run -p zant-tsx-demo
cargo run -p zant-tsx-bench-app
cargo run -p zant-tsx-gallery
cargo run -p zant-tsx-todos
cargo run -p zant-tsx-reader    # from the repository root (the PDF fixture path)
cargo run -p zant-tsx-notes
./scripts/tsx/examples-check.sh
```

Known compiler gaps the examples work around are noted in comments next to the
workaround (for example the `Modal` footer above).
