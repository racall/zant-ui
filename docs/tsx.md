# TSX 组件（编译成 Zig）

用 React 的写法写 Zant 组件：`.tsx` 文件在**编译期**由 `zant-tsx` 转成 Zig，
产物是纯原生代码，运行时没有 JS 引擎。示例：`examples/tsx/counter.tsx`（`zig build run-tsx`）。

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

生成的 Zig（`zant-tsx counter.tsx counter.zig`）大致是：

```zig
pub const Counter = struct {
    count: u32 = 0,

    pub fn render(self: *Counter) ui.Node { return ui.view(.{ ... }); }
    fn onClick0(self: *Counter) void { ... }
};
```

`zant.launch.run(gpa, Counter{}, Counter.render, .{ ... })` 就能运行（见 `examples/tsx/main.zig`）。

TSX 只是静态类型的 **TypeScript 子集**：能编译的代码在 TS 里语义相同；用到子集之外的语法时，
编译器在 `.tsx` 的行列上报错，不会生成半成品。实现参考了 esbuild（解析）、Babel（JSX 文本空白规则）、
Mitosis（组件 = 状态 + 函数 + 绑定）和 Static Hermes（用类型标注生成静态数值类型）。

## 构建

```zig
const counter = addTsx(b, tsx_tool, b.path("examples/tsx/counter.tsx"), "counter.zig", zant_mod);
exe.root_module.addImport("counter_tsx", counter);
```

`addTsx`（`build.zig`）在构建时运行 `zant-tsx`，把输出作为导入 `zant` 的模块。

**整个应用用 TS + TSX 写**（`zant create` 选 TS，不写 Zig）时用 `addTsxApp`：`src/` 下所有 `.tsx` 和 `.zig`
（原生模块）一起编译，入口 `main.zig` 由 `zant-tsx --main App` 生成，运行 `App.tsx` 的默认导出；窗口标题、尺寸等配置来自
`app_config.zig`（`zant sync` 从 `app.config.json` 生成）。主题默认跟随系统，`ZANT_THEME=light|dark` 可以固定。
示例 `examples/tsx/app/`（`zig build run-tsx-app`）。

```zig
const app = zant_build.addTsxApp(b, zant_dep, .{ .target = target, .optimize = optimize });
const exe = b.addExecutable(.{ .name = "app", .root_module = app });
```

多文件项目用 `addTsxProject`：所有 `.tsx` 一起编译，每个文件生成同名的 `.zig`，放在同一个输出目录里，
文件之间的 `import { NoteCard, type Note } from "./NoteCard"` 变成 `const _NoteCard = @import("NoteCard.zig");`。
入口文件导出根组件，其他文件只导出子组件和类型。

```zig
const notes = addTsxProject(b, tsx_tool, &.{
    b.path("examples/tsx/notes/App.tsx"),
    b.path("examples/tsx/notes/NoteCard.tsx"),
    b.path("examples/tsx/notes/format.zig"),
}, "App", zant_mod);
exe.root_module.addImport("notes_tsx", notes);
```

命令行：`zant-tsx in.tsx out.zig`（单文件），`zant-tsx --out <目录> a.tsx b.tsx …`（项目）。

## 第一阶段支持的子集

### 文件结构

- `import { ... } from "zant"`、`import type { ... } from "zant"`：也可以从 `"react"` 导入同样的名字；
  可导入的名字：`useState`、各个标签、数值类型（`u8` `u16` `u32` `u64` `i8` `i16` `i32` `i64` `usize` `isize` `f32` `f64`）。
  标签不导入也能用（导入只是为了编辑器的类型检查）。
- 兼容 `import React from "react"`、`import * as React from "react"` 和默认 + 具名混合导入，
  默认 / 命名空间绑定可任意命名，不生成运行时代码。`React.useState/useEffect/useMemo/useRef` 与直接调用相同，
  `<React.Fragment>` 与 `<>` 相同（子节点展开，根节点生成无样式的 View）；`React.ReactNode` / `React.ReactElement` 可作元素类型。
  具名导入复用 Zant 验证，`useReducer` 等未支持名字仍报错；`React.FC<P>` 请改写成 `function X(props: P)`。
  不支持副作用导入（如 CSS）或其他 npm 包，只接受 `"zant"`、`"react"`、相对 `.tsx` / `.zig` 导入。
