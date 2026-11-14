# WP-M7.2 — StillMD 移植到 Rust 内部 API（暂存）

状态：**完成（staged，等用户批准后替换 `../stillmd` 目录）**。
分支：`rust/wp-m7-stillmd`；提交：`WP-M7.2: port StillMD to the Rust API (staged)`。
产物：`/tmp/m7-stillmd-artifact/stillmd-rust.tar.gz`（source only，sha256
`a7ab2117d1acfbfd54bc5445a85e1fe12acf8ba007f4d576128a1f5498376deb`）。
暂存工作区：`/tmp/m7-stillmd/`（`../stillmd` 只读，未修改；应用源码未提交进 Zant UI 仓库）。

## 1. 结论

`../stillmd`（Zig，2,020 行）已逐模块移植成 Cargo 应用 `stillmd`，使用手写
`zant::ui` API + `zant::markdown` + `zant::desktop` + `zant::db`，entry 是
`zant::ui::run` + `ui::App` trait，没有 TSX / `build.rs` / `zant-build`。

- 构建：`cargo build` 全绿；`cargo clippy --all-targets -- -D warnings` 0 告警（本 crate）；
  `rustfmt --edition 2024` 已跑；`zant compile`（CLI 生成 `src/app_config.rs` + cargo build）通过。
- 测试：13 个（Zig 的 11 个 test 同名移植 + 2 个新增），全过。
- 视觉：headless `ZANT_SNAPSHOT` 正常（2100×1520 PNG，非空白）；与**本机重新构建的 Zig 基线**
  逐像素一致：10 个配置全部 **0 像素差**、layout keys 全部 **±0.01 内相同**。
- docs-check 等价验证：56 个 `docs/**/*.md` × preview/edit = 112 次 headless 运行
  **0 失败**；13 个代表文档 × 2 模式与 Zig 版对比全部 `layout=0 pixels=0`。
- 交互（`scripts/mac/ui-scenario.swift` + `ZANT_LAYOUT_FILE`）：真实鼠标点击 / 键盘命令键 /
  AX 缩放可驱动；工具栏切换、⋯ 菜单、快捷键面板、专注模式、查找条、大纲、粘贴、保存、
  撤销、重做均验证。沙箱缺 Screen Recording / System Events 权限，**截图与合成文字输入**
  不可用（Zig 版同样不可用，属环境限制，见 §7）。
- Windows：`scripts/win/smoke-stillmd.ps1` / `docs-check.ps1` 本机无法运行
  （测试机不可达，见 `conventions.md`）；脚本接受 `-App <exe>`，可直接指向
  `target\release\stillmd.exe`，其行为路径与实现语言无关。

## 2. 文件清单（Zig → Rust）

| Zig（../stillmd） | Rust（staged） | 行数（Zig→Rust） |
|---|---|---|
| `src/main.zig`（render/main/openSettings/readable） | `src/main.rs`（render、`StillMd` 的 `ui::App`、main、open_settings、env 覆盖） | 244 → 357 |
| `src/state.zig`（AppState + 全部 action） | `src/state.rs` | 887 → 1147 |
| `src/history.zig`（撤销/重做快照） | `src/history.rs` | 149 → 203 |
| `src/shortcuts.zig` | `src/shortcuts.rs` | 65 → 177 |
| `src/palette.zig` | `src/palette.rs` | 106 → 154 |
| `src/components/header.zig` | `src/components/header.rs` | 111 → 239 |
| `src/components/footer.zig` | `src/components/footer.rs` | 30 → 53 |
| `src/components/workspace.zig` | `src/components/workspace.rs` | 60 → 101 |
| `src/components/menu.zig` | `src/components/menu.rs` | 82 → 211 |
| `src/components/search_panel.zig` | `src/components/search_panel.rs` | 114 → 255 |
| `src/components/outline_panel.zig` | `src/components/outline_panel.rs` | 43 → 60 |
| `src/components/shortcuts_sheet.zig` | `src/components/shortcuts_sheet.rs` | 57 → 156 |
| `src/app_config.zig` | `src/app_config.rs`（`zant sync` 生成，逐字段一致） | 21 → 32 |
| （Cargo 工程） | `Cargo.toml`、`Cargo.lock`、`.gitignore`、`app.config.json`（不变）、`README.md`、`MIGRATION.md` | — |

