# SQLite 持久化

`zant.db` 是内置的 SQLite 持久化库。日常使用 **不需要写 SQL**：定义 Struct，
CLI 会自动建表并生成增删改查。

## 定义模型

```zig
const User = struct {
    id: i64 = 0,           // 自动主键
    name: []const u8,      // TEXT
    age: i32,              // INTEGER
    active: bool,          // INTEGER (0/1)

    pub const table_name = "users";
};
```

字段类型自动映射：

| Zig | SQLite |
|---|---|
| `i8..i64` / `u8..u32` | INTEGER |
| `f32/f64` | REAL |
| `bool` | INTEGER |
| `[]const u8` / `[]u8` | TEXT |
| `?T` | 可为 NULL |

## 打开与建表

```zig
var db = try zant.db.Database.open(allocator, "app.sqlite");
defer db.close();

var users = db.table(User);
try users.create();   // CREATE TABLE IF NOT EXISTS + 主键/非空
```

`zant.db.Database.openMemory(allocator)` 用于测试。

## 增删改查

```zig
const id = try users.insert(.{
    .name = "Ada",
    .age = 36,
    .active = true,
});

const user = (try users.get(id)).?;
defer users.free(user);

var updated = user;
updated.age = 37;
try users.update(updated);

const count = try users.count();

const all_users = try users.all();
defer users.freeAll(all_users);

try users.delete(id);
```

- `insert` 返回自增主键。
- `get` 返回 `?User`，字符串字段由库分配，使用后调用 `free`。
- `all` 返回 `[]User`，使用后调用 `freeAll`。
- 不需要手写 INSERT/SELECT/UPDATE/DELETE。
- 所有值通过 prepared statement 参数绑定，避免 SQL 注入。

## 事务

```zig
try db.begin();
errdefer db.rollback() catch {};
try users.insert(...);
try users.insert(...);
try db.commit();
```

或自动回滚：

```zig
fn saveAll(d: *zant.db.Database) !void {
    var users = d.table(User);
    try users.insert(.{ .name = "A", .age = 1, .active = true });
    try users.insert(.{ .name = "B", .age = 2, .active = true });
}

try db.transaction(saveAll, .{});
```

## 设计原则

- 业务代码不写 SQL，不关心列名/占位符/结果集映射。
- 表名默认取 Struct 名，可用 `pub const table_name` 覆盖。
- 字段名就是列名；`id` 自动作为自增主键。
- 不提供 ORM 关系、迁移框架、连接池；复杂查询可以后续扩展 Query builder。
- 底层 `exec/query/prepare` 仍保留给高级用户和框架内部使用。