- 恰好一个组件：`export default function Name() { ... }`，或者 `function Name` / `const Name = () => ...`
  加上 `export default Name;`。
  组件没有参数（props 在第二阶段）。生成 `pub const Name = struct { ... }`。

### 类型

| TS | Zig |
|---|---|
| `number` | `f64` |
| `boolean` | `bool` |
| `string` | `[]const u8`（状态里是 `zant.tsx.String`） |
| `u32`、`i32`、`f32`、`usize` 等（`from "zant"`） | 同名 Zig 类型 |

- 整数字面量可以给任何数值类型，小数字面量只能给浮点类型；没有类型标注的字面量是 `number`（`f64`）。
- 整数可以隐式转为浮点（例如传给 `f32` 的属性）；浮点不会隐式转为整数，两种整数类型之间也不会互相转换。
- 整数运算按 Zig 语义：`/` 向零取整（`@divTrunc`），`%` 是 `@rem`，溢出在 Debug/ReleaseSafe 下报错。
  需要 JS 的数值语义就用 `number`。
- 条件、`&&`、`||`、`!` 的操作数必须是 `boolean`（没有“真值”转换：写 `count > 0`，不要写 `count`）。

### 组件体

组件函数体按顺序可以有：

1. `const [x, setX] = useState<T>(初始值);`：只能写在组件函数体顶层。初始值必须是字面量
   （数字、字符串、布尔，或者 `-` 加数字）。`<T>` 可以省略，此时按初始值推断类型（`0` 推断为 `number`）。
   生成 struct 字段 `x: T = 初始值`。
2. `const`/`let` 派生值：`const doubled = count * 2;`，生成 render 里的局部变量。
3. 函数：`function inc() { ... }`、`const inc = () => { ... }`，可以有带类型标注的参数，返回类型默认为 `void`。
   生成方法 `fn inc(self: *Name, ...) ...`。在 render 里可以调用有返回值的函数。
4. `if (...) return <...>;` 提前返回，以及最后的 `return <...>;`。

### 状态与事件（React 语义）

- `setX(v)` 和 `setX(prev => expr)` 只能出现在事件处理函数里，在 render 里调用会报错。
- 事件处理函数看到的是**本次渲染时的值**：连续写两次 `setCount(count + 1)` 的结果是 +1，和 React 一样；
  `setCount(c => c + 1)` 拿到的是最新值。
- 事件属性可以接三种值：组件里的函数名（`onClick={increment}`）、setter
  （`onChange={setName}`，类型必须匹配），或者内联箭头函数（`onClick={() => setCount(0)}`、
  `onChange={(on) => setNotify(on)}`，参数类型从事件推断）。
- 事件处理函数可以读状态、自己的参数和局部变量，可以调用组件里的其他函数。
  第一阶段**不能读 render 里的局部变量**（闭包捕获在第二阶段），编译器会提示改成从状态推导。

### 表达式

字面量、模板字符串、`+ - * / %`、比较、`=== !==`（字符串按内容比较）、`&& || !`、`?:`、
字符串 `+` 拼接、`str.length`、调用组件里的函数、`console.log(...)`（生成 `std.debug.print`）。
事件处理函数里还可以给 `let` 局部变量赋值（`=`、`+=` 等）以及使用 `x++`。

### 循环与分支语句

事件处理函数、effect、组件内函数和数组方法的块体回调支持以下语句，循环体也可以不加花括号：

```tsx
function sum(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    if (i === 2) continue;
    if (i === 10) break;
    total += i;
  }
  return total;
}
const selected = items.filter(item => {
  for (const id of selectedIds) if (id === item.id) return true;
  return false;
});
```

- `for (let i = 0; 条件; i++)`：初始化只接受一个 `let` 声明、赋值或留空；条件必须为布尔值，
  留空等于 `true`；更新接受赋值、`++`、`--` 或留空。循环变量只在循环内可见。
- `for (const x of xs)` / `for (let x of xs)`：遍历数组（状态、局部变量、props 或函数返回值），
  数组表达式只求值一次。元素类型自动推断；`const` 不可重新赋值，`let` 可赋值；暂不支持解构。
- `while (条件)`、`do { … } while (条件);`：条件为布尔值；`do` 至少执行一次，
  `continue` 仍会检查条件。