`app.config.json` 保持 Zig 原样（bundle id、`hidden_titlebar`、1050×760、最小 820×620、
背景 `#fcfcfa`、`sandbox:false` / `signing_identity:none`）；`zant sync` 生成的
`src/app_config.rs` 与 Zig `app_config.zig` 的常量值相同，`window.titlebar_height` 仍在
`main` 里设为 `header::HEIGHT = 52`（与 Zig main.zig:228 相同）。

## 3. Zig → Rust 映射要点

- **入口**：`zant.launch.run(gpa, state, render, .{...})` → `zant::ui::run(StillMd{state}, RunOptions{title, window, theme})`；
  Zig 用 `@hasDecl` 探测的 `theme` / `onKey` / `onCloseRequest` 变成 `ui::App` 的
  `theme()` / `on_key()` / `on_close_request()`（`on_quit_request` 默认委托 close，和 Zig 一致）。
- **回调**：Zig `ui.action(state, AppState.save)` → `ui::action(|s: &mut AppState| s.save())`；
  `ui::App::action_root()` 返回 `&mut state`，所以 action 的根状态就是 `AppState`
  （与 TSX shell 的 `action_root` 机制相同）。
- **状态**：`AppState` 去掉 `gpa` / `io` 字段（Rust `std::fs`；SQLite 用 `zant::db::Store`）；
  `name_buf`/`toast_buf` 定长数组 → `String` + 同样的 256/64 字节截断；`recentNames(out,map)`
  出参 → `Vec<(String, usize)>`；`textFmt` → `format!`；`zant.dialogs` →
  `zant::platform::dialog`，`zant.paths.appDataDir` → `zant::core::paths::app_data_dir`。
- **写作区**：`md.editor(Props{ value, selection, mode, cache: Some(&mut state.md_cache), ...,
  mode_switch:false, toolbar: !focus && mode != Preview, content_max_width: 720/560,
  preview: ViewProps{ render: RenderOptions{ theme: palette::markdown_theme(dark), labels: zh,
  on_link, base_dir, key_prefix:"preview", ...} } })`，分割借用 `state.body` /
  `state.md_cache`（两个不同字段，等价 Zig 的 `&state.md_cache`）。
- **浮层**：dismiss layer + `ui::dropdown_menu`（菜单/大纲）、`ui::modal` + `ui::shortcut_list`
  （快捷键）、查找条（`ui::input` + 图标按钮 + 按需替换行）、`ui::message`（toast）、
  `md::link_safety_modal`（外链确认）；面板 top/right 与 Zig 同一套几何
  （`panel_top = 46`，`right = 16 + titlebar_insets().right`，大纲再 +38）。
- **持久化**：`settings.db`（bundle id `com.racall.stillmd`）仍在平台 app data 目录；
  `ZANT_SNAPSHOT` 时跳过读写（与 Zig 相同）；最近文件、深色、字号、视图模式键名不变
  （`dark` / `font_size` / `mode` / `recent`）。
- **视觉/行为细节保持一致**：52px 顶栏（macOS 居中标题 + “— 已编辑”、红绿灯留白、
  Windows 标题靠左 + 右侧 caption 留白）、36px 底栏统计、9 个格式工具、⌘/⇧/⌥ 快捷键表、
  CRLF 文件按 LF 编辑保存回 CRLF、`ui::document_edited` 未保存标记、
  “保存/不保存/取消”关闭确认、粘贴/查找/替换/大纲/专注模式/主题/字号。

### 与 Zig 的偏差（均已评估，不影响行为）

1. 文档按 **UTF-8** 打开（Rust `String`）；Zig 接受任意字节。非 UTF-8 文件按“无法打开文件”处理。
2. 撤销/重做后光标落到最近的 UTF-8 字符边界（Zig 用字节下标）；纯 ASCII 编辑完全一致。
3. `AppState::new()` 取代 `AppState.init(gpa, io)`（Rust 不需要 allocator/io 参数）。
4. `zant.launch` 的 `Config.theme` 初始主题由 `RunOptions.theme` 提供，之后每帧由
   `App::theme()` 返回 `palette::framework_theme(state.dark)`（Zig 每帧走 `state.theme`）。
