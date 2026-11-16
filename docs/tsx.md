# TSX 组件（编译成 Rust）

```tsx
import { useState, View, Text, Button } from "zant";
import type { u32 } from "zant";
export default function Counter() {
  const [count, setCount] = useState<u32>(0);
  return <View padding={24} gap={12}>
    <Text>计数：{count}</Text>
    <Button onClick={() => setCount(c => c + 1)}>+1</Button>
  </View>;
}
```

推荐用 TSX 写界面、Rust 写原生能力。TSX 是静态类型的 TypeScript 子集，编译期转为 Rust；运行时没有 JS 引擎，无需 React 或 npm。示例项目见 `scaffold/template-ts/`。

## 构建

```bash
zant create my-app --yes    # 去掉 --yes 可交互配置
cd my-app
zant run                   # 默认原地热刷新
zant run --watch            # 旧重启模式
zant run --no-hot           # 运行一次
# 仓库内单独调用编译器（调试生成的 Rust）：
cargo run -p zant-tsx --bin zant-tsx -- in.tsx out.rs
cargo run -p zant-tsx --bin zant-tsx -- --out out --src src
```

| 项目文件 | 用途 |
|---|---|
| `src/App.tsx` | 入口；存在即识别为 TSX 项目，`src/` 只放 `.ts` / `.tsx` |
| `src/components/`、`src/stores/`、`src/windows/` | 模板演示父子 props、共享状态、多窗口 |
| `native/main.rs` | `zant::app!();` |
| `native/build.rs` | `fn main() { zant_build::tsx_app(); }` |
| `Cargo.toml` | `build = "native/build.rs"`，`[[bin]] path = "native/main.rs"` |
| `native/*.rs` / `native/*.ts` | Rust 实现 / 接口规范，见[原生模块](tsx-native.md) |

