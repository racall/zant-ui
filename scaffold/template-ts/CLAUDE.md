# __APP_TITLE__

先读 [AGENTS.md](AGENTS.md) 和 [DESIGN.md](DESIGN.md)。

```bash
zant run              # 默认原地热刷新
zant run --no-hot     # 运行一次；验证 macOS 沙盒
cargo test            # 原生 Rust 测试
```

- 推荐 TSX：`src/App.tsx` 默认导出无 props 根组件，`src/` 只放 `.tsx` / `.ts`，无 JS / DOM / npm。
- Rust 全在 `native/`：`main.rs` 仅 `zant::app!();`，`build.rs` 调 `zant_build::tsx_app()`；
  `<名>.ts` 规范 + `.rs` 实现自动声明 / 注册，参照 `system.*`。生成代码全在 `OUT_DIR`，不手改。
- `app.config.json` 管理标识 `__BUNDLE_ID__`、元数据和窗口；模板 `window.remember: true` 记忆窗口位置 / 尺寸。
- 热刷新保留兼容状态 / 稳定 key 的视图状态；配置等改动重启 `zant run`；`--watch` 是旧重启模式。
  store / Rust 全局变量 / 外部资源不自动恢复，应用负责在卸载前清理资源。
- props 向下、回调向上，最新值用 `setX(prev => …)`，共享状态用 `createStore` / `useStore`；颜色用 `useTheme()`。
  界面检查不同尺寸、浅深主题和各状态；不提交密钥或构建产物。

[TSX 文档](https://github.com/racall/zant-ui/blob/main/docs/tsx.md)。
