# 待修复与优化清单（剩余项）

上一轮清单（审计基线 `be848f2`，40 条）中已完成的条目已移除，这里只列还没做或只做了一部分的条目，编号沿用原清单，
便于对照提交记录。已完成的工作见分支 `fix/needfix` 的提交（`git log main..fix/needfix`）。

已完成（不再列出）：1–9、11、13、17、20、23、27、28、31、33、34、37、38，以及 39 的 Button 部分。35、36 已决定不发布到 crates.io（所有 crate
`publish = false`，夹具留在仓库根 `tests/fixtures/`），关闭。

优先级：**P1** 显著影响通用性或可维护性，近期应排期；**P2** 一致性、清理、测试补强。每条给出位置、现状、修复与验收；
行号以写作时的分支为准。

## 总览

- [ ] 10 [P2] 模板是否默认开启单实例（只剩产品决定）
- [ ] 12 [P1] `core` 反向依赖具体组件，分层不闭环
- [ ] 14 [P2] `Generator` 是 45 字段 God object + 30 多个函数指针
- [ ] 15 [P1] 一次只报一个诊断，`LAST_DIAG` 旁路
- [ ] 16 [P1] 类型/属性检查与代码生成耦合
- [ ] 18 [P1] TSX 模板构建截图与打包资源检查没有进 CI
- [ ] 19 [P1] 没有应用菜单 API，macOS 菜单只支持中/英
- [ ] 23b [P2] 应用不能用 `locales/*.json` 覆盖或补充框架内置文案（`zant.*`）
- [ ] 21 [P1] 缺少通用覆盖层/弹层管理
- [ ] 22 [P1] 组件覆盖面对通用桌面应用偏窄
- [ ] 24 [P1] 主题 token 不完整，也不能从 TSX 定制
- [ ] 25 [P1] `PdfPreview` 在 build 路径同步打开并渲染 PDF
- [ ] 26 [P1] 缺少可访问性支持
- [ ] 29 [P2] 布局缺少 z-index、横向滚动、Grid
- [ ] 30 [P2] 没有自定义字体、SVG、通用图片加载器
- [ ] 32 [P2] Rust 侧的有状态组件用数字 id，其他组件用 key
- [ ] 39 [P2] Select / Tabs 与 antd 语义的缺口
- [ ] 40 [P2] 文档与路线图缺少“✅ 必须有测试”的约束

---

### 10. 模板是否默认开启单实例

**现状**

路标与 `docs/desktop.md` 已写清单实例 / 开机启动的启用条件与 `ZANT_ALLOW_MULTI_INSTANCE`（仅 Debug，变量存在即生效）。
`scaffold/template-ts/app.config.json` 没有 `single_instance`，默认 `false`。

**修复**

产品决定：桌面工具类应用通常应该单实例。若默认开启，模板写 `"single_instance": true`，并在模板 README 里说明
`zant run` 期间再开一份会被带到前台、调试多开用 `ZANT_ALLOW_MULTI_INSTANCE=1`。

**验收**

新建项目第二次启动时激活已有窗口；设置环境变量后可以多开。

### 12. `core` 反向依赖具体组件，分层不闭环

**位置**

