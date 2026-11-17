# 待修复与优化清单

审计基线：`be848f2`。来源：对 `zant` / `zant-tsx` / `zant-build` / `zant-cli` 的代码审计、`zant-tsx` 最小探针，以及
`cargo test --workspace`（本机沙箱有 1 个环境性失败，见第 38 条）。

本清单经过一轮对照代码的复核（编译器行为用探针重现）：改正了错误的位置引用、与代码不符的现状描述、
已经存在却被写成“缺失”的能力，以及行不通或不完整的修复方案。标注“探针结果”的现状均已在基线上实际编译验证。

优先级：

- **P0**：会静默丢功能、破坏文档承诺，或阻塞基本构建/使用。
- **P1**：显著影响通用性、可维护性或构建成本，近期应排期。
- **P2**：一致性、清理、测试补强，可以随后做。

每条都给出位置、现状、修复方案和验收标准；行号以当前基线为准。

共 40 条：P0 4 条、P1 22 条、P2 14 条。

## 进度（分支 `fix/needfix`）

- 已完成：1、2、3、4、5、6、7、8、9、11、13、17、20、28、33、34、37、38（勾选项）。macOS 与 Windows 测试机上 `clippy -D warnings`、
  `cargo test --workspace` 均通过；`node-parity.sh` 20 个探针与 Node 一致。
- 部分完成：
  - 10：路标、`desktop.md` 已写清条件与 `ZANT_ALLOW_MULTI_INSTANCE`；模板是否默认开单实例待决定；
  - 13：`widgets.rs` 拆成 `components/markdown.rs` 与 `components/pdf_viewer.rs`（旧路径保留重导出）；
    `pdf_preview` 留在 `ui/builders/`，因为标签生成器只从该目录发现 builder，已按 `pdf` feature 门控；
  - 15：生成代码解析失败不再 panic，改为“编译器内部错误”诊断并可用 `ZANT_TSX_DUMP` 保存；`Error::Tsx(Diag)` 与多诊断未做；
  - 18：`node-parity.sh` 与无可选 feature 的 `cargo check` 已进 CI；模板构建截图、打包资源测试未做；
  - 28：组件里的平台分支已移出（`Shortcut::key_caps`、`keys::LABEL_COLUMN_WIDTH`），两个 `Shortcut` 类型已合并；
    `Button` 的 Windows 标题栏颜色常量未动。
- 待决定：35、36（是否发布到 crates.io）。
- 20：`set_title` / `set_size` / `set_min_size` / `set_max_size` / `set_always_on_top` / `set_fullscreen` 已在 macOS 与 Windows
  桌面上实测（尺寸夹在上下限内、左上角不动、全屏进出恢复原位）；TSX 的 `mainWindow` 与 `createWindow` 句柄同名方法可用。
  `WindowOptions` 未加创建时的 `max_width` / `x` / `y` / `fullscreen`（创建后用 setter）。
- 未开始：12、14、16、19、21–27、29–32、39、40。

## 总览

- [x] 01 [P0] hooks 作用域校验缺失，hook 会静默失效
- [x] 02 [P0] 穷尽联合 `switch` 在组件内 `function` 声明和数组 / 列表回调里不生效
- [x] 03 [P0] `zant-tsx` 反向依赖 `zant`，每个 build.rs 都链接 Skia
- [x] 04 [P0] 没有 feature 边界，文档栈/SQLite/pdfium 强制进入所有应用
- [x] 05 [P1] 解构出来的 render 局部变量无法被命名处理函数捕获
- [x] 06 [P1] 泛型函数“实参推断”与跨文件泛型类型和文档不一致
- [x] 07 [P1] `...` 展开实参只支持 rest 参数，文档写得过宽
- [x] 08 [P1] 路标里的 `Object.*` 实际只有 4 个函数
- [x] 09 [P1] async/await 的限制说明不完整
- [ ] 10 [P1] 路标没写单实例/开机启动的启用条件
- [x] 11 [P1] `AGENTS.md` 的热更新说明与代码相反
- [ ] 12 [P1] `core` 反向依赖具体组件，分层不闭环
- [x] 13 [P1] 文档组件分散在三层（widgets / ui builder / pdf）
- [ ] 14 [P1] `Generator` 是 45 字段 God object + 30 多个函数指针
- [ ] 15 [P1] 诊断只有一条、内部错误 panic、thread_local 旁路
- [ ] 16 [P1] 类型/属性检查与代码生成耦合
- [x] 17 [P1] `pending.rs` 里有两份手写的图标拼写表，新增图标会漏改
- [ ] 18 [P1] TSX 应用的构建/截图脚本和 Node 对照没有进 CI
- [ ] 19 [P1] 没有应用菜单 API，macOS 菜单只支持中/英
- [x] 20 [P1] 窗口创建后不能改标题/尺寸/任意位置/全屏
- [ ] 21 [P1] 缺少通用覆盖层/弹层管理
- [ ] 22 [P1] 组件覆盖面对通用桌面应用偏窄
- [ ] 23 [P1] 框架内置文案无法本地化，Markdown labels 未从 TSX 暴露
- [ ] 24 [P1] 主题 token 不完整，也不能从 TSX 定制
- [ ] 25 [P1] `PdfPreview` 在 build 路径同步打开并渲染 PDF
- [ ] 26 [P1] 缺少可访问性支持
- [ ] 27 [P2] Markdown 的 `content_width` 与根节点宽度语义不一致
- [x] 28 [P2] 平台分支泄漏进通用组件
- [ ] 29 [P2] 布局缺少 z-index、横向滚动、Grid
- [ ] 30 [P2] 没有自定义字体、SVG、通用图片加载器
- [ ] 31 [P2] 焦点 API 不完整（tab_index / auto_focus / 焦点恢复）
- [ ] 32 [P2] Rust 侧的有状态组件用数字 id，其他组件用 key
- [x] 33 [P2] PNG 代码在 CLI 与 zant-build 中重复
- [x] 34 [P2] 两个 `layout` 模块命名冲突
- [ ] 35 [P2] 测试夹具放在仓库根 `tests/fixtures`，不属于任何 package
- [ ] 36 [P2] 发布元数据与示例 workspace 成员不一致
- [x] 37 [P2] 编译器测试缺口（平台常量、Date.now、Node 对照未进 `cargo test` 等）
- [x] 38 [P2] i18n 测试在沙箱写目录失败，`cargo test` 不是沙箱健壮的
- [ ] 39 [P2] 组件字段与 antd 语义的缺口（Select/Button/Tabs）
- [ ] 40 [P2] 文档与路线图缺少“✅ 必须有测试”的约束

---

## 一、P0：正确性与基本构建

### 1. hooks 作用域校验缺失，hook 会静默失效

- [x] 修复。

**位置**

