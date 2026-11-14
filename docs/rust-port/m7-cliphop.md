# WP-M7.1：ClipHop 原生引擎 Zig → Rust（staged）

**状态**：引擎移植与验证完成；**应用要能构建，需要先合入本文「框架修复」里的 `zant-tsx`
补丁**（当前 `feat/rust-port` 编译器还编不过 ClipHop 的 TSX）。迁移产物在
`/tmp/m7-cliphop-artifact/cliphop-ts-rust.tar.gz`（sha256 见下），应用源码**没有**写进
`../cliphop-ts`，等用户确认后由 lead 按产物里的 `MIGRATION.md` 应用。

- 产物：`/tmp/m7-cliphop-artifact/cliphop-ts-rust.tar.gz`
  sha256 `46e492fb0d3f39217c2712a047da48d453065fc63dc499fb53f8d621f846c881`（565 KB，
  内容为 `m7-cliphop-clean/`，含 `MIGRATION.md` 与 `zant-fix.patch`，无 `target/`、无 `.git/`）。
- TSX 与规格文件**逐字节未改**：`diff -r src/`（除新增 `main.rs`、`app_config.rs`）为空；
  `native/*.ts` 6 个文件 `cmp` 全部相同。
- 预期落点：`go.mod`/`build.zig` 形态换成 `scaffold/template-ts` 的 Cargo 应用形态
  （`Cargo.toml` + `build.rs` → `zant_build::tsx_app()` + `src/main.rs` + `native/*.rs` +
  `native/spec.rs` 生成 + `app.config.json` + `assets/`）。

## 1. 逐模块移植

Zig 源 3,508 行（`native/*.zig` 793 + `native/engine/*.zig` 2,678 + `native/tests.zig` 188
中的可执行部分），Rust 4,940 行（其中测试与注释按 1:1 保留）。

| Zig | Rust | 状态 |
|---|---|---|
| `native/capture.zig`（133） | `native/capture.rs` | 完整；worker 线程 + 互斥交接 + `poll` 一致 |
| `native/clips.zig`（177） | `native/clips.rs` | 完整；快照字段/`preview`/`linkHost`/`filesTitle`/缩略图一致 |
| `native/events.zig`（41） | `native/events.rs` + `spec::events` | 完整；`thread_local` 队列、破坏性 drain |
| `native/lan.zig`（155） | `native/lan.rs` | 完整；`thresholds`/`maxBytes`/peer/progress/发送/接收 |
| `native/session.zig`（37） | `native/session.rs` | 完整；`start`/`stop`/`running`/`dataDirectory` |
| `native/settings.zig`（71） | `native/settings.rs` | 完整；`validName` 逐条一致；JSON 字段与 Zig 相同 |
| `engine/backend.zig`（171） | `engine/backend.rs` | 完整；单例从 Zig 全局指针改为 UI 线程 `thread_local` |
| `engine/history.zig`（351） | `engine/history.rs` | 完整；同一内容去重、limit/trim、文件共享、搜索 |
| `engine/storage.zig`（258） | `engine/storage.rs` | 完整；`zant::db`（`#[derive(Table)]`）与 Zig 表结构一致；旧 `cliphop.json` 一次性导入；损坏库 `.bak` |
| `engine/files.zig`（257） | `engine/files.rs` | 完整；wyhash（`zant::core::hash::Wyhash`，与 Zig 同值）+ 缩略图 worker + `prune` |
| `engine/format.zig`（116） | `engine/format.rs` | 完整；`charCount`/`linkHost`/`bytes`/`preview`/`ago`/`filesTitle` |
| `engine/record.zig`（153） | `engine/record.rs` | 完整；`text`/`image`/`paths`/`received`/失败事件 |
| `engine/restore.zig`（61） | `engine/restore.rs` | 完整；缺失图片行丢弃、孤儿文件清理、limit |
| `engine/sync.zig`（484） | `engine/sync.rs`（619） | 完整；端口/常量/协议、UDP+TCP、进度、inbox/outbox |
| `engine/sync_discovery.zig`（220） | `engine/sync_discovery.rs` | 完整；macOS `getifaddrs` / Windows `GetIpAddrTable` 广播地址 |
| `engine/sync_send.zig`（200） | `engine/sync_send.rs` | 完整；每 peer 一个投递线程、`Counter` 进度 |
| `engine/transfer.zig`（338） | `engine/transfer.rs` | 完整；`D/F/E` 帧、`localPath` 安全校验、`freeName` 重名 |
| `native/tests.zig`（188）/ `tests/views.zig` | `native/tests.rs` / `src/main.rs` 的 `views_test` | 5 + 1 条全部移植 |

