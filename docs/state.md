# Zustand 风格状态

推荐用 TSX 编写应用。组件自己的状态用 `useState`，父组件通过 props 传只读值、回调接收事件；
跨组件、跨窗口的共享状态用 `createStore` / `useStore`，底层是 `zant::store::Store<T>`。

## TSX 用法

```tsx
import { createStore, useStore, Column, Text, Button } from "zant";

interface Counter {
  count: number;
  enabled: boolean;
}

// 文件顶层创建；可以 export 后由其他组件 import。
const counter = createStore<Counter>({ count: 0, enabled: true });

export default function App() {
  const count = useStore(counter, state => state.count);
  return (
    <Column>
      <Text>计数：{count}</Text>
      <Button onClick={() => counter.update({ count: counter.get().count + 1 })}>增加</Button>
    </Column>
  );
}
```

`useStore(store)` 读取整份状态，`useStore(store, selector)` 读取选择结果；放在组件顶层。
`get()` 取得当前值的快照，`set(value)` 整体替换，`update({ field: value })` 浅合并列出的字段；
`update` 参数须为对象字面量，不接收函数，也不支持展开。写操作放在事件或 effect 中，不能在 render 中执行。
界面动作完成后重建所有窗口；selector 用于选择值，没有按 selector 相等性跳过重建的订阅优化。

事件处理函数读到的组件状态是本帧快照；Store 需要最新值时在回调中用 `store.get()`。
`useState` 则可用 `setCount(previous => previous + 1)`。

## Rust 用法

`zant::store::Store<T>` 是 UI 线程容器，API 接近 Zustand。
下文用于原生模块或手写 Rust 应用，注意 Rust 的 `update` 接收闭包，与 TSX 的对象补丁不同。

```rust
use zant::store::Store;

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
let store = zant::store::create(State::default());
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
        zant::ui::column(zant::ui::BoxOptions {
            children: zant::nodes![zant::ui::text(format!("count = {count}"), Default::default())],
            ..Default::default()
        })
    }
}
```

需要在动作之外修改状态时，可在 UI 线程修改后用 `zant::desktop::invalidate_all()` 请求所有窗口重建，
或订阅 Store 的通知：

```rust
store.borrow_mut().subscribe(zant::desktop::invalidate_all);
```

`ui::request_rebuild()` 用于 effect 中请求下一次构建，不是通用的窗口唤醒接口。
后台线程把结果交回 UI 线程时用 `ui::post_wake()`，在 `on_wake` / `useWake` 里更新状态；之后所有窗口会重建。
`render` 直接读 Store 字段，保持状态只有一份。

Store 不是线程安全的（`Rc` + `RefCell`，`!Send`），只能在 UI 线程修改。Rust Store 没有 selector、取消订阅、middleware、
持久化或 devtools；需要持久化时用 `zant::db`（见 [SQLite 持久化](database.md)）。

`zant run` 热刷新只自动恢复兼容的组件状态等快照，全局 Store 不自动保存；需要跨启动保留的数据应写入数据库。
