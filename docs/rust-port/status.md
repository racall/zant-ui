# Rust 迁移进度（status）

> 每个执行者领取 / 完成 / 阻塞时更新自己那一行。状态取值：`待领取` / `进行中` / `完成` / `阻塞` / `待验证`。
> 分支：并行期间所有 WP 直接提交到 `feat/rust-port`（多人共用同一工作区，无法为每个 WP 开独立分支），
> 提交信息带 `WP-x.y: ` 前缀，由 lead 审核后提交。

## 当前快照（最近更新）

| 项 | 值 |
|---|---|
| 分支 | `feat/rust-port`（全部 WP 直接合入这里，提交带 `WP-x.y:` 前缀） |
| Rust 测试 | `cargo test --workspace` 全绿：**lib 427 + 集成 66 + zant-tsx 53 + 示例 5 ≈ 551** |
| Zig 测试 | `zig build test` 仍全绿（Zig 版未改行为，只加了 `ZANT_WINDOW_SIZE` 钩子与 `ZANT_STATIC_DATA` 原生补丁） |
| 质量门 | `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets -- -D warnings` 0 告警 |
| 已完成 | M0 全部（0.1 macOS/0.2/0.3/0.4/0.5）、1.1a–d、1.2、1.3、2.2、2.3、2.4、3.1、3.2、3.4、3.5、3.6、3.7、3.8、3.10、4.1、4.2、4.3、5.1 |
| 进行中 | 无（M1 已完成；下一批：2.1 收尾、2.5、2.6、2.7、2.8、2.9、2.10、2.11、3.3、3.9、3.11、4.4） |
| 下一步（依赖已就绪） | 1.4 完成 → 1.5 `zant-demo` + M1 parity → 2.1（input/IME，需 1.4）→ 2.5/2.6/2.7/2.8/2.9/2.10/2.11 → 3.3（markdown render，需 1.3+2.3 ✓ 可并行）→ 3.9（viewer）→ 3.11 → 4.4 → M5/M6/M7/M8 |
| Windows | 测试机 192.168.31.24 全程不可达；所有 Windows 验收待补 |

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
| 0.1 | 建分支、两平台基线测试、ReleaseFast 基线、Windows 机器确认 | lead | 完成（macOS）/待验证（Windows） | feat/rust-port | macOS 基线见上表；Windows 机器不可达，恢复后补 `zig build test` 与工具链检查 |
| 0.2 | Cargo workspace、rust-toolchain、空 crate 骨架、.gitignore、CI rust.yml | lead | 完成（macOS）/待验证（Windows） | feat/rust-port | `cargo build --workspace` 通过；模块树桩文件已预置（子模块声明齐全，并行 WP 不必抢改 mod.rs） |
| 0.3 | `zant-sys`：bindgen 签入 + build.rs 编原生代码 / 链 Skia | lead | 完成（macOS）/待验证（Windows） | feat/rust-port | bindgen 0.73.2 生成 56/50/12 个函数并签入；`cargo test -p zant-sys` 冒烟 2/2 通过（raster 画矩形读像素、TextLayout 量宽度）；Windows 静态数据 `ZANT_STATIC_DATA` 补丁已加但未在 Windows 上验证 |
| 0.4 | `scripts/parity/` 对照工具（capture/compare） | lead | 完成 | feat/rust-port | `capture.sh` + `compare.sh` + `layoutdiff.py` + `imgdiff.py`（纯标准库，无 Pillow 依赖）；Zig 自己对自己：zant-demo 浅/深色 × 600x620/900x640/1200x800 共 6 组，布局 8 个 key 一致、像素 0 差异 |
| 0.5 | `docs/rust-port/{plan,conventions,api,status}.md` | lead | 完成 | feat/rust-port | 另有 `agent-brief.md`（并行执行者工作说明） |

### M1 核心骨架

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 1.1a | geometry、theme、motion、layout/flex + 测试 | wp-1-1a | 完成（已并入） | 17 个测试全过；flex/motion 的 bit-exact 对照 658 行；`tests/flex_test.zig` 24 个 Tree 级用例归 WP-2.10 |
| 1.1b | core/{unicode,paths,hash} | wp-1-1b | 完成（已并入） | 9 个测试；hash 是 Zig Wyhash 4.2 的逐行移植并钉死 20 个 Zig 实测值；keys/text_select 改派 lead |
| 1.1c | renderer/{canvas,text,rich_text,renderer}、image | lead | 完成 | 依赖 0.3；14 个测试（renderer raster/像素、TextLayout 度量/换行/relayout/caret、rich text、image PNG/缩略图/data URI）；`Image` 是唯一 `unsafe impl Send + Sync` 的句柄 |
| 1.1d | platform/{platform,dialog}、core/events、core/keys | lead | 完成 | `Host` 回调表 → `Box<dyn HostHandler>` 蹦床；`ZANT_WINDOW_SIZE` 已照搬；`monotonic_us` 用 `Instant`（deviation）；事件 text 用 owned `String`；14 个测试 |
| 1.2 | actions、ui Element / 选项 / 构建上下文 | wp-1-2 | 完成（已并入） | 39 个新测试；52 字段 Element + 32 个选项结构体逐一对照 Zig；只做 column/row/text/button/styled，其余选项类型已声明 |
| 1.3 | node、tree、layout、components/button、text_select | wp-1-3 | 完成（已并入） | 24 个 flex_test + 25 个新单测；`Tree`/`RetainedNode`/`NodeData`/text 缓存/命中/滚动/焦点/输入编辑；Icon 重复已去重；rich text 选择高亮待接（range_rects 已补） |
| 1.4 | ui_runtime 构建/协调/patch/事件/帧循环、launch、App trait | wp-1-4 | 完成（worktree wp-1.4，待并入） | ★★★；`App`/`RunOptions`/`run()` + `runtime/` 15 个模块；51 个新测试（ui_runtime.zig / launch.zig 无 test 块，全部为新增 headless 测试；zant lib 测试 210 → 261）；Zig 环境变量除 `ZANT_AUTOPLAY`（Zig runtime 不读）与 `ZANT_SNAPSHOT_EXIT`（原生处理）外全部覆盖；多窗口 / caption 按钮 / select 弹层 / input·toggle·select 组件动画留给 WP-2.x；`ui::key_bounds` 用每帧快照而非活树（借用检查） |
| 1.5 | `zant-demo`（= `examples/declarative.zig`，注意不是 src/main.zig） | wp-1-5 | 完成（已并入） | **M1 验收通过**：parity 6/6（light/dark × 3 尺寸），layout 8 keys 一致、像素 0 差异；input 组件 paint 已补 |

