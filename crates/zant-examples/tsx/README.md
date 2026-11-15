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
  native/build.rs   fn main() { zant_build::tsx_app(); }
  app.config.json   title / bundle id / window options
  assets/icons/     statusbar / tray icons (copy of the template's, small PNGs)
  native/           system.ts (spec) + system.rs (impl) (自动声明与注册)
  src/App.tsx       the UI (TS + TSX only)
  native/main.rs    zant::app!();
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

`native/build.rs` 调用 `zant_build::tsx_app()`，生成配置、规范与模块树到 `OUT_DIR/zant_app.rs`，
普通入口为 `zant_main.rs`，热重载入口为 `zant_hot.rs`。项目目录不产生 Rust 文件。