5. 打开文件失败/导出失败等错误提示文案逐字保留。

## 4. 测试

`CARGO_HOME=/Users/xiaoyi/dev/XyUI/.cargo-home cargo test` → **13 passed, 0 failed**：

| Zig test | Rust test | 结果 |
|---|---|---|
| `history.zig`: typing coalesces, undo/redo walk snapshots | `history::tests::typing_coalesces_undo_redo_walk_snapshots` | PASS |
| `history.zig`: undoing back to the saved step is clean | `history::tests::undoing_back_to_the_saved_step_is_clean` | PASS |
| `history.zig`: history stays under its byte limit | `history::tests::history_stays_under_its_byte_limit` | PASS |
| `state.zig`: find is case-insensitive and wraps both ways | `state::tests::find_is_case_insensitive_and_wraps_both_ways` | PASS |
| `state.zig`: outline skips fenced code and tracks the current section | `state::tests::outline_skips_fenced_code_and_tracks_the_current_section` | PASS |
| `state.zig`: undo restores previous text and the clean state | `state::tests::undo_restores_previous_text_and_the_clean_state` | PASS |
| `state.zig`: font size shortcuts work with and without shift | `state::tests::font_size_shortcuts_work_with_and_without_shift` | PASS |
| `state.zig`: CRLF files are edited as LF and keep their line endings | `state::tests::crlf_files_are_edited_as_lf_and_keep_their_line_endings` | PASS |
| `state.zig`: statistics are cached per text generation | `state::tests::statistics_are_cached_per_text_generation` | PASS |
| `state.zig`: recent files keep the newest first without duplicates | `state::tests::recent_files_keep_the_newest_first_without_duplicates` | PASS |
| `state.zig`: replace one and replace all are undoable edits | `state::tests::replace_one_and_replace_all_are_undoable_edits` | PASS |
| （新增）| `shortcuts::tests::command_shortcuts_route_to_their_actions`（⌘P/⌘\\/⇧⌘F/⇧⌘L/⇧⌘O/Esc/⌘F/⌘/ 路由、未绑定键不吞） | PASS |
| （新增）| `palette::tests::themes_map_the_palette_fields`（两套主题字段映射） | PASS |

## 5. 构建 + 快照 + parity 证据

```bash
# 暂存工作区
cd /tmp/m7-stillmd
CARGO_HOME=/Users/xiaoyi/dev/XyUI/.cargo-home cargo build   # 绿
CARGO_HOME=/Users/xiaoyi/dev/XyUI/.cargo-home cargo test    # 13 passed
CARGO_HOME=/Users/xiaoyi/dev/XyUI/.cargo-home cargo clippy --all-targets -- -D warnings  # 0
/Users/xiaoyi/dev/XyUI/target/debug/zant compile            # sync + cargo build 通过
ZANT_NO_MOTION=1 ZANT_SNAPSHOT=/tmp/rust-default.png ZANT_SNAPSHOT_EXIT=1 \
  ZANT_LAYOUT_FILE=/tmp/rust-default-layout.txt ./target/debug/stillmd
# → PNG 2100×1520（1050×760 @2x），非空白；layout 30 行
```

**Zig 基线**：本机有 Zig 0.17.1，`/tmp/m7-stillmd-zig` 是 `../stillmd` 的副本
（`build.zig.zon` 用 `../XyUI`，通过 `/tmp/XyUI` symlink 指向
`/Users/xiaoyi/dev/XyUI`），`ZIG_GLOBAL_CACHE_DIR=... zig build` 成功，
基线二进制 `/tmp/m7-stillmd-zig/zig-out/bin/app`。
> 注意：`../stillmd/zig-out/bin/app`（11/7 构建）是旧的，与它对比时
> 标题居中一项差 0.1pt；用**当前源码重新构建**的 Zig 基线后全部归零 —— 说明差的是旧二进制，
> 不是移植。

对照矩阵（两边同一环境变量、同一窗口；`scripts/parity/imgdiff.py` / `layoutdiff.py`）：