### M2 组件与运行时补全

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 2.1 | input + runtime 键盘 / IME / 焦点 / 文本选择（runtime 交互接线收尾） | wp-2-1 | 完成（已并入） | `rust/wp-2.1`；补齐 status.md「已知框架缺口」1–6：toggle/radio/switch 的 BoolAction/IndexAction 派发（pointer click + Space/Enter）、image 路径加载 / 内存源接线、animate tick 覆盖 Input/Toggle/Select、select 弹层 hover/click/键盘 + popup 在 tree paint 之后绘制、rich text 选择高亮、input 助手三处重复合并到 `components/input.rs`。新增 8 个 headless 测试（lib 354，+8）；parity 6/6、0 像素。IME 真机（macOS 拼音）未自动化验证 |
| 2.2 | toggle、select、image + select 弹层、tooltip | wp-2-2 | 完成（已并入），runtime 接线待 1.4 | 32 个测试（含 raster 像素断言）；`Image::draw_rect` 已加；select/tooltip 的浮层生命周期、image load 调用点在 1.4 |
| 2.3 | ui.zig 其余构建器、core/icons | wp-2-3a + wp-2-3b | 完成（已并入） | icons + 30 个 builder（builders/ 拆 8 个文件）；53 个 builder 测试；`ui.icon` 与 Zig 一样生成 Vector 节点（Element 无独立 icon 色/描边字段） |
| 2.4 | memo、localState、portals、effects、commands | wp-2-4（local_state 由 lead 提前完成） | 完成（已并入） | `core/local_state.rs`（Store/Slot/child_id，Rc<RefCell<T>> + TypeId panic，child_id 与 Zig 钉死值一致）+ `ui/local_state` 包装；`ui::{memo,portals,commands}` 与运行时钩子（`memo_hash`/`memo_build`/`resolve_memo`、`take_portals`、`take_commands_for`/`requeue_command`/`set_command_owner`、`set_build_states`）完成；本模块新增 22 个测试 |
| 2.5 | menu、modal、segmented、message、shortcut_list、hotkey_input、view、window_controls | wp-2-5 | 完成（已并入） | 8 个 builder + 选项结构体（含 `TsxProps`）+ 47 个新测试（Zig 0 个 test 块：全部为新增结构 / raster 断言）；`ui::*` 导出待 lead 在 `ui/mod.rs` 加 8 行（见待决问题 4）；`desktop.Shortcut` 子集暂放 `components/hotkey_input.rs`（见待决问题 5） |
| 2.6 | components/list（虚拟列表）+ ListState | — | 待领取 | |
| 2.7 | 动效接入：presence、FLIP、layout_transition、runtime tick | wp-2-7 | 完成（worktree wp-2.7，待并入） | `rust/wp-2.7`。核对结论：presence（enter/exit keep-alive、revive、exit 结束由 tick 摘除并 free）、FLIP（`Tree::layout` 的 `flip_prev`/`flip_from`/`flip_delta` + paint 的旧盒绘制）、`next_frame_delay`、`ZANT_NO_MOTION`/`ZANT_SNAPSHOT`/reduce-motion 开关在基线（1.3/1.4/2.1/2.5）已实现且与 Zig 逐段一致，本 WP 未改生产代码；补齐 `tests/motion_test.zig` 后 6 个 runtime 用例（fake clock，同名同断言）+ 3 个本 WP 用例（`next_frame_delay`、modal/message presence、motion 关闭）。`tests/motion_test.zig` 10 个用例全部有了 Rust 对应（原文件 4 + 新增 6） |
| 2.8 | desktop、store、多窗口 | wp-2-8 | 完成（worktree wp-2.8，待并入） | `desktop/{mod,app,app_event,clipboard,hotkey,tray,window}.rs` + `runtime/window.rs`（共享 `SharedApp`、窗口注册表、`open_window` / 延迟关窗、`invalidateAll`）；`store.rs` 已在基线完成；34 个新测试（lib 346 → 380，全部 headless；真实托盘 / 热键 / 多窗口为手测项）；跨文件小改 3 处：`platform` 安全 app-event 蹦床、`Image::read_rgba`（托盘图标）、`FrameStats::for_window`，见待决问题 4；另 `hotkey_input` 的 `Shortcut` 重复待合并（待决问题 5） |
| 2.9 | examples/{declarative,components,gallery}、gallery.zig、app.zig | — | 待领取 | |
| 2.10 | tests/*_test.zig 对应测试移植 | wp-2-10 | 完成（worktree wp-2.10） | 分支 `rust/wp-2.10`。13 个新文件（`crates/zant/tests/{all,antd_components,component,lifecycle,local_state,mhchem_render,pointer,selection,store,text,text_align,ui,view}_test.rs`），新增 53 个测试；`tests/` 的 Rust 集成测试 72 → 134（含 merge 进来的其他 WP 新用例），全 crate `test -p zant` 595 passed / 0 failed；`clippy -p zant --all-targets -- -D warnings` 通过。Zig 侧非 TSX 测试已全部覆盖：`list_test.zig` 3 条已由 WP-2.6 在 `src/components/list.rs` 内同名移植，`motion_test.zig` 剩余 6 条由 WP-2.7 移植。key-bounds 修复（`cbab28c`）后 `pointer_test.zig` 的 pointer_area 用例已补齐。未移植 1 条：`component_test.zig` 的 gallery 用例（`Zig src/app.zig` 的 gallery 现在是 `zant-examples` 的 `gallery_app`，不是框架 API，不做跨 crate 测试依赖） |
| 2.11 | examples/bench.zig → zant-bench；性能闸门 | — | 待领取 | |

### M3 内容模块

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 3.1 | markdown/remend（522 条夹具） | wp-3-1 | 完成（已并入） | 522/522 夹具通过；5 个 Zig 测试移植 |
| 3.2 | markdown/parser + blocks/cache/html/safety/highlight | wp-3-2 | 完成（已并入） | 23 个内联测试 + 6/12 个 markdown_test（余 6 个需 render/editor → 3.3）；与 Zig 逐字节差分 15.2MB 输出 sha256 一致 |
| 3.3 | markdown/render、editor/edit、widgets 的 markdown 部分 | wp-3-3 | 完成（已并入） | 分支 `rust/wp-3.3`。render/12 文件 + editor/edit/theme + widgets 的 markdown 部分；Zig `edit.zig` 3/3 测试同名移植，`tests/markdown_test.zig` 剩余 6 个渲染/编辑器用例已补，另加 11 个模块单测与 5 个 headless 冒烟（README + 1 万行文档 + memo 跳过）。偏差见执行报告：editor 的模式切换暂用 `ui::button` 组合（`ui::segmented` 属 WP-2.5，缺失）；widgets 的 PDF 半（含其 1 个测试）留给 WP-3.9；`math/mod.zig:inlineDrawing` 暂落在 `render::spans`；`safety::Policy` 缺 `Clone` 由 render/options 补 impl；crate 根 `markdown/mod.rs` 的 re-export 需 lead 统一补 |
| 3.4 | markdown/mermaid | wp-3-4 | 完成（已并入） | 8 测试（Zig 2）；flowchart/sequence/pie + fallback 逐段移植 |
| 3.5 | math/{parser,symbols,fonts} | wp-3-5 | 完成（已并入） | 16 个 Rust 测试（Zig 2）；`parse(&str) -> Result<Vec<Node>, ParseError>`，Node 为 owned enum；symbols/fonts 数据逐条对照 |
| 3.6 | math/layout | wp-3-6 | 完成（已并入） | 6 测试（Zig 6）；入口 `layout(&str, Options) -> Drawing`；18 个示例公式排版非空；`math/mod.zig` 的 UI 层（block/render）留给 2.10/3.11 |
| 3.7 | math/mhchem（823 条夹具，逐字节对齐） | wp-3-7 | 完成（已并入） | 823/823；207 条状态机转移与 Zig 逐条对照；拆成 15 个文件；`to_tex(&str, Machine) -> Result<String, MhchemError>` |
| 3.8 | pdf/{document,text,renderer,annotations,native}、platform/pdf | wp-3-8 | 完成（已并入） | `Document` Send+Sync（原生层已串行化）；渲染 worker 的 wake 用注入回调（3.9 传 `ui::post_wake`）；缓存返回 `Arc` 快照；Windows PDFium 待验证 |
| 3.9 | pdf/viewer、ui.pdfPreview | wp-3-9 | 完成（分支 rust/wp-3.9，待并入） | 7 测试（Zig viewer 2 + 新增 5，pdf_viewer_test.zig 两条延迟用例已移植）；`Viewer` = `Rc<RefCell<Inner>>` 句柄（回调需 `'static` 闭包）；`ui::post_wake` 直接作 renderer wake，无需运行时改动；`on_change`/`on_annotation` 改为下一次 build 的 effect；`ui::segmented`（WP-2.5 未并入）暂用图标按钮，见报告 |
| 3.10 | db/sqlite → rusqlite + derive(Table) | wp-3-10 | 完成（已并入） | 生成的 SQL 与 Zig 逐字节一致（含迁移 ALTER）；Zig 建的库 Rust 可读已实测；fallback 表名差异登记见待决问题 |
| 3.11 | examples/{markdown,math,markdown_stream,pdf} | wp-3-11 | 完成（worktree wp-3.11） | `rust/wp-3.11`。4 个新 bin（`zant-markdown` / `zant-math` / `zant-md-stream` / `zant-pdf`）+ 9 个 headless 测试（markdown 3、math 3、stream 3、pdf 3；components 由 6 → 8）。parity：`zant-markdown` 6/6（0 像素）、`zant-math` 6/6（自定义截图；Zig 示例没有 key，`layout.txt` 为空，`capture.sh` 因此拒绝，见待决问题 9）、`zant-md-stream` 6/6（`ZANT_STREAM_CHARS=1000000 ZANT_STREAM_MS=16 ZANT_STREAM_HISTORY=3`，0 像素）、`zant-components` 6/6（`pdf-2002` 占位换成真 viewer；换之前 layout 6/6 FAIL）、`zant-demo` 6/6。`zant-pdf` 未过：Zig 示例必须带 argv、且 Rust viewer 工具条的批注切换用图标按钮（Zig 用 `ui.segmented`），layout 差 25 项、像素差 0.7–2.0%，全部在工具条，**页面区域与页面瓦片完全一致**（见待决问题 8）。性能（Release，`ZANT_FRAME_STATS=1`，history 8/16）：`[slow-frame]` 只有挂载首帧 29.5 ms（Zig 29.1 ms，两边 `text_layouts` 都是 **244**，history 8→16 内容翻倍仍是 244、不增长），流式帧 1–2 ms（最慢一帧 7.7 ms，Zig 1.3–2.2 ms），build avg 0.06–0.07 ms（Zig 0.04 ms）。偏差：`zant-pdf` 无 argv 时打开 `tests/fixtures/widgets.pdf`（Zig 打印 usage 退出）；`math/mod.zig` 的 UI 层 `block` 落在 example crate；`ui.pdfViewer` 框架侧未移植，example 内同名实现（待决问题 7/8）。 |

### M4 TSX 编译器前端

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 4.1 | lexer、ast、jsx_text、jsx_entities | wp-4-1 | 完成 | `rust/wp-4.1`；10/10 Rust 测试（Zig 可移植 10 个）；AST 字符串用 `Vec<u8>`（见待决问题）；`jsx_entities_test.zig` 另 2 个依赖 parser/compile 的测试待 4.2/4.3 补 |
| 4.2 | parser、expr_parser、jsx_parser、type_parser、loop_parser、import_parser、react_compat | wp-4-2 | 完成（已并入） | 33 测试；`dump_program`（确定性 AST dump，格式见报告）；仓库 32 个 testdata + examples/tsx 全部可解析 |
| 4.3 | hooks/{expand,rewrite,resolve,leftover} | wp-4-3 | 完成（已并入） | 41 个 zant-tsx 测试；5/5 Zig hooks 测试同名移植；真实 tests/tsx/hooks 项目与 Zig 编译器输出逐名对照 |
| 4.4 | tags、tag_aliases、string_method_names、诊断 | wp-4-4 | 进行中（worktree wp-4.4） | |

### M5 TSX 后端与运行时

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 5.1 | 代码生成设计文档 `tsx-codegen.md` | lead | 完成 | ★★★；见 docs/rust-port/tsx-codegen.md（类型映射、非 Copy 一律 clone、props/事件、组件/局部状态、JS 内建语义、黄金用例策略） |
| 5.2 | zant-macros derive(TsxProps) + zant-tsx-tags + dts | — | 待领取 | |
| 5.3 | codegen 核心（types/scope/expr/stmt/rust_text/builtins/narrowing/functions/loops/aggregate） | — | 待领取 | |
| 5.4 | JSX 生成（jsx/jsx_values/jsx_children/rich_text/list/events） | — | 待领取 | |
| 5.5 | 组件与状态（codegen/components/model/stateful/hooks/stores/array_methods/local_state） | wp-5-5 | 完成（分支 rust/wp-5.5） | 入口 `codegen::generate::generate(source_name, &Module, &Options)`（另有 `generate_with(.., install_extra)`）→ `GeneratedFile { source }`（syn + prettyplease 校验/格式化，头部 `@generated` 注释在格式化后拼回）。新增 `generate/components/stateful/hooks/stores/array_methods/string_methods/captures/callbacks/free_names/globals/spread/nesting` 模块；`mod.rs` 加 Generator 生产字段与 WP-5.4/5.6/5.8 扩展钩子。测试：lib 114（+5：free_names 2、nesting 2、string_methods 2、stateful 2、stores 2、globals 2、array 1 等）、集成 `tests/codegen_components.rs` 7 条黄金用例。5.4 未落地：JSX/事件用测试桩注入（`install_extra`），合并时按 `generate::install` 里的注释逐行填 `emit_jsx/scan_body/scan_expr/emit_setter/method_param/...`；细节见待决问题 |
| 5.6 | 应用层（imports/rust_imports/windows/desktop_hooks/zant_api/assets/entry/compile/main） | — | 待领取 | |
| 5.7 | src/tsx*.zig → zant/src/tsx/ | — | 待领取 | |
| 5.8 | native 模块（trait 化） | — | 待领取 | ★★★ |
| 5.9 | 黄金用例换成 .rs | — | 待领取 | |
| 5.10 | examples/tsx 入口、bench-tsx | — | 待领取 | |

### M6 CLI / 构建 / 脚手架 / 发布

| WP | 内容 | 负责人 | 状态 | 备注 |
|---|---|---|---|---|
| 6.1 | tools/zant → zant-cli | wp-6-1 | 完成（worktree wp-6.1，待并入） | 分支 `rust/wp-6.1`。7 个 Zig test 同名移植 + 28 个新增（29 单元 + 6 集成 = 35 全过）；`create`/`sync` 的 stdout/stderr、`app.config.json`、`Info.plist` 与 Zig 版实测逐字节一致；`run --watch` 手工冒烟通过（改 src 后重编译并重启）；`zant build` 走到 `macos.sh developer-id`（DMG 本轮沙箱里 hdiutil 被卡住，见报告）。TSX 编译接口以 `ensure_rust_project` 的明确报错占位，见「M6 接口约定」 |
| 6.2 | zant-build（tsx_app、图标、manifest、pdfium） | — | 待领取 | |
| 6.3 | scaffold/template-ts | — | 待领取 | |
| 6.4 | package 脚本产物路径 | — | 待领取 | |
| 6.5 | release / install / CI | — | 待领取 | |
| 6.6 | 预编译原生库（可选） | wp-6-6 | 完成（worktree wp-6.6，待并入） | 分支 `rust/wp-6.6`。`crates/zant-sys/build.rs`：`ZANT_NATIVE_PREBUILT`（目录或库文件）或 `<root>/third_party/zant-native-prebuilt/` 里有 `libzant_native.a`（macOS）/ `zant_native.lib`（Windows）+ `metadata.txt` 时，只链接归档、跳过 C++/ObjC/C 编译；`metadata.txt` 的 `source_hash` = 规范源清单 `crates/zant-sys/prebuilt/sources-{macos,windows}.txt`（含头文件）按顺序的路径+内容 SHA-256，与树不符/平台不符/无元数据时打印 `zant-sys: ignoring the prebuilt native library: <原因>; compiling the native sources instead` 并回退编译（不静默）；没有归档时行为与 WP-6.6 前一致。该清单同时驱动编译、`rerun-if-changed` 和哈希，三者不会漂移；`tests/prebuilt.rs`（同一份 `prebuilt.rs`）用标准 SHA-256 向量钉死实现。`ZANT_NATIVE_PREBUILT_OUT=<dir>` 让 build.rs 额外把本次对象合并成归档并写元数据（`scripts/release/prebuilt.sh` / `prebuilt.ps1` 调用它，release 构建，归档 212K）。CI：`.github/workflows/release.yml` 新增 `prebuilt` job（macos-15 + windows-2022 各自 fetch Skia → 构建归档 → `cargo test -p zant-sys` 用归档链接并跑冒烟 → 上传 artifact）；macos/windows 打包 job `needs: prebuilt`，下载归档后用 `ZANT_NATIVE_PREBUILT` 传给 `package.{sh,ps1}`，归档进 `zant-ui/third_party/zant-native-prebuilt/`（package 脚本缺归档直接失败，`ZANT_ALLOW_NO_PREBUILT=1` / `-AllowNoPrebuilt` 可跳过；`.sha256` 保留）；`install.sh` / `install.ps1` 只加了说明注释（仍需要集成的 linker；源码仍随包发布以便回退编译）。测试：`crates/zant-sys/tests/prebuilt.rs` 11 条（SHA-256 标准向量/分块、哈希覆盖路径+内容、元数据 round-trip/必需字段、候选与 verify 正反例、平台命名）+ `scripts/release/prebuilt-check.sh` 8 项端到端（构建归档 → 用独立 `shasum -a 256` 复算 source_hash → `ZANT_NATIVE_PREBUILT` 链接 zant-demo 并跑 `ZANT_SNAPSHOT` → 篡改元数据回退编译 → 默认目录布局 → 无归档冷路径 → `cargo test -p zant-sys` 有/无归档），全部通过；`cargo test -p zant-sys` 两种路径下都过（13 tests）。实测（M3 Max / macOS 27.2.1 / rustc 1.94.1）：**删掉原生 artifact 的 release app 构建 8.77s → 5.87s（-2.90s，-33%）**；原生编译本身 release 3.63s → 0.60s、debug 4.13s → 0.69s；全量 `cargo clean` 后 release app 21.43s → 21.37s、debug 8.43s → 8.12s（这台 14 核机器上原生编译与其他 crate 并行，墙钟收益基本被掩盖；Windows/MSVC 预期更大，待验证）。`zant-demo` parity 6/6、0 像素（release 二进制本身即用预编译归档链接）；`cargo clippy -p zant-sys --all-targets -- -D warnings` 与 `rustfmt --check` 干净；`bash -n` 检查三个脚本。待验证：Windows 预编译路径（测试机不可达；release.yml 的 windows prebuilt job 首次运行会验证；`prebuilt.ps1` 头部注明）。偏差/待决：新增任务清单外的文件 `crates/zant-sys/{prebuilt.rs,prebuilt/,tests/prebuilt.rs}` 与 `scripts/release/prebuilt.{sh,ps1,check.sh}`；`sources-*.txt` 取代了 build.rs 里散落的源列表；macOS 归档固定 `MACOSX_DEPLOYMENT_TARGET=11.0`（clang 默认会写构建机 OS 版本）；未改 `.github/workflows/rust.yml`（不在本 WP 范围），建议 lead 在 PR CI 加一步 `scripts/release/prebuilt-check.sh` 防止预编译路径回归。 |

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

## 并行执行与合并方式

- 每个 WP 一份 git worktree：`scripts/worktree.sh new wp-3.1` → `.worktrees/wp-3.1`，分支 `rust/wp-3.1`。
  执行者在 worktree 里提交，lead 逐个 `git merge rust/wp-x.y` 到 `feat/rust-port` 并统一跑
  `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo test --workspace`、`zig build test`。
- worktree 软链共享 `.cargo-home` / `.zig-global-cache` / `third_party`；沙箱里 `cargo` 一律经
  `scripts/cargo.sh`（见 conventions.md）。
- 模块声明（`lib.rs`、各级 `mod.rs`）已在基线里预置，执行者只填自己 WP 的文件；确实要加时在报告里说明。

## WP-2.10 补充待办（由 1.1a 移交）

- `tests/flex_test.zig` 的 24 个用例是 `Tree` / `Node` 级集成测试（`tree.addContainer` / layout / hitTest …），
  依赖 WP-1.3 的保留树，不能放在 1.1a。
- `tests/motion_test.zig` 后 6 个用例依赖 `ui` / `ui_runtime` / renderer / button / segmented
  （WP-1.2 / 1.3 / 1.4 / 1.1c），前 4 个纯 motion 用例已随 1.1a 移植；后 6 个已由 WP-2.7 移植到
  `crates/zant/tests/motion_test.rs`，WP-2.10 不要重复移植。

### WP-2.10 完成情况（wp-2-10，2026-11-13）

- 已移植 53 条（13 个新文件）：`view_test` 7、`selection_test` 6、`lifecycle_test` 6、
  `ui_test` 5、`component_test` 4、`antd_components_test` 12、`local_state_test` 2、
  `pointer_test` 3、`text_test` 3、`text_align_test` 1、`store_test` 1、
  `mhchem_render_test` 1、`all_test` 2（`tests/all.zig` 的 2 条内联用例）。
  其中 `pointer_test.zig` 的 pointer_area 用例在 lead 的 key-bounds 修复（`cbab28c`：快照 hook
  保留到整帧结束）merge 后补上，断言 `on_resize` 里 `ui::key_bounds("area")` 能读到新 bounds。
- 已由其他 WP 覆盖、未重复移植：`flex_test.zig` 24（WP-1.3）、`list_test.zig` 3（WP-2.6，
  在 `src/components/list.rs` 内同名移植）、`motion_test.zig` 前 4 条（WP-1.1a）、
  `markdown_test` / `markdown_render_test` / `math_test` / `pdf*_test` / `db_test` / `remend_test`。
- 仍不能移植（1 条）：`component_test.zig`「gallery button click, input editing, composition and
  focus」：它驱动的是 Zig `src/app.zig` 的 gallery 命令式 `App`（`App.create` / `handle` / `tree`）；
  Rust 侧的 gallery 是 `zant-examples` 的 `gallery_app`（WP-2.9），不是框架 API，`crates/zant/tests`
  不做跨 crate 依赖，故保留未移植（该应用另有 `crates/zant-examples/tests/gallery_smoke.rs` 覆盖）。
- 仍归其他 WP：`motion_test.zig` 后 6 条（WP-2.7）、`tsx*_test.zig`（M4/M5）。

## 已并入 feat/rust-port 的内容（按时间）

1. M0.2 骨架 + 模块树桩、M0.3 `zant-sys`、M0.4 对照工具、M0.5 文档。
2. WP-1.1a geometry/theme/motion/flex、WP-1.1b unicode/paths/hash（Wyhash 4.2 逐行移植 + 20 个 Zig 实测钉死值）、
   WP-1.1c renderer/image、WP-1.1d platform/dialog/events/keys。
3. WP-3.1 markdown/remend（522/522）、WP-3.5 phase 1（math symbols/fonts）、WP-4.1（TSX lexer/AST/JSX text/entities）、
   WP-3.10（db/sqlite + derive(Table)，SQL 与 Zig 逐字节一致）。
4. 早期落地（原属 1.3/2.8，为解除依赖提前做）：`core/vector.rs`、`core/icons.rs`（WP-2.3a）、`store.rs`、`Str`、
   renderer 的 path / simple-text / measure-ink 包装、`lib.rs` 的 `extern crate self as zant;`。

## 已知框架缺口（WP-1.5 parity 发现，2026-11-13）

> 1–6 已由 WP-2.1 修复（见 `rust/wp-2.1`）；记录保留供核对。

1. ~~**toggle/radio/switch 的 on_change 不派发**~~：已修复。runtime `toggle_set` / `toggle_flip` 从节点拷贝出 `IndexAction`/`BoolAction`/`Action` 后按 Zig `Tree.toggleSet` 语义调用（radio 只置 true）；pointer click 与 Space/Enter 共用。
2. ~~**按路径图片未加载**~~：已修复。`runtime/{reconcile,patch}.rs` 的 image 分支调用 `components::image::load` / `set_source`，比较 intrinsic 尺寸决定 `layout_changed`。
3. ~~**animate tick 跳过 Input/Toggle/Select**~~：已修复。`runtime/animate.rs` 调用三者 `animate(...)`，由 `Tree::poke` 只推进变化节点。
4. ~~**select 弹层 option hover/click 仍 TODO**~~：已修复。pointer move/down 用 `components::select::option_at` 命中，`paint_select_popup` 在 tree paint 之后调用 `paint_popup`。
5. ~~**rich text 选择高亮未绘制**~~：已修复。`core/tree/paint.rs` 用 `renderer::rich_text::Layout::range_rects` + 运行时 selection color。
6. ~~**input 编辑助手三处重复**~~：已修复。`core/tree/input_ops.rs` 与 `runtime/ime.rs` 改为调用 / 再导出 `components::input`。

## M6 接口约定（WP-6.1 提出，供 6.2 / 6.3 / 6.5 使用）

1. **TSX 应用编译**（给 6.2）：`zant run/compile/package` 现在只调用 `cargo build`；检测到 TS 项目（`src/App.tsx`）时直接报错，说明期望的接口是应用 `build.rs` 调用
   `zant_build::tsx_app()`（把 `src/**/*.tsx` 生成到 `OUT_DIR`、`src/main.rs` `include!`）。6.2 落地后请删除 `crates/zant-cli/src/main.rs` 的 `ensure_rust_project` 里该分支。
2. **模板目录常量**（给 6.3）：`crates/zant-cli/src/scaffold.rs` 的 `TEMPLATE_ZIG` / `TEMPLATE_TS` 是唯一模板路径；6.3 删除纯 Zig 模板后，把 `TEMPLATE_ZIG` 指向新的 Rust 模板（或让 `create` 默认 `ts` 并删掉 Zig 分支），并同步 `create` 的 `.gitignore` 文案与「下一步」提示。
3. **应用产物名**（给 6.3）：CLI 用 `cargo metadata` 读应用 `Cargo.toml` 的 bin 名，产物 `target/{debug,release}/<bin>`；模板的 `[package] name` 请与期望的二进制名一致。
4. **安装根目录**（给 6.5）：`find_root` 认定发布包布局是 `<home>/bin/zant` + `<home>/zant-ui/Cargo.toml`（zant-ui 为 Rust workspace）；`zant create` 写入的依赖路径是相对项目目录的 path 依赖。
5. **打包脚本路径**（给 6.4）：CLI 仍调用 `tools/zant/package/{macos.sh,windows.ps1}` 的既有 action，ZANT_* 变量名不变；输出目录从 `zig-out/` 改为 `target/`（`ZANT_OUT=target/dist`、`ZANT_DIR=target/<标题>`、临时文件 `target/build/`），脚本里若还写死 `zig-out` 需要同步。

## GUI 真实交互验证（2026-11-14，macOS 登录会话）

用 `scripts/mac/ui-scenario.swift`（本轮扩展：`clickfrac`/`clickat`/`scroll`/`scrollat`/`drag`/`layout`/`cmdkey` 与方向键等）
对 Rust 示例做**真实鼠标/键盘驱动**，证据读取 `[event]` stderr、`ZANT_LAYOUT_FILE` 快照差异与应用快照（系统截图权限被拒，
因此用应用自带 `ZANT_SNAPSHOT` 渲染的 PNG）。

- **zant-components**：38 个真实事件全通过 —— 9 个按钮（含 disabled 正确不触发）、输入框输入中文（逐字符事件）与密码、
  checkbox/switch/radio、select 弹层点选、segmented ×3、tabs ×2、progress ×4、菜单打开/菜单项、modal 打开/关闭、
  message、pointerArea 拖拽（pointer down/up）、虚拟列表点击+滚轮、Markdown 追加、深色切换、窗口 resize；
  弹层状态由 layout 快照证明（menu 85→83 keys、modal 94→93、message 出现 `message-pill`、vlist 24 行）。
  截图：浅/深色与 2500 高全页快照（含真实 PDF 阅读器）已检查。
- **zant-markdown**：真实点击「代码复制」→ 系统剪贴板得到代码块原文；表格 `copy-md`/`copy-csv` → 剪贴板为 Markdown / CSV；
  滚轮滚动全部块。与 Zig parity 6/6、0 像素（WP-3.11）。
- **zant-pdf**（真实 15 页 MEMGPT.pdf）：翻页（layout 页 key 变化）、缩放（页宽 868→1071）、fit 切换、搜索输入+下一个/上一个、
  页码输入跳转、⌘A+⌘C 整页文字进剪贴板（真实 PDF 文本）、批注工具切换与拖拽/撤销/重做。**工具栏改为 `ui::segmented` 后
  Rust/Zig parity 6/6（light/dark × 3 尺寸，layout 33–35 keys 全等、0 像素差）**。
- **zant-md-stream**：流式期间连续滚轮与点击，块 key 随内容增长（60→63→100 keys），完成行
  `[stream] done: 1990 bytes, 322 deltas, 14306 ms, 410 builds, build avg 0.08 ms, max 0.18 ms`。
- **zant-math**：滚动/缩放/resize 交互，应用快照渲染正确；与 Zig parity 6/6、0 像素（WP-3.11）。
- **zant-gallery**：命令式 retained-mode 示例（无 layout key / 事件日志）只能视觉+像素验收（WP-2.9：图像 parity 6/6）；
  如需交互断言，需要给 gallery 加 debug layout dump（未做，非必需）。

## 待决问题

### WP-5.5 提出（2026-11-14）

1. **WP-5.4/5.6/5.8 钩子待接线**：`codegen/generate.rs` 的 `install()` 里已按行列出
   `g.extensions.hooks.emit_jsx/scan_body/scan_expr/emit_setter/method_param/forward_arrow/parameter_type`、
   `extensions.process_imports/collect_windows/app_lifecycle/is_window_const`、`native` 的装配位置；
   `mod.rs` 已加对应扩展字段。合并各 WP 时把这些注释行改成赋值即可（都是单行）。
   在 5.4 落地前，任何渲染 JSX 的模块都会报 `JSX code generation is implemented by WP-5.4`。
2. **子组件回调 prop 的调用需要运行时 `ActionCx`**（设计缺口，需负责人裁定）：Rust 的 `ui::Action`
   只能由运行时用 `ActionCx` 调用；Zig 的 `tsx.Callback` 带根状态指针，允许在 handler 里直接
   `callback.call(...)`。当前实现：签名为 ui 事件（`()/bool/string/usize`）的回调 prop 用
   `ui::Action` 族，子 handler 调用时经 `ui::effect(...)` 延迟一帧执行（能编译、语义等价但晚一帧）；
   非 ui 事件签名（如 `(id: number) => void`）目前报诊断。要彻底解决需要在 `ui` 里增加
   “在 handler 中调用 Action”的能力（或给 handler 传 `ActionCx`）。
3. **`tsx::Store<T>` 的契约**（给 WP-5.7）：生成代码假设 `tsx::Store::new(initial)` 与 `&self` 方法
   `get/read/set/update(impl FnOnce(&mut T))`；每个 `createStore` 生成
   `thread_local!` 静态 + `settings_{get,read,set,update}` 四个 `pub fn`（Rust 无可变 `static`）。
4. **`tsx::strings` 的方法名**（给 WP-5.7）：`codegen/string_methods.rs::rust_name` 固定为
   `starts_with/ends_with/index_of/last_index_of/to_lowercase/to_uppercase/pad_start/pad_end/char_at/char_code_at/code_point_at/replace_all` 等，
   接收 `&str`/`f64`，字符串结果返回 `zant::Str`，`split` 返回 `tsx::List<zant::Str>`。
5. **`scope.CaptureSource` 缺类型与 narrowing**：5.3 的实现只存可见名字，`captures.zig` 需要的
   “外层 context 的类型 + narrowing 事实” 无法取到。WP-5.5 用 `Generator::capture_scope`
   快照（`captures::prepare` 时记录）绕过；代价：narrowing 后的 optional 以声明类型捕获、
   箭头套箭头的捕获不再向上递归。要补需改 `codegen/scope.rs`（5.3 文件）。
6. **`<List>` 的 `ui::ListState` 字段**：`Generator::lists` 目前是 `Vec<String>`（字段名），
   WP-5.4 的 `list.rs` 需要 `jsx`/`item_method`/`id_method`，合并时把该字段扩展成真正的 `ListInfo`。
7. **子组件本地状态的实现形态**：设计文档 §6.2 写 `ChildState` + 回调捕获 `Rc<RefCell<_>>`；Rust 里
   handler 要保持 `&mut self` 形态，所以子组件本地状态槽里放的是**整个子组件实例**（props+state），
   并通过 `_state: Weak<RefCell<Child>>` 让 action 在回调时 upgrade（方法签名不变）。属实现偏差，
   行为一致；如坚持 §6.2 字面形态需改 `scope::state_field` 的前缀规则（5.3 文件）。

> 执行者遇到计划没写清的设计问题，登记在这里并停下来等负责人裁定，不要自行发明方案。

1. **`zant::Error` 尚未定义**（WP-3.5 提出）。裁定：错误类型在 WP-1.2 随 §4 契约落地；
   在那之前各模块可用自己的 `thiserror` 错误类型（如 `math::parser::ParseError`、`db::DbError`），
   合入 `feat/rust-port` 后由 lead 统一收编到 `zant::Error`，不阻塞各 WP。
2. **`tests/flex_test.zig` 的归属**（WP-1.1a 提出）。裁定：归 WP-2.10（见上），1.1a 不实现 Tree。
3. **clippy `module_inception`**：`platform/mod.rs`、`renderer/mod.rs`、`motion/mod.rs` 的模块名与父模块同名
   （对应 Zig 的 `platform/platform.zig` 等）。裁定：保留同名，加 `#[allow(clippy::module_inception)]`，
   让 `-D warnings` 通过。