- `break` 退出最近的循环或 `switch`，`continue` 进入最近循环的下一轮；不接受标签，不能跨越回调边界。
  循环、`switch` 中的 `return` 返回当前函数或数组回调。
- `switch (值)` 支持数字、字符串、布尔值和字符串字面量联合；字符串按内容比较。
  判别值只求值一次，`case` 表达式按顺序比较，未命中时执行 `default`（位置不限）。
  相邻空 `case` 可以共用一个分支；非空分支进入下一个 `case` 前必须用 `break`、`return`
  或其他跳转结束，不支持隐式贯穿。最后一个分支可以省略 `break`。

组件的 render 体保持声明式，不能直接写循环或 `switch`；渲染列表使用 `.map` / `.filter`，
计算循环放进函数或块体数组回调。`let` 若从未重新赋值，生成的 Zig 仍用 `const`。

### JSX

- 文本按 React 的空白规则处理（相当于 Babel 的 `cleanJSXElementLiteralChild`）；文本和属性字符串支持完整的 XHTML 1.0 实体（253 个名字，区分大小写）。
- 数字实体为 `&#123;`（十进制）或 `&#x1F600;`（十六进制，前缀只接受小写 `x`）；至少一位数字，
  必须以 `;` 结束，码点不超过 `0x10FFFF`。无效、未知或未结束的引用保留原文，不报错。
  代理项按 SWC 的 JS 字符串规则保存为 WTF-8，相邻的高 / 低代理项合并；生成 Zig 时孤立代理项使用字节转义。
  XHTML 表不含 `&lbrace;` / `&rbrace;`，花括号可写 `&#123;` / `&#125;`。
- `<Text>`、`<Button>`、`<Checkbox>`、`<Switch>` 的文本子节点和 `{表达式}` 拼成一个字符串，
  例如 `Count: {count}`。容器里直接写文本会报错（和 React Native 一样，要包在 `<Text>` 里）。
- 子节点可以是元素、`{cond && <X/>}`、`{cond ? <A/> : <B/>}`、`{cond ? <A/> : null}`，
  也可以是保存了元素的局部变量。片段 `<>...</>` 在子节点位置会展开。
- `style={{ ... }}` 对象字面量里的键和普通属性等价（同一个元素上，后写的生效）。
- 不写值的属性（`<Input password />`）等于 `true`。

## 标签与属性（第一阶段）

属性名用 React Native 的 camelCase 写法；同一行里的别名等价。枚举值用字符串字面量，
也可以是字符串字面量组成的 `?:`。颜色可以写 `"#rgb"`、`"#rrggbb"`、`"#rrggbbaa"` 或 `0xAARRGGBB`。

### `<View>`（`ui.view`）、`<Column>`（`ui.column`）、`<Row>`（`ui.row`）

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
| `flexWrap`/`wrap` | `nowrap` `wrap` `wrap-reverse` |
| `position` | `"absolute"` / `"relative"`；配合 `top` `right` `bottom` `left` |
| `overflow` | `"hidden"`（裁剪）、`"scroll"`（纵向滚动）、`"visible"` |
| `backgroundColor`/`background`、`borderColor` | 颜色 |
| `borderWidth`、`borderRadius`/`radius` | 数值 |
| `hoverBackground`、`hoverBorderColor` | 悬停时的颜色 |
| `order` | 整数 |

`<View>` 还有：`flexDirection`/`direction`（`row` / `column`，默认 `column`）、
`shadowColor`、`shadowBlur`/`shadowRadius`、`shadowOffsetY`、`onClick`/`onPress`。

### `<Text>`（`ui.text`）

`key`、`fontSize`/`size`、`fontWeight`/`weight`（`regular` `medium` `semibold` `bold` 等，或者 `"100"`～`"900"` / `600`）、
`color`、`textAlign`（`left` `center` `right`）、`wrap`（布尔）、`numberOfLines`/`maxLines`、`lineHeight`、
`fontFamily`/`family`（`sans` `serif` `mono`）。

### `<Button>`（`ui.button`）

`key`、`text`（或者写成子文本）、`variant`（`primary` `default` `text` `link`）、`size`（`small` `middle` `large`）、
`disabled`、`width`、`fontSize`、`onClick`/`onPress`。

### `<Input>`（`ui.input`）