- [paint.rs](../crates/zant/src/core/tree/paint.rs#L176-L188) 直接调 `components::button::paint` / `input` / `toggle` / `image` / `select`；
- [intrinsic.rs](../crates/zant/src/core/layout/intrinsic.rs#L82-L90) 直接调 `components::input::measure_*`；
- `core` 里共 22 处 `crate::components` 引用，除上面两个文件外还有 `core/layout/container.rs`、`core/tree/input_ops.rs`、`core/tree/alloc.rs`；
- `ui` 重导出 `components`，[view.rs](../crates/zant/src/components/view.rs#L19) 又 import `ui::builders`。

**现状**

`core` 不是可复用的底层：新增**底层节点种类**（自己绘制 / 测量的控件）必须改 `core`，第三方也无法添加。
`NodeKind`（[data.rs](../crates/zant/src/core/node/data.rs#L21)）与 `NodeData` 都是封闭枚举。
由现有节点组合出来的控件（`Modal`、`DropdownMenu`、`ShortcutList` 等）不受影响，不需要动 `core`。

这条和已完成的 feature 门控（原第 4 条）没有牵连：`core` 引用的是 button / input / toggle / image / select 这些基础控件，它们不会被门控；
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

### 14. `Generator` 是 45 字段 God object + 30 多个函数指针

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

### 15. 一次只报一个诊断，`LAST_DIAG` 旁路

**位置**

- `Error::Tsx` 无负载、`fail()` 只保留第一条： [codegen/mod.rs](../crates/zant-tsx/src/codegen/mod.rs)（`enum Error` / `fn fail`）；
- `LAST_DIAG`： [generate.rs](../crates/zant-tsx/src/codegen/generate.rs)（`thread_local! LAST_DIAG`），由 `compile.rs` 取回。

**现状**

生成代码解析失败已改为“编译器内部错误”诊断（`check_and_format`，可用 `ZANT_TSX_DUMP` 保存生成代码），不再 panic。
仍然每次只报第一个错误；诊断经 thread_local 传回。

**修复**

- `Error::Tsx(Diag)`，删除 `LAST_DIAG`，让 `Result` 自带信息；
- 多诊断分层做：codegen 用 `?` 在第一个错误处返回，所以在“每个文件 / 每个顶层函数、组件”的边界上捕获、记录后继续；
  同一函数体内的多条错误靠第 16 条的独立检查 pass；
- 跨边界继续时避免连锁误报（前一个组件失败后，引用它的地方不要再报“未知组件”）。

**验收**

- 两个组件（或两个文件）里各有一个独立错误时能一次报出；
- 现有诊断测试仍通过（它们断言第一条诊断）。

### 16. 类型/属性检查与代码生成耦合

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

### 18. TSX 模板构建截图与打包资源检查没有进 CI

**现状**

CI（[rust.yml](../.github/workflows/rust.yml)）已跑 bindings 同步、fmt、clippy、`cargo test --workspace`、生成表格检查、
`node-parity.sh`（macOS）与“`zant` 不开可选 feature”的 `cargo check`。还没有：
[build-template-check.sh](../scripts/tsx/build-template-check.sh)（TSX 模板项目构建并出截图）、
[examples-check.sh](../scripts/tsx/examples-check.sh)、打包资源（图标、`locales/`、Windows `pdfium.dll`）的自动检查，
`distribute/` 下不需要签名身份的步骤也没有测试。

**修复**

- CI 增加一个 macOS job 跑 `build-template-check.sh`；`examples-check.sh` 放夜间或手动触发；
- 打包测试补资源复制检查与 `distribute` 的无签名步骤；
- Windows 桌面脚本（`components-check.ps1` 等）需要已登录桌面，继续在测试机上定期跑。

**验收**

- PR 会构建一次 TSX 模板项目并产出截图；
- 打包资源缺失有自动化失败信号。

### 19. 没有应用菜单 API，macOS 菜单只支持中/英

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

### 23b. 应用不能覆盖或补充框架内置文案

**现状**

框架内置文案（`zant::i18n::builtin`，`zant.markdown.*` / `zant.editor.*` / `zant.pdf.*` / `zant.pdfPreview.*` / `zant.hotkeyInput.*`）
已按应用当前语言在英文和简体中文之间切换，显式传入的文案优先。但应用在 `locales/*.json` 里写 `zant.*` 键不会生效
（i18n 编译器忽略它们），所以英文、简体中文以外的语言只能逐个组件传文案。

**修复**

- i18n 编译器接受 `zant.*` 键（只允许 catalog 里已有的键，拼错报错），生成的描述带上这些覆盖；
- `builtin::text` 先查应用的覆盖，再查内置的英文 / 中文；
- macOS 菜单的标准项（第 19 条）也走这套 key。

**验收**

- 应用在 `locales/ja.json` 写 `zant.markdown.copy` 后，日语下 Markdown 的复制按钮显示该文案；
- 未知的 `zant.*` 键是编译错误。

### 21. 缺少通用覆盖层/弹层管理

**位置**

- [dropdown-menu.md](components/dropdown-menu.md#L23-L25)：菜单不会自动定位、翻转或关闭；
- TSX 没有 `keyBounds`；
- [message.rs](../crates/zant/src/components/message.rs#L35-L52)：没有时长/队列；
- 应用需要自己放根容器、加 `DismissLayer`、算窗口坐标。

**现状**

每个菜单、Popover、Drawer、Toast 都把布局/窗口坐标泄漏进应用代码。

**修复**

- 运行时 overlay 栈：锚点（key/rect）、placement + flip、边缘避让；
- 统一 `DismissLayer` / Esc 栈与 z 顺序（焦点范围的保存 / 恢复已在运行时实现：`focusScope` 出现时取走焦点、消失时归还）；
- 提供 `Popover`、`Drawer`、`Toast`；`Message` 增加 duration/queue；
- 在 `DropdownMenu` 内部使用 overlay，不再要求应用自己定位。

**验收**

- 菜单在窗口边缘自动上翻/翻转；
- 点击外部、Esc、窗口 resize 都正确关闭；
- 示例应用不再手算 `key_bounds`。

### 22. 组件覆盖面对通用桌面应用偏窄

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

### 24. 主题 token 不完整，也不能从 TSX 定制

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

### 25. `PdfPreview` 在 build 路径同步打开并渲染 PDF

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

### 26. 缺少可访问性支持

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

### 29. 布局缺少 z-index、横向滚动、Grid

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

### 30. 没有自定义字体、SVG、通用图片加载器

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
- 新写一个框架级远程图片加载器：后台下载（依赖 `fetch` feature）+ 解码 + LRU + `post_wake`，并实现现有的 `ImageLoader` 回调协议，
  这样 Markdown 和普通 `Image` 共用一份；
- `ImageOptions` / `<Image>` 增加 URL 入口与加载中、失败占位。

**验收**

- 应用能加载品牌字体；
- 普通视图写一个 URL 就能显示远程图片，Markdown 里的远程图片不需要应用再提供 loader；
- 图片缓存有上限，并在窗口销毁时释放。

### 32. Rust 侧的有状态组件用数字 id，其他组件用 key

**位置**

- Rust 的 `markdown(id: u64, options)` / `pdf_viewer(id: u64, options)`： [markdown.rs](../crates/zant/src/components/markdown.rs)、[pdf_viewer.rs](../crates/zant/src/components/pdf_viewer.rs)；
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

### 39. Select / Tabs 与 antd 语义的缺口

**位置**

- `SelectOptions` 的选项模型只是 `Vec<Str>` + 选中下标： [options.rs](../crates/zant/src/ui/options.rs)（`pub struct SelectOptions`）；
  没有独立的 value、逐项禁用、placeholder、多选、搜索；
- `TabsOptions` 已是受控的（`active` / `on_change`），但 `TabItem` 只有 label/content。

（Button 的 `danger` / `block` / `loading` 见本轮提交，已完成。）

**修复**

- `SelectOption { value, label, disabled }` + `placeholder` + `multiple` + `search`（与第 22 条的“富 Select”是同一件事，只做一次；
  搜索下拉依赖第 21 条的弹层）；
- `TabItem` 增加 icon、禁用；Tabs 增加方向键切换（roving focus）与溢出滚动（依赖第 29 条的横向滚动）；
- 按 antd 5 的尺寸/状态实现，更新 `docs/components/*.md` 与截图；`#[derive(TsxProps)]` 的字段变了要重新生成 `ui_tags.rs` / `zant.d.ts`。

**验收**

- 与 antd 的常用属性一一对应；
- 深/浅色、禁用/聚焦状态都有测试或截图，macOS 与 Windows 都验证。

### 40. 文档与路线图缺少“✅ 必须有测试”的约束

**现状**

路标/文档的 ✅ 与测试之间没有强制关系；上一轮审计发现了几个 ✅ 行缺少行为断言，以及多个 ✅ 行与代码不一致或写得过宽（均已修正）。

**修复**

- 路标的一行往往包含好几项能力（例如第 28 行同时有 Image、HotkeyInput、`isMacos` / `isWindows`、`keyLabel` 等），所以映射是一对多的：
  维护一张“能力 → 测试名 / Node 对照探针”的表（可以就放在 [tsx-roadmap.md](tsx-roadmap.md) 旁边），按能力而不是按行登记；
- CI 加一个轻量检查：表里登记的测试名真实存在，并且会被 CI 执行（`node_parity` 探针要先按第 18 条进 CI）；
- 新增能力先写测试，再把 ⬜ 改 ✅；
- 这个检查只能防止“忘了登记测试”，不能证明测试断言了对的东西（上一轮发现的“导入了 `isMacos` 却没用它”就是例子），评审时仍要看断言内容。

**验收**

- 每项标 ✅ 的能力都能在表里找到至少一个行为测试或 Node 对照探针；
- 表里的测试名失效（改名、删除）时 CI 报错。

## 建议执行顺序

1. **诊断与检查**：15 → 16（先能一次报多个错误，再拆 check pass），之后 14。
2. **弹层与布局**：21 与 29 的 z-index 一起设计（同一套层叠、命中顺序），随后 29 的横向滚动 / Grid。
3. **框架 API**：19 菜单（复用内置文案 catalog）、24 主题 seed、25 PdfPreview 并入 PdfViewer。
4. **组件**：22、39（富 Select 依赖 21，Tabs 溢出依赖 29），26 可访问性分阶段。
5. **其余**：12（第三方控件需求出现时再做开放节点）、18、30、32、40、10。

## 验证命令

```bash
./scripts/cargo.sh fmt --all -- --check
./scripts/cargo.sh clippy --workspace --all-targets -- -D warnings
./scripts/cargo.sh test --workspace
./scripts/tsx/node-parity.sh                       # 需要 Node 22.6+
scripts/win/sync.sh && scripts/win/remote.sh XyUI 'cargo test --workspace'   # Windows 测试机
```

> 说明：每条修复都要补测试，并在 macOS 与 Windows 测试机上都通过；提交信息里引用条目编号。