4. **WP-2.8 的 3 处跨文件小改**（计划里没有预置，缺了桌面集成无法实现；均不改变既有行为）：
   (a) `platform/platform.rs` 新增安全 `AppEventHandler` 蹦床（`install_app_event_handler` /
   `clear_app_event_handler`），因为 `zant_app_set_handler` 是 `unsafe`，而 `desktop` 不允许 `unsafe`
   （plan §3 rule 5）；(b) `image.rs` 新增 `Image::read_rgba`（托盘图标要把解码后的像素交给
   `platform::set_tray`，该 C 函数在 `zant-sys` 里只有 `unsafe` 声明，`desktop` 不能直接调）；
   (c) `runtime/debug.rs` 新增 `FrameStats::for_window`（第二个窗口的 `ZANT_FRAME_STATS` 标签；
   `FrameStats` 字段私有）。三处都加了对应测试。
5. **`Shortcut` 有两份**（WP-2.5 提出，WP-2.8 已实现完整版）：WP-2.8 把 `Shortcut` 放在
   `crate::desktop`（`label` / `format` / `parse` / `from_key_event` / `eql` / `register_hotkey`，
   Zig 同名方法齐备）。WP-2.5 在 `components/hotkey_input.rs` 里先移植了 recorder 需要的子集；
   两者合并后让 `hotkey_input` 改为 `use crate::desktop::Shortcut`（字段与语义一致，desktop 版多
   `format` / `parse` 与注册函数）。