移植方式：函数名 snake_case、逻辑顺序/常量/边界/注释逐条对应；Zig 的 `gpa/io/arena`
去掉，改用 `Vec`/`String`/`Arc`/`Mutex`；`[]u8` 交接改为拥有所有权的 `Vec<u8>`；
跨线程共享由 Zig 的裸指针 + 互斥换成 `Arc<Store>` / `Arc<Engine>` + `Mutex`；
`ui::post_wake`、`zant::db`、`zant::desktop`（剪贴板）、`zant::image` 按框架 API 调用。

**行为兼容性**：用用户现有的 Zig 写的数据库（19 条历史、图片/缩略图、`settings` JSON）
直接运行成功——表结构、行 JSON、图片文件 id（wyhash 一致）全部兼容；点击设置里的同步
开关后，数据库里的 `settings` 行变为
`{"device_id":"701ab1b2fc97a547","device_name":"BrianMBP","sync_enabled":false,...}`，
字段与 Zig 版一致。

## 2. 测试

`cargo test`：**26 passed / 0 failed**（Zig 26 个可移植 test 全部有同名 Rust 测试：
`settings` 1、`tests.zig` 5、`backend` 2、`format` 4、`history` 5、`storage` 2、`files` 2、
`sync` 2、`transfer` 2、`views.zig` 1）。用临时目录，不碰真实数据库/剪贴板/网络。

| 适配（都在报告里列出，不是静默改动） | 原因 |
|---|---|
| `settings` 的 `validName("\xff")` 改成 DEL 控制字符 | Rust `&str` 不可能承载非法 UTF-8；等价于 `0x7f` 控制字符被拒 |
| `tests.zig` 的「存储不可用 / 写失败」用「无数据目录的后端 / images 是普通文件的后端」各起一个实例 | Rust `Arc<Store>` 的目录不可变；Zig 测试是临时改 `files.images_dir` |
| `Storage.Db` 不存 `Collection`（生命周期），每次方法内 `store.collection::<Clip>()` | Rust 借用；生成的 SQL 与 Zig 逐字节一致 |
| `sync` 引擎的线程取消：Zig `Io.Group.cancel` → Rust 轮询 `stopping` + socket 超时；每条连接/每次投递的线程 detach | std 没有可取消的阻塞 socket；`stop` 仍会 join 四个主循环 |
| 设备 id 的随机数：`io.random` → 时间 + pid + wyhash | 只为生成一次设备 id；格式仍 16 位十六进制 |
| 事件队列/快照不再有帧 arena | Rust 值自己拥有；TSX 侧的 `tsx::List`/`Str`/`Rc<Image>` 由 codegen 复制 |

## 3. 构建 + 无窗口运行 + 点击证据

验证用的是 Zant UI 当前 checkout 的**临时补丁副本**（`/tmp/zant-fw` + 补丁见 §4；补丁只
动 `zant-tsx` codegen 和 `zant` 的 `Image`/`Node` `PartialEq`），应用本身没有为通过编译做过
任何改动（TSX/规格逐字节相同）。

```bash
# 干净迁移树 + 补丁框架
cp -a m7-cliphop-clean /tmp/m7-final
sed -i '' 's#<XyUI>/crates/#/tmp/zant-fw/crates/#g' Cargo.toml
CARGO_HOME=<XyUI>/.cargo-home CARGO_TARGET_DIR=/tmp/m7-target cargo test   # 26 passed
CARGO_HOME=<XyUI>/.cargo-home CARGO_TARGET_DIR=/tmp/m7-target cargo build
ZANT_NO_MOTION=1 ZANT_SNAPSHOT=main.png ZANT_SNAPSHOT_EXIT=1 \
  ZANT_LAYOUT_FILE=layout.txt /tmp/m7-target/debug/cliphop
```

- 无窗口快照 `/tmp/m7-final-shots/main.png`（sha256
  `6d412da4038bfc206ddf107b541b56372952705a44d2dde6b6bfe0e73169d6dc`，880×1280 Metal 渲染）：
  历史页，19 条从 Zig 版数据库恢复的记录（文字/链接/图片缩略图），顶部标签、搜索框、
  “正在寻找设备”、底部“共 19 条，最多保留 500 条 / 清空”。
- 点击（`scripts/mac/ui-scenario.swift` + `ZANT_LAYOUT_FILE`）：
  - 点“设置”标签后 layout 出现 `sync-switch`、`device-name`、`panel-hotkey`、
    `login-switch`、`close-action`、`history-limit`、`settings-clear`（历史页的
    `history-search`/`history-k*` 消失）→ 页面切换 + 设置页真实渲染。
  - 点“清空”后 layout 出现 `clear-dialog`、`clear-dialog-mask`、`clear-dialog-card`、
    `clear-cancel`、`clear-confirm` → 真实点击命中按钮、Modal 打开。
  - 点设置里的同步开关 → 数据库 `settings` 行的 `sync_enabled` 从 `true` 变 `false`
    （§1 的 JSON），即点击 → TSX action → `native/settings::set` → SQLite 落盘全链路。
