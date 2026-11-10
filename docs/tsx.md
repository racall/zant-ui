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
    b.path("examples/tsx/notes/format.zig"),
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
  函数类型（`onToggle: (id: number) => void`）。子组件可以有自己的状态（见下面「第三阶段」）。
- **字符串**：`str.length` 与 TypeScript 相同，是 UTF-16 码元数（`"周报".length` 是 2），不是 UTF-8 字节数。
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

只有有状态的组件（用了 `useState` / `<List>`，或者渲染了有状态的子组件，跨文件也会分析）才带 id，
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
  暂不支持返回清理函数（会报错）。
- 这三个 hook 都只能在组件体顶层调用，不能改名导入；用了它们的子组件算有状态组件（带实例 id）。

框架接口 `ui.effect(action)` / `ui.requestRebuild()` 手写 Zig 也能用。

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
const visible = notes.filter((note) => note.title.toLowerCase().includes(query.trim().toLowerCase()));
```

### 待做

- effect 清理函数、编译期自动 memo（不依赖状态的子树跳过比对，参考 Solid / React Compiler）、
  `zant.store`。

## 后续阶段

- 第四阶段：`zant.d.ts`（编辑器补全，从标签表生成，参考 nsh）、`zant new --tsx`、`zant run --watch` 监视 `.tsx`。
