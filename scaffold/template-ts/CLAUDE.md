# __APP_TITLE__

先读 [AGENTS.md](AGENTS.md) 和 [DESIGN.md](DESIGN.md)。

这是一个 Zant UI 桌面应用：界面用 TypeScript + TSX（`src/App.tsx` 根组件、`src/components/` 业务组件），
底层原生模块用 Rust（`native/<名字>.ts` 规范 + `native/<名字>.rs` 实现，构建时自动登记）。
TSX 在编译期转成 Rust，无 JS 运行时；不要添加 npm 依赖或手写界面到 `native/main.rs`。

- 入口 `native/main.rs`（一行 `zant::app!();`）；`native/build.rs` 调 `zant_build::tsx_app()`。
- 应用标识 `__BUNDLE_ID__`；用 `zant run` / `zant run --watch` 运行，`zant compile` 只编译，
  `cargo test` 跑原生模块的测试。

[TSX 文档](https://github.com/racall/zant-ui/blob/main/docs/tsx.md)。

应用的 `src/` 只放 `.tsx` / `.ts`，所有 Rust 文件都在 `native/`。
`Cargo.toml` 设置 `build = "native/build.rs"` 和 `[[bin]] path = "native/main.rs"`。
构建器在 `$OUT_DIR/zant_app.rs` 生成配置、原生模块树、规范类型与注册、TSX 模块声明；
`zant_main.rs` 启动应用，`zant_hot.rs` 提供热重载入口（`zant::hot_app!()`）。
新增原生能力只需 `native/<名字>.ts` + `native/<名字>.rs`；顶层无规范的 `.rs` 是内部模块，
自动声明但不注册，子目录由普通 `mod` 引入。生成代码不写回项目目录。