6. **多窗口的关闭是延迟的**（WP-2.8 deviation）：Zig `Window.close` 在回调里直接销毁运行时；
   Rust 版先向原生窗口发关闭请求，`WindowClose` 事件入队，主窗口的 wake 回调里再
   `process_pending_closes`（避免在窗口自己的事件回调里销毁原生窗口）。`close()` 立即生效的时机
   是"当前事件回调返回后的下一个事件循环轮次"；`open_window` 在没有 `ui::app::run` 主窗口时返回
   `PlatformError::UnsupportedPlatform`（Zig 是编译期就必须在 run 之后调用）。
7. ~~**`ui::key_bounds` 在 `on_resize` 里返回 `None`**~~（WP-2.10 提出，已修复 `cbab28c`）：Zig 的
   `ui.keyBounds` 读活树，所以 `BoxOptions::on_resize` 回调里能拿到控件的新 bounds；Rust
   `runtime::build_tree` 原本在 build 末尾 `set_key_bounds_hook(None)`，而 `notify_resizes`（触发
   `on_resize`）在随后的 `render_frame` 里才跑，hook 已清空 → `None`。lead 采用方案 (a)：build
   结束不再清 hook，快照在每次 layout 后由 `refresh_key_bounds` 刷新、runtime drop 时才清，整帧内
   `ui::key_bounds` 都可用。WP-2.10 的 `pointer_test.rs` 已补上原先被阻塞的 pointer_area 用例并通过。