`key`、`value`、`placeholder`、`disabled`、`readOnly`、`password`、`multiline`、`width`、`height`、
`fontSize`、`onChange`（`(value: string) => void`）、`onSubmit`。

### `<Checkbox>`（`ui.checkbox`）、`<Switch>`（`ui.switch`）

`key`、`label`（或者写成子文本）、`checked`、`disabled`、`fontSize`、`onChange`（`(checked: boolean) => void`）。

## 第二阶段（进行中）：全部组件与 React 常用写法

### 全部 `ui.*` 组件

每个返回 `ui.Node` 的 `ui.*` 组件都是一个标签，标签名是函数名的 PascalCase：`ui.radioGroup` → `<RadioGroup>`、
`ui.iconButton` → `<IconButton>`、`ui.@"switch"` → `<Switch>`、`ui.dropdownMenu` → `<DropdownMenu>`。
标签表（`tools/tsx/ui_tags.zig`）由 `zig build tsx-tags` 在编译期反射 `ui.*` 的选项结构体生成并提交，
`zig build test` 检查它与源码一致，所以框架新增的组件和字段自动可用。属性名是字段名的 camelCase
（`on_change` → `onChange`、`width_percent` → `widthPercent`）；第一阶段的 React Native 写法
（`backgroundColor`、`paddingHorizontal`、`flex`、`position`、`overflow`、`fontSize`、`onPress` …）作为别名保留。

字段类型与 TSX 值：

| Zig 字段 | TSX |
|---|---|
| `f32` / 整数 / `bool` / `[]const u8`（可选的 `?T` 同） | number / number / boolean / string |
| 枚举（含 `Icon`） | 字符串字面量，接受 `chevron_down`、`chevron-down`、`chevronDown` 三种写法 |
| `?u32` 颜色 | `"#rrggbb"`、`0xAARRGGBB`、主题颜色 |
| `geometry.Edges` | 数字（四边相同）或 `{ top, right, bottom, left, horizontal, vertical }` 对象 |
| `ui.Node` / `?ui.Node` | 元素：`<Tooltip text="…" child={<Button … />} />`，或唯一的子元素 |
| `[]const ui.Node` | 子元素 |
| `[]const []const u8` | 字符串数组：`<Select options={["A", "B"]} />` |
| `[]const SomeStruct` | 对象数组：`<Tabs items={[{ label: "A", content: <Text>…</Text> }]} />` |
| 嵌套的选项结构体（`ClickableOptions.box`） | 展开成同一层的属性 |
| `?actions.Action` / `BoolAction` / `TextAction` / `IndexAction` | `() => void` / `(on: boolean) => void` / `(text: string) => void` / `(index: usize) => void` |

没有默认值的字段是必填属性，缺了会在标签处报错。暂不支持的字段类型（指针回调等）不出现在标签表里。

### 语言

- **类型**：`T[]`、`Array<T>`、`interface` / `type` 对象类型（字段可选 `x?: T`）、字符串字面量联合
  （`type Filter = "all" | "done"` → Zig 枚举）、`T | null`。
- **数组**：字面量和展开 `[...todos, item]`、`.length`、`a[i]`、`.map((x, i) => …)`、`.filter(x => …)`、
  `.find`、`.some`、`.every`、`.indexOf`、`.includes`、`.slice`、`.join`（字符串数组）。
- **对象**：字面量、展开 `{ ...todo, done: true }`、成员访问、`?.`、`??`。
- **状态**：`useState` 可以存数组和对象。React 的不可变更新照常写：
  `setTodos([...todos, { id, title, done: false }])`、`setTodos(todos.filter((t) => t.id !== id))`。
  setter 把新值深拷贝进这个状态自己的内存，旧值保留到下一帧开始（事件处理函数里读到的快照一直有效）。
- **列表渲染**：子节点里的 `{todos.map((t) => <TodoRow key={t.id} … />)}`；`key` 是字符串或数字。
- **闭包**：事件处理函数可以读 render 里的局部变量和 `.map` 的参数（`onClick={() => remove(t.id)}`），
  捕获的值按值复制进帧内存。
