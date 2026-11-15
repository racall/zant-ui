# __APP_TITLE__

用 TypeScript + TSX 写界面、Rust 写底层模块的原生桌面应用（Zant UI）。
TSX 在编译期转成 Rust，没有 JS 运行时，也不需要 npm。应用标识 `__BUNDLE_ID__`。

```bash
zant run
zant run --watch      # 修改 src/ 后重新编译并重启，状态会重建
zant compile          # 只编译
zant sync             # 重新生成本平台的打包文件（macOS：Info.plist / entitlements）
zant package          # 本机测试包
zant build            # 发布构建
```

## 目录

- `src/App.tsx`：主窗口，持有计数状态，提供设置窗口入口。
- `src/windows/SettingsWindow.tsx`：独立设置窗口，重复打开会前置，关闭后主窗口继续运行。
- `src/stores/profile.ts`：共享资料 store，在主窗口和设置窗口中修改名称会实时同步。
- `src/components/`：Dashboard / StatCard / CounterCard，通过 props 和回调共享状态。
- `native/main.rs`：一行 `zant::app!();`，引入构建生成的模块与入口。
- `native/`：原生模块。`system.ts` 是接口规范，`system.rs` 是 Rust 实现，构建时自动登记；
  TSX 用 `import { info } from "native/system"` 直接调用，类型不一致时编译报错。
- `native/build.rs`：调用 `zant_build::tsx_app()`：`src/**/*.tsx` → `OUT_DIR`（模块 + `main.rs`），
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

应用的 `src/` 只放 `.tsx` / `.ts`，所有 Rust 文件都在 `native/`。
`Cargo.toml` 设置 `build = "native/build.rs"` 和 `[[bin]] path = "native/main.rs"`。
构建器在 `$OUT_DIR/zant_app.rs` 生成配置、原生模块树、规范类型与注册、TSX 模块声明；
`zant_main.rs` 启动应用，`zant_hot.rs` 提供热重载入口（`zant::hot_app!()`）。
新增原生能力只需 `native/<名字>.ts` + `native/<名字>.rs`；顶层无规范的 `.rs` 是内部模块，
自动声明但不注册，子目录由普通 `mod` 引入。生成代码不写回项目目录。