4. **组合组件未从 `ui::*` 导出**（WP-2.5 提出）：8 个组件按计划落在 `crates/zant/src/components/*.rs`，
   `components/mod.rs` 已预先声明模块；但 `crates/zant/src/ui/mod.rs` 属于 WP-1.x 的文件，WP-2.5 不改。
   裁定建议：lead 在 `ui/mod.rs` 的 `pub use` 区块追加
   `pub use crate::components::menu::{dropdown_menu, MenuItem, MenuItemKind, MenuOptions};`、
   `modal::{modal, ModalOptions}`、`segmented::{segmented, SegmentedOption, SegmentedOptions}`、
   `message::{message, MessageKind, MessageOptions}`、
   `shortcut_list::{shortcut_list, Shortcut, ShortcutGroup, ShortcutListOptions}`、
   `hotkey_input::{hotkey_action, hotkey_input, HotkeyAction, HotkeyInputOptions, Shortcut}`、
   `view::{view, ViewOptions}`、`window_controls::{window_controls, WindowControlsOptions, CAPTION_MAXIMIZE_KEY}`，
   使 Rust 侧与 Zig 的 `ui.modal` / `ui.dropdownMenu` / … 同名可用（当前可经 `components::…` 使用）。
5. **`desktop.Shortcut` 的临时位置**（WP-2.5 提出）：`hotkey_input.zig` 依赖 `desktop.zig` 的 `Shortcut`
   （WP-2.8 才移植）。WP-2.5 在 `components/hotkey_input.rs` 内先移植了 recorder 需要的子集
   （`fromKeyEvent` / `isValid` / `eql` / `label`，含平台 key 白名单与 macOS / Windows 标签）；
   `format` / `parse` 留给 WP-2.8。裁定建议：WP-2.8 把 `Shortcut` 放到 `crate::desktop`，并让
   `hotkey_input` 改为 `use crate::desktop::Shortcut;`（或在 desktop 里 `pub use` 本模块的类型），
   字段名与语义不变。

