# Zustand 风格状态

`zant.store.Store(T)` 是一个很小的 UI 线程状态容器，API 接近 Zustand。

```zig
const State = struct {
    count: u32 = 0,
    enabled: bool = false,
};

const AppStore = zant.store.Store(State);
const store = try AppStore.init(allocator, .{});
defer store.deinit();

store.update(.{ .count = 3 });   // 局部更新并通知
store.get().enabled = true;      // 直接修改
store.notify();                  // 直接修改后手动通知
```

## API

| 方法 | 作用 |
|---|---|
| `init(allocator, initial)` | 创建 Store |
| `deinit()` | 释放监听器和 Store |
| `get()` | 可变状态指针 |
| `getConst()` | 只读状态指针 |
| `update(.{...})` | 按字段浅合并后通知 |
| `mutate(fn)` | 调用 `fn(*T)` 后通知 |
| `set(next)` | 整体替换后通知 |
| `subscribe(ctx, fn)` | 注册变更监听 |
| `notify()` | 手动通知 |

## 与 Runtime 集成

```zig
const runtime = try Runtime.init(allocator, store, render, .{ ... });
try store.subscribe(runtime, Runtime.invalidateCallback);
```

`ui.*` 回调直接把 Store 作为 context：

```zig
ui.button(.{
    .text = "保存",
    .on_click = ui.action(store, save),
})
```

Store 不是线程安全的，只能在 UI 线程修改。没有 middleware、selector、
持久化或 devtools。