- **子组件**：同一文件里的其他函数组件，或从项目里另一个文件导入（`import { TodoRow, type Todo } from "./TodoRow"`，
  被导入的组件和类型要 `export`；跨文件共享的嵌套对象类型和联合类型要写成具名的 `interface` / `type`）。参数是一个 props 对象
  （`function TodoRow(props: Props)` 或解构 `function TodoRow({ todo, onToggle }: Props)`），回调属性的类型写成
  函数类型（`onToggle: (id: number) => void`）。子组件可以有自己的状态（见下面「第三阶段」）。
- **字节**：`Uint8Array` 对应 Zig 的 `[]const u8`（剪贴板图片、文件内容），`.length` 是字节数，`data[i]` 是 `u8`。
  从 Zig 导入的函数用 `zant.tsx.Bytes` 表示字节参数和返回值（直接写 `[]const u8` 是字符串）。
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
  （`zant.tokens.Theme` 的字段，camelCase）。默认跟随系统浅色 / 深色。
- **长列表**：`<List data={todos} renderItem={(t, i) => <Row …/>} keyExtractor={(t) => t.id} />`
  （React Native 的 `FlatList` 写法）编译成虚拟列表 `ui.list`，只构建看得见的行；
  `estimatedItemHeight`、`itemHeight`、`followTail`、`gap`、`padding` 同 `ui.ListOptions`。

### 内置对象

- `Math`：`floor`、`ceil`、`round`、`trunc`、`abs`、`sign`、`sqrt`、`cbrt`、`pow`、`exp`、
  `log`、`log2`、`log10`、`sin`、`cos`、`tan`、`atan`、`atan2`、`hypot`，以及 `PI`、`E`。
  `min(...值)` / `max(...值)` 接受零到多个参数，空参数返回正 / 负 Infinity；`hypot()` 为 0。
  `round` 按 JS 规则向最近整数舍入，恰好一半时向正无穷（`Math.round(-2.5)` 为 -2）。
- 显式整数的 `floor` / `ceil` / `round` / `trunc` / `abs` 保留整数类型，不经过浮点转换；
  `min` / `max` 按整数混用规则选类型，其他 Math 运算返回 `number`。有符号最小值的 `abs`
  沿用原生整数溢出规则。`random()` 使用进程内只播种一次的非密码学 PRNG，返回 `[0, 1)`。
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

字符串结果在当前帧内有效；数字转字符串、随机数和时钟调用不能用于文件顶层常量，纯数值运算和内置常量可以。
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
<Text size={16}>
  读 <Text weight="bold">这里</Text>，或者 <Link onClick={openDocs}>打开文档</Link>{count > 0 && `（${count}）`}
