# TS + TSX 应用路线图（目标：ClipHop 能用 TS + TSX 写）

`zant create` 可以选择 Zig 或 TS + TSX。TS + TSX 项目里，**界面和应用逻辑都用 TS 写**（状态、托盘、全局快捷键、
剪贴板、多窗口、计时器），只有多线程网络这类底层引擎留在项目里的 `.zig` 文件（原生模块，TS 直接导入调用）。
验收标准：ClipHop 的 TS + TSX 版（cliphop 仓库新分支）功能与 Zig 版一致，截图逐张对比、帧耗时持平。

每一项都是一个小任务：一份说明、一个分支（Codex 在独立 worktree）、审查后一次提交。状态：⬜ 未开始、🔄 进行中、✅ 完成。

## A. 语言

| # | 内容 | 负责 | 状态 |
|---|---|---|---|
| A1 | 顶层普通函数（工具函数，不是组件）：`function bytes(n: number): string`，组件和函数里都能调用，可跨文件 `export` / `import` | Claude | ✅ |
| A2 | 语句：`for (let i = 0; …)`、`for (const x of xs)`、`while`、`break` / `continue`、`switch` | Codex | ✅ |
| A3 | 内置：`Math.*`、`Number()` / `String()`、`toFixed`、`parseInt` / `parseFloat`、`Date.now()` | Codex | ✅ |
| A4 | 字节：`Uint8Array` ↔ `[]const u8`（剪贴板图片、文件内容） | Claude | ✅ |
| A5 | 从 Zig 导入类型：`import type { Peer } from "./sync.zig"`（结构体、枚举），Zig 函数的参数和返回值可以是它们和它们的切片 | Claude | ✅ |
| A6 | 资源导入：`import trayIcon from "../assets/tray.png"`（编译期嵌入，`Uint8Array`） | Claude | ✅ |

以下几项来自 SWC 解析器测试集的统计（`scripts/tsx/swc-corpus.sh`：981 个 TS / TSX / JSX 输入，0 崩溃、0 卡死，
按被拒绝的原因排序，就是真实代码里最常见、我们还不支持的写法）：

| # | 内容 | 负责 | 状态 |
|---|---|---|---|
| A7 | 兼容 `import React from "react"` / `import * as React from "react"`（接受并忽略，复制来的 React 代码常带） | Codex | ✅ |
| A8 | 参数默认值：`function f(n = 1)`、解构 props 的默认值 `({ size = 14 }: Props)` | Claude | ✅ |
| A9 | JSX 展开属性：`<Button {...common} onClick={…} />`（对象的字段必须是这个标签 / 组件的属性，按源码顺序合并） | Claude | ✅ |
| A10 | `enum Kind { Text, Image }` / `enum Kind { Text = "text" }` → Zig 枚举 | Codex | ⬜ |
| A11 | JSX 实体：完整 XHTML 实体表、数字实体规则与 SWC / Babel 一致（`&#x` 小写、码点范围） | Codex | ✅ |
| A14 | 命名的处理函数读 render 局部变量：`const copy = () => …` 传给 `onClick={copy}` / 回调 prop / `useHotkey`，也可被其他处理函数调用 | Claude | ✅ |
| A13 | 可选值当条件用并收窄类型：`peer ? peer.name : "none"`、`if (peer) { peer.name }`（TS 里很常见） | Claude | ✅ |
| A12 | 泛型函数 `function first<T>(xs: T[]): T`（ClipHop 不需要，排在最后） | — | ⬜ |

## B. 组件属性

| # | 内容 | 负责 | 状态 |
|---|---|---|---|
| B1 | `<Image image={img}>`：内存图片类型 `Image`（`import type { Image } from "zant"`） | Claude | ✅ |
| B2 | `<HotkeyInput value onRecord>`：快捷键用字符串表示（`"ctrl+cmd+v"`），回调 `(shortcut: string) => void` | Claude | ✅ |
| B3 | 平台信息：`isMacos` / `isWindows`、`keyLabel("mod+f")`、`titlebarInsets()`、`wakeAfter(ms)`（单调时钟用已有的 `performance.now()`） | Claude | ✅ |

## C. 应用运行时

| # | 内容 | 负责 | 状态 |
|---|---|---|---|
| C1 | 入口：TS 项目不写 `main.zig`，工具链按 `app.config.json` 和 `App.tsx` 的默认导出生成入口 | Claude | ✅ |
| C2 | 应用 hooks：`useCloseRequest`、`useQuitRequest`、`useKeyDown`、主题跟随系统 | Claude | ✅ |
| C3 | 桌面事件：`useClipboardChange`、`useHotkey`、`useTray`（图标、菜单、点击）、`useReopen` | Claude | ✅ |
| C4 | `useWake`：原生模块在后台线程完成后 `ui.postWake()`，TS 在 UI 线程取结果 | Claude | ✅ |
| C5 | 多窗口：`createWindow(Panel, { … })`，句柄 `show` / `hide` / `toggle` / `place` / `focus`，窗口自己的按键、关闭、失焦 | Claude | ✅ |
| C6 | 桌面函数：剪贴板（文字 / 图片 / 文件 / 变化计数 / 监听）、`quit`、开机启动、下载目录、设备名、应用数据目录、单实例 | Claude | ✅ |

## D. 工具链

| # | 内容 | 负责 | 状态 |
|---|---|---|---|
| D1 | `zant create` 选择语言（Zig / TS + TSX），TS 模板 | Codex | ✅ |
| D2 | `zant run` / `compile` / `build` / `run --watch` 支持 TS 项目 | Codex | ✅ |
| D3 | 编辑器：从标签表生成 `zant.d.ts`，模板带 `tsconfig.json` | Codex | ✅ |

## E. ClipHop TS + TSX 版

| # | 内容 | 负责 | 状态 |
|---|---|---|---|
| E1 | 新分支，原生模块整理：同步、传输、文件、存储保留 Zig，导出 TS 能调用的函数 | Codex | ⬜ |
| E2 | 界面移植：列表行、历史页、设置页、面板、同步提示、对话框、标题栏 | Codex | ⬜ |
| E3 | 应用逻辑移植：`state.zig` → TS | Claude | ⬜ |
| E4 | 两版在浅色 / 深色、主窗口 / 面板 / 提示窗、多种尺寸下截图对比，帧耗时对比 | Claude | ⬜ |

## 顺序

1. A1（Claude）‖ A2 → A3 → A7 → A11（Codex）
2. A4、A5、A9（Claude）‖ B1 → B2 → B3 → A6 → A8 → A10（Codex）
3. C1、C2、C5（Claude）‖ D1 → D2 → D3 → C6（Codex）
4. C3、C4（Claude）‖ E1（Codex）
5. E2（Codex）‖ E3（Claude），最后 E4