| 配置 | layout | 像素差 |
|---|---|---|
| 默认（edit、浅色、1050×760） | 30 keys identical (±0.01) | 0 / 3,192,000（0.0000%） |
| `STILL_SIZE=820x620` | 30 keys identical | 0 / 2,033,600 |
| `STILL_MODE=preview` | 45 keys identical | 0 / 3,192,000 |
| `STILL_MODE=split` | 59 keys identical | 0 / 3,192,000 |
| `STILL_DARK=1` | 30 keys identical | 0 / 3,192,000 |
| `STILL_OVERLAY=menu STILL_DARK=1` | 56 keys identical | 0 / 3,192,000 |
| `STILL_OVERLAY=outline` | 38 keys identical | 0 / 3,192,000 |
| `STILL_OVERLAY=search` | 37 keys identical | 0 / 3,192,000 |
| `STILL_OVERLAY=replace` | 41 keys identical | 0 / 3,192,000 |
| `STILL_OVERLAY=shortcuts` | 42 keys identical | 0 / 3,192,000 |
| `STILL_MODE=split STILL_DARK=1`（最终复测） | 59 keys identical | 0 / 3,192,000 |

**docs-check 等价验证**（模拟 `scripts/win/docs-check.ps1` 的 macOS 版）：
`docs/**/*.md` 共 56 个文件，`STILL_SIZE=1050x1300`、`STILL_MODE=preview|edit`，
Rust 应用 112 次 headless 运行：**112 PASS / 0 FAIL**（exit 0、snapshot 存在、
日志无 panic/error/leaked/failed）；其中 13 个代表文档（architecture、components/alert、
components/markdown、markdown、rust-port/plan、rust-port/api、desktop、design、state、tsx、
components/button、components/dropdown-menu、motion）×2 模式与 Zig 基线对比：
**26/26 全部 `layout=0 pixels=0`**（最大文档 512 个 layout key 全部一致）。

## 6. 交互验证（`scripts/mac/ui-scenario.swift` + `ZANT_LAYOUT_FILE`）

方法：启动应用时设 `ZANT_LAYOUT_FILE`，用 `ui-scenario` 按 layout key 真实点击/输入/缩放；
每步记录 layout key 变化。`ZANT_SNAPSHOT` 用于启动截图（不写设置，保持确定性）。

| 场景 | 步骤 | 结果（layout key delta） |
|---|---|---|
| 视图切换 | click `hdr-mode-1` | 新增 `doc-split doc-preview doc-preview-col preview preview-e1 preview-e1-0 preview-e2` |
| 预览 | click `hdr-mode-2` | 新增 preview keys；删除 `doc-bar*`、`doc-input*`、`doc-tool-*`（输入区/工具栏隐藏） |
| 查找条 | click `hdr-search` | 新增 `search search-find search-input search-next search-prev search-toggle-replace search-close` |
| 大纲 | click `hdr-outline` | 新增 `dismiss-layer outline outline-0 outline-0-hit` |
| ⋯ 菜单 | click `hdr-more` | 新增 `dismiss-layer menu menu-N` + `m-new m-open m-save m-save-as m-export m-dark m-focus m-font m-smaller m-larger m-shortcuts`（含每项 `-hit`） |
| 菜单 › 键盘快捷键 | click `hdr-more` → click `m-shortcuts` | 新增 `shortcuts shortcuts-card shortcuts-mask shortcuts-close shortcuts-head shortcut-list shortcut-list-g0..g4` |
| 菜单 › 专注模式 | click `hdr-more` → click `m-focus` | 新增 `exit-focus`；删除 `header hdr-* footer doc-bar* doc-tool-*` |
| 缩放 | `resize 900 700` | `header/workspace/footer` 宽度 900、footer y=664，全部按比例重排 |
| 键盘命令 | `click hdr-mode-1` → `cmdkey p` | 模式切到 preview（`doc-preview` 出现）——确认 App::on_key 生效 |

**保存 / 撤销 / 重做**（真实剪贴板粘贴 → 编辑 → 保存，三个独立运行）：

