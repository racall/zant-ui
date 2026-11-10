# __APP_TITLE__

先读 [AGENTS.md](AGENTS.md) 和 [DESIGN.md](DESIGN.md)。
本项目用 TypeScript + TSX，入口 `src/App.tsx`；组件在 `src/components/`；
Zig 原生模块在 `native/`（`<名字>.ts` 规范 + `<名字>.zig` 实现，`lib.zig` 登记）。
应用标识 `__BUNDLE_ID__`，配置在 `app.config.json`。
使用 `zant run` / `zant run --watch`，只编译用 `zant compile`。
TSX 编译为原生 Zig，无 JS 运行时；不要添加手写 `main.zig` 或 npm 依赖。

[TSX 文档](https://github.com/racall/zant-ui/blob/main/docs/tsx.md)。