- 产物在 `$OUT_DIR`，不必手写生成文件或模块清单；构建器处理图标和 Windows 资源。
- 框架固定 Rust **1.94.1**；`zant create` 复制框架工具链版本，读不到时回退为 `stable`。
- 热刷新保留兼容的根状态、组件本地状态和稳定 key 的滚动 / 焦点 / 输入选区；store、Rust 全局状态和外部资源不自动恢复。编译失败显示错误面板，修复后刷新。文件分类、重启规则、资源清理与沙盒限制见[热更新](scaffold.md#热更新)。

### 应用配置与系统菜单

```json
{ "title": "我的应用", "bundle_id": "com.example.app", "version": "0.1.0",
  "window": { "width": 680, "height": 700, "min_width": 600, "min_height": 620, "remember": true } }
```

- `app.config.json` 字段用 snake_case，TSX 属性用 camelCase。元数据还有 `description`、`company`、`copyright`、`macos.category`、`macos.minimum_system_version`（默认 `"13.0"`），用于打包、关于面板和 Windows 文件属性，见[脚手架](scaffold.md)。
- 上例是模板窗口尺寸，非 API 默认值。`remember: true` 按 `bundle_id` 保存并恢复主窗口及 `createWindow` 窗口的位置 / 大小；旧配置省略时为 `false`。
- macOS 自动提供应用、文件、编辑、显示、窗口、帮助菜单；编辑命令作用于焦点控件，关闭 / 退出经过相应 hook。

### 编辑器类型

- 模板带 `zant.d.ts` 和 `tsconfig.json`（`jsx: "preserve"`、`strict`、`noEmit`、`types: []`），`paths` 将 `"zant"` 指向本地声明；已安装 TypeScript 时可运行 `tsc --noEmit -p .`。
- API 以 [`zant.d.ts`](../crates/zant-tsx/zant.d.ts) 为速查；编辑器不能检查所有子集和嵌套限制，最终以 `zant-tsx` 为准。`.rs` 导出在编辑器中为 `any`，资源默认导出为 `Uint8Array`。
- 多文件各生成同名 `.rs`；`./NoteCard` 对应 `crate::note_card`（文件 stem 转 snake_case），同名输出冲突会报错，不按目录区分。组件、类型、普通函数及带注解或字面量类型的 `export const` 均可按名导入。

## 基础语法

### 文件结构与类型

示例：顶层 `import React from "react";`，组件顶层 `const [count, setCount] = React.useState(0);`；显式类型请用直接导入的 `useState<T>`。

| 写法 | 支持与限制 |
|---|---|
| 导入 | `"zant"` / `"react"`、相对 `.tsx` / `.ts` / `.rs`、资源、`"native/模块"`；不支持副作用导入（CSS 等）和其他 npm 包 |
| React 兼容 | 默认、命名空间、默认 + 具名混合导入；绑定可任意命名；`React.useState/useRef/useMemo/useEffect`、`React.Fragment`、`React.ReactNode/ReactElement`；不支持 `useReducer`、`React.FC<P>` |
| 入口 | 一个默认导出的无 props 根组件；也可声明 `function Name` / `const Name = () => …` 后 `export default Name`；同文件可有其他组件和普通函数 |
| 标签 | PascalCase；编译器允许不导入，编辑器检查需导入；不支持小写 HTML 标签 |
| 数值 | `number` → `f64`；`u8/u16/u32/u64/i8/i16/i32/i64/usize/isize/f32/f64` 从 `"zant"` 导入，对应同名 Rust 类型 |
| 其他类型 | `boolean`、`string`、`T[]` / `Array<T>`、`interface` / `type` 对象、可选字段 `x?: T`、字符串字面量联合、`T \| null` |

- 无注解的数值字面量为 `number`；整数字面量可用于任意数值类型，小数字面量只用于浮点。整数可转浮点或更宽整数，一般赋值不允许浮点转整数或整数窄化。
- 整数 `/` 向零取整、`%` 取余，溢出 panic（工作区 / 模板 release 也检查溢出）；需要 JS 数值语义用 `number`。
- 混合整数运算选能容纳双方的类型（`u8 + usize` → `usize`，`u16 + i32` → `i32`），不能容纳时用 `number`（如 `u64 + i32`）。标签整数属性接受 `number`，向零取整并钳到字段范围。

### 组件体、状态与事件

```tsx
// 组件体片段，useState、Button 从 zant 导入：
const [count, setCount] = useState(0);
function increment() { setCount(c => c + 1); }
return <Button onClick={increment}>{count * 2}</Button>;
```

- 组件顶层按顺序写 hooks、派生值、函数、提前 `if (…) return <…>` 与最终 return；`const` / `let` 可保存派生值，组件内函数返回类型默认 void，有返回值需标注。
- `useState<T>(初始值)` 可省略类型；初值是数字、字符串、布尔、`null`、数组、对象字面量或可编译期求值的文件常量，不支持惰性初始化函数。
- setter 只能用于事件、effect 等非渲染回调；事件读取本帧快照，两次 `setCount(count + 1)` 只加 1，`setCount(c => c + 1)` 每次读最新值。
- 事件属性接受函数名、类型匹配的 setter 或内联箭头；参数类型可从事件推断。处理函数可调用组件函数，按值捕获渲染局部变量和 `.map` 参数。
- 读渲染局部变量的命名箭头处理函数也按快照捕获，可传给控件 / 子组件 / `useHotkey`，或在其他事件与 effect 中调用；应用 hook 不能调用它。

### 表达式、循环与分支

```tsx
// 普通函数片段，可在 render 中调用 sum(5)：
function sum(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) total += i;
  return total;
}
```

| 类别 | 支持 |
|---|---|
| 表达式 | 字面量、模板字符串、字符串 `+`、`+ - * / %`、比较、`=== !==`（字符串按内容）、`&& \|\| !`、`?:`、函数调用、`console.log`（stderr） |
| 局部赋值 | 事件等语句体中 `let` 支持 `=`、`+=` 等及 `++` / `--` |
| `for` | `for (let i = 0; 条件; i++)`；初始化为一个 `let`、赋值或空，布尔条件 / 更新可空；变量只在循环内可见 |
| `for…of` | `const` / `let` 遍历数组或字符串；数组表达式只求值一次，字符串按 Unicode 码点遍历；不支持解构 |
| 其他控制流 | `while`、`do…while`、`break`、`continue`、`return`；不支持标签或跨回调跳转 |
| `switch` | 数字、字符串、布尔、字符串联合；case 按源码顺序求值，非空分支不能隐式贯穿，末分支可省 `break` |

循环 / `switch` 用于事件、effect、函数或数组方法块体回调；render 体不能直接写，列表用 `.map` / `.filter`。循环内 `return` 返回当前函数 / 回调；普通条件须为布尔，可选值的真值判断见下文。

### JSX

```tsx
// 组件体片段，ready 为 boolean：
return <View style={{ padding: 16 }}><Input password />
  <Text>保存 &amp; 继续</Text>{ready && <Button>继续</Button>}
</View>;
```

- 支持 `{值}`、条件元素、`null`、元素局部变量、`<>…</>`；片段在子节点处展开，根片段为无样式 View。
- 文本遵循 React 空白规则；文本 / 属性支持 XHTML 1.0 命名实体（区分大小写）、十进制 `&#123;`、十六进制 `&#x1F600;`（小写 `x`，须有数字和 `;`，码点 ≤ `0x10FFFF`）。无效引用保留原文；孤立代理项不能生成 Rust 字符串，相邻高低代理项可合并。
- 文本类标签把文字与表达式拼成字符串；容器内文字须放进 `<Text>`。无值属性等于 `true`。
- `style={{ … }}` 的键等同普通属性；`<Button {...look} disabled />` 按源码顺序合并，后写覆盖。可展开字面量、变量、成员路径；调用结果先存 `const`，可选字段不能展开。枚举也可用字符串联合变量或字面量三元表达式。

## 常用标签与属性

```tsx
// 组件体片段，hooks 与标签从 zant 导入：
const [name, setName] = useState("");
const [enabled, setEnabled] = useState(false);
const theme = useTheme();
return <View padding={16} gap={8} background={theme.colorBgContainer}>
  <Text color={theme.colorText} fontSize={18}>设置</Text>
  <Input value={name} onChange={setName} placeholder="名称" />
  <Row gap={8}><Checkbox checked={enabled} onChange={setEnabled}>启用</Checkbox>
    <Switch checked={enabled} onChange={setEnabled} /></Row>
  <Button variant="primary" disabled={!enabled}>保存</Button>
</View>;
```

| 标签 | 常用属性（完整列表见 `zant.d.ts`） |
|---|---|
| `View` / `Column` / `Row` | `key`、`gap/rowGap/columnGap`、`padding` / `margin`（含 Horizontal/Vertical/Top/Right/Bottom/Left）、`width/height`（数值或 `"50%"`）、min/max 宽高 |
| Flex | `flex`（grow=n、shrink=1、basis=0）、`flexGrow/grow`、`flexShrink/shrink`、`flexBasis/basis`、`justifyContent/justify`、`alignItems`、`alignSelf`、`alignContent`、`order` |
| 布局 / 外观 | `flexWrap/wrap`、`position` + 四边、`overflow`、`backgroundColor/background`、`borderColor`、`borderWidth`、`borderRadius/radius`、`hoverBackground/hoverBorderColor` |
| `View` 另有 | `flexDirection/direction`（row/column，默认 column）、`shadowColor`、`shadowBlur/shadowRadius`、`shadowOffsetY`、`onClick/onPress` |
| `Text` | `fontSize/size`、`fontWeight/weight`、`color`、`textAlign`、`wrap`、`numberOfLines/maxLines`、`lineHeight`、`fontFamily/family` |
| `Button` | `text` 或文字子节点、`variant`（primary/default/text/link）、`size`（small/middle/large）、`disabled`、`width`、`fontSize`、`onClick/onPress` |
| `Input` | `value`、`placeholder`、`disabled/readOnly/password/multiline`、`width/height`、`fontSize`、`onChange(string)`、`onSubmit` |
| `Checkbox` / `Switch` | `label` 或文字子节点、`checked`、`disabled`、`fontSize`、`onChange(boolean)` |

- 对齐值：`flex-start/start`、`center`、`flex-end/end`；按属性还可用 `stretch`、`auto`、`space-between/around/evenly`，具体见声明。横向支持 `wrap`，纵向换行、`wrap-reverse`、auto/calc 尚未实现。
- `position="absolute"/"relative"`；`overflow="hidden"/"scroll"/"visible"`（scroll 为纵向）。字号为数值，字重接受 regular/medium/semibold/bold 等、`"100"`～`"900"` 或数值；字体为 sans/serif/mono。
- 颜色接受 `"#rgb"`、`"#rrggbb"`、`"#rrggbbaa"`、`0xAARRGGBB`；应用优先用 `useTheme()` 的 camelCase token（`colorPrimary`、`colorText` 等），默认随系统，可用 `ZANT_THEME=light|dark` 固定。

## 组件与 React 常用写法

### 组件与 props

```tsx
import { View, Text, Tooltip, Button } from "zant";
interface BadgeProps { size?: number; children: string; }
function Badge({ size = 14, children }: BadgeProps) { return <Text fontSize={size}>{children}</Text>; }
export default function App() {
  return <View><Badge>新消息</Badge><Tooltip text="提示"><Button>保存</Button></Tooltip></View>;
}
```

| 值 / 类型 | 写法 |
|---|---|
| 组件 | 同文件函数或 `import { TodoRow, type Todo } from "./TodoRow"`；被导入项须 `export`，跨文件共享嵌套对象 / 联合类型须具名 |
| props | 一个带类型的对象参数，支持解构；回调如 `onToggle: (id: number) => void`；可选 prop 可设常量默认值，必填 prop 不能设默认值 |
| 内置组件 | 支持的 `ui::*` builder 对应 PascalCase 标签（radio_group → RadioGroup）；字段变为 camelCase，别名保留；缺必填字段报错，不支持的字段类型不暴露 |
| 枚举 / 图标 | 字符串字面量；接受 `chevron_down` / `chevron-down` / `chevronDown` |
| 边距 | 数字或 `{ top, right, bottom, left, horizontal, vertical }` |
| 元素 / 数组属性 | `<Tooltip text="提示" child={<Button>保存</Button>} />`、`<Select options={["A", "B"]} />`、`<Tabs items={[{ label: "A", content: <Text>A</Text> }]} />` |
| 图像 / 快捷键 | `<Image image={image} />` 接受 `Image`（原生模块或 `assetImage`）；`HotkeyInput.value` 为快捷键字符串，`""` 表示无；`onRecord(string)` 可接给 `useHotkey` |

### 数组、对象与函数

```tsx
import { useState, View, Button, Text } from "zant";
interface Todo { id: number; title: string; done: boolean; }
export default function App() {
  const [todos, setTodos] = useState<Todo[]>([{ id: 1, title: "写文档", done: false }]);
  return <View>{todos.filter(t => !t.done).map(t => <View key={t.id}>
    <Text>{t.title}</Text><Button onClick={() => setTodos(todos.filter(x => x.id !== t.id))}>删除</Button>
  </View>)}</View>;
}
```

- 数组支持字面量、`[...xs, item]`、`.length`、`a[i]`、`map/filter/find/findIndex/some/every/reduce/indexOf/includes/slice/concat`、字符串数组 `join`；状态用展开 / 过滤 / 替换更新，不支持 `push/splice`。
- 对象支持字面量、`{ ...todo, done: true }`、成员、`?.`、`??`；状态可存数组 / 对象，字符串与集合可跨帧保存并廉价克隆。
- 可选值支持条件收窄：`peer ? peer.name : "无"`、`peer !== null && peer.online`、`!peer || peer.files.length === 0`、提前 `if (!entry) return …`；对象非 null 即真，字符串还须非空、数字非 0、布尔为 true。分支内箭头捕获非空值；可变 `let` 不收窄，用 `!` 或改 `const`。
- 普通函数小写开头，参数须标类型，有返回值须写返回类型；支持递归、导出、当事件回调，可读写 store，不能用 hook。参数可设常量默认值（注解或默认值推断类型）。大写开头才是组件。
- `Uint8Array` 用于静态字节（资源导入、纯整数字面量数组、Rust 的 `&'static [u8]`）；剪贴板图片为运行时字节，当前不能传给 `setImage/assetImage` 或赋给 `Uint8Array` 注解。

```tsx
// 长列表片段，todos 为上例数组；须从 zant 导入 List：
<List data={todos} keyExtractor={t => t.id} estimatedItemHeight={32}
  renderItem={t => <Text>{t.title}</Text>} height={400} />
```

List 只构建可见行；itemHeight/followTail/gap/padding 控制布局，scrollToIndex 变化时滚到该行（scrollAlign 选 start/center/end）；行变化后更新数字 layoutKey 可重新测量并再次滚到目标。

### 内置对象

示例：`Number("1.25").toFixed(1)` → `"1.3"`，`Math.round(-2.5)` → `-2`，`String(true)` → `"true"`。

| API | 行为 / 限制 |
|---|---|
| `Math` | floor/ceil/round/trunc/abs/sign/sqrt/cbrt/pow/exp/log/log2/log10/sin/cos/tan/atan/atan2/hypot/min/max/random，常量 PI/E |
| Math 边界 | round 半数向正无穷；min/max 零参数为正/负 Infinity，hypot()=0；random 为一次播种的非密码学随机数 `[0,1)` |
| 整数 Math | floor/ceil/round/trunc/abs 保留整数，abs 最小负数可溢出；min/max 按混合整数规则，其余返回 number |
| `Number(x)` | 数值 / 布尔 / 字符串；去 JS 空白，空串为 0，完整匹配十进制、Infinity、无符号 0x/0b/0o，否则 NaN |
| `Number` | isNaN/isFinite/isInteger 不隐式转换；MAX_SAFE_INTEGER、EPSILON |
| `String(x)` / 数值 `.toString()` | 数值 / 布尔 / 字符串；最短可往返格式，整数无 .0、负零为 0，绝对值 <1e-6 或 ≥1e21 用科学计数；显式整数文本精确；不支持 toString(radix) |
| `.toFixed(digits?)` | 默认 0，截断后 0–20 位，否则 panic；按实际二进制值舍入，1.25→1.3（1 位）、0.995→0.99（2 位）；NaN/Infinity/绝对值 ≥1e21 沿用 String |
| `parseInt(s, radix?)` | 最长整数前缀，基数 2–36，默认十进制，默认 / 16 支持 0x；无数字为 NaN |
| `parseFloat(s)` | 最长十进制前缀（含指数、有符号 Infinity），忽略尾部 |
| 时钟 | `Date.now()` 为 Unix 墙钟毫秒；`performance.now()` 为含小数的单调毫秒，独立于 UI 帧时钟；不支持完整 Date 对象 |

参数按源码顺序各求值一次；可选数值先用 `??` / `!` 解包。文件常量可用纯数值运算 / 内置常量，不能用数值转字符串、随机数或时钟。

### 嵌套规则

```tsx
// 组件体片段，标签均从 zant 导入：
return <View><Text>读 <Text weight="bold">这里</Text><Link onClick={() => console.log("文档")}>文档</Link></Text>
  <Button><Icon name="check" />保存</Button></View>;
```

| 标签 | 子节点 |
|---|---|
| View/Row/Column/Card/Clickable/Space | 元素、条件表达式、片段；文字须包 Text |
| Text | 文字、值、条件文字、嵌套 Text / 行内 Link；不能放块元素 |
| Heading/Title/Paragraph | 文字与值；局部样式改用 Text |
| Button/Link/Checkbox/Switch/Tag | 文字与值；Button 另可放一个 Icon |
| Tooltip/Modal | 恰好一个元素，或 `child` / `content` 属性 |
| Markdown | Markdown 文字与值，或 source；不能放元素 |
| Input/Image/Icon/Divider/PdfPreview/PdfViewer 等叶子 | 无 |

- 富文本样式继承 size/weight/color/family，span 另可用 italic/underline/strike/background；行内 Link 只接 onClick、取主题主色。带 span 的 Text 不支持 maxLines/textAlign/wrap。
- 点击只触发最内层可点击元素，不冒泡；Modal/Message 自动放到窗口根节点，覆盖窗口。
- List/PdfViewer 须放在高度有限的普通容器，不能嵌入滚动容器或另一个 List 行。
- 同层字面量 key 不可重复；`.map` / List 行里的有状态组件（含 Markdown/PdfViewer）必须给稳定 key。实例由父组件 + 组件名 + key（无 key 时为位置）决定；子组件 key 覆盖其根元素 key。
- 自定义 children：ReactNode / JSX.Element 为单元素，ReactNode[] / JSX.Element[] 为元素集合，string 为文字和值；声明也可用 `JSX.Element | JSX.Element[]`。

### 性能与示例

- 帧对比：`scripts/tsx/bench-tsx.sh`，生成代码相对手写 Rust 的帧耗时须 ≤ **1.2×**。
- 内存 soak：`ZANT_BENCH_SOAK=1 ./target/release/zant-tsx-frame-bench`（也可参数 `soak`），比较 RSS、存活节点、local-state 槽、文字布局及帧耗时；可按 todos-churn/local-churn/list-churn/counter-churn 筛选。`leak` 是回归信号，非对象泄漏证明。
- 编译基准：`zant-tsx-bench`；示例在 `crates/zant-examples/tsx/`（counter/demo/bench/gallery/todos/reader/notes），`scripts/tsx/examples-check.sh` 检查构建、截图与布局。

## 组件状态与 hooks

```tsx
import { useState, View, Button } from "zant";
function Counter({ label }: { label: string }) {
  const [count, setCount] = useState(0);
  return <Button onClick={() => setCount(c => c + 1)}>{label}: {count}</Button>;
}
export default function App() { return <View><Counter label="A" /><Counter label="B" /></View>; }
```

每个实例独立保存状态；卸载释放，再挂载从初值开始。稳定 key 让状态跟随列表项；List 滚动状态、文档组件及其他 hooks 也跟随实例。

### `useRef`、`useMemo`、`useEffect`

```tsx
import { useState, useRef, useMemo, useEffect, Button } from "zant";
export default function App() {
  const [count, setCount] = useState(0);
  const clicks = useRef(0);
  const doubled = useMemo(() => count * 2, [count]);
  useEffect(() => { console.log(count); return () => console.log("清理"); }, [count]);
  return <Button onClick={() => { clicks.current += 1; setCount(c => c + 1); }}>{doubled}</Button>;
}
```

| hook | 行为 / 限制 |
|---|---|
| `useRef<T>(初值)` | 只经 `.current` 读写；读最新值，事件 / effect 中写，不触发重建；初值支持数字、字符串、布尔、null、数组、对象字面量 |
| `useMemo(() => 值, [依赖])` | 依赖按内容比较（含字符串 / 数组 / 对象），未变不计算；比较 O(依赖大小)，结果独立保存；不能缓存元素 |
| `useEffect(() => { … }, deps?)` | 协调后运行；[] 仅挂载，有依赖仅变化后，省略则每次构建；setter / 组件函数 / 回调 prop 会请求再构建，只写 ref 不会 |
| effect 清理 | `return () => …` 在下次运行前 / 卸载时执行；可读状态、ref、effect 局部值，不能直接读 props，先 `const name = label` 再捕获 |

三个 hook 只在组件体顶层调用，不能改名导入；子组件使用后成为有状态实例。无依赖 effect 每次调 setter 可能持续重建。

### 自定义 hook（`useXxx`）

```tsx
// 文件片段，useState、Button 从 zant 导入：
function useCounter(start: number) {
  const [count, setCount] = useState(start);
  return { count, increment: () => setCount(c => c + 1) };
}
// 在组件顶层调用：
const { count, increment } = useCounter(0);
return <Button onClick={increment}>{count}</Button>;
```

- `use` 开头，可导出并跨文件按名导入，可调用其他 hook；只在组件 / hook 顶层调用，末尾只 return 一次。
- 支持 `useX()`、`const v = useX()`、数组解构、对象解构、结果成员访问；返回数组 / 对象时用字面量。
- 参数可为普通值或回调；名字 / 字面量代入，其他表达式先求值一次。每次调用独立保存状态，名字自动隔离，依赖导入自动转到调用方；错误指向 hook 原文件。
- 应用 hooks、托盘、快捷键也可封装，使用范围仍受调用组件限制。

### Markdown 与 PDF

```tsx
// 组件体片段，View、Markdown、PdfViewer 从 zant 导入：
return <View><Markdown streaming caret="block"># 回复</Markdown>
  <PdfViewer src="" toolbar annotationTools height={400} /></View>;
```

| 组件 | 属性与行为 |
|---|---|
| Markdown | source 或文字子节点二选一；streaming 配合默认开启的 parseIncomplete 修复未完成语法，复用块缓存；caret=none/block/circle（只在 streaming 显示） |
| Markdown 选项 | fontSize/contentWidth/lineNumbers/codeMaxHeight、codeCopy/codeDownload、tableCopy/tableDownload、mermaidCopy/mermaidDownload、onLink/baseDir/selectable/singleDollarMath/latexDelimiters |
| PdfViewer | src 必填，空串显示提示，无子节点；src/password 改变重新打开，否则保留滚动 / 缩放 / 选择 / 搜索 / 批注；失败不逐帧重试，页面在后台分块渲染 |
| PDF 选项 | toolbar/annotationTools/selectionMenu/pageGap/padding/background、onLink/onChange/onAnnotation、width/height/grow（指定 height 默认不伸长）、emptyText/loadingText/errorText |

组件随主题切换；实例缓存卸载即释放，多文档互不影响，列表中给稳定 key。复杂 Markdown 扩展 / PDF labels 可从 Rust 传入；见 [Markdown](components/markdown.md)、[PDF](components/pdf.md)。

## 应用 hooks

```tsx
// 根组件体片段，hooks / Text / KeyEvent 从 zant 导入：
const [ask, setAsk] = useState(false);
useCloseRequest(() => { setAsk(true); return false; });
useQuitRequest(() => true);
useKeyDown((e: KeyEvent) => { if (e.key === "Escape") { setAsk(false); return true; } return false; });
return <Text>{ask ? "请先保存" : "就绪"}</Text>;
```

| hook | 范围 / 回调 |
|---|---|
| useCloseRequest | 根 / 无 props 窗口组件；关闭按钮、⌘W，返回 false 阻止关闭 |
| useQuitRequest | 根组件；⌘Q / Dock 退出，省略时询问 useCloseRequest |
| useKeyDown | 根 / 无 props 窗口组件；先于焦点控件，true 表示已处理 |
| useWindowFocus | 仅窗口组件，收到 focused 布尔值 |
| useClipboardChange / useReopen | 仅根组件；剪贴板变化 / 再次打开应用 |
| useWake | 仅根组件；后台调用 `zant::ui::post_wake()` 后在 UI 线程取结果，再重建所有窗口 |
| useAppExit | 仅根组件；退出或主窗口关闭时保存数据、停止线程，回调返回后进程结束 |

`KeyEvent.key` 同 DOM（Escape/Enter/Tab/ArrowUp/F1 或字符）；`mod` 是 ⌘ / Ctrl，另有 shift/alt/ctrl/meta/repeat。应用 hook 在事件到达时读当前状态，可用 setter/store，不能捕获 render 局部变量。

### 托盘、全局快捷键、剪贴板

```tsx
import { useTray, useHotkey, useClipboardChange, useReopen, mainWindow } from "zant";
import trayIcon from "../assets/icons/statusbar.png";
// 根组件体片段：
const shown = useTray({ icon: trayIcon, template: true, tooltip: "我的应用",
  items: [{ title: "打开", onSelect: () => mainWindow.activate() }, { separator: true }],
  onClick: () => mainWindow.activate() });
const ok = useHotkey("ctrl+cmd+v", () => mainWindow.activate());
useClipboardChange(() => console.log("剪贴板变化"));
useReopen(() => mainWindow.activate());
```

- 一个应用一个托盘，图标为 PNG 静态字节；菜单项另有 checked/disabled。Windows 左键触发 onClick，右键显示菜单。
- 托盘 / 快捷键在渲染时应用，内容变化才调用系统接口；回调可捕获 render 局部变量。快捷键被占用时返回 false。

### 桌面函数（从 `"zant"` 导入）

```tsx
import { clipboard, app, mainWindow, keyLabel, titlebarInsets } from "zant";
// 组件体片段，View、Button 从 zant 导入：
const insets = titlebarInsets();
return <View paddingLeft={insets.left} paddingRight={insets.right}>
  <Button onClick={() => clipboard.setText(app.deviceName())}>{keyLabel("mod+c")}</Button>
  <Button onClick={() => mainWindow.hide()}>隐藏</Button>
</View>;
```

| API | 返回 / 用途 |
|---|---|
| clipboard.text/setText | string（无文字为 ""）/ boolean |
| clipboard.image/setImage | PNG 运行时字节或 null / boolean（当前只接受静态字节） |
| clipboard.files/setFiles | string 或 null（绝对路径，一行一个）/ boolean |
| clipboard.changeCount | 变化计数，含自己写入 |
| app.launchAtLogin/setLaunchAtLogin | off/on/needsApproval / boolean；needsApproval 表示已登记但要用户在系统设置里允许（macOS 登录项、Windows 任务管理器里被禁用）；设置后请重新读取 |
| app.canLaunchAtLogin | 这个构建能否设置开机启动：打包的 .app（macOS 13+）、Windows release 构建；`zant run`（含 `--release` 热刷新）为 false |
| app.openLoginItemSettings | 打开系统的启动项设置（macOS 登录项、Windows 设置 › 应用 › 启动） |
| app.launchedAtLogin/deviceName/downloadsDir/dataDir | boolean / string / string 或 null / string 或 null（按 bundle_id） |
| app.quit | 直接退出，不再询问关闭 / 退出 hook |
| mainWindow.show/hide/activate/focus(key) | 显示 / 隐藏 / 激活 / 聚焦带 key 控件 |
| mainWindow.visible/active/minimized | boolean |
| keyLabel/shortcutLabel | 平台快捷键文字；keyLabel 参数须字面量（mod+f → ⌘F / Ctrl+F），shortcutLabel 无效时为 "" |
| titlebarInsets / isMacos / isWindows | 渲染时读 `{ left, right }` / 平台布尔常量 |
| wakeAfter(ms) | 渲染时安排一次重建，如提示到期；可配 performance.now() |

这些名字须导入，可被局部变量遮蔽。修改剪贴板 / 窗口 / 应用的调用限事件与 effect，读取不限；返回字符串存入 state 时复制。`app.config.json` 的 `"single_instance": true` 让第二次启动激活已有实例。

### 定时器：`setTimeout` / `clearTimeout`

```tsx
// 根组件体片段，useState/useEffect/Text 从 zant 导入：
const [ready, setReady] = useState(false);
useEffect(() => {
  const timer = setTimeout(() => setReady(true), 300);
  return () => clearTimeout(timer);
}, []);
return <Text>{ready ? "就绪" : "等待"}</Text>;
```

- 全局函数，无需 import；返回 number 编号，可 clearTimeout 取消。仅根 / 无 props 窗口组件的事件或 effect 可设置，子组件用回调交给上层；render 用 wakeAfter。
- UI 线程执行，随后重建所有窗口；回调只能捕获数字 / 布尔 / 枚举，字符串 / 数组 / 对象放 state 后运行时读。state 读的是执行时当前值。

### 多窗口：`createWindow`

```tsx
import { createWindow, useWindowFocus, View, Text, Button } from "zant";
export function Panel() {
  useWindowFocus((focused: boolean) => { if (!focused) panel.hide(); });
  return <Text>剪贴板</Text>;
}
export const panel = createWindow(Panel, { title: "剪贴板", style: "panel", width: 380, height: 480 });
export default function App() { return <View><Button onClick={() => panel.toggle()}>切换面板</Button></View>; }
```

- 首次使用时创建，初始隐藏，之后窗口与状态保留；任一窗口动作重建所有窗口，共享数据用 store。窗口组件的关闭 / 按键 / 焦点 hook 作用于该窗口。
- createWindow 接受无 props 的窗口组件；选项：title、style（native/hidden-titlebar/borderless/panel）、width/height/minWidth/minHeight、background、resizable、alwaysOnTop、movableByBackground、center、titlebarHeight。
- 句柄：show/hide/toggle/activate/visible/active/minimized/focus；`place("center"|"topLeft"|"topRight"|"bottomLeft"|"bottomRight"|"trayCorner", margin?)` 定位，trayCorner 靠托盘。详见[桌面集成](desktop.md)。

### 资源文件

示例：文件顶层 `import logo from "../assets/logo.png";`，组件中 `<Image image={assetImage(logo)} />`；Image / assetImage 从 zant 导入。

- 非 .tsx/.ts/.rs 的相对导入视为资源，仅默认导入；路径相对当前文件，可在 src 外，编译期嵌入为 Uint8Array，缺文件在 import 处报错。
- 仅项目模式（tsx_app 或 zant-tsx --out）可用；导入资源变化触发重建。assetImage 返回 Image 或 null，首次解码后缓存；字节也可传原生模块。

### 原生模块：`native/`

示例：顶层 `import { copy } from "native/clips";`，事件中 `copy("已复制");`（规范声明 `copy(text: string)`）。

`native/clips.ts` 用 `export declare function` 与类型写规范，`native/clips.rs` 实现；构建器自动登记，trait 检查一致性。完整示例、类型和后台任务见 [tsx-native.md](tsx-native.md)。

## 独立编译器的 Rust 导入

示例：顶层 `import { plural } from "./format.rs";`，组件中 `const label = plural(2, "note");`；Rust 签名为 `pub fn plural(n: u32, unit: &str) -> String`。

- 兼容能力：`zant-tsx --out out App.tsx format.rs` 读取顶层 `pub fn` 签名，支持按名 / as 别名导入并检查参数；单文件模式不支持。应用使用 native 规范，tsx_app 不允许 src 混放 Rust。
- 支持 bool、String/zant::Str/&str、数值、`&'static [u8]`、`zant::image::Image`、同文件 pub 结构体 / 无数据枚举、Option/Vec（含字符串数组），返回值另支持 ()。
- 不支持其他指针、泛型参数、impl Trait、Result、trait 方法。编译器不复制或自动声明 Rust 模块，最终 crate 须自行声明；同名 .tsx/.rs 输出冲突报错。I/O、环境变量、长期资源用 native 模块，勿返回裸指针。

类型示例：`import { peers, type Peer } from "./sync.rs";` 后可写 `const online: Peer[] = peers().filter(p => p.online);`（Rust 为 `pub fn peers() -> Vec<Peer>`，Peer 有 `pub online: bool`）。

结构体需 Clone，枚举需 Copy/Clone；pub 字段保留原名，枚举为保留变体拼写的字符串联合（Kind::Laptop → "Laptop"），支持嵌套 / Option / Vec。不支持的字段不暴露，此类结构体勿在 TSX 新建；返回字符串和切片拥有存储，可跨帧保存。

## 共享状态：`createStore` / `useStore`

```tsx
import { createStore, useStore, Switch } from "zant";
interface Settings { dark: boolean; }
export const settings = createStore<Settings>({ dark: false });
export default function App() {
  const dark = useStore(settings, s => s.dark);
  return <Switch checked={dark} onChange={on => settings.update({ dark: on })} />;
}
```

- createStore<T>(初值) 放文件顶层，类型必填，可跨文件导入；跨组件 / 窗口共用一份，只在 UI 线程修改。
- useStore(store, selector?) 在组件顶层读渲染快照；store.get() 随时读当前值，事件 / effect 中 update({字段}) 浅合并或 set(新值) 替换，修改重建所有窗口。
- 值可复制或廉价克隆；另一窗口尚未重建时，其事件可能读到旧快照，需要最新值用 get()。

## 字符串方法

```tsx
// 组件体片段，View/Text 从 zant 导入：
const notes: string[] = ["周报", "计划"];
const query = " 周报 ".trim().toLowerCase();
return <View>{notes.filter(n => n.toLowerCase().includes(query)).map((n, i) => <Text key={i}>{n}</Text>)}</View>;
```

| 方法 | 行为 / 限制 |
|---|---|
| length、includes/startsWith/endsWith/indexOf/lastIndexOf | 长度 / 位置按 UTF-16 码元；查询不分配，索引为 isize，未找到 -1；不支持额外起止位置 |
| trim/trimStart/trimEnd | JS 空白，含 Unicode Zs、BOM、行分隔符 |
| toLowerCase/toUpperCase | 只转换 ASCII |
| slice(start,end?) / substring(start,end?) | 数值向零取整并钳位；slice 支持负索引，substring 负数钳零、倒置交换 |
| padStart/padEnd(n,fill?)、repeat(n) | 默认空格；repeat 截断后负数 / 无穷或无法分配时 panic，无 JS 异常 |
| replace/replaceAll(a,b) | 首个 / 全部字符串匹配；替换支持 $$、$&、$`、$'，无捕获组的 $1 等保留原文；不支持正则 |
| split(sep) | 保留空字段；空分隔符按 Unicode 码点分割 |
| charAt(i) / at(i) | 越界为 "" / null；at 支持负索引，返回 string 或 null，可用 ?? / ! |
| charCodeAt(i) / codePointAt(i) | UTF-16 码元（越界 NaN）/ 从该位置起的码点（越界 null） |

- 字符串为 UTF-8，新字符串 / 数组拥有存储，可跨帧保存；代理对内部位置向前取 UTF-8 码点边界：`"😀".slice(0,1)` / charAt(0) 为空，slice(1,2) / charAt(1) 为完整 emoji；填充可能少一个码元，`"😀".replaceAll("", "-")` 为 `"--😀-"`。
- 可选接收者先用 ?? / ! 解包；不支持 `name?.trim()`、locale/normalize 等方法，未知方法会列出支持项。

## 诊断速查

- 编译错误定位 `.tsx` 行列；检查子集外语法、缺必填属性、非法嵌套、重复 / 缺 key、同名输出冲突、找不到资源、渲染时调用 setter / 桌面写操作、hook 位置 / 范围。自定义 hook 错误定位原文件。
- `ZANT_INPUT_TRACE=1 zant run`：指针处理与帧开始时间，macOS 另含呈现时间；基准中勿开启。
- `ZANT_THEME=light|dark`、`ZANT_RENDERER=raster`、`ZANT_SNAPSHOT=<PNG>`、`ZANT_SNAPSHOT_EXIT=1`、`ZANT_LAYOUT_FILE=<文件>`：主题 / 无窗口截图 / 带 key 布局；`ZANT_NO_MOTION=1` 关闭动效，`ZANT_FRAME_STATS=1` 记录帧耗时，详见[环境变量](development.md#环境变量)。
- 热刷新诊断：`ZANT_HOT_TRACE=1`、`ZANT_HOT_SNAPSHOT=<目录>`、`ZANT_HOT_POLL=1`、`ZANT_HOT_CONTROL=<FIFO>`；用法与限制见[开发指南](development.md#热刷新验证)。