</Text>
<Button variant="primary" onClick={save}><Icon name="check" /> 保存</Button>
```

- 嵌套的 `<Text>` 编译成一个 `ui.richText`：每段文字一个 span，样式继承外层（`size` `weight` `color` `family`，
  另外 span 可以用 `italic` `underline` `strike` `background`）；行内 `<Link>` 只接受 `onClick`，颜色取主题主色。
  带 span 的文字不能用 `maxLines` / `textAlign` / `wrap`。
- 点击只给最里层的可点击元素（不冒泡），所以 `<Clickable>` 卡片里放 `<Button>` 是安全的：点按钮只触发按钮。
- `Modal`、`Message` 可以写在任何位置，渲染时放到窗口根节点（`ui.portal`，同 React portal），总是盖住整个窗口。
- 会自己滚动的元素（`<List>`、`<PdfViewer>`）不能放进滚动容器（`scroll` / `overflow="scroll"`）里，也不能放进另一个 `<List>` 的行里：
  给它一个高度有限的普通容器。
- 同一层的字面量 `key` 不能重复；`.map` 和 `<List>` 行里的**有状态子组件（以及 `<Markdown>`、`<PdfViewer>`）必须有 `key`**，否则它们的状态会混在一起。
  子组件的 `key` 会覆盖它根元素上的 `key`（和 React 一样，`key` 属于组件实例）。
- 子组件的 `children` 类型决定能放什么：`ReactNode` 一个元素，`ReactNode[]` 任意元素，`string` 文字和 `{值}`
  （`<Badge>new {count}</Badge>`）。
- 整数属性（`page`、`maxLines` …）也接受 TS 的 `number`：向零取整，并钳到字段的取值范围。

### 性能

生成的代码与手写 Zig 一样：没有虚拟 DOM、没有运行时解释，状态是 struct 字段，事件是函数指针。
`zig build bench-tsx` 用同样的输入对比 TSX 与手写 Zig 的帧耗时；每次改代码生成都要复测，差距超过 5% 视为回归。
目前（ReleaseFast，Apple Silicon）：计数器点击 0.093 / 0.093 ms；1000 条待办（`examples/tsx/todos.tsx`
对 `todos_manual.zig`，React 式不可变更新对原地修改）添加 0.89 / 0.88 ms、勾选 1.55 / 1.55 ms、筛选 3.3 / 3.4 ms；
1 万行 `<List>`（`list_bench.tsx` 对 `list_manual.zig`）滚动 0.41 / 0.41 ms、点击 0.37 / 0.38 ms。

示例：`zig build run-tsx`（计数器）、`run-tsx-gallery`（组件展示）、`run-tsx-todos`（待办）、
`run-tsx-notes`（两个文件的便签：`examples/tsx/notes/`）。

## 第三阶段（进行中）：组件状态与 hooks

### 子组件的 `useState`

子组件里的 `useState` 和 React 一样属于**这个实例**：同一个 `<Counter>` 渲染三次就有三份独立的状态，
实例卸载（父组件不再渲染它）时状态释放，再出现时从初始值开始。

```tsx
function Counter({ label }: { label: string }) {
  const [count, setCount] = useState(0);
  return <Button onClick={() => setCount(count + 1)}>{label}: {count}</Button>;
}
```

编译结果：状态字段放进 `_Local` 结构体，由框架的 `ui.localState` 按实例 id 持有；构建函数多一个 id 参数，
调用处用 `tsx.childId(父实例 id, "Counter", key)` 计算（没有 key 时按调用位置编号 `tsx.siteId`）。
所以实例的身份和 React 一样由**父组件 + 组件名 + key（或位置）**决定：列表里换了顺序、删了别的项，
带 key 的实例状态跟着 key 走。`.map` 里的有状态组件一定要给 `key`（React 也要求），否则它们共用一份状态。
`<List>` 的滚动状态同样保存在所在组件的实例里，子组件里也可以用 `<List>`。

只有有状态的组件（用了 `useState` / `<List>` / 文档组件，或者渲染了有状态的子组件，跨文件也会分析）才带 id，
无状态组件的生成代码和以前一样，没有额外开销。

性能（ReleaseFast）：300 行、每行一个有状态 `<Counter>`（`local_bench.tsx` 对手写 `ui.localState` 的
`local_manual.zig`），逐行挂载 0.444 / 0.428 ms，点击 0.562 / 0.558 ms（`zig build bench-tsx -- local`）。

### `useRef`、`useMemo`、`useEffect`

和状态一样，它们的存储是组件结构体（根组件）或 `_Local` 槽位（子组件）里的字段，跟着实例走。

- `const nextId = useRef<u32>(1)`：只能通过 `nextId.current` 读写；读到的总是当前值（不是快照），
  在事件处理函数和 effect 里写，不触发重建。初始值是字面量。
- `const visible = useMemo(() => items.filter(…), [items, query])`：依赖列表**按值**比较
  （字符串、数组、对象按内容），不变就直接用缓存，函数体不执行；结果复制进缓存自己的内存。
  比较本身是 O(依赖大小)，适合包住比它贵的计算（过滤、排序、统计）。不能缓存元素（每帧照常构建）。
- `useEffect(() => { … }, [deps])`：在这次构建协调完之后运行（`ui.effect`）。`[]` 只在挂载后运行一次，
  有依赖时依赖变了才运行，不写依赖列表则每次构建后都运行。effect 里调用了 setter（或组件函数、回调属性）
  时会再构建一次；只写 ref 的不会，所以不写依赖的 effect 不会死循环。
- 清理函数：effect 里 `return () => { … }`，在下一次运行之前、以及组件卸载时运行（React 语义）。
  它可以读 effect 里的局部变量（按值复制保存）、状态和 ref；**不能读 props**（卸载时父组件的数据可能已经不在了），
  需要的话先在 effect 里复制成局部变量：`const name = label; return () => log(name);`。
- 这三个 hook 都只能在组件体顶层调用，不能改名导入；用了它们的子组件算有状态组件（带实例 id）。

框架接口 `ui.effect(action)` / `ui.requestRebuild()` 手写 Zig 也能用。

### Markdown 与 PDF

`<Markdown>` 和 `<PdfViewer>` 是有独立状态的原生文档组件，标签和属性由同一份
`ui.*` 反射表生成（`zig build tsx-tags`）。`(u64, Options)` builder 的标签带
`instance_id` 标记，编译器自动传入父实例 + 标签名 + key / 调用位置计算的 id；
包含它们的子组件也自动带 id。`.map` 中请给每个实例稳定的 `key`。

```tsx
<Markdown source={reply} streaming={busy} caret="block" onLink={openLink} />
<Markdown>{reply}</Markdown>
<PdfViewer src={path} toolbar annotationTools onChange={() => console.log("changed")} height={600} />
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
`onLink`、`onChange`、`onAnnotation` 同 `pdf.ViewOptions`；`width` / `height` / `grow`
控制容器尺寸，指定 height 时默认不伸长。空 / 加载提示可用 `emptyText` / `loadingText` 修改。