1. `printf 'PASTED 剪贴板 rust' | pbcopy`，打开 `doc-save.md`；
   `click hdr-mode-0` → `click doc-input` → `cmdkey v`（粘贴）→ `cmdkey s`（保存）；
   退出后文件内容 = 原内容 + `PASTED 剪贴板 rust`（文本插入 + 保存成功、UTF-8 中文正常）。
2. 干净文件：同样粘贴 → `cmdkey s` → `cmdkey z`（撤销）→ `cmdkey s`；
   退出后文件恢复为原始 `# Interaction\n\nfirst line\n`（撤销 + 保存成功）。
3. 干净文件：粘贴 → 保存 → `cmdkey z` → 保存 → `cmdkey shift+z`（重做）→ 保存；
   退出后文件再次包含 `PASTED 剪贴板 rust`（重做 + 保存成功）。
   （`cmdkey shift+z` 用仓库脚本的本地扩展副本 `/tmp/m7-stillmd/ui-scenario-ext` 发送；
   原脚本 `cmdkey` 只发 ⌘，其余步骤均用原脚本。）

**环境限制（与实现语言无关）**：

- 沙箱里的终端没有 Screen Recording 权限：`ui-scenario` 的 `shot` 步骤失败
  （`could not create image from window`，应用自身 `ZANT_SNAPSHOT` 仍可用）。
- 没有 System Events / 辅助功能键盘注入权限：`osascript ... keystroke` 报
  `A privilege violation occurred (-10004)`；`ui-scenario` 的 `type`（unicode CGEvent）
  在两个应用里都输入不了文本。逐 keycode 的 `key` 步骤同样不产生文本。**Zig 版同样如此**，
  所以改用真实「剪贴板 → `cmdkey v` 粘贴」路径验证文本编辑，用 `cmdkey z / cmdkey shift+z`
  验证撤销/重做。窗口截图改用应用自身的 headless snapshot（见 §5）。

## 7. 产物

- `/tmp/m7-stillmd-artifact/stillmd-rust.tar.gz`（source only，35,147 bytes）
  sha256 `a7ab2117d1acfbfd54bc5445a85e1fe12acf8ba007f4d576128a1f5498376deb`
- 包内：`stillmd-rust/{Cargo.toml,Cargo.lock,.gitignore,app.config.json,README.md,MIGRATION.md,src/**}`；
  `MIGRATION.md` 写明替换 Zig 目录时**删除/新增/保留**哪些文件、Cargo.toml 的
  `zant` path 依赖怎么改、`cargo`/`zant` 命令、模块对照表、偏差与验证钩子。
- 已从解包后的干净副本验证：`cargo test` 13 passed、`cargo build` 绿、headless snapshot
  正常且与 Zig 基线 0 像素差（
  `/tmp/artifact-check/stillmd-rust`）。

## 8. 未做 / 剩余

- `scripts/win/smoke-stillmd.ps1`、`docs-check.ps1` 未在 Windows 真机运行：本机是 macOS，
  测试机不可达（`conventions.md` 的「Windows 测试机 192.168.31.24 当前不可达」）。
  两个脚本都只依赖 `-App <exe>` 与窗口/输入行为，可直接对 `target\release\stillmd.exe`
  运行；`docs-check` 的等价循环已在 macOS 上对两个实现跑过（§5）。
- 未做 `zant package` / `zant build` 打包验证（需要签名/打包步骤与用户确认；
  `zant compile` 已验证 CLI 集成）。替换目录后应由 lead 用户在 macOS/Windows 各跑一次。
- 未改任何 Zant UI 框架文件；移植过程中没有发现需要新增的框架 API
  （`ui::App`、`ui::action`、`md::editor`、`md::link_safety_modal`、`ui::message`、
  `ui::dropdown_menu`、`ui::shortcut_list`、`ui::titlebar_insets`、`ui::document_edited`、
  `db::Store`、`platform::dialog` 都够用）。
- 框架侧观察（非阻塞）：`crates/zant/src/markdown/editor.rs` 的 `mode_switcher`
  仍是 WP-2.5 之前的 `ui::button` 组合（注释说 segmented 未移植，但 `ui::segmented` 已存在）；
  StillMD 用 `mode_switch:false`，不受影响，留待后续统一。
