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

## 后续阶段

- 第二阶段：所有 `ui.*` 组件（从 `src/ui.zig` 反射生成标签表）、`.map` 列表和 key、闭包捕获、
  数组 / 对象 / 联合类型的状态、带 props 的子组件、主题 token。
- 第三阶段：子组件内部状态（`ui.localState`）、`useMemo` / `useRef` / `useEffect`、
  从 Zig 导入函数、`zant.store`。
- 第四阶段：`zant.d.ts`（编辑器补全）、`zant new --tsx`、`zant run --watch` 监视 `.tsx`。