7. **`math/mod.zig` 的 UI 层未移植**（WP-3.11 提出）：Zig `math/mod.zig` 的 `block` / `render` /
   `inlineText` / `inlineDrawing` 属 UI 层，WP-3.6 只移植了 `layout` 引擎（`math::layout`）。
   WP-3.11 在 `crates/zant-examples/src/math_app.rs` 里按 Zig 逐行实现 `math_block`（display、
   上下各半 em 边距、100% 宽居中 row，见 `zant-math` 测试），`inlineDrawing` 已在 WP-3.3 的
   `markdown::render::spans::inline_drawing`。裁定建议：把 `block` / `render` 移进 `crate::math`
   （约 40 行，example 改调用 `zant::math::block`）；`zant-math` parity 已证明绘制结果与 Zig 逐像素一致。
8. **`ui.pdfViewer` 组件与 PDF 工具条 segmented**（WP-3.11 提出）：`src/components/widgets.zig` 的
   PDF 半（`PdfViewerOptions` / `PdfViewerState` / `pdfViewer`，含 `password identity` 测试）没有任何
   WP 认领（WP-3.3 声明留给 3.9，3.9 只做了 viewer）。WP-3.11 在
   `crates/zant-examples/src/components_app/pdf_viewer.rs` 里逐行实现同名组件（local slot + 惰性 open
   effect + empty/loading/failed 表面 + `pdf::view` 透传），`zant-components` parity 6/6，并且
   `pdf-2002-zin` 的 on_change 事件恢复（`components-check.ps1` 的第 162–163 行可用）。
   同时 3.9 的 `pdf/viewer/annotate.rs::tool_switch` 用图标按钮替代了 Zig 的 `ui.segmented`
   （当时的 base 没有 2.5 的 `ui::segmented`；本 base 已有）。后果：`zant-pdf` layout 差 25 项
   （Zig `pdf-tools*` 11 个 key vs Rust `pdf-tool0..4`，另 8 个 x 偏移）、像素差 0.7–2.0%，差异全部
   在工具条（y ≤ 70）；`pdf-p0*` 页面瓦片坐标与像素完全一致。裁定建议：(a) 把 `pdf_viewer` 移入
   `crate::components::widgets`（+ 其 1 个测试）；(b) `tool_switch` 改成 `ui::segmented`
   （Zig 原样：key `{key}-tools`、`height: 24`、`font_size: 12`，5 个 label/icon，`on_change` → `tb_tool`），
   之后 `zant-pdf` 应能过 parity。
