# Zustand 风格状态

`zant::store::Store<T>` 是一个很小的 UI 线程状态容器，API 接近 Zustand。
TSX 应用用 `createStore` / `useStore`（编译成同一套 Store），本文件是手写 Rust 时的用法。

```rust
use zant::store::{Store, create};

#[derive(Default)]
struct State {
    count: u32,
    enabled: bool,
}

let mut store = Store::new(State::default());

store.update(|state| state.count = 3);   // 局部更新并通知
store.get_mut().enabled = true;          // 直接修改
store.notify();                          // 直接修改后手动通知
```

需要共享句柄时用 `create` 得到 `Rc<RefCell<Store<T>>>`（类型别名 `SharedStore<T>`）：

```rust
let store = create(State::default());
store.borrow_mut().update(|state| state.count = 3);
```

## API

| 方法 | 作用 |
|---|---|
| `Store::new(initial)` | 创建 Store |
| `create(initial)` | 创建共享 Store：`Rc<RefCell<Store<T>>>` |
| `get()` / `get_mut()` | 只读 / 可变状态 |
| `update(fn)` | 调用闭包修改后通知（对应 Zustand 的 `set(partial)`） |
| `set(next)` | 整体替换后通知 |
| `subscribe(fn)` | 注册变更监听（`FnMut()`） |
| `notify()` | 手动通知 |

## 与 Runtime 集成

最直接的用法是把 Store 放进应用状态，`render` 里读、动作里改；改完 Runtime 会重建：

```rust
struct App {
    store: zant::store::SharedStore<State>,
}

impl zant::ui::App for App {
    fn render(&mut self) -> zant::ui::Node {
        let count = self.store.borrow().get().count;
        zant::ui::text(format!("count = {count}"), Default::default())
    }
}
```

需要在动作之外改状态（后台线程的结果回到 UI 线程后）时，改完调用 `zant::ui::request_rebuild()`
（多窗口场景用 `zant::desktop::invalidate_all()`）：

```rust
store.borrow_mut().subscribe(move || zant::ui::request_rebuild());
```

更常见的做法是让 `render` 直接读 Store 的字段（状态只有一份）。
TSX 的 `createStore` 写在文件顶层，`useStore(store, selector?)` 在组件顶层读取；修改会让所有窗口重建。

Store 不是线程安全的（`Rc` + `RefCell`，`!Send`），只能在 UI 线程修改。没有 middleware、selector、
持久化或 devtools；需要持久化时用 `zant::db`（见 [SQLite 持久化](database.md)）。
