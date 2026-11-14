# __APP_TITLE__

先读 [DESIGN.md](DESIGN.md)，它是界面与交互的最高优先级规范。

## 项目

用 TypeScript + TSX 写界面的原生桌面应用（Zant UI 框架）：TSX 在编译期转成 Rust，
没有 JS 运行时，也不需要 npm。应用标识 `__BUNDLE_ID__`，配置在 `app.config.json`。

## 目录

- `src/App.tsx`：根组件（默认导出）。界面和业务逻辑都写 TSX，自定义组件放 `src/components/*.tsx`。
- `src/main.rs`：入口，只负责 include 生成的组件树、注册原生模块并调用 `zant::tsx::app::run`；
  不要在这里写界面。
- `src/app_config.rs`：由 `zant` 从 `app.config.json` 生成（`zant create` / `zant sync`），不要手改。
- `native/<名字>.ts`：原生模块规范（唯一来源，只写类型与 `export declare function`）；
  `native/<名字>.rs` 是 Rust 实现，`native/lib.rs` 登记并注册。示例模块是 `native/system`。
  内部代码放 `native/` 的子目录；耗时工作放后台线程，不要放在渲染路径。
- `build.rs`：调用 `zant_build::tsx_app()`，把 `src/**/*.tsx` 编译成 Rust（`OUT_DIR/app.rs`）。
- `Cargo.toml`：单个 Cargo package；`zant` / `zant-build` 是相对路径依赖（`zant create` 写入）。
- `assets/icons/`：图标；代码里用 `include_bytes!("../assets/icons/<文件>")` 引用。
- `zant.d.ts` / `tsconfig.json`：`import … from "zant"` 的类型与补全。

## 约定

- 状态单一来源：父组件持有状态，props 向下、回调向上；跨窗口状态用 `createStore` / `useStore`，
  hooks 只在组件顶层调用。TSX 是静态 TypeScript 子集，不是浏览器 React，不引入 npm 或 DOM API。
- 业务组件只组合 Zant 的组件；颜色用 `useTheme()` 的 token，保证浅色 / 深色主题都正确。
- 通用组件（模态框、菜单、快捷键列表…）属于 Zant UI 框架，应用里只写业务数据和组合。
- 修改界面后检查不同窗口尺寸、浅 / 深色主题和各交互状态；不要提交 `target/` 构建缓存或密钥。

## 命令

```bash
zant run              # 编译并运行
zant run --watch      # 重新编译并重启（内存状态会重建）
zant compile          # 只编译
zant sync             # 从 app.config.json 重新生成 src/app_config.rs
zant package          # 本机测试包
zant build            # 发布构建
cargo test            # 原生模块的 Rust 测试
```

[TSX 语法与 hooks](https://github.com/racall/zant-ui/blob/main/docs/tsx.md)
[原生模块](https://github.com/racall/zant-ui/blob/main/docs/tsx-native.md)
[脚手架](https://github.com/racall/zant-ui/blob/main/docs/scaffold.md)
[发布构建](https://github.com/racall/zant-ui/blob/main/docs/build.md)
