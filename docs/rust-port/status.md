# Rust 迁移进度（status）

> 每个执行者领取 / 完成 / 阻塞时更新自己那一行。状态取值：`待领取` / `进行中` / `完成` / `阻塞` / `待验证`。
> 分支：并行期间所有 WP 直接提交到 `feat/rust-port`（多人共用同一工作区，无法为每个 WP 开独立分支），
> 提交信息带 `WP-x.y: ` 前缀，由 lead 审核后提交。

## 环境与基线（M0 记录）

| 项 | 值 |
|---|---|
| 迁移起点 | `feat/tsx-compiler` HEAD `905955f`（本仓库即该提交） |
| 迁移分支 | `feat/rust-port` |
| Zig | 0.17.1（`/opt/homebrew/bin/zig`） |
| Rust | stable 1.94.1，edition 2024；`rust-toolchain.toml` 用 `channel = "stable"`（沙箱内 rustup 不能装精确版本，见 conventions.md） |
| 基线机器 | macOS 27.2.1，arm64（Apple Silicon） |
| `zig build test`（macOS arm64） | **通过**：6 个测试二进制，共 **380** 个测试（tests 聚合 124、框架单元 99、zant CLI 8、TSX 编译器 149）。文档里旧的「164/164」是过时数字 |
| ReleaseFast 基线 | `zig-out-baseline/bin/`（`zant-demo`、`zant-components`、`zant-gallery`、`zant-markdown`、`zant-math`、`zant-pdf`、`zant-md-stream`、`zant-bench`、`zant`） |
| Windows 基线 | **待验证**：测试机 `192.168.31.24` 当前不可达（No route to host）。恢复后要补：`zig build test`、`build_bridge.ps1`、rustup / VS Build Tools / 1.94.1 工具链检查 |
| 沙箱注意 | `cargo` 一律通过 `./scripts/cargo.sh`（CARGO_HOME 指向 `.cargo-home/`）；Zig 命令带 `ZIG_GLOBAL_CACHE_DIR=$PWD/.zig-global-cache` |

## 工作包进度

### M0 基线与地基

| WP | 内容 | 负责人 | 状态 | 分支 | 备注 |
|---|---|---|---|---|---|
| 0.1 | 建分支、两平台基线测试、ReleaseFast 基线、Windows 机器确认 | lead | 进行中 | feat/rust-port | macOS 基线已记录；Windows 机器不可达，待验证 |
| 0.2 | Cargo workspace、rust-toolchain、空 crate 骨架、.gitignore、CI rust.yml | lead | 完成 | feat/rust-port | `cargo build --workspace` 通过（macOS）；Windows 待验证 |
| 0.3 | `zant-sys`：bindgen 签入 + build.rs 编原生代码 / 链 Skia | lead | 待领取 | feat/rust-port | |
| 0.4 | `scripts/parity/` 对照工具（capture/compare） | lead | 待领取 | feat/rust-port | |
| 0.5 | `docs/rust-port/{plan,conventions,api,status}.md` | lead | 完成 | feat/rust-port | |

### M1 核心骨架

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 1.1a | geometry、theme、motion、layout/flex + 测试 | agent-1 | 待领取 | |
| 1.1b | core/{unicode,keys,paths,hash,text_select} | agent-2 | 待领取 | |
| 1.1c | renderer/{canvas,text,rich_text,renderer}、image | lead | 待领取 | 依赖 0.3 |
| 1.1d | platform/{platform,dialog}、core/events | lead | 待领取 | 依赖 0.3 |
| 1.2 | actions、ui Element / 选项 / 构建上下文 | lead | 待领取 | ★★★ |
| 1.3 | node、tree、layout、vector、components/button | lead | 待领取 | ★★★ |
| 1.4 | ui_runtime 构建/协调/patch/事件/帧循环、launch、App trait | lead | 待领取 | ★★★ |
| 1.5 | `zant-demo`（对应 src/main.zig） | lead | 待领取 | |

