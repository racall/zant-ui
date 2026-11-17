# 架构

```tsx
// src/App.tsx：推荐用 TSX 写应用，编译期生成 Rust，没有 JS 运行时。
import { useState, View, Button } from "zant";

export default function App() {
  const [count, setCount] = useState(0);
  return <View padding={24}>
    <Button onClick={() => setCount(prev => prev + 1)}>点击 {count}</Button>
  </View>;
}
```

## 应用与构建

```text
src/                   只放 .tsx / .ts；App.tsx 默认导出无参数根组件
native/main.rs         zant::app!();
native/build.rs        fn main() { zant_build::tsx_app(); }
native/<模块>.ts       原生接口规范
native/<模块>.rs       Rust 实现；后台任务、网络等底层能力
app.config.json        元数据、窗口与平台配置
```

`zant-build` 调用 `zant-tsx`，在 `$OUT_DIR` 生成 `zant_app.rs`（配置、模块与注册）、
`zant_main.rs`（可执行入口）、`zant_hot.rs`（热更新入口）。`native/` 顶层除 main/build 外的
`.rs` 自动声明为模块；有同名 `.ts` 规范时自动注册该模块的 `Native`。无需手写模块表。

| 配置 | 用途 |
|---|---|
| `title` / `bundle_id` / `version` | 应用标题、标识、版本 |
| `description` / `company` / `copyright` | 描述、发布者、版权元数据 |
| `window.remember` | 按应用标识保存、恢复主窗口和 `createWindow` 窗口位置尺寸；解析默认 false |
| `macos` / `windows` | 平台配置；详见 [窗口配置](window-config.md) |

## 框架分层

```text
TSX → zant-tsx → Rust ui::Node
                   ↓
ui/、components/   声明式 builder、样式、组件
runtime/           协调、事件、焦点、IME、帧调度
core/tree/         保留树、命中测试、绘制
core/layout/       布局遍历；flex.rs（`zant::flex`）分配 Flex 尺寸
motion/            状态过渡、presence、FLIP
renderer/          Skia 文本与绘制桥
platform/          Rust 平台接口
zant-sys/native/   C ABI；AppKit / Win32、Metal / Direct3D 12
```

- `tsx/` 提供 hooks、store 接入、原生注册表、定时器与多窗口运行时。
- `desktop/` 提供托盘、全局快捷键、剪贴板等；`markdown/`、`math/`、`pdf/` 提供文档组件。
- macOS 使用 AppKit 原生窗口、Skia + Metal；Windows 使用 Win32、Skia + Direct3D 12。
  GPU 不可用或 `ZANT_RENDERER=raster` 时用 CPU 光栅。
- macOS 自动安装标准菜单栏（应用、文件、编辑、显示、窗口、帮助），编辑菜单接入文本操作。

## 协调与调度

- 应用状态变化时构造 Node，Runtime 协调到保留树；有 key 按 key 匹配，无 key 按顺序匹配。
- `ui::memo` 的 hash 不变时跳过子树；组件本地状态由 `LocalStateStore` 持有，卸载时释放。
- 布局后绘制；文本缓存当前和上一个宽度的排版，自然宽度独立缓存；`List` 只创建视口附近条目。
- 动画只推进绘制，不重建应用；空闲窗口不持续刷新。后台任务用 `ui::post_wake()` 请求更新。
- 指针、键盘与 IME 由平台送入 Runtime；`ZANT_INPUT_TRACE=1` 输出输入处理时间诊断。

## 开发热更新

`zant run` 默认保留宿主窗口，重编译并替换应用动态库；框架通过 `crates/zant-dylib`
加载一次。成功替换并销毁旧实例后，旧应用库默认卸载。兼容状态与按稳定 key 标识的交互状态尽量恢复。

| 改动 | 行为（`crates/zant-cli/src/hot/classify.rs`） |
|---|---|
| `src/*.tsx/.ts`、`native/*.rs/.ts`（含子目录）、已导入资源 | 原地刷新 |
| Cargo 清单 / 锁文件、`app.config.json`、`native/build.rs`、工具链、`assets/icons/` | 重启 `zant run` |
| 文档、README、tsconfig、未导入资源、临时 / 隐藏文件 | 忽略 |

`--no-hot` 编译运行一次；`--watch` 使用旧的重启模式。详见 [脚手架](scaffold.md)。
工作区固定 Rust **1.94.1**（`rust-toolchain.toml`）；生成应用使用 stable。