- [app_hooks.rs](../crates/zant-tsx/src/codegen/app_hooks.rs#L71-L77) 的 `useWindowFocus` 标记为 `root_only: false`；
- [app_hooks.rs](../crates/zant-tsx/src/codegen/app_hooks.rs#L401) 根生命周期对 `"on_focus" => {}` 什么也不生成；
- [windows.rs](../crates/zant-tsx/src/codegen/windows.rs#L362-L400) 窗口只转发 `on_key` / `on_focus` / `on_close_request`，不转发 `on_quit_request`；
  运行时的 `tsx::window::Component`（[window.rs](../crates/zant/src/tsx/window.rs#L48-L56)）本身也没有 `on_quit_request`，`ui::App`（[app.rs](../crates/zant/src/ui/app.rs#L97-L130)）没有 `on_focus`；
- [app_hooks.rs](../crates/zant-tsx/src/codegen/app_hooks.rs#L163-L187) 的 `collect()` 只拦了两种情况：`root_only` 的 hook 出现在非根组件、以及“有 props 的子组件”；
  非根组件一律生成 `_window_on_*` 方法，不管它是不是某个 `createWindow` 的组件。

**现状**

以下写法都能编译通过，但运行时永远不会执行（均已用探针确认）：

1. 根组件里 `useWindowFocus(...)`：生成 `_root_on_focus`，但 `impl ui::App` 没有 `on_focus`；
2. `createWindow` 窗口组件里 `useQuitRequest(...)`：生成 `_window_on_quit_request`，但 `impl tsx::window::Component` 没有转发；
3. 既不是根、也没被 `createWindow` 引用的无 props 子组件（`<Child/>`）里的 `useCloseRequest` / `useQuitRequest` / `useKeyDown` / `useWindowFocus`：
   生成 `_window_on_*` 方法，但没有任何 trait 转发。

文档本身是对的：[docs/tsx.md](tsx.md#L357-L365) 的“范围”列已经写明 `useQuitRequest` 只属于根组件、`useWindowFocus` 只属于窗口组件。
问题只在编译器没有按这张表拒绝。

**修复**

- 在生成器里区分 `ComponentRole { Root, Window, Child }`：根由默认导出决定；窗口由 `createWindow(Component, …)` 引用决定；其余为 `Child`。
  同一个组件既被 `createWindow` 引用、又被当作 `<Panel/>` 渲染时，子元素那个实例的 hook 同样不会触发，要一并报错（或只对子元素用法报错）；
- 在 `collect()` 里按角色**只允许**下列 hook，其余报错并说明该放到哪里：
  - `Root`：`useCloseRequest` / `useQuitRequest` / `useKeyDown` / `useClipboardChange` / `useReopen` / `useWake` / `useAppExit`（即除 `useWindowFocus` 外的全部）；
  - `Window`：`useCloseRequest` / `useKeyDown` / `useWindowFocus`；
  - `Child`：全部拒绝，并提示“把 hook 放到根组件或窗口组件”；
- 不建议给窗口补 `on_quit_request`：退出是应用级事件，文档已规定只属于根组件，直接拒绝即可；
  根组件的 `useWindowFocus` 同理先拒绝（要支持主窗口焦点得先给 `ui::App` 加 `on_focus`，属于新功能）；
- 自定义 hook 在生成前已展开进调用它的组件（`hooks::expand`），角色按展开后的组件判断，报错位置要指回自定义 hook 里的调用；
- `docs/tsx.md` 的表不用改；给 [dts_api.d.ts](../crates/zant-tsx/src/bin/zant-tsx-dts/dts_api.d.ts) 的各 hook 加范围注释，并修正
  [app_hooks.rs](../crates/zant-tsx/src/codegen/app_hooks.rs#L1-L15) 顶部“无 props 子组件得到 `_window_on_*`”的模块注释。

**验收**

- 新增测试：根 `useWindowFocus`、窗口 `useQuitRequest`、无 props 普通子组件里的任意应用 hook 都必须编译失败且信息指向正确位置；
- 现有 [codegen_app.rs](../crates/zant-tsx/tests/codegen_app.rs#L553) 只测了“有 props 的子组件”，补充无 props 子组件；
- `useCloseRequest` / `useKeyDown` 在根和窗口中仍然工作，窗口里的 `useWindowFocus` 仍然工作（现有 `codegen_app.rs` 第 274 行附近的用例不回归）。

---

### 2. 穷尽联合 `switch` 在组件内 `function` 声明和数组 / 列表回调里不生效

- [x] 修复。

**位置**

- “穷尽”是生成 `switch` 时才登记的： [loops.rs](../crates/zant-tsx/src/codegen/loops.rs#L296-L303) 的 `EXHAUSTIVE`（thread_local，按判别式位置记录，`stmt::returns_all` 读它）；
- 先 `returns_all` 再生成语句（顺序反了）的地方：
  - [generate.rs](../crates/zant-tsx/src/codegen/generate.rs#L1503) 的 `method_block`（组件方法，也被 `.then` 回调经 [reaction.rs](../crates/zant-tsx/src/codegen/promises/reaction.rs#L55-L57) 调用）；
  - [callbacks.rs](../crates/zant-tsx/src/codegen/callbacks.rs#L135)（`map` / `filter` 等数组回调）；
  - [list.rs](../crates/zant-tsx/src/codegen/list.rs#L494)（`renderItem`）；
  - [reaction.rs](../crates/zant-tsx/src/codegen/promises/reaction.rs#L99) 的 `reaction_block`（有返回值的 Promise 回调）；
- 顺序正确、可作参照的两处： [functions.rs](../crates/zant-tsx/src/codegen/functions.rs#L593-L601)（顶层函数）与
  [closures.rs](../crates/zant-tsx/src/codegen/closures.rs#L219-L223)（函数值 / `const f = () => {…}`），都是先生成 statements 再 `returns_all`。

**现状**（探针结果）

- 顶层函数：通过；
- 组件体内 `const label = (x: Kind): string => { switch … }`、以及在事件里调用它：通过（走 `closures.rs`）；
- 组件体内 `function label(x: Kind): string { switch … }`：`error: function must return a value on every path`（字符串联合与可辨识对象联合都一样）；
- `.map((x) => { switch (x) { case …: return …; } })`（返回值或返回 JSX）：`error: array callback must return on every path`。

这与 [docs/tsx.md](tsx.md#L206) “覆盖全部成员的 `switch` 无需 `default`”以及路标第 29 行不一致。

**修复**

- 把上面四处检查移到 `stmt::statements(...)` 之后（和 `functions.rs` / `closures.rs` 一致）。
  `callbacks.rs` / `list.rs` / `reaction_block` 在检查之后还要建标签和临时变量，移动时保持这些语句的生成顺序不变；
- 或者增加一个预扫描，在生成前标记“这个 `switch` 覆盖了联合的全部成员”（需要判别式的类型，比移动检查贵）；
- 组件 render 自身的检查（[generate.rs](../crates/zant-tsx/src/codegen/generate.rs#L505-L508)）要求“最后一条语句是 `return`”，是另一条规则，不在本条范围；
- 不要只改文档：这是用户最常见的 union 写法之一。

**验收**

- 组件体内 `function`、`.map` 回调、`renderItem`、`.then` 回调各加一个“无 default 的穷尽 switch”测试；
- 非穷尽 switch 仍然报错；
- [codegen_unions.rs](../crates/zant-tsx/tests/codegen_unions.rs) 现有顶层测试不回归。

---

### 3. `zant-tsx` 反向依赖 `zant`，每个 build.rs 都链接 Skia

- [x] 修复。

**位置**

- [crates/zant-tsx/Cargo.toml](../crates/zant-tsx/Cargo.toml#L14) 的 `zant.workspace = true`；
- 表格工具： [table.rs](../crates/zant-tsx/src/bin/zant-tsx-tags/table.rs#L11-L12) 的 `zant::ui::options::TSX_REGISTRY` / `tsx_props`；
- **库代码本身也有 5 处真实调用**（不是生成字符串）：
  - [i18n/emit.rs](../crates/zant-tsx/src/i18n/emit.rs#L114) `zant::i18n::plural::cldr_rules`、[第 125 行](../crates/zant-tsx/src/i18n/emit.rs#L125) `zant::i18n::plural::Rules::parse`；
  - [i18n/catalog.rs](../crates/zant-tsx/src/i18n/catalog.rs#L65) `zant::i18n::locale::canonical`；
  - [assets.rs](../crates/zant-tsx/src/assets.rs#L61) `zant::core::hash::Wyhash::hash`；
  - [codegen/regexps.rs](../crates/zant-tsx/src/codegen/regexps.rs#L111) `zant::tsx::RegExp::new`（编译期校验正则字面量）；
- 传递依赖： `zant-build → zant-tsx`、`zant-cli → zant-tsx`（`zant-build` / `zant-cli` 自己不直接依赖 `zant`，只经 `zant-tsx` 带入）。

**现状**

`cargo tree -p zant-tsx -e normal` 带出 `zant → zant-sys`（还有 `rusqlite`、`reqwest`）。也就是说，每个应用的 `build.rs` 和 `zant` CLI
为了把 TSX 翻译成 Rust，都要为宿主再编译并链接一份框架与 Skia；Release 构建里构建脚本用的是另一套 profile，这份框架不能和应用自己那份复用。

**修复**

- 先把库代码用到的五个纯逻辑点下沉到一个不依赖 `zant-sys` 的叶 crate（例如 `zant-base`）：`i18n::locale` / `i18n::plural`（含 CLDR 规则表）、
  `core::hash::Wyhash`、`tsx::RegExp` 的模式校验（基于 `regress`，`zant-tsx` 已直接依赖 `regress`）。`zant` 重导出它们，路径不变；`zant-tsx` 改为依赖叶 crate。
  这一步是前提：直接删掉 `zant` 依赖会编译失败。必须共用同一份实现，不要在编译器里复制一份（复数规则、资源文件名哈希、正则语义在编译期与运行期必须一致）；
  `plural` 的操作数格式化还用到 `tsx::builtins_format::write_js_number`（[operands.rs](../crates/zant/src/i18n/plural/operands.rs#L131)），要一起下沉；
- 新建 `crates/zant-tables`（或 workspace 的 `xtask`），把 `src/bin/zant-tsx-tags/` 移过去；该 crate 依赖 `zant` + `zant-tsx`，只用于生成表格。
  `zant-tsx-dts` 只读 `zant_tsx::tags`、不依赖 `zant`，可以留在原处（为了两个工具放在一起也可一并移走）；
- 然后从 `zant-tsx` 的普通依赖中删除 `zant`；
- [ui_tags.rs](../crates/zant-tsx/src/ui_tags.rs) / [zant.d.ts](../crates/zant-tsx/zant.d.ts) 继续 check in，CI 现有的“generated TSX tables are up to date”一步改用新 crate 的 `--check`；
  同步更新调用这两个工具的脚本与文档（`rg "zant-tsx-tags|zant-tsx-dts"`）；
- `zant` 改放 `zant-tsx` 的 `[dev-dependencies]`：[native_specs.rs](../crates/zant-tsx/tests/native_specs.rs#L43-L45) 用 `include!` 把
  `fixtures/native_spec.rs` 真正编译进测试（里面用到 `zant::tsx`、`zant::Str`、`zant::hot_put!`），测试仍然需要框架。

**验收**

- `./scripts/cargo.sh tree -p zant-tsx -e normal,build`（`zant-build`、`zant-cli` 同理）不再出现 `zant` / `zant-sys`。
  不加 `-e` 的 `cargo tree` 会把 dev-dependencies 也列出来，那里仍然会有 `zant`，不能作为判据；
- `cargo build -p zant-tsx` 不编译 `zant-sys`；
- 编译期与运行期的复数规则、资源 key、正则校验结果一致（现有 i18n / assets / regexp 测试不回归）；
- `scripts/tsx/build-template-check.sh` 和表格 `--check` 仍通过。

---

### 4. 没有 feature 边界，文档栈/SQLite/pdfium 强制进入所有应用

- [x] 修复。

**位置**

- [crates/zant/Cargo.toml](../crates/zant/Cargo.toml#L9-L25) 只有 `fetch`，且默认开启；
- [lib.rs](../crates/zant/src/lib.rs#L18-L31) 无条件 `pub mod markdown / math / pdf / db`；
- [windows.rs](../crates/zant-build/src/windows.rs#L30) 每次 Windows 构建都 `install_pdfium`；Release 缺 `pdfium.dll` 直接失败（[windows.rs](../crates/zant-build/src/windows.rs#L94-L135)）；
- [build.rs](../crates/zant-dylib/build.rs#L1-L10) 为 bundled SQLite 导出 `sqlite3_*` 符号；
- `markdown` 约 14.6k 行、`math` 约 9.3k 行、`pdf` 约 5.3k 行、`db` 约 914 行。

**现状**

一个只画按钮和文本的应用，也会编译 Markdown、数学、PDF、SQLite（约 3 万行）并承担对应的构建时间；`reqwest` 已经是可选依赖，但默认开启。
Windows 上凡是用 `zant-build` 的应用都会尝试安装 `pdfium.dll`：Debug 缺失只警告，非 Debug 缺失直接构建失败；运行时 PDFium 是动态加载的，缺失只影响 PDF。
最终二进制体积受链接器裁剪影响，目前没有数据，修复前后各量一次再下结论。

**修复**

- `zant/Cargo.toml` 增加 `markdown`（依赖 `math`）、`math`、`pdf`、`db`，保留已有的 `fetch`。
  `default` 取哪一组要先定：`default = []` 对现有 Rust 应用是破坏性变更（要在发布说明里写明并同步改模板 / 示例）；
  保守做法是第一步让 `default` 等于全部 feature（行为不变），模板用 `default-features = false` 加所需 feature；
- 只门控 `lib.rs` 的模块入口不够，下面这些也要按 feature 包 `#[cfg]`：[ui/mod.rs](../crates/zant/src/ui/mod.rs#L69-L95) 的重导出、
  [components/widgets.rs](../crates/zant/src/components/widgets.rs)（无条件 `use` 了 markdown 与 pdf）、
  [ui/builders/pdf_preview.rs](../crates/zant/src/ui/builders/pdf_preview.rs)（直接调 `platform::pdf`）、`TSX_REGISTRY` 里对应的条目（见第 13 条，先把文档组件归位再门控更省事）；
- TSX 应用的 feature 写在应用自己的 `Cargo.toml`，构建脚本改不了依赖的 feature。所以编译器在遇到 `<Markdown>` / `<PdfViewer>` / `fetch` 等而对应
  feature 没开时，要报“在 Cargo.toml 给 zant 打开 feature `xxx`”，不能让用户看到生成代码里的 `unresolved import`；
  `zant create` 的模板与 `zant-tsx` 的标签表（[ui_tags.rs](../crates/zant-tsx/src/ui_tags.rs)）保持全集，按 feature 过滤只发生在报错这一步；
- `db` feature 绑定 `dep:rusqlite`；`zant-dylib` 增加同名的透传 feature（`db = ["zant/db", "dep:libsqlite3-sys"]` 等），
  [build.rs](../crates/zant-dylib/build.rs) 的 `sqlite3_*` 导出只在 `db` 下进行；`zant run` 的热更新宿主 / 隐藏 crate 要把应用开的 feature 原样传给 `zant-dylib`，
  否则热更新与发布构建的 feature 集不一致；
- `pdfium.dll` 的安装/失败策略与 `pdf` 绑定：没有 PDF 的应用不应因缺 DLL 而构建失败。应用的构建脚本看不到依赖的 feature，
  需要显式通道：例如让 `zant`（或 `zant-sys`）的构建脚本通过 `links` 元数据导出 `DEP_ZANT_PDF`，`zant_build` 读它决定是否调用
  [install_pdfium](../crates/zant-build/src/windows.rs#L102)；`zant-sys` 里 PDF 的原生桥（PDFKit / PDFium）也要同步受 `pdf` 控制；
- 打破 `math → markdown` 的反向引用：只有一处，[math/mod.rs](../crates/zant/src/math/mod.rs#L20) 重导出了
  `markdown::render::spans::inline_drawing`。这个函数（[spans.rs](../crates/zant/src/markdown/render/spans.rs#L139-L149)）只用 `math::layout` 和 `core::vector`，
  直接搬进 `math`，`markdown` 反过来从 `math` 引用即可；`markdown → math` 是正常方向，不需要另建共享模块；
- 中长期可拆成 `zant-markdown` / `zant-math` / `zant-pdf` / `zant-db` 独立 crate。

**验收**

- 一个只依赖基础 UI 的 hello-world 应用：`cargo tree -e normal` 不含 `rusqlite` / `reqwest`，Windows Release 构建不要求 `pdfium.dll`
  （`pdfium` 不是 Cargo 依赖，`cargo tree` 里本来就看不到，要用实际构建验证）；
- 没开 `markdown` 却写了 `<Markdown>` 的 TSX 应用得到指明 feature 名的编译错误；
- PDF 示例开启 `pdf` 后行为不变；`zant run` 热更新在开 / 不开 `db` 时都能链接；
- `cargo test --workspace` 在默认 feature、`--no-default-features` 与 `--all-features` 下都通过（CI 至少加一个 `--no-default-features` 的 `cargo check`）。

---

## 二、P1：路线图与文档一致性

### 5. 解构出来的 render 局部变量无法被命名处理函数捕获

- [x] 修复。

**位置**

- [generate.rs](../crates/zant-tsx/src/codegen/generate.rs#L1524-L1548) 的 `render_names` 只登记 `ast::Pattern::Ident`，跳过 `Pattern::Array` / `Pattern::Object`；
- [generate.rs](../crates/zant-tsx/src/codegen/generate.rs#L1077-L1105) 的 `mark_inline` 靠这张名单判断“组件内函数读了 render 局部值，要在传递处内联生成”；
  名单里没有的名字，函数就被当成普通组件方法生成，方法里自然找不到这个局部变量；
- [captures.rs](../crates/zant-tsx/src/codegen/captures.rs#L57-L79) 的 `snapshot` 没有问题：它按作用域里的全部绑定取值，解构出来的绑定也在其中。

**现状**

```tsx
const { name } = user;
const [first, second] = pair;
function go() { setText(name); }         // error: unknown name `name`
const go2 = () => { setText(first); };   // error: unknown name `first`
<Button onClick={() => setText(name)} /> // 正常：内联箭头走 snapshot
```

简单绑定 `const name = user.name` 正常；内联事件箭头捕获解构绑定也正常。出问题的只有**命名**处理函数（`function go()` 与 `const go = () => …`）。
文档与路标第 25 行都说“命名处理函数捕获本帧局部值”，没有这个例外。

**修复**

- 在 `render_names` 里把 `Pattern::Array`（跳过空位）与 `Pattern::Object`（取 `local` 名）的每个绑定也登记进去。
  这些绑定在 render 里本来就是普通局部变量，不需要另外保留字段 / 下标的访问代码；
  `const [value, setValue] = useState(…)` 也是数组模式，`mark_inline` 已经用 `scope::state_index` 把 state 名排除，不受影响；
- `render_names` 现在只下探 `Block` 与 `If`，顺手确认 `for` / `try` 等语句体里的声明是否也该登记（同一类漏报）；
- 做不到时，至少改成明确诊断，不要报 `unknown name`。

**验收**

- 对象解构、元组解构各加一个捕获测试；
- `function` 声明、`const` 箭头、命名函数再被另一个命名函数调用（`mark_inline` 的传递闭包）、传给子组件四种用法都覆盖；
- 内联箭头的现有行为不回归。

---

### 6. 泛型函数“实参推断”与跨文件泛型类型和文档不一致

- [x] 修复文档或实现。

**位置**

- [generic_functions.rs](../crates/zant-tsx/src/codegen/generic_functions.rs#L107) 的 `cannot infer`；
- [codegen_generic_functions.rs](../crates/zant-tsx/tests/codegen_generic_functions.rs#L72-L84) 固化的是“无实参 `make()`”与“空数组 `first([])`”的推断失败
  （这两种 TypeScript 自己也推不出有用的类型，属于合理行为）；对象字面量实参的失败没有测试；
- [docs/tsx.md](tsx.md#L211) 写“从实参推断”“各文件同一实例即同一类型”；
- 跨文件 `Box<T>`： [types.rs](../crates/zant-tsx/src/codegen/types.rs#L283-L295) 的 `generic_error` 直接报“declare it in this file”。

**现状**（探针结果）

- 字面量直接当实参：`id({ a: 1 })`、`first([{ a: 1 }])` 报 `cannot infer T`；
- 基本类型与数组字面量可以推断：`id(1)`、`id("s")`、`first([1, 2])`；
- 先存进变量就可以，不要求具名接口：`const p = { x: 1 }; id(p)` 与 `const u: User = …; id(u)` 都通过；
- 从其他文件导入的泛型类型（`import { Box } from "./types"` 后写 `Box<number>`）直接报错，和文档“各文件同一实例即同一类型”矛盾；
- 跨文件调用泛型函数报错，这一点文档和路标（⬜ 行）已经如实写明，不算缺陷。

**修复**

- 优先补实现：对象字面量在别处已经能无注解推断（`const p = { x: 1 }`），实参位置复用同一条路径即可让 `id({ a: 1 })` 通过；
  做不到再把文档降级为“对象字面量实参请显式传类型参数或先存进变量”；
- 把“跨文件泛型类型”加入 [tsx-roadmap.md](tsx-roadmap.md#L36) 的 ⬜ 行，并删掉 `docs/tsx.md` 里“各文件同一实例即同一类型”这半句；或实现跨文件实例；
- 错误信息已经给了“显式传类型参数”这一种改法，再补上“或先存进变量”。

**验收**

- 文档、路标与测试一致；
- 对象字面量实参要么通过（补对象字面量、对象数组字面量、嵌套对象测试），要么有一条指名测试固化诊断文本；
- 空数组 / 无实参的现有诊断测试不回归。

---

### 7. `...` 展开实参只支持 rest 参数，文档写得过宽

- [x] 修复文档。

**位置**

- [expr.rs](../crates/zant-tsx/src/codegen/expr.rs#L100-L105) 的 spread 诊断；[functions.rs](../crates/zant-tsx/src/codegen/functions.rs#L490-L499) 的“只能填 rest 参数”诊断；
- [tsx-roadmap.md](tsx-roadmap.md#L33) 写“rest 参数与展开实参”；[docs/tsx.md](tsx.md#L211) 的说法是准确的（“调用处多余实参与 `...arr` 展开合成数组”）。

**现状**（探针结果）

- 可用：数组 / 对象字面量里的展开；自定义函数的 rest 参数（`sum(...xs)`、`sum(0, ...xs)`）；内置的 `push` / `unshift` / `splice`（`ys.push(...xs)`）；
- 不可用：`Math.max(...xs)` / `Math.min(...xs)` 报 “`...` spreads into array literals, object literals and a function's rest parameter”；
  把元组展开到固定参数（`two(...pair)`）报 “a spread argument can only fill a rest parameter”。

**修复**

- 路标改成“rest 参数，以及展开到 rest 参数 / `push` 等的 `...arr`”；
- `Math.max(...xs)` / `Math.min(...xs)` 是很常见的写法，建议单独补上（对数组折叠即可，注意空数组分别得到 `-Infinity` / `Infinity`），不必实现通用展开；
- 元组展开到固定参数可以留作不支持。

**验收**

- 路标不再暗示任意调用都能展开；
- 如果补了 `Math.max` / `Math.min`，加测试并进 `node_parity` 探针；`sum(...xs)`、`push(...xs)`、`[...a, ...b]` 的现有行为不回归。

---

### 8. 路标里的 `Object.*` 实际只有 4 个函数

- [x] 修复文档。

**位置**

- [tsx-roadmap.md](tsx-roadmap.md#L31)；
- 实现： [dicts.rs](../crates/zant-tsx/src/codegen/dicts.rs#L629-L660)；[docs/tsx.md](tsx.md#L205) 已经只列了 4 个。

**现状**

只有 `Object.keys / values / entries / fromEntries`。其他 `Object.*` 调用都会报错，但报的是参数个数：
`Object.assign({}, a)`、`Object.hasOwn(r, "a")` 得到 “this Object function takes 1 argument”，没有说明这个函数不支持。

**修复**

- 路标行改成同样的四个函数；
- `object_call` 先判断函数名：不在四个之内时报“支持的是 Object.keys / values / entries / fromEntries”，并给替代写法（`Object.assign` → `{ ...a, ...b }`，`Object.hasOwn(r, k)` → `k in r`）；
- 若要支持更多，先补实现再改文档。

**验收**

- 路标、`docs/tsx.md`、代码三处一致。

---

### 9. async/await 的限制说明不完整

- [x] 修复文档。

**位置**

- [tsx-roadmap.md](tsx-roadmap.md#L44)；
- 较完整的列表： [docs/tsx.md](tsx.md#L486)（比路标多列了 `switch`、条件运算右侧、`catch` / `finally`、`new Promise`，但仍缺一条，见下）；
- 实现： [async_lower/mod.rs](../crates/zant-tsx/src/async_lower/mod.rs#L233)（`if` / 循环 / `switch`）、[第 244 行](../crates/zant-tsx/src/async_lower/mod.rs#L244)（`catch` / `finally`）、
  [第 248–253 行](../crates/zant-tsx/src/async_lower/mod.rs#L248-L253)（`try` 里有 `return` 且后面还有语句）、[第 320 行](../crates/zant-tsx/src/async_lower/mod.rs#L320)（条件运算右侧）。

**现状**

路标只写了“`if` / 循环里的 await 待做”；实际还不支持：

- `switch` 里的 await；
- `catch` / `finally` 块里的 await；
- `&& || ?? ?:` 右侧的 await；
- `try` 块里有 `return`、而 `try` 之后还有语句（报 “a `return` inside this `try` would also skip the statements after it”）。**这条 `docs/tsx.md` 也没写**；
- 手写 `new Promise((resolve) => …)`。它不是 `async_lower` 拒绝的，报的是通用的 “`new Promise` is not supported; `new` creates a Map or a Set here”，没有提示改用 `Promise.resolve` / async 函数。

**修复**

- 先把 `try` + `return` 这条补进 `docs/tsx.md` 的“暂不支持”；
- 路标改成“🚧 部分实现”并链接 `docs/tsx.md` 的那一段，不要在两处各维护一份列表；
- `new Promise` 的诊断补一句可行的替代写法。

**验收**

- 文档与 `async_lower` 的错误信息一致；
- 现有 [codegen_async.rs](../crates/zant-tsx/tests/codegen_async.rs#L60-L101) 只覆盖了 `if`、`||`、`catch`、非 async 函数里的 await；
  补循环、`switch`、`finally`、`&&` / `??` / `?:`、`try` + `return`、`new Promise` 的诊断测试。

---

### 10. 路标没写单实例/开机启动的启用条件

- [ ] 修复路标（可同时改模板）。工作量很小：参考文档已经写清，只有路标那一行缺限定。

**位置**

- [tsx-roadmap.md](tsx-roadmap.md#L53)；
- 单实例默认值： [config.rs](../crates/zant-build/src/config.rs#L123)；调试逃生口： [tsx/app.rs](../crates/zant/src/tsx/app.rs#L333-L337)；
- 开机启动条件： [platform.rs](../crates/zant/src/platform/platform.rs#L724-L732) 与 macOS 原生检查（`zant_launch_at_login_supported`：macOS 13+ 且是 `.app`）；
- 已有的说明： [docs/desktop.md](desktop.md#L110-L138)（开机启动的支持条件、`Unsupported` / `Failed`、TSX 的返回值与 `app.canLaunchAtLogin()`）、
  [docs/window-config.md](window-config.md#L52) 与 [docs/tsx.md](tsx.md#L418)（`single_instance`）。

**现状**

- 单实例要 `app.config.json` 的 `"single_instance": true`，默认 `false`，模板也没有开；
- 开机启动：macOS 要求 13+ 且是打包后的 `.app`（不看 Debug / Release）；Windows 要求非 Debug 构建；
  `zant run` 的热更新宿主在两个平台都不支持，`--release` 也一样；
- 以上在 `docs/desktop.md` 等参考文档里都有，失败返回值 / 错误类型也已写明；缺的只是路标的限定语；
- `ZANT_ALLOW_MULTI_INSTANCE` 已经实现（仅 Debug 构建，变量存在即生效），但没有任何文档提到它。

**修复**

- 路标加限定并链接 `desktop.md`：“（单实例需配置；开机启动需打包的 .app / Windows release 构建，macOS 13+）”；
- 把 `ZANT_ALLOW_MULTI_INSTANCE` 写进 `docs/desktop.md` 或 `docs/development.md`；
- 模板是否默认 `single_instance: true` 是产品决策：开了以后 `zant run` 期间再启动一份会被拦下，文档要同时说明上面的逃生口。

**验收**

- 路标、参考文档与新生成项目的行为一致；
- 单实例、开机启动各有一个手动验证步骤。

---

### 11. `AGENTS.md` 的热更新说明与代码相反

- [x] 修复文档。

**位置**

- [AGENTS.md](../AGENTS.md#L291)：“旧库暂不卸载，长会话会累积映射”；
- 代码： [library.rs](../crates/zant-hot-host/src/library.rs#L84-L95) 默认卸载，`ZANT_HOT_KEEP_LIBS=1` 才保留；
- [docs/scaffold.md](scaffold.md#L210-L211) 与代码一致。

**修复**

- 把这句改成“成功替换后默认卸载旧应用库；`ZANT_HOT_KEEP_LIBS=1` 可保留用于诊断”；
- 顺手修正 [library.rs](../crates/zant-hot-host/src/library.rs#L3-L6) 顶部“Unloading is opt-in”的过时注释。

**验收**

- 三处文档/注释一致；
- 搜索“暂不卸载”不再命中。

---

## 三、P1：工程结构

### 12. `core` 反向依赖具体组件，分层不闭环

- [ ] 重构。

**位置**

- [paint.rs](../crates/zant/src/core/tree/paint.rs#L176-L188) 直接调 `components::button::paint` / `input` / `toggle` / `image` / `select`；
- [intrinsic.rs](../crates/zant/src/core/layout/intrinsic.rs#L82-L90) 直接调 `components::input::measure_*`；
- `core` 里共 22 处 `crate::components` 引用，除上面两个文件外还有 `core/layout/container.rs`、`core/tree/input_ops.rs`、`core/tree/alloc.rs`；
- `ui` 重导出 `components`，[view.rs](../crates/zant/src/components/view.rs#L19) 又 import `ui::builders`。

**现状**

`core` 不是可复用的底层：新增**底层节点种类**（自己绘制 / 测量的控件）必须改 `core`，第三方也无法添加。
`NodeKind`（[data.rs](../crates/zant/src/core/node/data.rs#L21)）与 `NodeData` 都是封闭枚举。
由现有节点组合出来的控件（`Modal`、`DropdownMenu`、`ShortcutList` 等）不受影响，不需要动 `core`。

这条和第 4 条的 feature 门控没有牵连：`core` 引用的是 button / input / toggle / image / select 这些基础控件，它们不会被门控；
markdown / math / pdf / db 都没有被 `core` 引用。

**修复**

- 第一步（同一个 crate 内，收益是分层清晰、`core` 可独立测试）：把 `core` 对具体控件的调用收敛到一张按 `NodeKind` 分派的表
  （`paint` / `measure_intrinsic` / 输入处理 / 分配）。编译期的静态表就够，不需要“启动时注册”——运行时注册会带来初始化顺序问题，
  而且测试里直接构造 `Tree` 时没有“启动”这一步；
- 第二步（要支持第三方控件时再做）：只按现有 `NodeKind` 查表并不能让外部添加新节点，还需要一个开放的节点种类
  （例如 `NodeKind::Custom` + 带 `dyn Widget` 的数据载荷），并定义它的状态存储、命中测试、焦点与热更新快照怎么走；
- 目标是 `core` 不 import 具体组件，并由测试保证。

**验收**

- `rg "crate::components" crates/zant/src/core` 为空（或只剩那张分派表）；
- 第二步完成后：新增一个自绘示例控件不需要改 `core`；
- 现有截图/像素测试不回归，`zant-bench` 的帧时间不回退（绘制路径从直接调用变成查表）。

---

### 13. 文档组件分散在三层

- [x] 重构。

**位置**

- [widgets.rs](../crates/zant/src/components/widgets.rs) 同时放 `MarkdownOptions`（[第 31 行](../crates/zant/src/components/widgets.rs#L31)）、
  `PdfViewerOptions`（[第 184 行](../crates/zant/src/components/widgets.rs#L184)）和两个 builder（`markdown` 在第 108 行、`pdf_viewer` 在第 298 行）；
- [pdf_preview.rs](../crates/zant/src/ui/builders/pdf_preview.rs#L11-L12) 在通用 builder 目录里调 `platform::pdf` 和剪贴板。

**修复**

- 拆成 `components/markdown.rs`、`components/pdf_viewer.rs`；
- `pdf_preview` 移到 `pdf/preview.rs`，由 `ui` 在 `pdf` feature 下重导出；
- `ui/builders` 只保留基础原语。

**验收**

- `ui/builders` 不再 import `pdf` / `markdown`；
- 文档组件可以整体 `#[cfg(feature = "pdf")]` / `markdown`。

---

### 14. `Generator` 是 45 字段 God object + 30 多个函数指针

- [ ] 重构。

**位置**

- [codegen/mod.rs](../crates/zant-tsx/src/codegen/mod.rs#L474-L553)：`Generator` 有 45 个 `pub` 字段；
- [Extensions](../crates/zant-tsx/src/codegen/mod.rs#L397)（9 个函数指针字段 + 2 个分派列表）与 [Hooks](../crates/zant-tsx/src/codegen/mod.rs#L441)（23 个 `Option<fn>`），
  在 [generate.rs](../crates/zant-tsx/src/codegen/generate.rs#L121-L175) 逐个安装。

**现状**

模块之间互相调用，靠函数指针解耦；`current_component` / `child_component` / `type_scope` / `capture_scope` 构成隐式状态机。
emitter 并非完全不能单测：[codegen_jsx.rs](../crates/zant-tsx/tests/codegen_jsx.rs#L116-L155) 就是构造默认 `Generator`、按需装 hook 后直接调 `jsx::emit` / `events::emit_setter`。
但每个这样的测试都要手工装配整份上下文，知道该装哪些 hook。

**修复**

- 拆 `ModuleSink`（consts/types/enums/imports）、`ComponentCtx`（states/methods/hooks/effects）、`Walk`（counter/depth/handler_state）；
- `Extensions` / `Hooks` 改成带默认空实现的 trait，去掉 `Option<fn>` + `Some(...)` 装配；
- 每类 emitter 提供独立的输入/输出测试。
- 这是纯重构，没有用户可见收益，排在第 15、16 条之后做：那两条会改动同一批签名，先做它们可以少返工一次。

**验收**

- `jsx`、`components`、`events` 的测试只需要构造各自用到的那部分上下文，不再手工装 hook；
- 行为不变（golden 全通过）。

---

### 15. 诊断只有一条、内部错误 panic、thread_local 旁路

- [ ] 修复。

**位置**

- `Error::Tsx` 无负载： [codegen/mod.rs](../crates/zant-tsx/src/codegen/mod.rs#L134-L140)；
- `fail()` 只保留第一条： [codegen/mod.rs](../crates/zant-tsx/src/codegen/mod.rs#L142-L149)；
- `LAST_DIAG`： [generate.rs](../crates/zant-tsx/src/codegen/generate.rs#L85-L94)，由 [compile.rs](../crates/zant-tsx/src/compile.rs#L167) 取回；
- 生成代码解析失败直接 panic： [codegen/mod.rs](../crates/zant-tsx/src/codegen/mod.rs#L642-L649)。

**现状**

用户每次只能看到一个错误；编译器内部 bug 会在用户的 `build.rs` 里 panic 并打印整份生成代码。

**修复**（按收益排序，前两项独立、可以先做）

- 内部错误返回 `Error::Internal { source }`，CLI 打印“这是编译器 bug，请附上生成文件”，转储受 `ZANT_TSX_DUMP` 控制；不要把 panic 当作错误处理；
- `Error::Tsx(Diag)`，删除 `LAST_DIAG`，让 `Result` 自带信息；
- 多诊断要分层做，不能只把 `fail()` 改成往 `Vec<Diag>` 里追加：codegen 用 `?` 在第一个错误处返回，后面的代码根本不执行。
  现实的粒度是“每个文件 / 每个顶层函数、组件各报第一条”：在这些边界上捕获错误、记录后继续下一个；
  同一函数体内的多条错误要靠第 16 条的独立检查 pass；
- 跨边界继续时要避免连锁误报（前一个组件失败后，引用它的地方不要再报“未知组件”）；
- severity / warning 等有具体的警告需求时再加。

**验收**

- 两个组件（或两个文件）里各有一个独立错误时能一次报出；
- 内部错误不再有 panic backtrace；
- 现有诊断测试仍通过（它们断言的是第一条诊断）。

---

### 16. 类型/属性检查与代码生成耦合

- [ ] 重构。

**位置**

- 阶段顺序： [compile.rs](../crates/zant-tsx/src/compile.rs#L126-L260)（parse → generics::validate → hooks::expand → generate）；
- tag/prop 检查散在 codegen 里按需做：`jsx`、`jsx_values`（必填项）、[components.rs](../crates/zant-tsx/src/codegen/components.rs#L792) 的 `emit_call`（自定义组件的未知 / 必填 prop）等。

**现状**

类型检查与 emission 交织，第一个错误就中止；很难只做“检查”而**不**生成代码。

**修复**

- 增加 `check(module, &tags) -> Vec<Diag>`，先覆盖只依赖标签表、不依赖类型信息的检查：tag 存在、未知 prop、必填 prop、字面量枚举拼写、回调参数个数；
- 这些检查从 codegen 里移走后，codegen 对它们可以只做内部断言；
  需要作用域和类型的检查（条件表达式的类型、联合类型的枚举属性、表达式赋给 prop 时的类型匹配等）继续留在 codegen，不能假定“输入已全部检查过”；
- 表的规模（[ui_tags.rs](../crates/zant-tsx/src/ui_tags.rs#L14)）：38 个 tag、468 个 prop（含嵌套字段 492 个），另有 46 个主题字段，实现成本可控。

**验收**

- `check` 可以单独跑在测试里；
- 同一个文件里多个 tag / prop 错误一次报出；
- 同一个错误不会被 `check` 和 codegen 各报一遍；
- 生成产物不变。

---

### 17. `pending.rs` 里有两份手写的图标拼写表，新增图标会漏改

- [x] 修复。

**位置**

- `TSX_REGISTRY`： [ui/options.rs](../crates/zant/src/ui/options.rs#L810-L870)；
- 手写的图标拼写表（两份完整列表）： [pending.rs](../crates/zant-tsx/src/bin/zant-tsx-tags/pending.rs#L27)（`DropdownMenu` 的 `MenuItem.icon`）、
  [pending.rs](../crates/zant-tsx/src/bin/zant-tsx-tags/pending.rs#L52)（`Segmented` 的 `SegmentedOption.icon`）；
- 生成文件里同一份拼写出现 5 次： [ui_tags.rs](../crates/zant-tsx/src/ui_tags.rs#L228)、`L315`、`L322`、`L403`、`L416`。
  其中 3 次是生成器从 `Icon` 枚举派生的（`source.rs` 读枚举成员，`table.rs` 生成 snake / kebab / camel 三种拼写），只有上面两次来自手写；
- `Icon::ALL`： [icons.rs](../crates/zant/src/core/icons.rs#L108)。

**现状**

- 从枚举派生拼写的逻辑已经有了，生成物与源码的同步也有测试（[generated_tables.rs](../crates/zant-tsx/tests/generated_tables.rs#L73-L81) 的 `--check`，CI 也跑）；
  “每个 builder 都是 tag 或显式跳过”也已有测试（[tags.rs](../crates/zant-tsx/src/tags.rs#L671) 的 `every_rust_ui_builder_is_a_tag_or_skipped`）；
- 漏洞只在 `pending.rs` 的两份手写列表：新增图标后 `<Icon>` / `<Button icon>` 自动可用，`DropdownMenu` 与 `Segmented` 的 `icon` 却不认识它，
  没有任何测试会报。

**修复**

- `pending.rs` 的两处 `icon` 改为调用现有的枚举派生逻辑，删掉手写列表；
- 更彻底的做法是让 `DropdownMenu` / `Segmented` 也走 `#[derive(TsxProps)]`，从 pending 表里移走；
- 生成物里把重复 5 次的拼写表提成一个共享常量（减小 `ui_tags.rs`，非必须）；
- 加一个测试：所有 `icon` 枚举属性的取值集合都等于 `Icon::ALL` 派生出的拼写。

**验收**

- 新增一个图标只改 `core/icons.rs` 并重新生成表格，五处同时更新；
- 人为从 pending 表里删掉一个图标时，测试报错。

---

### 18. TSX 应用的构建/截图脚本和 Node 对照没有进 CI

- [ ] 修复。

**位置**

- [.github/workflows/rust.yml](../.github/workflows/rust.yml)（macOS + Windows）跑：bindings 同步检查、fmt、clippy、`cargo test --workspace`、生成表格检查；
- `cargo test --workspace` 已经包含 CLI 集成测试 [cli.rs](../crates/zant-cli/tests/cli.rs)：`create → sync`（第 96 行）、普通 Rust 应用的 compile / run（第 271 行）、
  macOS 打包成 `.app` 并检查可执行文件与 Info.plist（第 298 行）；[distribute.rs](../crates/zant-cli/src/distribute.rs#L380) 有 3 个单元测试；
- [release.yml](../.github/workflows/release.yml) 在打 `v*` 标签时跑两个平台的发布包脚本；
- 没有被任何 workflow 调用的： [build-template-check.sh](../scripts/tsx/build-template-check.sh)、[build-check.sh](../scripts/tsx/build-check.sh)、
  [examples-check.sh](../scripts/tsx/examples-check.sh)（TSX 模板 / 示例的 `build → headless 截图`）、
  [node-parity.sh](../scripts/tsx/node-parity.sh)（TSX 编译产物与 Node 输出逐行对照，见第 37 条）。

**现状**

PR 上已经有 `zant create` 和打包的基本信号；缺的是“TSX 模板项目真的能构建并出图”和“语言特性与 Node 一致”这两类验证，目前只能手工跑脚本。
`distribute/` 下的平台步骤（DMG / 公证 / Inno Setup / MSIX）只在发布时执行，平时没有信号。

**修复**

- CI 增加一个 job（至少 macOS）跑 `build-template-check.sh`；`examples-check.sh` 较慢，可放夜间或手动触发；
- `node-parity.sh` 进 PR CI（脚本要求 Node 22.6+ 的内置类型剥离，用 `actions/setup-node` 固定版本），这是语言特性回归最直接的信号；
- 打包测试补资源复制（图标、`locales/`、Windows 的 `pdfium.dll`）与 `distribute` 里不需要签名身份的步骤；
- Windows 的 [components-check.ps1](../scripts/win/components-check.ps1) / [resize-check.ps1](../scripts/win/resize-check.ps1) 已写在
  [development.md](development.md#L237-L240)，需要已登录桌面，CI runner 上跑不了，保持在测试机上定期执行。

**验收**

- PR 会构建一次 TSX 模板项目并产出截图；
- `node-parity.sh` 的失败会让 PR 变红；
- 打包资源缺失有自动化失败信号。

---

## 四、P1：通用性与适用性

### 19. 没有应用菜单 API，macOS 菜单只支持中/英

- [ ] 实现。

**位置**

- [window.m](../crates/zant-sys/native/platform/macos/window.m#L960-L1037)：固定菜单，`zant_menu_text(en, zh)` 按系统首选语言二选一；
- `App` trait 没有菜单钩子： [app.rs](../crates/zant/src/ui/app.rs#L70-L140)；
- 平台 ABI 也没有菜单导出。

**现状**

- 文档应用无法添加 `File > Open / Save / Export`；
- 菜单语言不跟随应用 i18n（系统中文、应用英文时仍是中文）；
- Windows 没有菜单栏。

**修复**

- 框架层定义 `Menu` / `MenuItem`（action id、label、shortcut、enabled、checked、separator、submenu）；
- `App::menu(&self) -> Option<Menu>` 或 `ui::set_menu`；
- macOS 合并进标准 Edit/Window 菜单；Windows 先用框架自绘菜单栏；
- 标准项使用 `i18n` key，应用项使用应用文案；菜单状态在 rebuild 时同步。

**验收**

- 示例应用能加“打开/保存”菜单项并触发 action；
- 切换语言后菜单文字变化；
- macOS/Windows 行为在文档中写清。

---

### 20. 窗口创建后不能改标题/尺寸/任意位置/全屏

- [x] 实现。

**位置**

- [WindowOptions](../crates/zant/src/platform/platform.rs#L196-L235) 只有创建参数（含 `min_width` / `min_height` / `always_on_top`，没有最大尺寸）；
- [platform.h](../crates/zant-sys/native/platform/platform.h#L190-L227) 没有 `set_title` / `set_frame` / `set_fullscreen`；
- [desktop/window.rs](../crates/zant/src/desktop/window.rs#L168-L240) 已有 `show` / `hide` / `activate` / `close`、状态查询（`is_visible` / `is_minimized` / `is_active`）、
  预设位置 `place(Place, margin)`、浮层用的 `cover_screen`、`focus(key)`；
- [ui/commands.rs](../crates/zant/src/ui/commands.rs#L164-L172) 已有 `window_minimize` / `window_maximize`（给自绘标题栏用）。

**现状**

已经能显示、隐藏、最小化、最大化 / 还原、放到预设位置。缺的是：运行时改系统标题（文档应用无法把文件名显示到标题栏）、
任意 frame / 尺寸、最大尺寸与运行时改尺寸上下限、普通窗口全屏、运行时切换置顶。

**修复**

- 增加 ABI/API：`set_title`、`set_frame`（或 `set_size` + `set_position`）、`set_min_size` / `set_max_size`、`set_fullscreen`、`set_always_on_top`；
- `WindowOptions` 补 `max_width/max_height`、显式 `x/y`、`fullscreen`（或在 `Window` 上设置）；
- TSX 在现有的 `mainWindow` / `WindowHandle`（[zant.d.ts](../crates/zant-tsx/zant.d.ts#L108-L120)）上补 `setTitle(...)` / `setSize(...)` / `toggleFullscreen()`，
  和已有的 `place` / `focus` 一样限事件与 effect 里调用；
- 标题在 runtime 保存，重建时只在变化时下发；
- 改了 `platform.h` 要同步 `zant-sys` 的 bindgen 绑定（`scripts/gen_bindings.sh`），macOS 与 Windows 两套实现都要补。

**验收**

- 多窗口示例中每个窗口标题可独立变化；
- 窗口尺寸/全屏在 macOS 与 Windows 都生效；
- 记忆尺寸与手动设置不冲突。

---

### 21. 缺少通用覆盖层/弹层管理

- [ ] 实现。

**位置**

- [dropdown-menu.md](components/dropdown-menu.md#L23-L25)：菜单不会自动定位、翻转或关闭；
- TSX 没有 `keyBounds`；
- [message.rs](../crates/zant/src/components/message.rs#L35-L52)：没有时长/队列；
- 应用需要自己放根容器、加 `DismissLayer`、算窗口坐标。

**现状**

每个菜单、Popover、Drawer、Toast 都把布局/窗口坐标泄漏进应用代码。

**修复**

- 运行时 overlay 栈：锚点（key/rect）、placement + flip、边缘避让；
- 统一 `DismissLayer` / Esc 栈、焦点保存/恢复、z 顺序；
- 提供 `Popover`、`Drawer`、`Toast`；`Message` 增加 duration/queue；
- 在 `DropdownMenu` 内部使用 overlay，不再要求应用自己定位。

**验收**

- 菜单在窗口边缘自动上翻/翻转；
- 点击外部、Esc、窗口 resize 都正确关闭；
- 焦点在打开/关闭时保存与恢复；
- 示例应用不再手算 `key_bounds`。

---

### 22. 组件覆盖面对通用桌面应用偏窄

- [ ] 补齐。

**现状**

TSX 只有 39 个组件/原语（见 [zant.d.ts](../crates/zant-tsx/zant.d.ts)）。缺少常见的：

- 数据展示：Table（排序/选择/虚拟行）、Tree、Pagination、Breadcrumb、Steps、Descriptions；
- 表单：Form/Field 校验、DatePicker、Slider、Autocomplete、Upload；
- 容器/反馈：Drawer、Popover、Popconfirm、Notification、Skeleton、Empty、Spin。

**修复**

建议顺序（每项都按 antd 5 规格，并进 [components_app](../crates/zant-examples/src/components_app) + Windows [components-check.ps1](../scripts/win/components-check.ps1)）：

1. Table（先只读 + 虚拟行，再排序/选择）；
2. Form/Field + 校验错误态；
3. DatePicker / Calendar；
4. 富 Select（value/label/disabled/multi/search）；
5. Drawer / Popover；
6. Pagination / Breadcrumb / Steps / Upload。

**验收**

- 每个组件有文档、深/浅色截图、可交互示例；
- 键盘与焦点行为按 antd；
- 不引入应用专属逻辑。

---

### 23. 框架内置文案无法本地化，Markdown labels 未从 TSX 暴露

- [ ] 实现。

**位置**

- Markdown `Labels` 默认英文： [options.rs](../crates/zant/src/markdown/render/options.rs#L195-L213)；
- TSX `MarkdownProps` 没有 `labels`： [zant.d.ts](../crates/zant-tsx/zant.d.ts#L2112-L2140)；
- PDF 阅读器的 `Labels` 默认英文，另有一个公开的中文预设 `Labels::ZH`： [pdf/viewer/mod.rs](../crates/zant/src/pdf/viewer/mod.rs#L133-L160)（只是可选预设，默认值不用它）；
  Markdown 的 `Labels` 同样带一份中文预设（[options.rs](../crates/zant/src/markdown/render/options.rs#L216)）；
- 已经能从 TSX 逐项覆盖的：`HotkeyInput` 的 `placeholder` / `recordingText`（[zant.d.ts](../crates/zant-tsx/zant.d.ts#L1909-L1910)）、
  `PdfViewer` 的 `emptyText` / `loadingText` / `errorText`（[zant.d.ts](../crates/zant-tsx/zant.d.ts#L2158-L2160)）；
- 完全没有覆盖入口的：`PdfPreview` 的文案写死在 [pdf_preview.rs](../crates/zant/src/ui/builders/pdf_preview.rs#L59) 里，Rust 的 `PdfOptions` 也没有对应字段。

**现状**

框架默认文案不跟随应用的 i18n。TSX 能手工覆盖其中一部分（上面列的几项，应用自己 `t()` 后传入）；
Markdown 的 labels（Copy / Download 等）和 PDF 阅读器工具栏的 labels 只能从 Rust 传；`PdfPreview` 两边都改不了。

**修复**

- 框架自带 `zant.*` catalog（英文 + 中文起步），各组件的默认文案按 `i18n::context::current_tag()`（[context.rs](../crates/zant/src/i18n/context.rs#L268)）解析；
  应用没有启用 i18n 时保持英文（框架默认英文）；
- `Labels::default()` 现在是不依赖运行时状态的纯值，Markdown 的 labels 还参与缓存哈希（`hash_labels`）。改成“按当前语言解析”时，
  语言切换必须让用到默认 labels 的节点失效重建，这一点要有测试；
- TSX 给 `Markdown` / `PdfViewer` 暴露 `labels`（对象，逐项可选），显式传入的优先于 catalog；
- 两个中文预设（PDF 的 `Labels::ZH` 与 Markdown 的那份）是公开 API：catalog 就绪后标 `#[deprecated]` 并让它们取 catalog 的中文值，不要直接删；
- `PdfPreview` 的文案也走 catalog（或按第 25 条并入 `PdfViewer`）；
- 菜单标准项（第 19 条）走同一套 key。

**验收**

- 应用切到中文时，Markdown / PdfViewer / PdfPreview / HotkeyInput 的默认文案跟随，且不需要重启；
- TSX 不改任何代码即可本地化，显式传入的文案仍然优先；
- 框架里只有 catalog 这一处存放翻译。

---

### 24. 主题 token 不完整，也不能从 TSX 定制

- [ ] 实现。

**位置**

- [tokens.rs](../crates/zant/src/theme/tokens.rs#L44-L67)：没有阴影/elevation、字体族、行高 token；
- 预设只有 `ant_light` / `ant_dark`。Rust 侧可以覆盖：`Theme` 的字段全部公开，`RunOptions::theme` 与 `App::theme()`（[app.rs](../crates/zant/src/ui/app.rs#L89-L93)）
  接受任意 `Theme`。缺的是从 seed 派生整套色板的算法（改 primary 后 hover / active / 浅底色不会跟着算）和 TSX 侧的写入口（`useTheme()` 只读）；
- Modal/Menu/Message/Segmented/Select 重复阴影字面量；
- `PdfPreview` 的失败占位用了固定的浅色底（`0xfff5f5f5`）和固定文字色： [pdf_preview.rs](../crates/zant/src/ui/builders/pdf_preview.rs#L53-L70)；
  外层背景来自 options（默认无）、工具栏按钮走主题，并不是整体忽略主题。

**修复**

- 补 elevation / typography token；替换重复字面量。z-index token 要等第 21、29 条定下层叠顺序才有意义，届时一起设计；
- `ThemeSeed { color_primary, radius, font_size }` + 按 antd 的色板算法派生完整 `Theme`（浅 / 深各一套），再提供 `Theme::merge(base, overrides)` 做逐项覆盖；
- TSX 侧：主题是整个应用一份（`useTheme()` 只读），TSX 也没有 context 机制，不必引入 `ThemeProvider` 组件。
  在 `app.config.json` 里声明 seed，或提供 `app.setTheme(seed)` 这样的命令式入口；
- `PdfPreview` 的占位改用当前主题的 token。

**验收**

- 只改 primary 色/圆角能贯穿所有组件（含 hover / active / 禁用态），浅色、深色都正确；
- 深色模式下 PDF 预览的占位不再是亮块（PDF 页面本身是白纸，不算）；
- 阴影/字号来自 token，而不是散落常量。

---

### 25. `PdfPreview` 在 build 路径同步打开并渲染 PDF

- [ ] 修复。

**位置**

- [pdf_preview.rs](../crates/zant/src/ui/builders/pdf_preview.rs#L28-L42)：节点构造时同步调 `pdf::page_count` 和 `pdf::render_page`；
- 原生层有一份 PNG 文件缓存（macOS： [pdf.mm](../crates/zant-sys/native/platform/macos/pdf.mm#L18-L22)，按路径 / 页码 / 尺寸命名的临时文件，存在即返回；
  Windows： [pdf_win.cpp](../crates/zant-sys/native/platform/windows/pdf_win.cpp#L301-L310)），命中时不重新栅格化；
  但每次重建仍要同步打开文档、查页数，未命中时栅格化也阻塞在构造路径上，没有后台 worker；
- macOS 的缓存 key 不含文件修改时间（Windows 的含）：同一路径的文件内容变了，macOS 上仍然拿到旧图；
- 失败占位已有（“Can't preview this PDF”），但文案、占位颜色、`‹` / `›` 字形是硬编码（主题与文案部分见第 23、24 条）。

**修复**

- 首选：给 `PdfViewer` 加一个单页 / 缩略图模式，再废弃 `PdfPreview`（`PdfViewer` 已有 runtime slot、后台 worker、页面缓存和主题），避免维护两条 PDF 渲染路径；
- 若保留：改成 runtime slot + 后台渲染 + `post_wake`，增加 loading 状态，macOS 的缓存 key 加入文件修改时间；
- 按钮/翻页 glyph 用 `Icon`，不要用字符。

**验收**

- 重建窗口不在 UI 线程上打开或栅格化 PDF（`ZANT_FRAME_STATS=1` 下首帧与翻页不出现 `[slow-frame]`）；
- 文件被覆盖后预览更新；
- 深/浅色与窗口尺寸变化正确，无文档时显示有主题的占位。

---

### 26. 缺少可访问性支持

- [ ] 立项。

**现状**

- [AGENTS.md](../AGENTS.md#L386) 已列为已知限制；
- 核心组件没有 role/label/value/state 的 a11y 语义；
- 没有 NSAccessibility / UIA 桥。

**修复（分阶段）**

1. `RetainedNode` 增加 `role` / `label` / `value` / `state`；
2. Button/Input/Select/List/Modal/Tabs 先补语义；
3. macOS 接 NSAccessibility，Windows 接 UIA；
4. 键盘焦点与读屏顺序统一；
5. 发布前提供 a11y 检查清单。

**验收**

- VoiceOver / Narrator 能读出按钮、输入框、列表项；
- 焦点顺序与视觉顺序一致；
- 新增组件必须填 a11y 语义。

---

## 五、P2：质量与一致性

### 27. Markdown 的 `content_width` 与根节点宽度语义不一致

- [ ] 修复。

**位置**

- 根节点：Streamdown 的根默认 100% 宽，Markdown 外壳清掉了这个百分比，让父布局和 `styled` 决定宽度： [widgets.rs](../crates/zant/src/components/widgets.rs#L170-L175)；
- 公式与图片的 `max_width` 取 `content_width`： [blocks.rs](../crates/zant/src/markdown/render/blocks.rs#L408)（公式）、[第 487 行](../crates/zant/src/markdown/render/blocks.rs#L487)（图片）；
  Mermaid 取 `content_width - 32`： [diagram.rs](../crates/zant/src/markdown/render/diagram.rs#L29)；
- 默认值 680： [options.rs](../crates/zant/src/markdown/render/options.rs#L277) 与 [widgets.rs](../crates/zant/src/components/widgets.rs#L69)。

**现状**

正文跟随容器宽度，公式 / 图片 / 图表却按固定的 `content_width`（默认 680）限宽：容器比 680 宽时它们比正文窄，容器比 680 窄时可能溢出，除非应用自己把容器宽度传进 `content_width`。

**修复**

- 公式和图表的排版发生在构建节点时，那时还不知道父容器的宽度，所以“默认跟随父容器”不能只把类型改成 `Option<f32>`：
  要么用外壳上一帧的布局宽度（`onResize` / `key_bounds` 同源的数据）作为 `content_width` 并在宽度变化时失效重建，
  要么把限宽下放到布局阶段（节点自身 `max_width: 100%`，绘制时按实际宽度缩放）；
- 如果保留固定内容列，正文/公式/图片统一套同一个 `max_width` wrapper；
- 宽度变化会让公式 / 图表的缓存失效，流式输出时不能每帧重排（改完用 Release 的 `zant-md-stream` + `ZANT_FRAME_STATS=1` 复测）；
- 文档说明宽窗口下的行为。

**验收**

- 宽窗口下正文与公式对齐；
- 窄窗口不溢出；
- 现有 Markdown 截图更新。

---

### 28. 平台分支泄漏进通用组件

- [x] 清理。

**位置**

- `ShortcutList` 按 `IS_MACOS` 选 250/310： [shortcut_list.rs](../crates/zant/src/components/shortcut_list.rs#L62-L66)；
- `HotkeyInput` 自带一个 `Shortcut` 类型和它的标签 / 键帽拆分逻辑： [hotkey_input.rs](../crates/zant/src/components/hotkey_input.rs#L92-L145)、
  [第 273 行](../crates/zant/src/components/hotkey_input.rs#L273)。它重复的是 [desktop/hotkey.rs](../crates/zant/src/desktop/hotkey.rs#L50) 的 `Shortcut::label`
  （文件顶部自己留了 TODO：改用 `desktop::Shortcut`），不是 `keys::label`；
- 通用 `Button` 带 Windows caption 颜色： [button.rs](../crates/zant/src/components/button.rs#L27-L29)，`ButtonType::Caption` / `CaptionClose` 是给 `WindowControls` 用的。

`components/` 里共 13 处 `IS_MACOS`（没有 `IS_WINDOWS`）：非测试代码 5 处，其余 8 处在测试里断言各平台的期望文案。

**修复**

- 列宽改为测量文本或平台 capability；
- `HotkeyInput` 删掉自带的 `Shortcut`，改用并重导出 `desktop::Shortcut`，键帽拆分也做成它的公共方法。
  不能直接换成 `keys::label`：`keys` 处理的是抽象的 `mod`（Windows 上显示 `Ctrl`），而录制到的是实体修饰键，Win 键要显示成 `Win`；
- caption 颜色移到 platform/theme；
- `ButtonType::Caption` 明确标注为平台专用，或拆出通用按钮。

**验收**

- `components/` 的非测试代码不再出现 `IS_MACOS` / `IS_WINDOWS` 分支；测试里按平台断言期望值可以保留；
- 快捷键标签只有一份实现，各平台按各自习惯显示（macOS 用符号，Windows 用 `Ctrl+Shift+S` 形式），两个平台的现有标签测试不回归。

---

### 29. 布局缺少 z-index、横向滚动、Grid

- [ ] 实现。

**位置**

- [BoxOptions](../crates/zant/src/ui/options.rs#L47-L67) 没有 `z_index`；
- `scroll` 只有纵向；
- 没有 Grid 容器；
- 纵向 wrap / auto / calc 仍是限制： [flex.md](flex.md#L52)。

**修复**

- `z_index`：绘制、命中测试、悬停、焦点 / dismiss 的“最上层”判断必须用同一个顺序。
  现在命中测试按 children 逆序走（[hit.rs](../crates/zant/src/core/tree/hit.rs#L21-L26)），焦点范围的查找也依赖 children 顺序；
  只改绘制顺序会出现“弹层画在上面，点击却落到下面的控件”；
- `scroll_x` / 二维滚动（含横向滚动条、Shift+滚轮与触控板横向手势）；
- Grid 容器（列定义 + gap）。等宽列现在可以用换行的 flex（`wrap`）加百分比宽度凑出来，Grid 解决的是跨行对齐和不等宽轨道，优先级低于前两项；
- 与 overlay manager（第 21 条）一起设计，避免两套层叠逻辑。

**验收**

- 弹层/拖拽 ghost 可以置顶，且重叠区域的点击、悬停落在最上层的节点上（加重叠交互测试）；
- 宽表格/代码块可以横向滚动；
- 简单 grid 布局不需要绝对定位。

---

### 30. 没有自定义字体、SVG、通用图片加载器

- [ ] 实现。

**位置**

- 字体只有 5 个 `Family`： [text.rs](../crates/zant/src/renderer/text.rs#L33-L40)；
- 文件格式只有 PNG/JPEG/GIF/BMP/ICO，没有 SVG： [image.md](components/image.md#L6-L13)；
- 普通 `Image` 接受文件路径 `src` 或内存图片 `image`（[options.rs](../crates/zant/src/ui/options.rs#L461-L464)，TSX 的 `<Image image>` / `assetImage()`）。
  应用可以自己下载、解码后传入，所以远程图片不是做不到，而是下载、缓存、加载状态都要应用自己写；
- Markdown 的 `ImageLoader`（[options.rs](../crates/zant/src/markdown/render/options.rs#L30-L40)）只是一个 `Fn(&str) -> Option<Image>` 回调协议，
  下载和缓存同样由应用实现，框架里没有现成的远程加载器可以“提升”。

**修复**

- 字体注册表：`register_font(bytes)` + `Family::Named`；
- SVG 建议在构建期栅格化，避免引入完整 SVG 运行时；
- 新写一个框架级远程图片加载器：后台下载（依赖 `fetch` feature，见第 4 条）+ 解码 + LRU + `post_wake`，并实现现有的 `ImageLoader` 回调协议，
  这样 Markdown 和普通 `Image` 共用一份；
- `ImageOptions` / `<Image>` 增加 URL 入口与加载中、失败占位。

**验收**

- 应用能加载品牌字体；
- 普通视图写一个 URL 就能显示远程图片，Markdown 里的远程图片不需要应用再提供 loader；
- 图片缓存有上限，并在窗口销毁时释放。

---

### 31. 焦点 API 不完整（tab_index / auto_focus / 焦点恢复）

- [ ] 实现。

**位置**

- [focus.rs](../crates/zant/src/core/tree/focus.rs#L145-L183)；
- 已有的：命令式聚焦 `ui::focus(key)`（[commands.rs](../crates/zant/src/ui/commands.rs#L147)，TSX 为 `mainWindow.focus(key)` / 窗口句柄的 `focus(key)`）、
  `focus_scope` / `focusScope`（Tab 不出范围）、`focusable`；
- Modal 只设 `focus_scope`，打开不聚焦第一个控件，关闭不恢复；
- `BoxOptions` 没有 `tab_index` / `auto_focus`。

**修复**

- `BoxOptions` 增加 `tab_index` / `auto_focus`；
- runtime 维护焦点栈；Modal 打开时聚焦第一个可聚焦控件，关闭时恢复；
- Tabs / Select 用 roving tabindex；
- 把“打开即聚焦”和“Esc 关闭后恢复”写进组件测试。

**验收**

- 键盘可以完整操作 Modal/Tabs/Select；
- 关闭弹层后焦点回到触发控件；
- 不与 `focus_scope` 冲突。

---

### 32. Rust 侧的有状态组件用数字 id，其他组件用 key

- [ ] 统一（只涉及 Rust API）。

**位置**

- Rust 的 `markdown(id: u64, options)` / `pdf_viewer(id: u64, options)`： [widgets.rs](../crates/zant/src/components/widgets.rs#L108)、[widgets.rs](../crates/zant/src/components/widgets.rs#L298)；
- 其他组件用 `key`；
- TSX 没有这个问题：编译器已经从 `key`（没有 key 时从调用点）派生稳定 id，并把父实例和标签名算进去
  （[jsx.rs](../crates/zant-tsx/src/codegen/jsx.rs#L1066-L1088) 的 `tsx::child_id` / `tsx::site_id`），只在循环里强制要求写 `key`。

**修复**

- Rust 侧增加按 key 取状态槽的入口。Rust 没有函数重载，不能“保留 `id` 重载”：
  新增不同名的函数（例如 `markdown_keyed(key, options)`）最稳妥；把参数改成 `impl Into<SlotId>` 也可以，
  但传整数字面量的现有调用可能因类型推断失败而要改写成 `1u64`，先编译确认；
- 从 key 派生 id 时沿用 TSX 运行时的做法，把父实例 / 组件类型作为命名空间（[tsx/ids.rs](../crates/zant/src/tsx/ids.rs#L20)），避免不同位置的相同 key 撞槽；
- TSX 侧不用改，继续不暴露数字 id；
- 文档说明：列表里的实例必须给稳定 key；静态位置可以省略。

**验收**

- Rust 的两种写法与 TSX 都可用且行为一致；
- 同一命名空间下重复的 key 在 Debug 构建里给出明确警告（或文档写清不允许）。

---

### 33. PNG 代码在 CLI 与 zant-build 中重复

- [x] 清理。

**位置**

- [png.rs](../crates/zant-cli/src/png.rs) 与 [png.rs](../crates/zant-build/src/png.rs)：仅注释/变量名不同；注释自己写“the same code”。

**修复**

- `zant-cli` 已依赖 `zant-build`：把 `zant_build::png` 公开，CLI 直接使用；
- 或抽一个 `zant-assets` 叶 crate，两边都依赖；
- 删除手工保持“逐字节一致”的注释。

**验收**

- 只剩一份实现；
- 图标/ICO 测试仍通过。

---

### 34. 两个 `layout` 模块命名冲突

- [x] 清理。

**位置**

- [zant/src/layout/flex.rs](../crates/zant/src/layout/flex.rs#L1)：flex 类型 + 分配算法；
- [core/layout/mod.rs](../crates/zant/src/core/layout/mod.rs#L1)：布局引擎；
- 应用到处写 `zant::layout::flex`。

**修复**

- 改名为 `zant::flex`，或移到 `core::layout::style`；
- 更新所有 import 与文档；
- 在 [architecture.md](architecture.md) 只保留一个布局入口。

**验收**

- 不再有两个 `layout`；
- 编译与截图不回归。

---

### 35. 测试夹具放在仓库根 `tests/fixtures`，不属于任何 package

- [ ] 清理（只在决定发布到 crates.io 时才有实际影响，见第 36 条）。

**位置**

- 根 [tests/fixtures](../tests/fixtures)：`remend_cases.json`（522 条）、`widgets.pdf`；
- 使用者主要是 `zant` 自己的集成测试，通过 `CARGO_MANIFEST_DIR/../../tests/fixtures` 引用：
  [remend_test.rs](../crates/zant/tests/remend_test.rs)、[pdf_test.rs](../crates/zant/tests/pdf_test.rs)、
  [pdf_viewer_test.rs](../crates/zant/tests/pdf_viewer_test.rs#L43)、[widgets_test.rs](../crates/zant/tests/widgets_test.rs#L37)；
- `zant-examples` 的 PDF 示例把 `tests/fixtures/widgets.pdf` 当默认文档（[pdf_app.rs](../crates/zant-examples/src/pdf_app.rs#L26)，相对仓库根的路径）。

**现状**

在仓库里一切正常。问题只出现在 `cargo package` / `cargo publish`：打出来的 `zant` 包不含 package 目录之外的文件，包内测试找不到夹具。

**修复**

- 夹具放进使用它的 package：两份都移到 `crates/zant/tests/fixtures/`，`zant-examples` 改为引用那里（它本来就依赖仓库布局）；
  移到 `crates/zant-examples` 下解决不了问题，因为主要使用者是 `zant`；
- 更新所有引用路径（测试、示例、`tests/fixtures/README.md` 及其他提到该路径的文档）；
- 如果第 36 条决定不发布，这条可以关闭，只在文档里注明夹具位置。

**验收**

- `cargo package -p zant --list` 包含夹具；
- `cargo test --workspace` 不回归。

---

### 36. 发布元数据与示例 workspace 成员不一致

- [ ] 决策。

**现状**

- 只有 `zant-dylib` 与 `zant-hot-host` 标了 `publish = false`；其余 crate 名义上可发布，但没有 `repository` / `readme` / `keywords` / `categories`；
- `zant-sys` 依赖仓库外的二进制资产（Skia、PDFium、字体）：源码树里没有 `third_party/` 时，[build.rs](../crates/zant-sys/build.rs#L186-L212) 会回退到已安装的发布包
  （`$ZANT_HOME/zant-ui`，默认 `~/.zant` / `%LOCALAPPDATA%\zant`）；两处都没有的干净环境无法构建，所以它还不是一个能独立从 crates.io 使用的包；
- 7 个 TSX 示例是 workspace 成员，`cargo build/test --workspace` 每次都构建它们，成本目前没有量化。

**修复**

- 如果暂不发布：统一 `publish = false`，文档写清“只从源码/安装包使用”；
- 如果要发布：补元数据，`zant-sys` 在找不到资产时给出指向安装脚本的错误信息，并做 `cargo publish --dry-run`；
- 示例成员先量化再决定：分别记录带 / 不带这 7 个成员的 `cargo build --workspace` 冷、热构建时间。
  要减少成员的话，[examples-check.sh](../scripts/tsx/examples-check.sh) 是按 package 名逐个构建并运行这 7 个示例的，必须同步改；
  `zant-examples/build.rs` 现有的生成机制只服务基准应用（复制 `App.tsx` 和指定的 view 文件），不能直接拿来生成完整示例；
  更简单的替代是保留成员、用 `default-members` 把它们排除在日常的 `cargo build` 之外。

**验收**

- 发布决策与 manifest 一致；
- 有构建时间的前后对比数据；7 个示例仍然被 `examples-check.sh` 覆盖；
- 不会发布一个无法构建的 crate。

---

### 37. 编译器测试缺口（平台常量、Date.now、Node 对照未进 `cargo test` 等）

- [x] 补测试。

**缺口**

- RegExp：`exec` / `match` / `matchAll` / `replace` / `split` / `search` 的行为已经由 Node 对照探针覆盖
  （[node_parity/regexp.ts](../crates/zant-tsx/tests/node_parity/regexp.ts)、[strings_regex.ts](../crates/zant-tsx/tests/node_parity/strings_regex.ts)：编译、运行、与 Node 输出逐行比较），
  [codegen_regexps.rs](../crates/zant-tsx/tests/codegen_regexps.rs) 的 2 个测试另外覆盖了字面量校验、`exec` 循环和几条诊断。
  真正的缺口是 [node-parity.sh](../scripts/tsx/node-parity.sh) 是手动脚本，`cargo test` 和 CI 都不跑它（见第 18 条）；
- `isMacos` / `isWindows` 都没有被断言：[codegen_app.rs](../crates/zant-tsx/tests/codegen_app.rs#L586) 只是导入了 `isMacos`，组件里没有用，也没有检查生成的 `IS_MACOS` / `IS_WINDOWS`；
- `Date.now` 没有直接测试，只靠 golden（`testdata/tsx/builtins.tsx`）；
- 没有“解构局部变量被命名处理函数捕获”的测试（第 5 条）；
- 没有“无 props 子组件 / 错误角色里的应用 hook”的测试（第 1 条）；
- 没有“组件内 `function`、数组回调、`renderItem`、`.then` 回调里的穷尽 switch”的测试（第 2 条）。

**修复**

- 逐项补最小测试，优先覆盖本清单 1、2、5；
- `isMacos` / `isWindows` 各加一个真正用到它并断言生成代码的测试；`Date.now` 加一个直接断言；
- 让每个 ✅ 路标行至少有一个指名测试（见第 40 条）。

**验收**

- 针对第 1、2、5 条的回归测试在修复前失败、修复后通过；针对已支持能力（平台常量、`Date.now`）的补充测试在当前代码上就应该通过；
- 不再出现“只有 golden、没有行为断言”的关键能力。

---

### 38. i18n 测试在沙箱写目录失败，`cargo test` 不是沙箱健壮的

- [x] 修复。

**位置**

- [context.rs](../crates/zant/src/i18n/context.rs#L382-L399)：`a_remembered_choice_is_read_on_the_first_install` 在真实的应用数据目录下
  （`preference_file` → `core::paths` 的应用数据目录：macOS 是 `~/Library/Application Support/<app_id>`，Windows 是 AppData）建目录、写文件，
  对 `create_dir_all` / `write` 直接 `unwrap()`；
- 沙箱不允许写这个目录，测试在创建目录或写文件时 panic（`PermissionDenied`）。

**修复**

- 给 `preference_file` 加一个仅测试可用的存储位置注入（或让 `Setup` 带一个可选的目录覆盖），预写、`install_with`、`set_locale_with` 三步共用
  `std::env::temp_dir()` 下的唯一目录。只把预写位置换成临时目录是不够的：`install` 读的仍然是 `preference_file` 算出来的真实路径；
- 不要用“写失败就 `return`”来处理：那样测试在受限环境里静默跳过，等于没有验证；
- 测试结束清理临时目录（现在如果中途断言失败，会在真实数据目录里留下 `zant-i18n-test-<pid>`）；
- 顺手排查其他测试是否也写真实的用户目录（`rg "app_data_dir|paths::" crates/zant/src --glob '*.rs'` 里位于 `#[cfg(test)]` 的调用）。

**验收**

- 在应用数据目录只读的环境里（macOS 的 Application Support、Windows 的 AppData）`cargo test --workspace` 仍通过；
- 测试仍然验证“首次安装读取已保存选择”，且不在用户目录留下文件。

---

### 39. 组件字段与 antd 语义的缺口（Select/Button/Tabs）

- [ ] 补齐。

**位置**

- `SelectOptions` 的选项模型只是 `Vec<Str>` + 选中下标（另有 `width` / `height` / `disabled` / `tooltip` / `on_change`）： [options.rs](../crates/zant/src/ui/options.rs#L489-L498)；
  没有独立的 value、逐项禁用、placeholder、多选、搜索；
- `ButtonOptions` 没有 loading/danger/block： [options.rs](../crates/zant/src/ui/options.rs#L263-L282)；`ButtonType` 也没有危险变体。
  带图标的按钮已有（`ButtonOptions.icon`），纯图标按钮已有专门的 `IconButton`；
- `TabsOptions` 已是受控的（`active` / `on_change`），但 `TabItem` 只有 label/content： [options.rs](../crates/zant/src/ui/options.rs#L432-L448)。

**修复**

- `SelectOption { value, label, disabled }` + `placeholder` + `multiple` + `search`（与第 22 条的“富 Select”是同一件事，只做一次）；
- Button 增加 `loading`、`danger`、`block`；文档写清 `Button icon` 与 `IconButton` 的分工；
- `TabItem` 增加 icon、禁用；Tabs 增加 roving focus（依赖第 31 条）、溢出滚动（依赖第 29 条的横向滚动）；
- 按 antd 5 的尺寸/状态实现，并更新 `docs/components/*.md` 与截图；`#[derive(TsxProps)]` 的字段变了要重新生成 `ui_tags.rs` / `zant.d.ts`。

**验收**

- 与 antd 的常用属性一一对应；
- 深/浅色、禁用/加载/聚焦状态都有测试或截图。

---

### 40. 文档与路线图缺少“✅ 必须有测试”的约束

- [ ] 建立机制。

**现状**

路标/文档的 ✅ 与测试之间没有强制关系；本次就发现了几个 ✅ 行缺少行为断言（第 37 条），以及多个 ✅ 行与代码不一致或写得过宽（第 1、2、5–10 条）。

**修复**

- 路标的一行往往包含好几项能力（例如第 28 行同时有 Image、HotkeyInput、`isMacos` / `isWindows`、`keyLabel` 等），所以映射是一对多的：
  维护一张“能力 → 测试名 / Node 对照探针”的表（可以就放在 [tsx-roadmap.md](tsx-roadmap.md) 旁边），按能力而不是按行登记；
- CI 加一个轻量检查：表里登记的测试名真实存在，并且会被 CI 执行（`node_parity` 探针要先按第 18 条进 CI）；
- 新增能力先写测试，再把 ⬜ 改 ✅；
- 这个检查只能防止“忘了登记测试”，不能证明测试断言了对的东西（第 37 条里“导入了 `isMacos` 却没用它”就是例子），评审时仍要看断言内容。

**验收**

- 每项标 ✅ 的能力都能在表里找到至少一个行为测试或 Node 对照探针；
- 表里的测试名失效（改名、删除）时 CI 报错。

---

## 六、建议执行顺序

### 里程碑 1：正确性与文档对齐（1 周内）

- 第 1 条 hooks 作用域；
- 第 2 条穷尽 switch；
- 第 5 条解构捕获；
- 第 6–11 条文档 / 路标对齐（其中第 7、8 条顺带补 `Math.max(...xs)` 与 `Object.*` 的诊断）；
- 第 38 条测试不写真实用户目录；
- 第 37 条补对应回归测试；
- 第 18 条里的 `node-parity.sh` 进 CI（先做，后面每次改编译器都受它保护）。

**完成标准**：新增测试全绿；`cargo test --workspace` 在沙箱通过；根/窗口/子组件 hooks 行为与文档一致；路标与 `docs/tsx.md` 不再有与代码矛盾的 ✅。

### 里程碑 2：构建与解耦（1–2 周）

- 第 3 条：先下沉叶 crate，再拆 `zant-tables`，最后去掉 `zant-tsx → zant`；
- 第 13 条文档组件归位（放在第 4 条之前，门控时只需要给整个文件加 `#[cfg]`）；
- 第 4 条 feature 门控 + pdfium 条件化 + 去掉 `math → markdown` 的重导出；
- 第 33 条合并 PNG；
- 第 34 条 `layout` 改名（纯机械改动，和上面的模块搬迁一起做，少一轮全仓 import 变更）。

**完成标准**：`cargo tree -p zant-tsx -e normal,build` 无 `zant` / `zant-sys`；无 PDF/DB 的应用不编译这些模块、Windows Release 不要求 pdfium；构建时间与体积有前后对比数据。

### 里程碑 3：框架 API（2–4 周）

- 第 21 条 overlay manager，连同第 29 条的 z-index（同一套层叠顺序）与第 31 条的焦点保存 / 恢复；
- 第 19 条菜单与第 23 条内置文案（共用 `zant.*` catalog，先做 23）；
- 第 20 条窗口 setter；
- 第 24 条主题 token/seed；
- 第 25 条 PdfPreview（并入 PdfViewer 或异步化）；
- 第 28、32 条小清理。

**完成标准**：示例应用能加菜单/改标题/弹层自动定位；切换语言后框架文案跟随；深色模式无亮块。

### 里程碑 4：通用能力与长期架构（持续）

- 第 15、16 条诊断与 check pass，之后再做第 14 条 Generator 拆分；
- 第 12 条 core 分派表（第三方控件需求出现时再做开放节点）；
- 第 29 条的横向滚动与 Grid、第 30 条字体 / SVG / 远程图片、第 27 条 Markdown 宽度；
- 第 22、39 条组件补齐（Tabs 溢出依赖横向滚动，Select 搜索依赖 overlay）；
- 第 26 条 a11y；
- 第 18 条其余部分（模板构建截图、打包资源测试）与第 40 条的能力 ↔ 测试映射；
- 第 35、36 条等发布决策定了再处理。

**完成标准**：`core` 不 import 具体组件；多个组件 / 文件的错误能一次报出；通用组件与 a11y 覆盖主要场景。

---

## 七、验证命令

```bash
# 基础质量
./scripts/cargo.sh fmt --all -- --check
./scripts/cargo.sh clippy --workspace --all-targets -- -D warnings
./scripts/cargo.sh test --workspace

# 编译器探针（按需；探针文件放在仓库外的临时目录）
./scripts/cargo.sh build -p zant-tsx --bin zant-tsx
target/debug/zant-tsx "$TMPDIR/probe/app.tsx" "$TMPDIR/probe/out.rs"                      # 单文件
target/debug/zant-tsx --out "$TMPDIR/probe/out" --src "$TMPDIR/probe/src" --main App     # 多文件项目（跨文件导入要用这种）

# 语言特性与 Node 对照（需要 Node 22.6+）
./scripts/tsx/node-parity.sh

# 结构验收（第 3、4 条完成后的预期；现在跑会看到 zant-sys / rusqlite）
./scripts/cargo.sh tree -p zant-tsx -e normal,build        # 不应出现 zant / zant-sys
./scripts/cargo.sh tree -p zant -e normal --no-default-features   # 不应出现 rusqlite / reqwest

# 端到端
./scripts/tsx/build-template-check.sh
./scripts/tsx/build-check.sh
./scripts/tsx/examples-check.sh
```

> 说明：本文件只列问题与方案，不代表已实现。修复时请为每条补测试，并在 PR 描述里引用条目编号。
