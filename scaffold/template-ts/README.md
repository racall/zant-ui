# __APP_TITLE__

用 TypeScript + TSX 写界面、Rust 写底层模块的原生桌面应用（Zant UI）。
TSX 在编译期转成 Rust，没有 JS 运行时，也不需要 npm。应用标识 `__BUNDLE_ID__`。

```bash
zant run
zant run --watch      # 修改 src/ 后重新编译并重启，状态会重建
zant compile          # 只编译
zant sync             # 从 app.config.json 重新生成 src/app_config.rs
zant package          # 本机测试包
zant build            # 发布构建
```

## 目录

- `src/App.tsx`：根组件，持有输入框和计数状态。
- `src/components/`：Dashboard / StatCard / CounterCard，通过 props 和回调共享状态。
- `src/main.rs`：入口；include `build.rs` 生成的组件树，注册原生模块，用 `zant::tsx::app::run` 启动。
- `src/app_config.rs`：`zant` 从 `app.config.json` 生成（`zant create` / `zant sync`），不要手改。
- `native/`：原生模块。`system.ts` 是接口规范，`system.rs` 是 Rust 实现，`lib.rs` 登记；
  TSX 用 `import { info } from "native/system"` 直接调用，类型不一致时编译报错。
- `build.rs`：调用 `zant_build::tsx_app()`：`src/**/*.tsx` → `OUT_DIR`（模块 + `main.rs`），
  Windows 的 manifest / `.ico` 与 `pdfium.dll` 也在这一步处理。
- `Cargo.toml` / `rust-toolchain.toml`：单个 Cargo package，stable 工具链；
  Zant UI 的依赖是相对路径（由 `zant create` 写入）。项目是独立 workspace，
  也可以放在别的 Cargo workspace 里。
- `assets/icons/`：替换 PNG 即可修改图标，SVG 是可编辑源文件。
- `zant.d.ts` / `tsconfig.json`：编辑器类型与补全；本机有 TypeScript 时用 `tsc --noEmit -p .` 检查。

原生模块的单元测试用 `cargo test`。

语法与 hooks：[TSX 文档](https://github.com/racall/zant-ui/blob/main/docs/tsx.md)；
原生模块：[native 文档](https://github.com/racall/zant-ui/blob/main/docs/tsx-native.md)；
脚手架：[docs/scaffold.md](https://github.com/racall/zant-ui/blob/main/docs/scaffold.md)；
发布：[构建文档](https://github.com/racall/zant-ui/blob/main/docs/build.md)。