每个实例的缓存或 `Viewer` 都由 `ui.localSlot` 持有，卸载时自动 `deinit`；再次挂载从初始状态开始。
内部节点 key 使用实例 id 命名空间，多个文档不会互相影响。手写 Zig 对应
`ui.markdown(id, options)` / `ui.pdfViewer(id, options)`，复杂的 Markdown 渲染扩展和 PDF `labels`
结构体仍可从 Zig 传入；TSX 只反射支持的字段类型。

示例：`zig build run-tsx-reader`（`examples/tsx/reader.tsx`），用按钮逐块追加回复，PDF 使用仓库内的
小型测试文件 `tests/fixtures/widgets.pdf`，也可输入其他路径。
基准：`zig build bench-tsx -Doptimize=ReleaseFast -- markdown`，同样的输入逐帧对比手写 Streamdown。
本机 ReleaseFast（640×900 CPU 光栅、8208 字节、每帧追加 24 字节、各 1368 帧）：
TSX 平均 0.348 ms，手写 0.339 ms，差距 2.61%。

### 应用 hooks（只在根组件里用）

```tsx
useCloseRequest(() => {            // 点关闭按钮 / ⌘W：返回 false 不关（比如先弹确认框、隐藏到托盘）
  if (dirty) { setAsk(true); return false; }
  return true;
});
useQuitRequest(() => true);        // ⌘Q / Dock 退出；不写时问 useCloseRequest
useKeyDown((e: KeyEvent) => {      // 全局按键，在获得焦点的控件之前；返回 true 表示处理了
  if (e.key === "Escape") { closeDialogs(); return true; }
  if (e.mod && e.key === "f") { focusSearch(); return true; }
  return false;
});
```

- `KeyEvent`：`key` 和 DOM 的 `KeyboardEvent.key` 一样（`"Escape"` `"Enter"` `"Tab"` `"ArrowUp"` `"F1"` … 或输入的字符），
  `mod` 是平台的命令键（macOS ⌘、Windows Ctrl），另有 `shift` `alt` `ctrl` `meta` `repeat`。
- 回调像事件处理函数一样运行（读状态快照、调 setter、读写 store），不能读渲染体里的局部变量。

### 从 Zig 导入函数

项目里的 TSX 可以从相对路径的 `.zig` 文件按名导入顶层 `pub fn`，也支持 `as` 别名：

```tsx
import { plural } from "./format.zig";
// render 或事件处理函数里都能调用，参数个数和类型在 TSX 编译时检查
<Text>{plural(notes.length, "note")}</Text>
```

函数签名支持 `bool`、`[]const u8`（字符串）、所有上述数值类型，以及这些类型的 `?T`、
`[]const T`（包括字符串数组）；返回值还支持 `void`，可在事件处理函数里作为语句调用。
不支持结构体、其他指针、`anytype`、comptime 参数、错误联合或泛型，也不能用 `import type` 导入。
返回字符串或切片时，内存必须是静态内存，或至少在当前帧有效（用 `zant.ui.frameAllocator()` 分配）。

把 `.zig` 文件也加进 `addTsxProject` 的 sources，或运行
`zant-tsx --out <目录> App.tsx format.zig`。编译器用 Zig AST 读取签名，把手写文件原样复制到
生成文件旁边；`format.tsx` 和 `format.zig` 输出重名会报错。单文件模式（`addTsx` / `in.tsx out.zig`）
不支持 Zig 导入。完整用法见 `examples/tsx/notes/`。

