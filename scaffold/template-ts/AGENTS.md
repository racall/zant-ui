# __APP_TITLE__

先读 [DESIGN.md](DESIGN.md)，它是界面与交互的最高优先级规范。

- 应用标识：`__BUNDLE_ID__`；配置在 `app.config.json`，不要手改生成的 `src/app_config.zig`。
- 界面和业务逻辑写 TypeScript + TSX，入口是 `src/App.tsx` 的默认导出，不要添加 `src/main.zig`。
- 自定义组件在 `src/components/*.tsx`；状态用 `useState`，跨组件通过 props 向下、回调向上。
- 跨窗口状态用 `createStore` / `useStore`；hooks 只在组件顶层调用。
- 业务组件组合 Zant 组件；颜色用 `useTheme()` 的 token，适配深浅色主题。
- TSX 是静态 TypeScript 子集，不是浏览器 React，不引入 npm 或 DOM API。
- 底层引擎可写相邻 `.zig` 原生模块，TSX 按名导入；耗时工作不要放在渲染路径。
- `zant run` 运行；`zant run --watch` 重新编译并重启（内存状态会重建）；`zant compile` 只编译。
- `zant sync` 同步配置；`zant package` 本机打包；`zant build` 发布。
- 修改界面后检查不同窗口尺寸、浅/深色主题和交互状态；不要提交构建缓存或密钥。

[完整 TSX 语法、构建与 hooks](https://github.com/racall/zant-ui/blob/main/docs/tsx.md)。
[脚手架](https://github.com/racall/zant-ui/blob/main/docs/scaffold.md)。