- 限制：本沙箱没有 Screen Recording 权限（`CGWindowListCreateImage` /
  `screencapture` 都返回 “could not create image”），也没有键盘事件注入权限，所以
  **实时窗口截图、搜索框输入、⌘⌃V 面板没能自动化验证**；行级复制/删除按钮的点击在
  沙箱里未命中。UI 视觉用无窗口快照，交互用 layout 变化 + 数据库效果作证据；这两项
  （键盘输入、面板、行点击）留给 M8 的人工/Windows 脚本验收。
- **没有**构建 Zig 版做逐像素对照（按任务要求跳过：需要旧框架 + Zig 工具链），
  E4 的逐像素部分仍需在 M8 前用真实两平台跑；本次证明了 Rust 版与旧数据/新编译器
  的完整链路。

## 4. 需要的框架修复（`zant-fix.patch`，10 个文件，24 个 hunk）

产物里 `zant-fix.patch` 是完整的可 `git apply` 补丁。**不应用时** `cargo build` 的报错：

1. `zant-build: native/clips.ts:1:37: error: unknown import `Image`; this name is not supported by Zant`
   —— 规格里 `import type { Image } from "zant"` 不支持。补丁：spec 生成器把 `Image`
   映射为 `std::rc::Rc<zant::image::Image>`。
2. `error: unsupported type `Kind``（`src/actions/sync.tsx`）—— `.tsx` 的 interface 字段里引用
   native 规格类型时，`imports::resolve` 对 `native/*` 直接返回 None。补丁：解析规格类型
   （含规格之间的 `import type { Kind } from "./clips"`，生成 `crate::native::spec::clips::Kind`）。
3. `error: function-name callbacks need WP-5.4 (`events.functionArrow`); pass an arrow`
   （`useHotkey(tried, togglePanel)`）—— `useHotkey`/`useTray` 的回调只支持箭头；补丁：函数名
   包成 `() => f()`（与 JSX handler 的 `function_arrow` 同一规则）。
4. `expected Option<Node>, found Node`（`Modal.footer`）—— `OptionalNode` prop 没有包 `Some`。
5. `cannot add f64 to isize` / `cannot multiply f64 by f32`——`as` 转换右边是复合表达式时
   少了内层括号（`(scheme + 3isize as f64)`）；补丁用优先级括号化。
6. 字符串拼接/模板字面量丢字面量：`` `${x} MB` `` 只剩 `x`、`out + "…"` 只剩 `out`、
   `out += " "` 变成 `out = out.clone()`（`Format::only_string` 在追加字面量后没复位）。
   补丁：`Format::text` 里复位 `only_string`。
7. List 的 `renderItem`/`keyExtractor` 闭包捕获 `self.props`：`lifetime may not live long
   enough` + `cannot move out of self`（`src/views/HistoryView.tsx`、`PanelView.tsx`）。
   补丁：把整份 props 捕获成 `_cap_props = self.props.clone()`，闭包内不再用 `self`。
8. 规格生成的 struct/enum 缺 `PartialEq`/`Debug` 依赖的类型：`Image`/`Node` 没有
   `PartialEq`（props 都 `#[derive(PartialEq)]`），规格 enum 没有 `Debug`（测试 `assert_eq!`）。
   补丁：`impl PartialEq for Image`（id 相等）、`impl PartialEq/Eq for Node`（`Rc::ptr_eq`）、
   规格 enum 加 `Debug`。

其中 1/2/4/6/7 是**编译器功能缺口**（对应 status.md「待决问题」里 WP-5.8/5.6/5.4 未闭合
的部分），建议 lead 把这些补丁按 WP 拆开正式合入 `zant-tsx` 并加回归用例
（`crates/zant-tsx/tests/native_specs.rs`、`codegen_app.rs`），而不是留在应用侧。

## 5. 剩余缺口

- 框架补丁未合入 → 当前主 checkout 上应用编不过（这是唯一的硬阻塞）。
- 键盘输入/⌘⌃V 面板/行级点击的端到端点击验证（见 §3 限制）；两机局域网同步。
- 应用文档（`README*.md`、`AGENTS.md`、`CLAUDE.md`）与 `zant.d.ts` 还是 Zig 项目描述；
  `zant.d.ts` 指向 `../XyUI/tools/tsx/zant.d.ts`（M8 删除 `tools/` 前需要换成 Rust 生成的
  d.ts）。`docs/ts-port.md`（Zig 原生模块说明）在迁移时删除。
- `native/engine/sync.rs` 619 行（>500 行建议值），可按 `sync/discovery/send` 再拆；
  本次保持与 Zig 文件一一对应，未拆。
- TSX 编译器的 7 个补丁需要正式的回归测试与 `docs/rust-port/status.md` 登记（lead 合并时做）。
