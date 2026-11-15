# __APP_TITLE__

先读 [DESIGN.md](DESIGN.md)，它是界面与交互的最高优先级规范。

## 项目

```tsx
import { useState, Button } from "zant";

export default function App() {
  const [count, setCount] = useState(0);
  return <Button onClick={() => setCount(prev => prev + 1)}>次数：{count}</Button>;
}
```

- `src/App.tsx` 默认导出无 props 的根组件；推荐 TSX，编译成 Rust，无 JS / DOM / npm。
- 应用标识 `__BUNDLE_ID__`；元数据、窗口、`window.remember` 在 `app.config.json`。
  模板启用窗口位置 / 尺寸记忆；删除此字段则默认关闭。macOS 自动提供标准菜单栏。

## 目录

| 路径 | 用途 |
| --- | --- |
| `src/` | 只放 `.tsx` / `.ts`；组件、store、窗口分别在 `components/`、`stores/`、`windows/` |
| `native/main.rs` / `native/build.rs` | 分别为 `zant::app!();` / `fn main() { zant_build::tsx_app(); }` |
| `native/system.ts` / `system.rs` | 原生接口规范 / Rust 实现；TSX `import { info } from "native/system"` |
| `Cargo.toml` | `build = "native/build.rs"`、`[[bin]] path = "native/main.rs"`；框架路径依赖由 CLI 写入 |
| `rust-toolchain.toml` | 创建时继承框架固定工具链 |
| `assets/icons/` | PNG 图标；TSX 用资源 import |
| `zant.d.ts` / `tsconfig.json` | 编辑器类型与补全 |

- 所有 Rust 放 `native/`；顶层 `.rs`（排除 `main` / `build`）自动声明，有同名 `.ts` 才注册；子目录用普通 `mod`。
  新模块实现参照 `native/system.rs`，接口类型由生成 trait 检查。
- 构建器生成 `$OUT_DIR/zant_app.rs`（配置 / 规范 / 注册 / 模块）、`zant_main.rs`（普通入口）、
  `zant_hot.rs`（隐藏库 `zant::hot_app!()` 的入口）；不手改生成代码，不写回 `src/`。

## 约定与命令

```bash
zant run              # 原地热刷新
zant run --no-hot     # 运行一次；验证 macOS 沙盒用此模式
cargo test            # 原生 Rust 测试
```

- `zant run --watch` 为旧重启模式，内存状态重建；`zant compile` 只编译，`zant sync` 同步本平台配置，
  `zant package` 生成本机测试包，`zant build` 发布（`build.config.json`）。
- `src/` TSX / TS、`native/` RS / TS、导入资源原地刷新；配置 / Cargo / 图标 / `native/build.rs` 等重启 `zant run`。
  兼容状态与稳定 key 的视图状态保留；store / Rust 全局变量 / 外部资源不自动恢复，卸载前清理线程与资源。
- 状态单一来源：props 向下、回调向上，最新值用函数式 setter；跨组件 / 窗口用 `createStore` / `useStore`。
  hooks 放组件顶层；UI 只在主线程修改，阻塞工作在 `native/` 后台线程处理。
- 颜色用 `useTheme()` token，通用组件补进 Zant UI；`View` / `Column` / `Row` / `Clickable` 支持
  `motion` / `motionOrigin` / `layoutTransition`。完成后检查尺寸、浅深主题和交互状态，不提交构建缓存或密钥。
- 调试：`ZANT_INPUT_TRACE=1 zant run` 看输入时序，`ZANT_HOT_TRACE=1 zant run` 看热刷新计时。

[TSX](https://github.com/racall/zant-ui/blob/main/docs/tsx.md) ·
[原生模块](https://github.com/racall/zant-ui/blob/main/docs/tsx-native.md) ·
[脚手架 / 热更新](https://github.com/racall/zant-ui/blob/main/docs/scaffold.md) ·
[发布](https://github.com/racall/zant-ui/blob/main/docs/build.md)