### M2 组件与运行时补全

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 2.1 | input + runtime 键盘 / IME / 焦点 / 文本选择 | — | 待领取 | |
| 2.2 | toggle、select、image + select 弹层、tooltip | — | 待领取 | |
| 2.3 | ui.zig 其余构建器、core/icons | — | 待领取 | |
| 2.4 | memo、localState、portals、effects、commands | — | 待领取 | |
| 2.5 | menu、modal、segmented、message、shortcut_list、hotkey_input、view、window_controls | — | 待领取 | |
| 2.6 | components/list（虚拟列表）+ ListState | — | 待领取 | |
| 2.7 | 动效接入：presence、FLIP、layout_transition、runtime tick | — | 待领取 | |
| 2.8 | desktop、store、多窗口 | — | 待领取 | |
| 2.9 | examples/{declarative,components,gallery}、gallery.zig、app.zig | — | 待领取 | |
| 2.10 | tests/*_test.zig 对应测试移植 | — | 待领取 | |
| 2.11 | examples/bench.zig → zant-bench；性能闸门 | — | 待领取 | |

### M3 内容模块

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 3.1 | markdown/remend（522 条夹具） | agent-3 | 待领取 | |
| 3.2 | markdown/parser + blocks/cache/html/safety/highlight | agent-4 | 待领取 | |
| 3.3 | markdown/render、editor/edit、widgets 的 markdown 部分 | — | 待领取 | |
| 3.4 | markdown/mermaid | — | 待领取 | 依赖 1.3 |
| 3.5 | math/{parser,symbols,fonts} | agent-5 | 待领取 | |
| 3.6 | math/layout | — | 待领取 | 依赖 1.3 |
| 3.7 | math/mhchem（823 条夹具，逐字节对齐） | agent-6 | 待领取 | |
| 3.8 | pdf/{document,text,renderer,annotations,native}、platform/pdf | — | 待领取 | 依赖 1.1 |
| 3.9 | pdf/viewer、ui.pdfPreview | — | 待领取 | |
| 3.10 | db/sqlite → rusqlite + derive(Table) | agent-7 | 待领取 | |
| 3.11 | examples/{markdown,math,markdown_stream,pdf} | — | 待领取 | |

### M4 TSX 编译器前端

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 4.1 | lexer、ast、jsx_text、jsx_entities | agent-8 | 待领取 | |
| 4.2 | parser、expr_parser、jsx_parser、type_parser、loop_parser、import_parser、react_compat | — | 待领取 | 依赖 4.1 |
| 4.3 | hooks/{expand,rewrite,resolve,leftover} | — | 待领取 | 依赖 4.2 |
| 4.4 | tags、tag_aliases、string_method_names、诊断 | — | 待领取 | |

### M5 TSX 后端与运行时

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 5.1 | 代码生成设计文档 `tsx-codegen.md` | lead | 待领取 | ★★★ |
| 5.2 | zant-macros derive(TsxProps) + zant-tsx-tags + dts | — | 待领取 | |
| 5.3 | codegen 核心（types/scope/expr/stmt/rust_text/builtins/narrowing/functions/loops/aggregate） | — | 待领取 | |
| 5.4 | JSX 生成（jsx/jsx_values/jsx_children/rich_text/list/events） | — | 待领取 | |
| 5.5 | 组件与状态（codegen/components/model/stateful/hooks/stores/array_methods/local_state） | — | 待领取 | |
| 5.6 | 应用层（imports/rust_imports/windows/desktop_hooks/zant_api/assets/entry/compile/main） | — | 待领取 | |
| 5.7 | src/tsx*.zig → zant/src/tsx/ | — | 待领取 | |
| 5.8 | native 模块（trait 化） | — | 待领取 | ★★★ |
| 5.9 | 黄金用例换成 .rs | — | 待领取 | |
| 5.10 | examples/tsx 入口、bench-tsx | — | 待领取 | |

### M6 CLI / 构建 / 脚手架 / 发布

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 6.1 | tools/zant → zant-cli | — | 待领取 | |
| 6.2 | zant-build（tsx_app、图标、manifest、pdfium） | — | 待领取 | |
| 6.3 | scaffold/template-ts | — | 待领取 | |
| 6.4 | package 脚本产物路径 | — | 待领取 | |
| 6.5 | release / install / CI | — | 待领取 | |
| 6.6 | 预编译原生库（可选） | — | 待领取 | |

### M7 下游应用与文档

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 7.1 | ../cliphop-ts native 引擎移植 | — | 待领取 | |
| 7.2 | ../stillmd 移植 | — | 待领取 | |
| 7.3 | 文档重写 | — | 待领取 | |

### M8 切换

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 8 | 全量验证、移动原生源码、删除 Zig、CI、合并 main、发布 | lead | 待领取 | **需用户确认后执行** |

## 待决问题

> 执行者遇到计划没写清的设计问题，登记在这里并停下来等负责人裁定，不要自行发明方案。

（暂无）

## Zig 变更日志

> Zig 版冻结功能开发，只修 bug；每一条都要同步到已移植的 Rust 模块。

（暂无）