### 共享状态：`createStore` / `useStore`

跨组件、跨文件的应用状态，写法接近 Zustand（框架的 `zant.store` 是同一思路的 Zig 版）：

```tsx
// store.tsx
export interface Settings { dark: boolean; fontSize: u32; }
export const settings = createStore<Settings>({ dark: false, fontSize: 14 });

// 任意组件（import { settings } from "./store"）
const dark = useStore(settings, (s) => s.dark);   // 选择器可省略：useStore(settings)
<Switch checked={dark} onChange={(on) => settings.update({ dark: on })} />
```

- `createStore<T>(初始值)` 写在文件顶层（类型参数必填），生成文件级的 `pub var settings: tsx.Store(T)`；
  其他文件像导入组件一样导入它。
- `useStore(store, selector?)` 是 hook，在组件顶层调用，得到渲染时的值（和状态一样，事件处理函数读到的是这一帧的快照）。
- 在事件处理函数和 effect 里修改：`store.update({ 字段: 值 })` 按字段浅合并，`store.set(新值)` 整体替换，
  `store.get()` 读当前值（任何地方都能调用）。修改会让所有窗口重建。
- 和根组件的状态一样，旧值在下一次读取它的构建开始时释放；多个窗口共用时，一个窗口在另一个窗口重建之前
  收到事件、并且回调里用到旧值，是目前已知的边界情况。

### 字符串方法

字符串保持 UTF-8 存储，位置和长度按 UTF-16 码元计算（与 `.length` 一致）：

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
- 取字符：`charAt(i)` 越界返回 `""`；`at(i)` 支持负索引，返回可选字符串 `?[]const u8`，
  越界为 `null`（TS 的 `undefined` 在此沿用可选值模型），可用 `??` / `!`。

所有返回的新字符串和数组在当前帧内有效，与 `tsx.fmt` 相同；存入状态时会深拷贝。
位置落在代理对内部时沿用 `unicode.utf16ToByte` **向前取 UTF-8 码点边界**，始终保持合法 UTF-8。
例如 `"😀".slice(0, 1)` / `.charAt(0)` 是空串，`.slice(1, 2)` / `.charAt(1)` 是完整 emoji；
填充文本截断也向前取边界，可能比目标少一个码元；空模式 `replaceAll` 的两个代理对内部相邻边界
会映射到同一位置（`"😀".replaceAll("", "-")` 为 `"--😀-"`）。这些与 JS 的孤立代理项结果有意不同。
可选接收者先用 `??` 或 `!` 解包，暂不支持 `name?.trim()`；不支持正则、locale/normalize 等方法，
未知方法会列出支持项。查询暂不接受额外的起止位置参数。

```tsx
const needle = query.trim().toLowerCase();
const visible = notes.filter((note) => note.title.toLowerCase().includes(needle));
```

### 从 Zig 导入类型

原生模块里的结构体和枚举可以直接在 TSX 里用：

```tsx
import type { Peer, Kind } from "./sync.zig";   // pub const Peer = struct { … }; pub const Kind = enum { … };
import { peers } from "./sync.zig";             // pub fn peers() []const Peer
const online: Peer[] = peers().filter((p) => p.online);
if (peer.kind === "laptop") { … }               // 枚举按字符串字面量比较
```

- 结构体变成对象类型（字段名和 Zig 一样，比如 `last_seen`），枚举变成字符串字面量联合；嵌套结构体、`?T`、`[]const T`
  都可以。Zig 函数的参数和返回值也可以是这些类型。
- TSX 表示不了的字段（指针、互斥锁……）不出现在 TSX 类型里，读其他字段没问题；这种结构体不要在 TSX 里新建。
- 返回的字符串和切片的内存规则和导入的函数一样：静态内存，或者至少这一帧有效。

### 待做

- 编译期自动 memo（不依赖状态的子树跳过比对，参考 Solid / React Compiler）。

## 后续阶段

- 第四阶段：`zant.d.ts`（编辑器补全，从标签表生成，参考 nsh）、`zant new --tsx`、`zant run --watch` 监视 `.tsx`。
