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

多文件项目用 `addTsxProject`：所有 `.tsx` 一起编译，每个文件生成同名的 `.zig`，放在同一个输出目录里，
文件之间的 `import { NoteCard, type Note } from "./NoteCard"` 变成 `const _NoteCard = @import("NoteCard.zig");`。
入口文件导出根组件，其他文件只导出子组件和类型。

```zig
const notes = addTsxProject(b, tsx_tool, &.{
    b.path("examples/tsx/notes/App.tsx"),
    b.path("examples/tsx/notes/NoteCard.tsx"),
}, "App", zant_mod);
exe.root_module.addImport("notes_tsx", notes);
```

命令行：`zant-tsx in.tsx out.zig`（单文件），`zant-tsx --out <目录> a.tsx b.tsx …`（项目）。

## 第一阶段支持的子集

### 文件结构

- `import { ... } from "zant"`、`import type { ... } from "zant"`：只能从 `"zant"` 导入；
  可导入的名字：`useState`、各个标签、数值类型（`u8` `u16` `u32` `u64` `i8` `i16` `i32` `i64` `usize` `isize` `f32` `f64`）。
  标签不导入也能用（导入只是为了编辑器的类型检查）。
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

### JSX

- 文本按 React 的空白规则处理（相当于 Babel 的 `cleanJSXElementLiteralChild`）；支持 `&amp;` 等 HTML 实体。
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
  函数类型（`onToggle: (id: number) => void`）。子组件没有自己的状态（第三阶段），由父组件传值和回调。
- **主题**：`const theme = useTheme();`，`theme.colorPrimary`、`theme.colorText`、`theme.colorBgContainer` …
  （`zant.tokens.Theme` 的字段，camelCase）。默认跟随系统浅色 / 深色。
- **长列表**：`<List data={todos} renderItem={(t, i) => <Row …/>} keyExtractor={(t) => t.id} />`
  （React Native 的 `FlatList` 写法）编译成虚拟列表 `ui.list`，只构建看得见的行；
  `estimatedItemHeight`、`itemHeight`、`followTail`、`gap`、`padding` 同 `ui.ListOptions`。

### 性能

生成的代码与手写 Zig 一样：没有虚拟 DOM、没有运行时解释，状态是 struct 字段，事件是函数指针。
`zig build bench-tsx` 用同样的输入对比 TSX 与手写 Zig 的帧耗时；每次改代码生成都要复测，差距超过 5% 视为回归。
目前（ReleaseFast，Apple Silicon）：计数器点击 0.093 / 0.093 ms；1000 条待办（`examples/tsx/todos.tsx`
对 `todos_manual.zig`，React 式不可变更新对原地修改）添加 0.89 / 0.88 ms、勾选 1.55 / 1.55 ms、筛选 3.3 / 3.4 ms；
1 万行 `<List>`（`list_bench.tsx` 对 `list_manual.zig`）滚动 0.41 / 0.41 ms、点击 0.37 / 0.38 ms。

示例：`zig build run-tsx`（计数器）、`run-tsx-gallery`（组件展示）、`run-tsx-todos`（待办）、
`run-tsx-notes`（两个文件的便签：`examples/tsx/notes/`）。

## 后续阶段

- 第三阶段：子组件内部状态（`ui.localState`）、`useMemo` / `useRef` / `useEffect`、
  编译期自动 memo（不依赖状态的子树跳过比对，参考 Solid / React Compiler）、从 Zig 导入函数、`zant.store`。
- 第四阶段：`zant.d.ts`（编辑器补全，从标签表生成，参考 nsh）、`zant new --tsx`、`zant run --watch` 监视 `.tsx`。
