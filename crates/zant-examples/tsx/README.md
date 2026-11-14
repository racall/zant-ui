# TSX example apps (`crates/zant-examples/tsx/`)

WP-5.10 ports the Zig TSX examples' entry points to Rust; WP-5.10b adds the
remaining apps of the plan's M5 acceptance. Each directory is a small **Cargo
workspace member** with the same layout as a `zant create` project
(`scaffold/template-ts/`), so plain `cargo build --workspace` /
`cargo test --workspace` build and check them:

```
tsx-counter/   bin zant-tsx-counter     corpus/counter.tsx (1:1 port)
tsx-demo/      bin zant-tsx-demo        corpus/app/App.tsx (showcase app)
tsx-bench/     bin zant-tsx-bench-app   corpus/settings.tsx (frame-benchmark workload)
tsx-gallery/   bin zant-tsx-gallery     examples/tsx/gallery.tsx (component gallery)
tsx-todos/     bin zant-tsx-todos       examples/tsx/todos.tsx (todo app)
tsx-reader/    bin zant-tsx-reader      examples/tsx/reader.tsx (Markdown + PDF reader)
tsx-notes/     bin zant-tsx-notes       examples/tsx/notes/ (two files + native module)
  Cargo.toml
  build.rs          include!("../app_config_build.rs"); main() → build_app()
  app.config.json   title / bundle id / window options
  assets/icons/     statusbar / tray icons (copy of the template's, small PNGs)
  native/           system.ts (spec) + system.rs (impl) + lib.rs (registration)
  src/App.tsx       the UI (TS + TSX only)
  src/main.rs       zant::tsx::app::run, includes the generated modules
```

The four WP-5.10b apps are ports of `examples/tsx/{gallery,todos,reader}.tsx`
and `examples/tsx/notes/`: the TSX is the Zig source verbatim except for the
file header comments and, in `tsx-notes`, the `format` module:

- `examples/tsx/notes/format.zig` → `tsx-notes/native/format.ts` (spec) +
  `tsx-notes/native/format.rs` (`impl FormatSpec for Native`). The Zig app
  imported a plain Zig module (`import { plural } from "./format.zig"`); the
  Rust TSX compiler rejects Zig imports, so the same helper is a native module
  and `src/App.tsx` imports it from `"native/format"` — compiled to a direct
  `format::plural` call, checked by the generated trait at the `impl`.
- `tsx-gallery` declares its `Modal` footer as `footer={open ? (<Row>…) :
  null}`: the codegen types an element for an `Option<Node>` prop as a `Node`
  and only wraps a null-union in `Some(..)` (reported under “WP-5.10b 提出” in
  [`docs/rust-port/status.md`](../../../../docs/rust-port/status.md)). `open` is
  always true in the `open && (…Modal)` branch, so the rendered tree is the Zig
  example's.
- `tsx-reader` opens the repo's small PDF fixture; the TSX keeps the Zig
  path (`tests/fixtures/widgets.pdf`), so run it from the repository root
  (`cargo run -p zant-tsx-reader`). The fixture is shared, not copied.

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
`crates/zant-examples/src/bin/zant-tsx-bench.rs`) against the Zig baseline
(`tools/tsx/compile_bench.zig`) with a ≤1.25× gate. With `ZANT_ZIG_BIN` pointing
at a `zig-out-baseline/bin` (the variable `scripts/parity/compare.sh` uses) it
also compares the four WP-5.10b apps with their Zig baseline binaries: identical
layout dumps and ≤ 0.1 % differing pixels.

```bash
cargo run -p zant-tsx-counter
cargo run -p zant-tsx-demo
cargo run -p zant-tsx-bench-app
cargo run -p zant-tsx-gallery
cargo run -p zant-tsx-todos
cargo run -p zant-tsx-reader    # from the repository root (the PDF fixture path)
cargo run -p zant-tsx-notes
./scripts/tsx/examples-check.sh
ZANT_ZIG_BIN=/path/to/zig-out-baseline/bin ./scripts/tsx/examples-check.sh
```

Known compiler gaps the examples work around are listed under “WP-5.10 提出”
and “WP-5.10b 提出” in
[`docs/rust-port/status.md`](../../../../docs/rust-port/status.md) (cross-file
window components, generated-code lints, `native/spec.rs` formatting, the
`desktop`/`entities` corpus files, `Option<Node>` props, rich-text children).
