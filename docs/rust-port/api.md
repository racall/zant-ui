# 核心 API 契约（api）

本文件是 [plan.md](plan.md) §4 的展开。**WP-1.x 实现，之后冻结**；所有后续 WP 都依赖这一节。
实现时发现行不通，登记 `status.md` 的「待决问题」，由负责人修订后统一通知。

## 字符串 `zant::Str`

廉价克隆的不可变字符串：

```rust
pub enum Str { Static(&'static str), Shared(Rc<str>) }
```

实现 `Deref<Target = str>`、`From<&'static str>`、`From<String>`、`Clone`、`PartialEq`、`Eq`、
`Hash`、`Default`。非静态 `&str` 用 `Str::copy(s)`。

## 声明节点

对应 `src/ui.zig:14-87`：

```rust
#[derive(Clone)]
pub struct Node(Rc<Element>);

#[derive(Clone, Default)]
pub struct Element {
    pub kind: NodeKind,
    pub key: Option<Str>,
    pub children: Vec<Node>,
    // 其余约 55 个字段与 Zig 的 Element 同名同序
}
```

节点自己拥有数据，没有帧 arena，也没有生命周期参数。`ui::make()` 的深拷贝逻辑不需要移植。

## 选项结构体

`BoxOptions`、`TextOptions`、`ButtonOptions`、`InputOptions` … 字段与 Zig 同名。
全部加 `#[derive(Clone, zant_macros::TsxProps)]` 并**手写 `impl Default`，默认值逐字段对照 Zig**
（Zig 里非零默认值很多，不能直接 derive）。

调用形态：

```rust
ui::button(ui::ButtonOptions {
    text: "Save".into(),
    variant: ui::ButtonVariant::Primary,
    ..Default::default()
})
```

## 子节点列表

宏 `nodes![a, b, maybe, list]`，元素可以是 `Node`、`Option<Node>`、`Vec<Node>`，
展平成 `Vec<Node>`（trait `IntoNodes`）。

## 回调

对应 `src/core/actions.zig`：

```rust
#[derive(Clone)]
pub struct Action(Rc<dyn Fn(&mut ActionCx)>);
// 同样形状：BoolAction(bool)、TextAction(&str)、IndexAction(usize)、RangeAction、
// PointerAction（返回 bool）、KeyAction、InputKeyAction —— 与 actions.zig 一一对应

pub struct ActionCx<'a> { /* Runtime 分发事件时构造 */ }
impl ActionCx<'_> { pub fn state<S: 'static>(&mut self) -> &mut S; /* 类型不符则 panic */ }

pub fn action<S: 'static>(f: impl Fn(&mut S) + 'static) -> Action;
pub fn text_action<S: 'static>(f: impl Fn(&mut S, &str) + 'static) -> TextAction;
// bool_action / index_action / … 同理；Action::new(|cx| ..) 是不绑定根状态的通用构造
```

Runtime 调用回调的固定顺序：**先把 `Action` 从树节点里 clone 出来，释放对树的借用，
再构造 `ActionCx` 调用**。

## 应用入口

替代 `launch.zig` 的 `@hasDecl` 探测：

```rust
pub trait App: 'static {
    fn render(&mut self) -> Node;
    fn theme(&self) -> Option<Theme> { None }
    fn on_key(&mut self, _e: &KeyEvent) -> bool { false }
    fn on_close_request(&mut self) -> bool { true }
    // on_quit_request / on_start / on_wake / on_app_event / on_exit：默认空实现
}

pub fn run(app: impl App, opts: RunOptions) -> Result<(), Error>;
```

## 局部状态

对应 `ui.localState` / `core/local_state.zig`：

```rust
pub fn local_state<T: 'static>(id: u64, init: impl FnOnce() -> T) -> Rc<RefCell<T>>;
```

存储按 `id` + `TypeId` 保存 `Rc<dyn Any>`；同一 id 两种类型 → panic；本次 rebuild 没被请求的
槽位被清除（drop 即析构）。memo 跳过的子树要保活它上次用到的 id（照搬 `local_state.zig:87-102`）。
回调里直接捕获这个 `Rc`：`Action::new(move |_| slot.borrow_mut().count += 1)`。
`ui::child_id(parent, name_or_index) -> u64` 的哈希算法照搬（Wyhash，`local_state.zig:141-157`），
**结果必须与 Zig 相同**以便对照。

## 构建上下文

Zig `ui.zig:94-301` 的 18 个 threadlocal 合并为 `ui/context.rs` 里的一个
`thread_local! { static CX: RefCell<BuildCx> }`，对外仍是 `ui::current_theme()`、`ui::effect()`、
`ui::request_rebuild()` 这些自由函数。
**硬规则：调用用户代码（`render`、memo 的构建闭包、任何 Action）之前必须先释放 `CX` 的借用**，
否则重入会 panic。

## 保留树

对应 `core/tree.zig`、`core/node.zig`：

```rust
pub struct Tree {
    pub nodes: Vec<RetainedNode>,
    pub free_ids: Vec<NodeId>,
    pub key_index: HashMap<(NodeKey, Str), NodeId>,
    pub animating: Vec<NodeId>,
    // ..
}
pub type NodeId = u32; // 下标，槽位带 alive 标志并复用（墓碑机制照搬）
pub enum NodeData { Root, Container(..), Text(..), Button(..), /* .. */ }
```

**硬规则：不跨越树的修改持有 `&mut RetainedNode`；递归前先把子节点 id 列表 clone 出来**。
布局函数签名照 Zig，传 `&mut [RetainedNode]` + id。

## Skia 句柄

`TextLayout`、`Image`、`Renderer`、`Canvas` 包装不透明指针并实现 `Drop`；默认 `!Send`，
只有 `Image`（原本就线程安全创建）`unsafe impl Send + Sync`。

## 性能闸门

去掉 arena 后每帧会有更多堆分配。M2 末用移植后的 `zant-bench` 与 Zig ReleaseFast 基线对比，
要求各场景单帧耗时不超过 Zig 的 1.25 倍且远低于 16.7 ms；不达标再考虑 `SmallVec` / bump 分配，
届时由负责人决定。

## 已冻结的补充约定（执行中新增，写在这里供后续 WP 引用）

（暂无）
