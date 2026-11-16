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
| ✅ | TypeScript `enum` / `const enum`（成员内联）、穷尽的联合 `switch` |
| ✅ | 基本类型联合与可辨识联合（Rust 枚举）、`typeof` / 判别字段收窄 |
| ✅ | `Record` / 索引签名 / `Map` / `Set` / `Object.*`、元组、`keyof` / `typeof` / `T["k"]`、对象字面量推断与结构赋值 |
| ✅ | 泛型类型（Rust 泛型结构体）与泛型函数（单态化、实参推断、默认值、约束） |
| ✅ | `satisfies`、`interface extends` / 交叉类型、rest 参数与展开实参 |
| ✅ | 数组 / 对象原地修改（可变值语义：push/splice/sort/字段赋值，共享时报错） |
| ✅ | 函数作为值（`Rc<dyn Fn>`，闭包按创建时复制捕获） |
| ⬜ | 泛型组件、跨文件泛型函数 |
| ✅ | `throw` / `try` / `catch` / `finally`、`Error` 及其子类、`instanceof` 收窄（Swift 式 `Result`，推断会抛出的函数） |
| ⬜ | `class`（引用语义） |
| ✅ | `JSON.parse` / `JSON.stringify`（按类型编解码，输出与 Node 一致，`SyntaxError` / `TypeError`） |
| ✅ | `RegExp`：字面量与构造、`test` / `exec`、`match` / `matchAll` / `replace` / `split` / `search`（`regress` 引擎，ECMAScript 语义） |
| ✅ | `let` 的控制流收窄（到下一次赋值为止，含 `while` / `for` 条件） |
| ⬜ | `Date` |
| ✅ | `Promise` 与 `.then` / `.catch` / `.finally`、`fetch`（UI 线程任务队列） |
| ⬜ | `async` / `await` |

## 运行时与工具链

| 状态 | 能力 |
|---|---|
| ✅ | useState 组件本地状态、createStore/useStore 共享状态、系统主题 |
| ✅ | 多语言：`locales/*.json`、编译期检查的 `t()`、ICU 复数 / select、语言协商与切换（[i18n](i18n.md)） |
| ✅ | 关闭 / 退出 / 键盘 / 窗口焦点、托盘、全局快捷键、剪贴板、唤醒与退出 hooks |
| ✅ | createWindow 多窗口、剪贴板文字 / 图片 / 文件、开机启动、单实例 |
| ✅ | setTimeout / setInterval 与 clear*；effect 清理函数在组件卸载时执行 |
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