9. **`zant-math` / `zant-pdf` 的 parity 调用方式**（WP-3.11 记录，非待决）：`zant-math` 的 Zig 示例
   没有任何 key，`ZANT_LAYOUT_FILE` 是空文件，`capture.sh` 的 `-s layout.txt` 检查会失败 → 用同一套
   `ZANT_SNAPSHOT*` / `ZANT_WINDOW_SIZE` 环境手动截图并只跑 `imgdiff.py`（本次 6/6、0 像素）；
   `zant-pdf` 的 Zig 示例必须带 argv（`zig-out-baseline/bin/zant-pdf tests/fixtures/widgets.pdf`），
   `compare.sh` 不支持传参 → 手动跑 `capture.sh` 的同一套环境变量并分别 `layoutdiff.py` / `imgdiff.py`。
   `zant-gallery` 的 Zig 基线不写 `layout.txt`（imperative 树没有 key），`compare.sh zant-gallery`
   在 capture 阶段就失败；与 3.11 无关，留给 lead 决定是否给 gallery 加 key 或换对照方式。

## Zig 变更日志

> Zig 版冻结功能开发，只修 bug；每一条都要同步到已移植的 Rust 模块。

1. `src/platform/platform.zig`：`createWindow` 新增 `ZANT_WINDOW_SIZE=WxH` 自动化钩子
   （只覆盖初始窗口尺寸），供 `scripts/parity/capture.sh` 在固定尺寸下截图对照。
   不影响正常使用；Rust 版 `platform/createWindow` 要照搬同一行为（WP-1.1d）。
2. `src/renderer/skia/skia_bridge_win.cpp` + `skia_bridge.h`：新增
   `zant_set_icu_data` / `zant_set_math_font`，用 `#ifdef ZANT_STATIC_DATA` 包住；
   Zig 的 DLL 构建不定义该宏，行为不变（WP-0.3）。
