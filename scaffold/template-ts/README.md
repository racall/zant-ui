# __APP_TITLE__

```bash
zant run              # 保存组件后原地热刷新
zant run --no-hot     # 编译运行一次；验证 macOS App Sandbox
```

用 TSX 写界面、Rust 写原生能力的 Zant UI 桌面应用；编译成 Rust，无 JS 运行时或 npm。
应用标识 `__BUNDLE_ID__`，工具链由创建时的框架版本固定。

## 修改界面

把 `src/App.tsx` 替换为：

```tsx
import { useState, Button } from "zant";

export default function App() {
  const [count, setCount] = useState(0);
  return <Button onClick={() => setCount(prev => prev + 1)}>次数：{count}</Button>;
}
```

| 文件 | 内容 |
| --- | --- |
| `src/App.tsx` / `components/` | 默认界面：计数通过 props / 回调传递 |
| `src/stores/profile.ts` | 主窗口与设置窗口共享资料 |
| `src/windows/SettingsWindow.tsx` | `activate()` 打开 / 前置；关闭时隐藏，复用窗口 |
| `native/system.ts` / `system.rs` | 原生规范 / Rust 实现，TSX 用 `import { info } from "native/system"` |
| `native/main.rs` / `native/build.rs` | `zant::app!();` / `fn main() { zant_build::tsx_app(); }` |
| `app.config.json` | 标题、标识、版本、描述、公司、版权、窗口；模板 `window.remember: true` 记忆位置 / 尺寸 |
| `assets/icons/` | 替换 PNG 修改图标；macOS 标准菜单栏由框架提供 |

- `src/` 只放 TSX / TS，所有 Rust 放 `native/`；新增原生模块参照 `system.*`，自动声明 / 注册并检查接口类型。
  构建生成 `$OUT_DIR/{zant_app.rs,zant_main.rs,zant_hot.rs}`，不要手改。
- `zant run` 保留兼容状态与稳定 key 的视图状态；项目配置 / Cargo / 图标等改动重启运行器。
  store / Rust 全局变量 / 外部资源不自动恢复；`--watch` 为旧重启模式，内存状态重建。

## 常用命令

| 命令 | 用途 |
| --- | --- |
| `zant compile` / `zant sync` | 只编译 / 同步本平台配置文件 |
| `zant package` / `zant build` | 本机测试包 / 发布（`build.config.json`） |
| `cargo test` | 原生模块单元测试 |
| `ZANT_INPUT_TRACE=1 zant run` | 输入延迟时序日志 |

[TSX](https://github.com/racall/zant-ui/blob/main/docs/tsx.md) ·
[原生模块](https://github.com/racall/zant-ui/blob/main/docs/tsx-native.md) ·
[脚手架 / 热更新](https://github.com/racall/zant-ui/blob/main/docs/scaffold.md) ·
[发布](https://github.com/racall/zant-ui/blob/main/docs/build.md)；界面设计先读 [DESIGN.md](DESIGN.md)。
