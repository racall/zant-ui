# TSX 组件（编译成 Rust）

**推荐用 TSX 编写 Zant 应用**，用 Rust 实现原生能力。用 React 的写法写组件：`.tsx` 文件在**编译期**由 `zant-tsx` 转成 Rust，
产物是纯原生代码，运行时没有 JS 引擎，也不需要 npm。示例：`scaffold/template-ts/` 生成的
TS 项目（`src/App.tsx`）。

> 编译器在 `crates/zant-tsx/`（词法 / 语法 / hooks / codegen、应用层的 imports、native trait、入口和
> `zant-tsx` CLI），应用的 `native/build.rs` 通过 `zant_build::tsx_app()` 调用它；仓库内的 TSX 示例见下方「示例」。

```tsx
import { useState, View, Row, Text, Button } from "zant";
import type { u32 } from "zant";

export default function Counter() {
  const [count, setCount] = useState<u32>(0);
  return (
    <View padding={24} gap={12}>
      <Text fontSize={20}>Count: {count}</Text>
      <Row gap={8}>
        <Button variant="primary" onClick={() => setCount(count + 1)}>+1</Button>
        <Button onClick={() => setCount(0)}>Reset</Button>
      </Row>
    </View>
  );
}
```

编译器把根组件状态生成为 Rust struct 字段，渲染生成为 `ui::*` builder 调用，
事件生成为 action / 闭包；子组件状态由 `ui::local_state` 按实例保存。
可用 `zant-tsx counter.tsx counter.rs` 查看完整产物。

应用中由 `native/main.rs` 的 `zant::app!()` 引入生成的运行入口。

TSX 是静态类型的 **TypeScript 子集**，沿用 React 的状态与组件写法；显式整数和部分字符串边界行为
有原生语义差异（见下文）。用到子集之外的语法时，
编译器在 `.tsx` 的行列上报错，不会生成半成品。实现参考了 esbuild（解析）、Babel（JSX 文本空白规则）、
Mitosis（组件 = 状态 + 函数 + 绑定）和 Static Hermes（用类型标注生成静态数值类型）。

## 构建

单个 `.tsx` 用 `zant-tsx` 命令行编译，输出 Rust 文件：

```bash
zant-tsx in.tsx out.rs                 # 单文件
zant-tsx --out <目录> a.tsx b.tsx …    # 多文件项目（每个 .tsx 一个同名 .rs）
```

**整个应用用 TS + TSX 写**时，`src/` 只放 `.tsx` / `.ts`，Rust 统一放在 `native/`。
`Cargo.toml` 设置 `build = "native/build.rs"` 和 `[[bin]] path = "native/main.rs"`。

```rust
// native/main.rs
zant::app!();
```

```rust
// native/build.rs
fn main() { zant_build::tsx_app(); }
```

构建器将组件、配置与规范编译到 `$OUT_DIR`。`zant_app.rs` 包含 `app_config`、
`native::spec`、原生模块声明与自动注册、TSX 模块声明；`zant_main.rs` 先注册再启动，
`zant_hot.rs` 先注册再 prepare（热重载库通过 `zant::hot_app!()` 引入）。
`native/` 顶层除 `main.rs` / `build.rs` 外的 `.rs` 文件自动声明为 `native::<文件名>`；
存在同名 `.ts` 规范时自动注册该模块的 `Native` 实现。子目录由这些 Rust 模块自行声明。
`src/App.tsx` 对应 `crate::app`，原生实现沿用 `crate::native::spec::<名字>` 路径。
不生成项目内的 Rust 文件，也不需要模块清单。图标与 Windows 资源仍由构建器处理。
主题默认跟随系统，`ZANT_THEME=light|dark` 可以固定。

仓库工具链固定为 Rust **1.94.1**（`rust-toolchain.toml`）；`zant create` 生成的应用工具链使用 `stable`。

### 新建 TSX 项目

```bash
zant create my-app          # 交互式
zant create my-app --yes    # 非交互，适合 CI
cd my-app
zant run
zant run --watch       # 可选：旧重启模式
zant run --no-hot      # 可选：运行一次
```

TSX 模板在 `scaffold/template-ts/`：`src/App.tsx` 是根组件，`src/components/` 演示父子组件的 props 与
回调；资料放在 `src/stores/profile.ts`，跨组件和设置窗口共享；根组件持有计数，子组件通过回调发事件。
`src/windows/` 演示 `createWindow`，`native/` 演示原生接口。
主题通过 `useTheme()` 跟随系统。
`src/App.tsx` 存在即被识别为 TS 项目。完整脚手架说明见 [脚手架](scaffold.md)。

`zant run` 默认热刷新：`src/` 下 `.tsx` / `.ts`、`native/` 下 `.rs` / `.ts` 和导入的资源改动后，
主窗口与宿主 PID 保持不变，根状态按字段名、组件本地状态按实例恢复；稳定 key 的滚动、焦点、
输入光标与选区也会恢复。类型不兼容只重置对应字段。框架通过 `crates/zant-dylib` 在进程中共享，
应用库成功替换后释放旧实例并默认卸载旧库；应用自己创建的线程和外部资源必须妥善清理。
`createStore` 和任意 Rust 全局状态不自动恢复。

