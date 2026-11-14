# __APP_TITLE__

先读 [AGENTS.md](AGENTS.md) 和 [DESIGN.md](DESIGN.md)。

这是一个 Zant UI 桌面应用：界面用 TypeScript + TSX（`src/App.tsx` 根组件、`src/components/` 业务组件），
底层原生模块用 Rust（`native/<名字>.ts` 规范 + `native/<名字>.rs` 实现，`native/lib.rs` 登记）。
TSX 在编译期转成 Rust，无 JS 运行时；不要添加 npm 依赖或手写界面到 `src/main.rs`。

- 入口 `src/main.rs`（只 include 生成的组件树并启动）；`build.rs` 调 `zant_build::tsx_app()`。
- `src/app_config.rs` 由 `zant sync` 从 `app.config.json` 生成，不要手改。
- 应用标识 `__BUNDLE_ID__`；用 `zant run` / `zant run --watch` 运行，`zant compile` 只编译，
  `cargo test` 跑原生模块的测试。

[TSX 文档](https://github.com/racall/zant-ui/blob/main/docs/tsx.md)。
