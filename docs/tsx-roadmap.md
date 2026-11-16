# TS + TSX 应用路线图

```tsx
import { useState, View, Button } from "zant";

export default function App() {
  const [count, setCount] = useState(0);
  return <View padding={24}>
    <Button onClick={() => setCount(prev => prev + 1)}>点击 {count}</Button>
  </View>;
}
```

TSX 是推荐的应用写法，编译期生成 Rust，无 JS 运行时。此表只记录本仓库实现；
具体语法见 [TSX](tsx.md)，原生接口见 [原生模块](tsx-native.md)。✅ 已实现，⬜ 未实现。

## 语言与组件

| 状态 | 能力 |
|---|---|
| ✅ | 普通函数、跨文件 export/import、自定义 hook 编译期内联 |
| ✅ | for、for-of、while、break/continue、switch |
| ✅ | Math、Number/String、toFixed、parseInt/parseFloat、Date.now、performance.now 的受支持子集 |
| ✅ | 参数与解构 props 默认值、JSX 属性展开、JSX 实体解码 |
| ✅ | 命名处理函数捕获本帧局部值；可选值条件判断与类型收窄 |
| ✅ | Uint8Array、资源导入；原生规范共享 interface 与字符串联合类型 |
| ✅ | 接受并忽略 React 默认 / 命名空间导入（不提供 React 运行时） |
| ✅ | Image 内存图片、HotkeyInput、isMacos/isWindows、keyLabel、titlebarInsets、wakeAfter |
| ⬜ | TypeScript `enum` 声明（字符串联合类型可用） |
| ⬜ | 泛型函数与泛型类型声明 |

## 运行时与工具链

| 状态 | 能力 |
|---|---|
| ✅ | useState 组件本地状态、createStore/useStore 共享状态、系统主题 |
| ✅ | 多语言：`locales/*.json`、编译期检查的 `t()`、ICU 复数 / select、语言协商与切换（[i18n](i18n.md)） |
| ✅ | 关闭 / 退出 / 键盘 / 窗口焦点、托盘、全局快捷键、剪贴板、唤醒与退出 hooks |
| ✅ | createWindow 多窗口、剪贴板文字 / 图片 / 文件、开机启动、单实例 |
| ✅ | setTimeout/clearTimeout 定时回调 |
| ✅ | `zant create` 生成 `src/` TSX/TS + `native/` Rust 项目；模板带原生模块示例 |
| ✅ | `native/main.rs` 用 `zant::app!()`；`native/build.rs` 调 `zant_build::tsx_app()` |
| ✅ | 自动生成 `$OUT_DIR/zant_app.rs`、`zant_main.rs`、`zant_hot.rs`，自动声明并注册原生模块 |
| ✅ | `native/<名字>.ts` 规范 + `.rs` 实现，Rust trait 检查接口；TS 从 `native/<名字>` 导入 |
| ✅ | `zant run` 默认原地刷新；`--watch` 重启模式、`--no-hot` 单次运行；compile/package/build |
| ✅ | 热更新框架动态库共享、成功替换后旧应用库默认卸载、兼容状态恢复、编译错误界面 |
| ✅ | app.config.json 元数据、window.remember；macOS 标准菜单栏 |
| ✅ | 从标签表生成 zant.d.ts、模板 tsconfig.json |

实现依据：`crates/zant-tsx/src/` 与 `tests/`、`crates/zant-build/src/entry.rs`、
`crates/zant-cli/src/hot/`、`crates/zant/src/tsx/`、`scaffold/template-ts/`。
这些能力仍受静态 TS 子集限制；不等于支持任意 TypeScript、npm 包或浏览器 API。