`Cargo.toml`、`Cargo.lock`、`app.config.json`、`native/build.rs`、`rust-toolchain.toml` 和
`assets/icons/` 的改动会重启 `zant run`；`docs/`、`README.md`、`tsconfig.json`、隐藏文件、
编辑器临时文件以及未导入的普通资源不触发构建。规则见 `crates/zant-cli/src/hot/classify.rs`。
`--watch` 显式选择旧重启模式，`--no-hot` 运行一次。已有 UI 运行时编译失败会显示错误面板，修复后刷新。
完整行为与沙盒限制见 [脚手架](scaffold.md#热更新)，调试开关见
[开发指南](development.md#热刷新验证)。无 GUI 创建冒烟：`scripts/tsx/create-smoke.sh`。

### 应用配置与系统菜单

应用元数据写在 `app.config.json`：`title`、`bundle_id`、`version`、`description`、`company`、
`copyright`，macOS 另有 `macos.category` 和 `macos.minimum_system_version`（默认 `"13.0"`）。
元数据供打包、系统“关于”面板和 Windows 文件属性等使用，具体字段见 [脚手架](scaffold.md)。
窗口配置用 snake_case（如 `min_width`、`titlebar_height`）；TSX 组件属性则用 camelCase。

模板窗口为 680×700，最小 600×620，`window.remember` 默认写为 `true`：按 `bundle_id`
保存主窗口和 `createWindow` 窗口的位置与大小，后续启动时恢复。旧配置省略该字段时默认为 `false`。
这些值与未提供配置时的构建器默认值不同，不应把模板尺寸当作 API 默认值。

macOS 自动提供标准菜单栏：应用（关于、服务、隐藏、退出）、文件（关闭窗口）、编辑
（撤销、重做、剪切、拷贝、粘贴、全选）、显示（全屏）、窗口和帮助。
编辑操作交给当前焦点控件；关闭和退出仍经过相应的应用 hook。

输入延迟诊断可运行 `ZANT_INPUT_TRACE=1 zant run`：记录指针按下 / 释放的处理时间和帧开始时间，
macOS 还记录帧呈现到屏幕的时间；此开关用于诊断，不要在性能基准中开启。

### 编辑器类型

TS 模板包含 `tsconfig.json`（`jsx: "preserve"`、`strict`、`noEmit`、`types: []`）及 `zant.d.ts`，
`paths` 将 `import … from "zant"` 指向本地声明，无需安装 React 或 npm 依赖。安装了 TypeScript 时可运行：

```bash
tsc --noEmit -p .
```

`zant.d.ts` 由 `zant-tsx-dts` 从合并后的标签表（包括别名）生成到 `crates/zant-tsx/zant.d.ts`，
`zant create` 从已安装框架复制声明到项目。生成内容包括组件、属性、枚举、事件、Theme；hooks、Store、
多窗口、虚拟列表等运行时声明手写在 `crates/zant-tsx/src/bin/zant-tsx-dts/dts_api.d.ts`（编译进生成结果）。改动组件字段后重新生成：

```bash
./scripts/cargo.sh run -p zant-tsx --bin zant-tsx-tags -- crates/zant-tsx/src/ui_tags.rs
./scripts/cargo.sh run -p zant-tsx --bin zant-tsx-dts  -- crates/zant-tsx/zant.d.ts
# CI 检查生成文件是否过期：
./scripts/cargo.sh run -p zant-tsx --bin zant-tsx-tags -- --check crates/zant-tsx/src/ui_tags.rs
./scripts/cargo.sh run -p zant-tsx --bin zant-tsx-dts  -- --check crates/zant-tsx/zant.d.ts
```

（`crates/zant-tsx/tests/generated_tables.rs` 会在 `cargo test` 时执行同样的 `--check`。）

JSX 的 `IntrinsicElements` 为空，只接受导入的 PascalCase 组件，不接受小写 HTML 标签。
`Text` 支持嵌套 `Text` / `Link`，容器接受元素，叶子组件没有 children。TypeScript 无法识别所有
Zant 的静态子集及精确的嵌套限制，最终仍由 `zant-tsx` 检查；`.rs` 原生模块值在编辑器里按 any 处理，
`.png` 默认导出为 `Uint8Array`。

多文件项目把 `.tsx` 一起编译，每个文件生成同名的 `.rs`，放在同一个输出目录里，
文件之间的 `import { NoteCard, type Note } from "./NoteCard"` 变成 `use crate::note_card::{NoteCard, Note};`
（模块名由文件 stem 转成 snake_case；同名输出冲突会报错，不按目录自动区分）。入口文件导出根组件，其他文件只导出子组件和类型，
也可以导出常量（`export const promptMs = 8000;`，导入的一方按注解或字面量得到类型）。

## 基础语法

### 文件结构

- `import { ... } from "zant"`、`import type { ... } from "zant"`：也可以从 `"react"` 导入同样的名字；
  可导入 hooks、各个标签、桌面 API 和数值类型（`u8` `u16` `u32` `u64` `i8` `i16` `i32` `i64` `usize` `isize` `f32` `f64`）。
  标签不导入也能用（导入只是为了编辑器的类型检查）。
- 兼容 `import React from "react"`、`import * as React from "react"` 和默认 + 具名混合导入，
  默认 / 命名空间绑定可任意命名，不生成运行时代码。`React.useState/useEffect/useMemo/useRef` 与直接调用相同，
  `<React.Fragment>` 与 `<>` 相同（子节点展开，根节点生成无样式的 View）；`React.ReactNode` / `React.ReactElement` 可作元素类型。
  具名导入复用 Zant 验证，`useReducer` 等未支持名字仍报错；`React.FC<P>` 请改写成 `function X(props: P)`。
  不支持副作用导入（如 CSS）或其他 npm 包，支持 `"zant"`、`"react"`、相对 `.tsx` / `.ts` / `.rs`、资源导入及 `"native/<模块>"` 规范导入。
- 应用入口需要一个默认导出的无 props 根组件：`export default function Name() { ... }`，或者 `function Name` / `const Name = () => ...`
  加上 `export default Name;`。
  根组件生成 `pub struct Name { ... }` + `impl zant::ui::App`；同文件可另写子组件和普通函数。
  其他模块无需默认导出，子组件可以接受 props（见下文）。

### 类型

| TS | Rust |
|---|---|
| `number` | `f64`（赋给 `u32` 等整数上下文时按上下文类型） |
| `boolean` | `bool` |
| `string` | `zant::Str`（字面量是 `Static`，运行时拼接用 `Str::copy(&format!(..))`） |
| `u32`、`i32`、`f32`、`usize` 等（`from "zant"`） | 同名 Rust 类型 |

- 整数字面量可以给任何数值类型，小数字面量只能给浮点类型；没有类型标注的字面量是 `number`（`f64`）。
- 整数可以隐式转为浮点或更宽的整数；一般数值赋值不把浮点隐式转为整数，也不允许整数窄化。
  标签的整数属性另外支持 `number` 的截断 / 钳位，混合整数运算见下文。
- 整数运算按 Rust 语义：`/` 向零取整，`%` 取余，溢出 panic（工作区与应用模板的 release
  profile 都开了 `overflow-checks = true`）。需要 JS 的数值语义就用 `number`。
- 普通条件使用 `boolean`（例如 `count > 0`）；可选值还支持条件收窄及相应的真值判断，见下文。

### 组件体

组件函数体按顺序可以有：

1. `const [x, setX] = useState<T>(初始值);`：只能写在组件函数体顶层。初始值支持字面量
   （数字、字符串、布尔、`null`、数组、对象）和可在编译期求值的文件常量；不支持惰性初始化函数。`<T>` 可以省略，此时按初始值推断类型（`0` 推断为 `number`）。
   生成 struct 字段 `x: T` 和 `Default` 里的初始值。
2. `const`/`let` 派生值：`const doubled = count * 2;`，生成 render 里的局部变量（非 `Copy` 的值读取时
   codegen 会 `.clone()`）。
3. 函数：`function inc() { ... }`、`const inc = () => { ... }`，可以有带类型标注的参数，返回类型默认为
   `void`。生成方法 `fn inc(&mut self, ...)`。在 render 里可以调用有返回值的函数。
4. `if (...) return <...>;` 提前返回，以及最后的 `return <...>;`。

### 状态与事件（React 语义）

- `setX(v)` 和 `setX(prev => expr)` 可以在事件处理函数、effect 等非渲染回调中调用，在 render 里调用会报错。
- 事件处理函数看到的是**本次渲染时的值**：连续写两次 `setCount(count + 1)` 的结果是 +1，和 React 一样；
  `setCount(c => c + 1)` 拿到的是最新值。
- 事件属性可以接三种值：组件里的函数名（`onClick={increment}`）、setter
  （`onChange={setName}`，类型必须匹配），或者内联箭头函数（`onClick={() => setCount(0)}`、
  `onChange={(on) => setNotify(on)}`，参数类型从事件推断）。
- 事件处理函数可以读状态、自己的参数和局部变量，可以调用组件里的其他函数。
  也可以按值捕获 render 里的局部变量和列表回调参数，见下文「闭包」。

### 表达式

字面量、模板字符串、`+ - * / %`、比较、`=== !==`（字符串按内容比较）、`&& || !`、`?:`、
字符串 `+` 拼接（生成 `Str::copy(&format!("{}{}", a, b))`）、`str.length`、调用组件里的函数、
`console.log(...)`（生成 `zant::tsx::builtins::log(...)`，写 stderr）。
事件处理函数里还可以给 `let` 局部变量赋值（`=`、`+=` 等）以及使用 `x++`。

### 循环与分支语句

事件处理函数、effect、组件内函数和数组方法的块体回调支持以下语句，循环体也可以不加花括号：

```tsx
import { View, Text } from "zant";
interface Item { id: number; }
function sum(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    if (i === 2) continue;
    if (i === 10) break;
    total += i;
  }
  return total;
}
export default function App() {
  const items: Item[] = [{ id: 1 }, { id: 2 }];
  const selectedIds: number[] = [2];
  const selected = items.filter(item => {
    for (const id of selectedIds) if (id === item.id) return true;
    return false;
  });
  return <View><Text>{sum(5)} / {selected.length}</Text></View>;
}
```

- `for (let i = 0; 条件; i++)`：初始化只接受一个 `let` 声明、赋值或留空；条件必须为布尔值，
  留空等于 `true`；更新接受赋值、`++`、`--` 或留空。循环变量只在循环内可见。
- `for (const x of xs)` / `for (let x of xs)`：遍历数组（状态、局部变量、props 或函数返回值），
  数组表达式只求值一次。元素类型自动推断；`const` 不可重新赋值，`let` 可赋值；暂不支持解构。
  遍历字符串时和 JS 一样按码点（`for (const ch of name)`，每个 `ch` 是一个字符的字符串）。
- `while (条件)`、`do { … } while (条件);`：条件为布尔值；`do` 至少执行一次，
  `continue` 仍会检查条件。
- `break` 退出最近的循环或 `switch`，`continue` 进入最近循环的下一轮；不接受标签，不能跨越回调边界。
  循环、`switch` 中的 `return` 返回当前函数或数组回调。
- `switch (值)` 支持数字、字符串、布尔值和字符串字面量联合；字符串按内容比较。生成的 Rust 是
  带标签的 `if/else` 链（`case` 表达式可能有副作用且按源码顺序求值，Rust `match` 表达不了），
  所以不支持隐式贯穿：非空分支进入下一个 `case` 前必须用 `break`、`return` 或其他跳转结束。
  最后一个分支可以省略 `break`。

组件的 render 体保持声明式，不能直接写循环或 `switch`；渲染列表使用 `.map` / `.filter`，
计算循环放进函数或块体数组回调。`let` 若从未重新赋值，生成的 Rust 仍用 `let`。

### JSX

- 文本按 React 的空白规则处理（相当于 Babel 的 `cleanJSXElementLiteralChild`）；文本和属性字符串支持完整的 XHTML 1.0 实体（253 个名字，区分大小写）。
- 数字实体为 `&#123;`（十进制）或 `&#x1F600;`（十六进制，前缀只接受小写 `x`）；至少一位数字，
  必须以 `;` 结束，码点不超过 `0x10FFFF`。无效、未知或未结束的引用保留原文，不报错。
  解析时代理项按 SWC 的 JS 字符串规则保存为 WTF-8，相邻的高 / 低代理项合并；
  仍有孤立代理项时，生成 Rust 字符串会报错（Rust 字符串必须是合法 UTF-8）。
- `<Text>`、`<Button>`、`<Checkbox>`、`<Switch>` 的文本子节点和 `{表达式}` 拼成一个字符串，
  例如 `Count: {count}`。容器里直接写文本会报错（和 React Native 一样，要包在 `<Text>` 里）。
- 子节点可以是元素、`{cond && <X/>}`、`{cond ? <A/> : <B/>}`、`{cond ? <A/> : null}`，
  也可以是保存了元素的局部变量。片段 `<>...</>` 在子节点位置会展开。
- `style={{ ... }}` 对象字面量里的键和普通属性等价（同一个元素上，后写的生效）。
- 不写值的属性（`<Input password />`）等于 `true`。

## 常用标签与属性

属性名用 React Native 的 camelCase 写法；同一行里的别名等价。枚举值用字符串字面量，
也可以是字符串字面量组成的 `?:`。颜色可以写 `"#rgb"`、`"#rrggbb"`、`"#rrggbbaa"` 或 `0xAARRGGBB`。

### `<View>`（`ui::view`）、`<Column>`（`ui::column`）、`<Row>`（`ui::row`）

| 属性 | 说明 |
|---|---|
| `key` | 字符串 |
| `gap`、`rowGap`、`columnGap` | 数值 |
| `padding`、`paddingHorizontal`、`paddingVertical`、`paddingTop/Right/Bottom/Left` | 内边距（按边组合） |
| `margin`、`marginHorizontal`、`marginVertical`、`marginTop/Right/Bottom/Left` | 外边距 |
| `width`、`height` | 数值，或 `"50%"`（百分比） |
| `minWidth` `minHeight` `maxWidth` `maxHeight` | 数值 |
| `flex` | `flex: n` 相当于 `grow = n, shrink = 1, basis = 0` |
| `flexGrow`/`grow`、`flexShrink`/`shrink`、`flexBasis`/`basis` | 数值 |
| `justifyContent`/`justify` | `flex-start` `center` `flex-end` `space-between` `space-around` `space-evenly`（也接受 `start`/`end`） |
| `alignItems` | `flex-start` `center` `flex-end` `stretch` |
| `alignSelf` | `auto` `flex-start` `center` `flex-end` `stretch` |
| `alignContent` | `flex-start` `center` `flex-end` `space-between` `space-around` `stretch` |
| `flexWrap`/`wrap` | `nowrap` `wrap`；仅横向换行布局，`wrap-reverse` 虽可解析但尚未实现反向换行 |
| `position` | `"absolute"` / `"relative"`；配合 `top` `right` `bottom` `left` |
| `overflow` | `"hidden"`（裁剪）、`"scroll"`（纵向滚动）、`"visible"` |
| `backgroundColor`/`background`、`borderColor` | 颜色 |
| `borderWidth`、`borderRadius`/`radius` | 数值 |
| `hoverBackground`、`hoverBorderColor` | 悬停时的颜色 |
| `order` | 整数 |

`<View>` 还有：`flexDirection`/`direction`（`row` / `column`，默认 `column`）、
`shadowColor`、`shadowBlur`/`shadowRadius`、`shadowOffsetY`、`onClick`/`onPress`。

### `<Text>`（`ui::text`）

`key`、`fontSize`/`size`、`fontWeight`/`weight`（`regular` `medium` `semibold` `bold` 等，或者 `"100"`～`"900"` / `600`）、
`color`、`textAlign`（`left` `center` `right`）、`wrap`（布尔）、`numberOfLines`/`maxLines`、`lineHeight`、
`fontFamily`/`family`（`sans` `serif` `mono`）。

### `<Button>`（`ui::button`）

`key`、`text`（或者写成子文本）、`variant`（`primary` `default` `text` `link`）、`size`（`small` `middle` `large`）、
`disabled`、`width`、`fontSize`、`onClick`/`onPress`。

### `<Input>`（`ui::input`）

`key`、`value`、`placeholder`、`disabled`、`readOnly`、`password`、`multiline`、`width`、`height`、
`fontSize`、`onChange`（`(value: string) => void`）、`onSubmit`。

### `<Checkbox>`（`ui::checkbox`）、`<Switch>`（`ui::switch`）

`key`、`label`（或者写成子文本）、`checked`、`disabled`、`fontSize`、`onChange`（`(checked: boolean) => void`）。

## 组件与 React 常用写法

### 全部 `ui::*` 组件

支持反射的 `ui::Node` builder 对应一个标签，标签名是函数名的 PascalCase：`ui::radio_group` → `<RadioGroup>`、
`ui::icon_button` → `<IconButton>`、`ui::r#switch` → `<Switch>`、`ui::dropdown_menu` → `<DropdownMenu>`。
标签表（`crates/zant-tsx/src/ui_tags.rs`）由 `zant-tsx-tags` 从 `#[derive(TsxProps)]` 的选项结构体与
`ui::*` builder 生成并提交，`cargo test` 检查它与源码一致，更新并重新生成标签表后，支持反射的组件和字段才会出现在 TSX API 中。
属性名是字段名的 camelCase（`on_change` → `onChange`、`width_percent` → `widthPercent`）；前面的
React Native 写法（`backgroundColor`、`paddingHorizontal`、`flex`、`position`、`overflow`、`fontSize`、`onPress` …）
作为别名保留。

字段类型与 TSX 值：

| Rust 字段 | TSX |
|---|---|
| `f32` / 整数 / `bool` / `Str`（可选的 `Option<T>` 同） | number / number / boolean / string |
| 枚举（含 `Icon`） | 字符串字面量，接受 `chevron_down`、`chevron-down`、`chevronDown` 三种写法 |
| `Option<u32>` 颜色 | `"#rrggbb"`、`0xAARRGGBB`、主题颜色 |
| `geometry::Edges` | 数字（四边相同）或 `{ top, right, bottom, left, horizontal, vertical }` 对象 |
| `ui::Node` / `Option<ui::Node>` | 元素：`<Tooltip text="…" child={<Button … />} />`，或唯一的子元素 |
| `Vec<ui::Node>` | 子元素 |
| `Vec<Str>` | 字符串数组：`<Select options={["A", "B"]} />` |
| `Vec<SomeStruct>` | 对象数组：`<Tabs items={[{ label: "A", content: <Text>…</Text> }]} />` |
| 嵌套的选项结构体（`ClickableOptions.box`） | 展开成同一层的属性 |
| `Option<ui::Action>` / `BoolAction` / `TextAction` / `IndexAction` | `() => void` / `(on: boolean) => void` / `(text: string) => void` / `(index: usize) => void` |
| `Option<zant::image::Image>`（`<Image image>`） | `Image` 类型的值（`import type { Image } from "zant"`），来自 Rust 原生模块（比如缩略图缓存） |
| `Option<desktop::Shortcut>`（`<HotkeyInput value>`） | 快捷键字符串：`"ctrl+cmd+v"`，`""` 表示没有 |
| `Option<ui::HotkeyAction>`（`<HotkeyInput onRecord>`） | `(shortcut: string) => void`，得到的字符串可以直接交给 `useHotkey` |

没有默认值的字段是必填属性，缺了会在标签处报错。暂不支持的字段类型（指针回调等）不出现在标签表里。

### 语言

- **类型**：`T[]`、`Array<T>`、`interface` / `type` 对象类型（字段可选 `x?: T`）、字符串字面量联合
  （`type Filter = "all" | "done"` → Rust 枚举）、`T | null`（→ `Option<T>`）。
- **数组**：字面量和展开 `[...todos, item]`、`.length`、`a[i]`、`.map((x, i) => …)`、`.filter(x => …)`、
  `.find`、`.findIndex`、`.some`、`.every`、`.reduce`、`.indexOf`、`.includes`、`.slice`、`.concat`、`.join`（字符串数组）。
  数组是 `tsx::List<T>`（`Rc<Vec<T>>`）；更新状态请用展开、过滤或替换数组，不支持直接调用 `push` / `splice`。
- **对象**：字面量、展开 `{ ...todo, done: true }`、成员访问、`?.`、`??`。
- **默认值**：参数 `function label(n: number, unit = "item")`（调用时可以省略，类型取注解或默认值的类型）；
  组件的可选 prop `function Badge({ size = 14 }: BadgeProps)`（`size?: number`，父组件没传时读到 14）。
  默认值是常量表达式（字面量、文件里的常量），给必填 prop 写默认值会报错。
- **可选值收窄**（和 TS 一样）：可选值可以直接当条件，条件成立的分支里它就是非空类型：
  `peer ? peer.name : "none"`、`if (label) { label.length }`、`{peer && <Text>{peer.name}</Text>}`、
  `peer !== null && peer.online`、`!peer || peer.files.length == 0`；`if (!entry) return <Empty/>;` 之后整段代码里
  `entry` 都是非空。真假按 JS 规则：对象非 null 即真，字符串还要非空、数字还要非 0、布尔值还要为 true。
  在收窄的分支里创建的箭头函数捕获的是非空值。`let` 变量不收窄（可能被重新赋值），请用 `!` 或改成 `const`。
- **状态**：`useState` 可以存数组和对象。React 的不可变更新照常写：
  `setTodos([...todos, { id, title, done: false }])`、`setTodos(todos.filter((t) => t.id !== id))`。
  生成的值遵守「要么 `Copy`，要么廉价 `Clone`」：每次读取非 `Copy` 变量都 `.clone()`，
  表示层通常不需要手工处理 Rust 借用。
- **列表渲染**：子节点里的 `{todos.map((t) => <TodoRow key={t.id} … />)}`；`key` 是字符串或数字。
- **闭包**：事件处理函数可以读 render 里的局部变量和 `.map` 的参数（`onClick={() => remove(t.id)}`），
  捕获的值按值复制进闭包（Rust 里是 `Rc<dyn Fn(..)>` 或生成的 action 方法）。
- **命名的处理函数**：`const copy = () => clipboard.setText(text);` 读了 render 里的局部变量时，不单独生成方法，
  而是在每个传入的地方（`onClick={copy}`、子组件的回调 prop、`useHotkey(k, copy)`）像内联箭头一样生成，捕获那次渲染的值；
  其他事件处理函数和 effect 里也可以调用它（`() => { copy(); close(); }`）。应用 hooks（`useKeyDown` 等）的回调
  不在渲染时创建，不能调用这种函数。
- **子组件**：同一文件里的其他函数组件，或从项目里另一个文件导入（`import { TodoRow, type Todo } from "./TodoRow"`，
  被导入的组件和类型要 `export`；跨文件共享的嵌套对象类型和联合类型要写成具名的 `interface` / `type`）。参数是一个 props 对象
  （`function TodoRow(props: Props)` 或解构 `function TodoRow({ todo, onToggle }: Props)`），回调属性的类型写成
  函数类型（`onToggle: (id: number) => void`）。子组件可以有自己的状态（见下文「组件状态与 hooks」）。
- **字节**：`Uint8Array` 在生成的 Rust 里是 `&'static [u8]`（编译期嵌入的静态字节切片：资源导入、
  `const icon: Uint8Array = [137, 80, …]` 字面量、tray 图标、Rust 原生模块返回的 `&'static [u8]`）。
  字节字面量必须是纯整数字面量数组（会 const-promote 成 `&[..]`）；运行时字节（剪贴板图片
  `clipboard.image()`）是 `tsx::Bytes`（`Rc<[u8]>`）。当前 TSX 编译器的 `setImage` / `assetImage` 参数
  仍要求静态字节，不能直接传 `clipboard.image()` 的结果，也不能把它赋给 `Uint8Array` 注解；
  Rust 包装函数接收 `impl AsRef<[u8]>` 并不代表 TSX 编译器支持这些转换。
- **数字混用**：TS 里都是 `number`。两个整数类型混用时取能放下双方的那个（`u8` 和 `usize` → `usize`，
  `u16` 和 `i32` → `i32`），放不下（`u64` 和 `i32`）时得到 `number`；整数往更宽的整数赋值会自动转换，往更窄的报错。
- **展开属性**：`<Button {...look} onClick={save} />`、`<Card {...card} title="新标题" />`，按源码顺序合并，
  后写的覆盖先写的（和 React 一样）。可以展开对象字面量、变量或成员路径（`{...props.common}`）；函数调用的结果先存进
  `const`；可选字段不能展开（属性可能不存在），请单独写。枚举属性也接受字符串字面量联合类型的变量（`variant={look.variant}`）。
- **普通函数**：小写字母开头的顶层函数是普通函数（大写开头的才是组件，和 React 的约定一样），参数要写类型，
  有返回值时写返回类型：`function bytes(n: number): string { … }`、`const half = (n: number): number => n / 2;`。
  组件渲染、事件处理函数、其他函数（可以递归）里都能调用，也能直接当事件处理函数（`onClick={reset}`）；
  可以 `export`，其他文件 `import { bytes } from "./format"`。普通函数没有状态，不能调用 hook，可以读写 store。
- **字符串**：`str.length` 与 TypeScript 相同，是 UTF-16 码元数（`"周报".length` 是 2），不是 UTF-8 字节数。
- **主题**：`const theme = useTheme();`，`theme.colorPrimary`、`theme.colorText`、`theme.colorBgContainer` …
  （`zant::theme::tokens::Theme` 的字段，camelCase）。默认跟随系统浅色 / 深色。
- **长列表**：`<List data={todos} renderItem={(t, i) => <Row …/>} keyExtractor={(t) => t.id} />`
  （React Native 的 `FlatList` 写法）编译成虚拟列表 `ui::list`，只构建看得见的行；
  `estimatedItemHeight`、`itemHeight`、`followTail`、`gap`、`padding` 同 `ui::ListOptions`。
  滚动是声明式的：`scrollToIndex={selected}`（配 `scrollAlign="start" | "center" | "end"`）在值变化时把那一行滚进视野
  （面板里 ↑↓ 选择）；`layoutKey={epoch}` 是一个数字，行变了（新的搜索、删除、上限变化）时换一个值，列表重新测量
  所有行并再滚到 `scrollToIndex`（`ui::ListState::follow`）。

### 内置对象

- `Math`：`floor`、`ceil`、`round`、`trunc`、`abs`、`sign`、`sqrt`、`cbrt`、`pow`、`exp`、
  `log`、`log2`、`log10`、`sin`、`cos`、`tan`、`atan`、`atan2`、`hypot`，以及 `PI`、`E`。
  `min(...值)` / `max(...值)` 接受零到多个参数，空参数返回正 / 负 Infinity；`hypot()` 为 0。
  `round` 按 JS 规则向最近整数舍入，恰好一半时向正无穷（`Math.round(-2.5)` 为 -2）。
- 显式整数的 `floor` / `ceil` / `round` / `trunc` / `abs` 保留整数类型，不经过浮点转换；
  `min` / `max` 按整数混用规则选类型，其他 Math 运算返回 `number`。有符号最小值的 `abs`
  沿用原生整数溢出规则。`random()` 使用进程内只播种一次的非密码学 PRNG，返回 `[0, 1)`。
  （实现：`zant::tsx::builtins`。）
- `Number(x)` 接受数值、布尔和字符串；字符串去除 JS 空白，空串为 0，完整匹配十进制、
  `Infinity`、无符号 `0x` / `0b` / `0o`，无效输入为 NaN。
  `Number.isNaN` / `isFinite` / `isInteger` 不做隐式转换；常量有 `MAX_SAFE_INTEGER`、`EPSILON`。
- `String(x)` 接受数值、布尔和字符串；数值的 `.toString()`、模板插值也使用 JS 最短可往返格式：
  整数不带 `.0`，NaN / Infinity 使用 JS 拼写，负零输出 `"0"`，绝对值小于 `1e-6` 或不小于 `1e21`
  时用科学计数法。显式整数保持精确的整数文本。
- 数值的 `.toFixed(digits?)` 默认零位，支持截断后为 0–20 的位数，按实际二进制数的精确值舍入；
  例如 `(1.25).toFixed(1)` 为 `"1.3"`，`(0.995).toFixed(2)` 为 `"0.99"`。
  无穷 / NaN 和绝对值不小于 `1e21` 的数沿用 `String` 格式。越界位数在原生运行时 panic。
- `parseInt(s, radix?)` 读取去掉前导空白、符号后的最长整数前缀；默认十进制，默认或 16 进制
  接受 `0x`，基数支持 2–36，遇到无效数字停止，没有数字时为 NaN。
  `parseFloat(s)` 读取最长十进制前缀（含指数和有符号 Infinity），其余尾部忽略。
- `Date.now()` 返回 Unix 纪元以来的墙钟毫秒数（`number`）；`performance.now()` 返回含小数的单调毫秒数，
  与 UI 帧时钟独立。完整 Date 对象暂不支持。未知成员会列出支持项。

字符串结果是拥有的 `Str`，可跨帧保存；数字转字符串、随机数和时钟调用不能用于文件顶层常量，纯数值运算和内置常量可以。
参数按源码顺序各求值一次；可选数值接收者先用 `??` / `!` 解包。暂不支持 `.toString(radix)`。

### 嵌套规则

每个标签有自己的“内容模型”（和 HTML 一样）：合法的嵌套直接能用，不合法的在编译时报错，
错误里说明这个标签能放什么。

| 标签 | 可以放 | 不可以 |
|---|---|---|
| 容器：`View` `Row` `Column` `Card` `Clickable` `Space`、带 `children` 的子组件 | 任意元素、`{表达式}`、片段 `<>…</>` | 直接写文字（要包在 `<Text>` 里） |
| `Text` | 文字、`{值}`、`{cond && "…"}`、嵌套的 `<Text>`（局部样式）、行内 `<Link onClick>` | 块级元素（`View`、`Button` …） |
| `Heading` `Title` `Paragraph` | 文字和 `{值}` | 元素（局部样式用 `<Text>`） |
| 标签类：`Button` `Link` `Checkbox` `Switch` `Tag` | 文字和 `{值}`（就是它的文字）；`Button` 还可以放一个 `<Icon name>` | 其他元素 |
| 单子元素：`Tooltip` `Modal` | 恰好一个元素（或用 `child` / `content` 属性） | 多个元素、文字 |
| `Markdown` | Markdown 文字和 `{值}`（等同 `source`） | 元素 |
| 叶子：`Input` `Image` `Icon` `Divider` `PdfPreview` `PdfViewer` … | 无 | 任何子节点 |

```tsx
import { View, Text, Link, Button, Icon } from "zant";
export default function App() {
  const count = 1;
  function openDocs() { console.log("打开文档"); }
  function save() { console.log("保存"); }
  return <View>
    <Text size={16}>
      读 <Text weight="bold">这里</Text>，或者 <Link onClick={openDocs}>打开文档</Link>{count > 0 && `（${count}）`}
    </Text>
    <Button variant="primary" onClick={save}><Icon name="check" /> 保存</Button>
  </View>;
}
```

- 嵌套的 `<Text>` 编译成 `ui::rich_text`：每段文字一个 span，样式继承外层（`size` `weight` `color` `family`，
  另外 span 可以用 `italic` `underline` `strike` `background`）；行内 `<Link>` 只接受 `onClick`，颜色取主题主色。
  带 span 的文字不能用 `maxLines` / `textAlign` / `wrap`。
- 点击只给最里层的可点击元素（不冒泡），所以 `<Clickable>` 卡片里放 `<Button>` 是安全的：点按钮只触发按钮。
- `Modal`、`Message` 可以写在任何位置，渲染时放到窗口根节点（`ui::portal`，同 React portal），总是盖住整个窗口。
- 会自己滚动的元素（`<List>`、`<PdfViewer>`）不能放进滚动容器（`scroll` / `overflow="scroll"`）里，也不能放进另一个 `<List>` 的行里：
  给它一个高度有限的普通容器。
- 同一层的字面量 `key` 不能重复；`.map` 和 `<List>` 行里的**有状态子组件（以及 `<Markdown>`、`<PdfViewer>`）必须有 `key`**，否则它们的状态会混在一起。
  子组件的 `key` 会覆盖它根元素上的 `key`（和 React 一样，`key` 属于组件实例）。
- 子组件的 `children` 类型决定能放什么：`ReactNode` 一个元素，`ReactNode[]` 任意元素（`JSX.Element | JSX.Element[]` 同样，一个子元素时 tsc 也接受），`string` 文字和 `{值}`
  （`<Badge>new {count}</Badge>`）。
- 整数属性（`page`、`maxLines` …）也接受 TS 的 `number`：向零取整，并钳到字段的取值范围。

### 性能

生成的代码与手写 Rust 一样：没有虚拟 DOM、没有运行时解释，状态是 struct 字段，事件是 action / 闭包。
修改代码生成后用 `scripts/tsx/bench-tsx.sh` 复测；默认闸门是 TSX 生成代码的帧耗时不超过
手写 Rust 对照组的 **1.2 倍**。

**帧对比基准**是 `zant-tsx-frame-bench`
（`crates/zant-examples/src/bin/zant-tsx-frame-bench.rs`）：`crates/zant-examples/build.rs` 用
`zant_build::tsx_app_in` 把六个基准页（`crates/zant-examples/tsx/corpus/` 的 `counter` / `todos` /
`local_bench` / `markdown_bench` / `settings`，以及 `tsx_views/ListBench.tsx`）编成 Rust，和
`controls/` 里手写的 `ui::App` 对照组跑同一个无窗口帧循环（640×900 raster、
假时钟、每帧 `render_frame` 全量重建/布局/绘制），交替重复取每侧最好一轮，逐场景（以及每个 app 的
汇总行）判 ≤1.2×。`scripts/tsx/bench-tsx.sh` 构建 Release 并打印对照表与闸门结论。

内存 soak 使用 `ZANT_BENCH_SOAK=1 ./target/release/zant-tsx-frame-bench` 或
`./target/release/zant-tsx-frame-bench soak`；可追加 `todos-churn` / `local-churn` /
`list-churn` / `counter-churn` 筛选。每个场景的 TSX 和手写对照各用独立进程、一个长期存活的
runtime，先跑一个预热循环，再分别跑 50 轮（每轮新增、切换、筛选并逐项删除 200 todos）、
500 次有状态子组件挂载/更新/卸载、200 次虚拟列表全程往返，以及 10 000 次计数点击 +
100 次输入替换。普通帧基准的行为和输出不变。

开始、中间、结束均在同一 UI 状态采样 RSS（macOS `task_info`、Windows `GetProcessMemoryInfo`）、
存活节点数、local-state slot 数和帧耗时 avg / p95 / max（毫秒）。start 的耗时来自预热循环，
middle / end 分别统计前半和后半，最终 verdict 的 avg 统计所有测量帧；计时缓冲区提前分配。
`text_layouts` 是存活的文字布局数（`zant::renderer::text::live_layouts()`）。结束的节点数、slot 数或文字布局数高于
预热后起点，或中间到结束 RSS 增长严格超过 10%，判 `leak`，否则 `stable`；全部场景跑完后，
有 leak 返回 1，采样/执行失败返回 2。RSS 判定是回归信号，不等于已证明存在对象泄漏。

编译基准是另一件事：`zant-tsx-bench` 把 `crates/zant-tsx/testdata/tsx/*.tsx` 逐个在进程内编译
（预热 + 迭代）并打印每个文件的耗时，由 `scripts/tsx/examples-check.sh` 跑并检查编译通过的文件数。

> **示例**：仓库内的 TSX 示例在 `crates/zant-examples/tsx/`：`tsx-counter`、`tsx-demo`、
> `tsx-bench`（帧基准的重负载页）、`tsx-gallery`、`tsx-todos`、`tsx-reader`、`tsx-notes`（含 native 模块
> `native/format.ts|rs`）七个 Cargo workspace 成员，形态与 `scaffold/template-ts/` 相同（`native/build.rs` →
> `zant_build::tsx_app()`）；`scripts/tsx/examples-check.sh` 构建、无窗口截图（`ZANT_SNAPSHOT` +
> `ZANT_LAYOUT_FILE`）并断言（截图非空白、窗口尺寸来自 `app.config.json`、两次运行逐字节一致、
> 带 key 的控件都有布局）。`scripts/mac/tsx-counter.steps` 是驱动 `zant-tsx-counter` 的真实 GUI 场景。

## 组件状态与 hooks

### 子组件的 `useState`

子组件里的 `useState` 和 React 一样属于**这个实例**：同一个 `<Counter>` 渲染三次就有三份独立的状态，
实例卸载（父组件不再渲染它）时状态释放，再出现时从初始值开始。

```tsx
import { useState, View, Button } from "zant";
function Counter({ label }: { label: string }) {
  const [count, setCount] = useState(0);
  return <Button onClick={() => setCount(count + 1)}>{label}: {count}</Button>;
}
export default function App() {
  return <View><Counter label="A" /><Counter label="B" /></View>;
}
```

编译结果：状态字段（以及 props）放进子组件的本地状态槽，由框架的 `ui::local_state` 按实例 id 持有；
构建函数多一个 id 参数，调用处用 `tsx::child_id(父实例 id, "Counter", key)` 计算（没有 key 时按调用位置编号
`tsx::site_id`）。所以实例的身份和 React 一样由**父组件 + 组件名 + key（或位置）**决定：列表里换了顺序、
删了别的项，带 key 的实例状态跟着 key 走。`.map` 里的有状态组件一定要给 `key`（React 也要求），否则它们共用一份状态。
`<List>` 的滚动状态同样保存在所在组件的实例里，子组件里也可以用 `<List>`。

只有有状态的组件（用了 `useState` / `<List>` / 文档组件，或者渲染了有状态的子组件，跨文件也会分析）才带 id，
无状态组件的生成代码和以前一样，没有额外开销。

### `useRef`、`useMemo`、`useEffect`

和状态一样，它们的存储是组件结构体（根组件）或本地状态槽（子组件）里的字段，跟着实例走。

- `const nextId = useRef<u32>(1)`：只能通过 `nextId.current` 读写；读到的总是当前值（不是快照），
  在事件处理函数和 effect 里写，不触发重建。初始值支持数字、字符串、布尔、`null`、数组或对象字面量。生成 `Rc<RefCell<T>>`（`tsx::Ref`）。
- `const visible = useMemo(() => items.filter(…), [items, query])`：依赖列表**按值**比较
  （字符串、数组、对象按内容），不变就直接用缓存，函数体不执行；结果复制进缓存自己的内存。
  比较本身是 O(依赖大小)，适合包住比它贵的计算（过滤、排序、统计）。不能缓存元素（每帧照常构建）。
- `useEffect(() => { … }, [deps])`：在这次构建协调完之后运行（`ui::effect`）。`[]` 只在挂载后运行一次，
  有依赖时依赖变了才运行，不写依赖列表则每次构建后都运行。effect 里调用了 setter（或组件函数、回调属性）
  时会再构建一次；只写 ref 不会。无依赖且每次调用 setter 的 effect 可能持续触发重建。
- 清理函数：effect 里 `return () => { … }`，在下一次运行之前、以及组件卸载时运行（React 语义）。
  它可以读 effect 里的局部变量（按值复制保存）、状态和 ref；**不能读 props**（卸载时父组件的数据可能已经不在了），
  需要的话先在 effect 里复制成局部变量：`const name = label; return () => log(name);`。
- 这三个 hook 都只能在组件体顶层调用，不能改名导入；用了它们的子组件算有状态组件（带实例 id）。

框架接口 `ui::effect(action)` / `ui::request_rebuild()` 手写 Rust 也能用。

### 自定义 hook（`useXxx`）

和 React 一样，把一组 hooks 抽成 `use` 开头的函数，放在任意文件里，组件按名导入调用：

```tsx
// src/hooks/useCounter.tsx
import { useState } from "zant";
export function useCounter(start: number) {
  const [count, setCount] = useState(start);
  return { count, increment: () => setCount(count + 1), reset: () => setCount(start) };
}

// src/App.tsx：根组件只做组合
import { Button } from "zant";
import { useCounter } from "./hooks/useCounter";
export default function App() {
  const { count, increment } = useCounter(0);
  return <Button onClick={increment}>{`${count}`}</Button>;
}
```

- 编译期展开：调用处替换成 hook 的函数体，里面的 `useState`、`useEffect`、应用 hooks、托盘和快捷键
  都属于调用它的组件（应用 hooks 仍受根组件 / 窗口组件的使用范围限制）。运行时没有额外开销。
  同一个 hook 调用两次就是两份独立的状态；子组件里调用时，每个实例各有一份。
- hook 里声明的名字会加上 hook 名前缀（生成的 Rust 里是 `use_counter_count`），不会和组件里的名字冲突。
  hook 文件导入的名字（store、native 模块、资源、其他文件的函数），编译器会自动导入到调用方，
  相对路径按调用方的位置重新计算。
- 调用写法：`useX(…);`、`const v = useX(…)`、`const [a, b] = useX(…)`（hook 返回数组字面量）、
  `const { a, b } = useX(…)` 或 `const r = useX(…); r.a`（hook 返回对象字面量）。
- 参数是普通值和回调；名字和字面量直接代入，其他表达式（包括箭头函数）在调用处先求值一次。
- hook 可以调用其他 hook。只能在组件或 hook 的顶层调用；写在条件、循环、事件处理函数或普通函数里会报错。
  hook 只在函数末尾 `return` 一次。
- 编译错误指向 hook 自己的文件和行。

### Markdown 与 PDF

`<Markdown>` 和 `<PdfViewer>` 是有独立状态的原生文档组件，标签和属性由同一份
`ui.*` 反射表生成（`zant-tsx-tags`）。`(u64, Options)` builder 的标签带
`instance_id` 标记，编译器自动传入父实例 + 标签名 + key / 调用位置计算的 id；
包含它们的子组件也自动带 id。`.map` 中请给每个实例稳定的 `key`。

```tsx
import { View, Markdown, PdfViewer } from "zant";
export default function App() {
  function openLink(url: string) { console.log(url); }
  return <View>
    <Markdown source="# 回复" streaming={true} caret="block" onLink={openLink} />
    <Markdown>另一段 Markdown</Markdown>
    <PdfViewer src="/path/to/document.pdf" toolbar annotationTools
      onChange={() => console.log("changed")} height={600} />
  </View>;
}
```

Markdown 的 `source` 必填，也可以用字符串子节点（不能同时写两者，不能放元素子节点）。
`streaming` 开启 Streamdown 的未完成语法修复，复用块缓存，只重新解析变化的块；
`caret` 为 `none` / `block` / `circle`，只在 streaming 时显示。常用属性还包括
`fontSize`、`contentWidth`、`lineNumbers`、`codeMaxHeight`、`codeCopy` / `codeDownload`、
`tableCopy` / `tableDownload`、`mermaidCopy` / `mermaidDownload`、`onLink`、`baseDir`、
`selectable`、`parseIncomplete`、`singleDollarMath`、`latexDelimiters`。默认随浅色 / 深色主题切换。

PDF 的 `src` 必填（空字符串显示提示），不接受子节点。`password` 改变也会重新打开文档；
路径 / 密码未变时保留滚动、缩放、选择、搜索和批注，打开失败不会逐帧重试。
文件在协调后的 effect 中打开，页面仍由 PDF 阅读器的后台线程分块渲染。
`toolbar`、`annotationTools`、`selectionMenu`、`pageGap`、`padding`、`background`、
`onLink`、`onChange`、`onAnnotation` 同 `zant::pdf::ViewOptions`；`width` / `height` / `grow`
控制容器尺寸，指定 height 时默认不伸长。空 / 加载提示可用 `emptyText` / `loadingText` 修改。

每个实例的缓存或 `Viewer` 都由 `ui::local_state` / `local_slot` 持有，卸载时自动释放；再次挂载从初始状态开始。
内部节点 key 使用实例 id 命名空间，多个文档不会互相影响。手写 Rust 对应
`ui::markdown(id, options)` / `ui::pdf_viewer(id, options)`，复杂的 Markdown 渲染扩展和 PDF `labels`
结构体仍可从 Rust 传入；TSX 只反射支持的字段类型。示例见
[Markdown 组件](components/markdown.md) 与 [PDF 阅读器](components/pdf.md)。

## 应用 hooks

`useCloseRequest`、`useKeyDown` 用于根组件或无 props 的窗口组件；`useWindowFocus` 用于窗口组件；
`useClipboardChange`、`useReopen`、`useWake`、`useAppExit` 只用于根组件。
可以放进[自定义 hook](#自定义-hookusexxx)，再在相应组件顶层调用。

```tsx
import { useState, useCloseRequest, useQuitRequest, useKeyDown, mainWindow, View, Text } from "zant";
import type { KeyEvent } from "zant";
export default function App() {
  const [dirty, setDirty] = useState(false);
  const [ask, setAsk] = useState(false);
  useCloseRequest(() => {           // 点关闭按钮 / ⌘W：返回 false 不关
    if (dirty) { setAsk(true); return false; }
    return true;
  });
  useQuitRequest(() => true);       // ⌘Q / Dock 退出；不写时问 useCloseRequest
  useKeyDown((e: KeyEvent) => {      // 在获得焦点的控件之前；true 表示已处理
    if (e.key === "Escape") { setAsk(false); return true; }
    if (e.mod && e.key === "f") { mainWindow.focus("search"); return true; }
    return false;
  });
  return <View><Text>{ask ? "请先保存" : "就绪"}</Text></View>;
}
```

- `KeyEvent`：`key` 和 DOM 的 `KeyboardEvent.key` 一样（`"Escape"` `"Enter"` `"Tab"` `"ArrowUp"` `"F1"` … 或输入的字符），
  `mod` 是平台的命令键（macOS ⌘、Windows Ctrl），另有 `shift` `alt` `ctrl` `meta` `repeat`。
- 应用 hook 回调在事件到达时读当前组件状态，可以调 setter、读写 store；它们不在 render 中创建，
  不能捕获渲染体里的局部变量。普通控件事件回调的状态快照规则仍如前文。

### 托盘、全局快捷键、剪贴板

```tsx
import { useState, useTray, useHotkey, useClipboardChange, useReopen, mainWindow, View } from "zant";
import trayIcon from "../assets/icons/statusbar.png";
export default function App() {
  const [sync, setSync] = useState(false);
  const shown = useTray({           // 一个应用一个托盘 / 菜单栏图标
    icon: trayIcon, template: true, tooltip: "ClipHop",
    items: [
      { title: "打开", onSelect: () => mainWindow.activate() },
      { separator: true },
      { title: "同步", checked: sync, onSelect: () => setSync(!sync) },
    ],
    onClick: () => mainWindow.activate(),  // Windows 左键；右键显示菜单
  });
  const ok = useHotkey("ctrl+cmd+v", () => mainWindow.activate());
  useClipboardChange(() => console.log("剪贴板变化"));
  useReopen(() => mainWindow.activate());
  return <View />;
}
```

- `useAppExit(() => { … })`（只在根组件）：应用马上要结束（退出、主窗口关闭）时调用，用来停后台线程、保存数据；
  回调返回后进程就退出（macOS 的 `applicationWillTerminate`、Windows 消息循环结束后）。
- `useWake(() => { … })`（只在根组件）：原生模块在后台线程做完事后调用 `zant::ui::post_wake()`，回调在 UI 线程运行，
  从模块里取结果（`const got = take(); if (got > 0) setValue(got);`），之后所有窗口重建。
- 托盘和快捷键在渲染时应用，只有内容变化才调用系统接口；回调像事件处理函数一样运行，可以读渲染体里的局部变量。
- 图标是 PNG 字节（`Uint8Array`），来自导入的图片文件或 Rust 原生模块。`useHotkey` 被别的应用占用时返回 false。
- 手写 Rust 的对应接口是 `zant::desktop` 的 `set_tray` / `register_hotkey` / `watch_clipboard` 等，
  在 `App::on_start` / `App::on_app_event` 里使用。

### 桌面函数（从 "zant" 导入）

```tsx
import { clipboard, app, mainWindow, wakeAfter, keyLabel, shortcutLabel, titlebarInsets,
  useState, View, Button, Text } from "zant";
import pngAsset from "../assets/icons/statusbar.png";
export default function App() {
  const [toastUntil, setToastUntil] = useState(0);
  const hint = `搜索 ${keyLabel("mod+f")}`;  // "⌘F" / "Ctrl+F"，参数须为字面量
  const insets = titlebarInsets();          // { left, right }，渲染时读
  if (toastUntil > performance.now()) wakeAfter(toastUntil - performance.now());
  function inspect() {
    const text = clipboard.text();          // string，没有文字时为 ""
    clipboard.setText(text);                // boolean
    const png = clipboard.image();          // 运行时 PNG 字节或 null（tsx::Bytes）
    clipboard.setImage(pngAsset);            // boolean；当前只接收静态字节
    const files = clipboard.files();        // string | null，绝对路径，一行一个
    if (files !== null) clipboard.setFiles(files);  // boolean
    console.log(clipboard.changeCount());   // 变化计数（包括自己写入）
    console.log(app.launchAtLogin());       // "off" | "on" | "needsApproval"
    console.log(app.launchedAtLogin());     // boolean，本次是否由系统登录启动
    console.log(app.deviceName());          // string
    console.log(app.downloadsDir() ?? ""); // string | null
    console.log(app.dataDir() ?? "");      // string | null，按 bundle_id
    console.log(mainWindow.visible(), mainWindow.active(), mainWindow.minimized());
    console.log(shortcutLabel("ctrl+cmd+v")); // "⌃⌘V"；无效快捷键返回 ""
    setToastUntil(performance.now() + 1000);
  }
  return <View paddingLeft={insets.left}>
    <Text>{hint}</Text>
    <Button onClick={inspect}>读取桌面状态</Button>
    <Button onClick={() => mainWindow.focus("search")}>聚焦 search</Button>
    <Button onClick={() => mainWindow.hide()}>隐藏</Button>
    <Button onClick={() => mainWindow.show()}>显示</Button>
    <Button onClick={() => mainWindow.activate()}>激活</Button>
    <Button onClick={() => app.setLaunchAtLogin(true)}>开机启动</Button>
    <Button onClick={() => app.quit()}>退出</Button>
  </View>;
}
```

- `app.quit()` 直接退出，不再询问关闭 / 退出 hook；`app.setLaunchAtLogin(on)` 返回是否成功，
  macOS 需要打包后的应用，`zant run` 的构建不支持；`isMacos` / `isWindows` 是判断平台的布尔常量。
- 这些名字要从 `"zant"` 导入才生效，不导入就是普通变量名，局部变量也可以遮蔽它们。
- 会改变剪贴板、窗口或应用状态的函数（`setText`、`setImage`、`setFiles`、`quit`、`setLaunchAtLogin`、
  `show` / `hide` / `activate` / `focus`）只能在事件处理函数和 effect 里调用，渲染时调用会报错；读取类函数在哪里都能用。
- 返回的字符串在 Rust 里是 `zant::Str` / `String`，存进 state 时会被复制。
- 单实例：`app.config.json` 里写 `"single_instance": true`，第二次打开应用时会把已运行的实例带到前面。
  生成的 `zant_main.rs` 把 `app_config::single_instance` 传给 `zant::tsx::app::Config`。

### 定时器：`setTimeout` / `clearTimeout`

```tsx
import { useState, useEffect, View, Button, Text } from "zant";
import type { i64 } from "zant";
export default function App() {
  const [prompt, setPrompt] = useState<i64>(0);
  const [ready, setReady] = useState(false);
  function expire(row: i64) { if (prompt === row) setPrompt(0); }
  const ask = (row: i64) => {
    setPrompt(row);
    setTimeout(() => expire(row), 8000);  // UI 线程运行，之后所有窗口重建
  };
  useEffect(() => {
    const t = setTimeout(() => setReady(true), 300);
    return () => clearTimeout(t);        // 卸载或依赖变化时取消
  }, []);
  return <View>
    <Text>{ready ? "就绪" : "等待"} / {prompt}</Text>
    <Button onClick={() => ask(1)}>提示</Button>
  </View>;
}
```

- 和浏览器一样是全局函数（不用 import），返回编号（number），`clearTimeout(id)` 取消；只能在事件处理函数和
  effect 里调用，渲染时请用 `wakeAfter(ms)` 安排一次重建。实现是单线程调度器 `zant::tsx::timers`。
- 只能在根组件或窗口组件（没有 props 的组件）里设置：它们和应用活得一样久。子组件把要做的事通过回调交给上层。
- 回调运行时设置它的那一帧已经结束，所以只能捕获数字、布尔值和枚举（复制进定时器）；字符串、数组、对象放进
  state，回调运行时再读。回调读 state 读到的是运行时的值。

### 多窗口：`createWindow`

```tsx
// src/App.tsx
import { createWindow, View, Button } from "zant";
import { Panel } from "./Panel";
export const panel = createWindow(Panel, {
  title: "剪贴板", style: "panel", width: 380, height: 480, resizable: false,
});
export default function App() {
  return <View><Button onClick={() => panel.toggle()}>切换面板</Button></View>;
}

// src/Panel.tsx
import { useKeyDown, useWindowFocus, View, Text } from "zant";
import type { KeyEvent } from "zant";
import { panel } from "./App";
export function Panel() {
  useKeyDown((e: KeyEvent) => { if (e.key === "Escape") { panel.hide(); return true; } return false; });
  useWindowFocus((focused: boolean) => { if (!focused) panel.hide(); });
  return <View><Text>剪贴板</Text></View>;
}
```

- 窗口在第一次使用时打开（开始是隐藏的），之后一直保留，组件的状态也跟着保留；窗口之间用 store 共享数据。
- 窗口组件里的 `useKeyDown`、`useCloseRequest`、`useWindowFocus` 就是这个窗口的（`useWindowFocus` 只在窗口组件里有效）。
- 选项：`title`、`style`（`"native"` `"hidden-titlebar"` `"borderless"` `"panel"`）、`width` `height` `minWidth` `minHeight`、
  `background`、`resizable`、`alwaysOnTop`、`movableByBackground`、`center`、`titlebarHeight`。
  位置：`"center"` `"topLeft"` `"topRight"` `"bottomLeft"` `"bottomRight"` `"trayCorner"`（菜单栏 / 托盘图标旁边）。
- 手写 Rust 用 `zant::desktop::open_window(..)`（`OpenOptions` + `WindowHooks`），见 `docs/desktop.md`。

### 资源文件

```tsx
import { useTray, View } from "zant";
import trayIcon from "../assets/icons/statusbar.png";  // Uint8Array，编译期嵌入
export default function App() {
  useTray({ icon: trayIcon, template: true, tooltip: "ClipHop" });
  return <View />;
}
```

- 相对路径、扩展名不是 `.tsx` / `.ts` / `.rs` 的导入就是资源文件，只能用默认导入（`import x from`）。
  路径相对于写导入的那个文件，可以在 `src/` 外面（比如项目的 `assets/`）。
- `zant-tsx` 生成 `const tray_icon: &[u8] = include_bytes!(..)`（或 `zant::tsx::Bytes`），构建会记录这些文件，
  改了图片会自动重新生成。文件不存在时报错指向那条 import。
- 要显示成图片时用 `assetImage(bytes)`：第一次调用时解码，之后一直复用，返回 `Image | null`，给 `<Image image>`。
- 只在项目构建里可用（`zant_build::tsx_app()`，或 `zant-tsx --out`）。字节也可以传给 Rust 原生模块
  （接口类型见 [原生模块](tsx-native.md)）。编辑器类型里 `.png` / `.jpg` / `.svg` / `.txt` 等的默认导出是 `Uint8Array`。

### 原生模块：`native/`

`src/` 只放 TS/TSX，Rust 代码放到 `native/`：`native/clips.ts` 用 `export declare function` 和类型
写接口规范，`native/clips.rs` 按规范实现，构建器自动登记模块；TSX 里
`import { copy } from "native/clips"` 直接调用，编译时由 trait 检查实现和规范是否一致。
详见 [tsx-native.md](tsx-native.md)。

## 独立编译器的 Rust 导入

以下是 `zant-tsx --out` 的兼容能力；新应用使用上面的 `native/` 规范，不在 `src/` 混放 Rust。
独立编译器里的 TSX 可以从相对路径的 `.rs` 文件按名导入顶层 `pub fn`，也支持 `as` 别名：

```tsx
import { View, Text } from "zant";
import { plural } from "./format.rs";
export default function App() {
  // 参数个数和类型在 TSX 编译时检查
  return <View><Text>{plural(2, "note")}</Text></View>;
}
```

函数签名支持 `bool`、`String` / `zant::Str` / `&str`（字符串）、所有上述数值类型、`&'static [u8]`（静态 `Uint8Array`）、
`zant::image::Image`、同文件的 `pub` 结构体和枚举（见下文“从 Rust 导入类型”），以及这些类型的 `Option<T>`、
`Vec<T>`（包括字符串数组）；返回值还支持 `()`，可在事件处理函数里作为语句调用。
不支持其他指针、泛型参数、`impl Trait`、错误联合（`Result`）或 trait 方法。
原生模块需要进程的 I/O、环境变量或长期内存时写成 `native/` 模块（见 [tsx-native.md](tsx-native.md)），
不要用 `.rs` 函数的返回值裸指针。

把 `.rs` 文件也加进多文件编译（`zant-tsx --out <目录> App.tsx format.rs`），独立编译器支持这种导入；应用的 `tsx_app()` 不允许 `src/` 放 Rust，请用 `native/*.ts` 规范。编译器用 `syn` 读取签名，不复制或自动声明这些 Rust 文件；最终 Rust crate 需自行声明相应模块。
`format.tsx` 和 `format.rs` 输出重名会报错。单文件模式（`in.tsx out.rs`）不支持 Rust 导入。

### 从 Rust 导入类型

原生模块里的结构体和枚举可以直接在 TSX 里用：

```tsx
import { View, Text } from "zant";
import type { Peer } from "./sync.rs";  // pub struct Peer { pub online: bool, pub kind: Kind }
import { peers } from "./sync.rs";      // pub fn peers() -> Vec<Peer>
export default function App() {
  const online: Peer[] = peers().filter((p) => p.online);
  return <View>{online.map((peer, i) =>
    <Text key={i}>{peer.kind === "Laptop" ? "笔记本" : "其他设备"}</Text>
  )}</View>;
}
```

- 结构体变成对象类型（字段名和 Rust 一样，比如 `last_seen`），枚举变成保留 Rust 变体拼写的字符串字面量联合
  （`Kind::Laptop` 对应 `"Laptop"`）；嵌套结构体、`Option<T>`、`Vec<T>`
  都可以。Rust 函数的参数和返回值也可以是这些类型。
- TSX 表示不了的字段（指针、互斥锁……）不出现在 TSX 类型里，读其他字段没问题；这种结构体不要在 TSX 里新建。
- 返回的字符串和切片在 Rust 里是拥有的值（`Str` / `Vec`），过帧后仍然有效。

### 共享状态：`createStore` / `useStore`

跨组件、跨文件的应用状态，写法接近 Zustand（框架的 `zant::store` 是同一思路的 Rust 版）：

```tsx
// src/store.ts
import { createStore } from "zant";
import type { u32 } from "zant";
export interface Settings { dark: boolean; fontSize: u32; }
export const settings = createStore<Settings>({ dark: false, fontSize: 14 });

// src/App.tsx
import { useStore, View, Switch } from "zant";
import { settings } from "./store";
export default function App() {
  const dark = useStore(settings, (s) => s.dark);  // 也可 useStore(settings)
  return <View><Switch checked={dark} onChange={(on) => settings.update({ dark: on })} /></View>;
}
```

- `createStore<T>(初始值)` 写在文件顶层（类型参数必填），生成文件级的状态 + 一组 `pub fn`（Rust 没有可变的
  `static`，实现用 `thread_local!` 的 `RefCell`）；其他文件像导入组件一样导入它。
- `useStore(store, selector?)` 是 hook，在组件顶层调用，得到渲染时的值（和状态一样，事件处理函数读到的是这一帧的快照）。
- 在事件处理函数和 effect 里修改：`store.update({ 字段: 值 })` 按字段浅合并，`store.set(新值)` 整体替换，
  `store.get()` 读当前值（任何地方都能调用）。修改会让所有窗口重建。
- 和根组件的状态一样，值按「要么 `Copy`，要么廉价 `Clone`」保存；多个窗口共用时，一个窗口在另一个窗口重建之前
  收到事件、并且回调里用到旧值，是目前已知的边界情况。

### 字符串方法

字符串保持 UTF-8 存储，位置和长度按 UTF-16 码元计算（与 `.length` 一致）；实现在 `zant::tsx::strings`：

- 查询：`includes(s)`、`startsWith(s)`、`endsWith(s)`、`indexOf(s)`、`lastIndexOf(s)`。
  查询不分配内存；索引返回 `isize`，未找到为 `-1`，可以直接写 `i >= 0`。
- 清理与大小写：`trim()`、`trimStart()`、`trimEnd()`，支持 JS 空白（包括 Unicode Zs、BOM、行分隔符）；
  `toLowerCase()` / `toUpperCase()` **只转换 ASCII**，其他 UTF-8 字节保持不变。
- 截取：`slice(start, end?)` 支持负数从末尾计数；`substring(start, end?)` 把负数钳到零，
  起止倒置时交换。数值向零取整，越界钳到字符串长度。
- 构造：`padStart(n, fill?)`、`padEnd(n, fill?)`（默认空格）、`repeat(n)`。
  `repeat` 截断后为负或无穷的计数、无法分配的长度在原生运行时 panic（没有 JS 异常机制）。
- 替换：`replace(a, b)`（首个匹配）、`replaceAll(a, b)`，只接受字符串模式；
  替换文本支持 JS 的 `$$`、`$&`、`` $` ``、`$'`，无捕获组时 `$1` 等保留原文。
- 分割：`split(sep)` 返回 `string[]`，保留空字段；空分隔符按 **Unicode 码点** 分割，
  不把 emoji 拆成两个 UTF-16 码元。
- 取字符：`charAt(i)` 越界返回 `""`；`at(i)` 支持负索引，返回可选字符串 `string | null`，
  越界为 `null`（TS 的 `undefined` 在此沿用可选值模型），可用 `??` / `!`。
- 字符编码：`charCodeAt(i)` 返回第 i 个 UTF-16 码元（越界为 NaN），`codePointAt(i)` 返回从那里开始的码点
  （可选值，越界为 `null`）。

返回的新字符串和数组拥有自己的存储（`Str` / `tsx::List`），可跨帧保存和廉价克隆。位置落在代理对内部时沿用
`unicode::utf16_to_byte` **向前取 UTF-8 码点边界**，始终保持合法 UTF-8。
例如 `"😀".slice(0, 1)` / `.charAt(0)` 是空串，`.slice(1, 2)` / `.charAt(1)` 是完整 emoji；
填充文本截断也向前取边界，可能比目标少一个码元；空模式 `replaceAll` 的两个代理对内部相邻边界
会映射到同一位置（`"😀".replaceAll("", "-")` 为 `"--😀-"`）。这些与 JS 的孤立代理项结果有意不同。
可选接收者先用 `??` 或 `!` 解包，暂不支持 `name?.trim()`；不支持正则、locale/normalize 等方法，
未知方法会列出支持项。查询暂不接受额外的起止位置参数。

```tsx
import { View, Text } from "zant";
interface Note { title: string; }
export default function App() {
  const query = " 周报 ";
  const notes: Note[] = [{ title: "周报" }, { title: "计划" }];
  const needle = query.trim().toLowerCase();
  const visible = notes.filter((note) => note.title.toLowerCase().includes(needle));
  return <View>{visible.map((note, i) => <Text key={i}>{note.title}</Text>)}</View>;
}
```

