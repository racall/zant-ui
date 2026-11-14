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

### 由 WP-1.2 实现后定稿的细节

1. **`Element` 的 `Default` 手写**（不是 derive）：Zig 的非零默认很多（`enabled = true`、`focus_ring = true`、
   `icon_size = 18`、`button_size = Middle`、`toggle_kind = Checkbox`、`image_fit = Cover`、
   `rich_line_height = 2.0`），`kind` 取 `NodeKind::Root`。
2. **选项结构体的 `Default`**：Zig 默认全为零/枚举默认的用 `derive`（手写会被 `clippy::derivable_impls`
   判错），其余逐字段手写。Zig 里必填的字段给中性 `Default` 以便 `..Default::default()`。
3. **`run_effects(cx: &mut ActionCx<'_>) -> bool`**：Rust 的 `Action` 需要 `ActionCx`；先取走队列、
   释放 context 借用，再逐个调用（与 Zig 语义一致）。
4. **raw identifier 保住 Zig 字段名**：`PointerAreaOptions.r#box`、`ClickableOptions.r#box`、
   `IconButtonOptions.r#box`、`AlertOptions.r#type`。`#[derive(TsxProps)]`（WP-5.2）反射字段名时要剥掉 `r#`。
5. **`Element.image_handle: Option<Rc<Image>>`**（Zig 为 `?*anyopaque` + `image_id`，后者保留）；
   `content/placeholder/tooltip/link/options` → `Str`/`Vec<Str>`；`children` → `Vec<Node>`；
   `drawing` → `Option<Rc<VectorDrawing>>`；`memo_build` → `Option<MemoBuild>`（`Rc<dyn Fn() -> Node>`）。
6. **新增小 API**：`Node::ptr_eq`（reconciler 身份比较）、`PointerInfo::default()`（`click_count = 1`）、
   `Node::with_key`、`append_children`。

### 决议（原契约未写清的地方）

7. **`zant::Error`**：`crates/zant/src/error.rs`，`enum Error { Io, Utf8, Message }` + `pub type Result<T>`。
   各模块自己的错误类型（`ParseError`、`DbError`、`MhchemError`、`RendererError`、`OpenError`…）保留，
   以后统一加 `From` 并入；新代码有真实失败时优先用 `zant::Error`。
8. **`ui::post_wake()`**：`platform::platform::post_wake()` 通过 `AtomicPtr<ZantWindow>` 保存唤醒目标
   （原生 `zant_window_post_wake` 文档保证线程安全），`Window::create` 默认把第一个窗口设为目标，
   runtime 多窗口时用 `platform::set_post_wake_window(raw)` 切换。`ui` 里不出现 unsafe。
9. **`ui::key_bounds(key)`**：`ui/context.rs` 里的 `KeyBoundsHook`（`Rc<dyn Fn(&str) -> Option<Rect>>`），
   runtime 每帧用 `set_key_bounds_hook` 装上「查保留树」的闭包；`ui` 不直接依赖 `core::tree`。
10. **不提供 `frameAllocator`**：Zig 的帧 arena 在 Rust 里删除。需要「活过本帧」的数据一律由闭包捕获
    `Rc`/`String`（回调生命周期即保证），字符串用 `Str::copy`。移植 pdf/markdown/tsx/components 时按此执行。
11. **`App: std::any::Any`**（WP-1.4 定稿）：`ActionCx::state::<S>()` 需要从 boxed app 里 downcast；
    对 `impl App for X` 的写法没有影响。
12. **`ui::key_bounds` 的实现**（WP-1.4 定稿）：runtime 每次 layout 后刷新一个
    `Rc<RefCell<HashMap<String, Rect>>>` 快照并安装为 hook（不是每帧查活树——那会在 build/effects 里
    重借 `RefCell<Tree>` panic）。语义仍是「上一帧的盒子」。
13. **`mut`/所有权形态**：`Collection` 式容器方法取 `&self` + 内部可变（`Rc<RefCell<..>>`）或 `&mut self`，
    按各 WP 的文件头说明；读路径优先返回 `Arc` 快照（PDF 缓存已如此）。
