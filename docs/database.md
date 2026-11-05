# 数据持久化

`zant.db` 的目标是让业务代码只保存和读取 Struct，不需要知道 SQLite、表、
连接、SQL 或结果集。

## 定义模型

```zig
const User = struct {
    id: i64 = 0,       // 自动生成
    name: []const u8,
    age: i32,
    active: bool,

    pub const table_name = "users"; // 可选
};
```

## 打开集合

```zig
var users = try zant.db.collection(User, allocator, "app.data");
defer users.close();
```

第一次打开会自动准备存储结构。不需要 `CREATE TABLE`，也不需要迁移脚本。

## 增删改查

```zig
const id = try users.add(.{
    .name = "Ada",
    .age = 36,
    .active = true,
});

const user = (try users.find(id)).?;
defer users.free(user);

var updated = user;
updated.age = 37;
try users.update(updated);

const all_users = try users.all();
defer users.freeAll(all_users);

try users.remove(id);

const count = try users.count();
```

## 开发者需要记住的 API

| 操作 | API |
|---|---|
| 写入 | `add(value)` |
| 按 id 读取 | `find(id)` |
| 全部读取 | `all()` |
| 修改 | `update(value)` |
| 删除 | `remove(id)` |
| 数量 | `count()` |
| 释放单条字符串 | `free(value)` |
| 释放列表 | `freeAll(list)` |
| 关闭 | `close()` |

不需要：

- 手写 SQL
- 管理 `sqlite3*`、连接、statement
- 建表
- 映射列索引
- 处理占位符

## 类型映射

| Zig | 存储 |
|---|---|
| `i8..i64`, `u8..u32` | 整数 |
| `f32/f64` | 浮点 |
| `bool` | 0/1 |
| `[]const u8` / `[]u8` | 文本 |
| `?T` | 可为空 |

字符串字段由库分配，`find/all` 返回后调用 `free/freeAll`。

## 事务

```zig
try users.begin();
errdefer users.rollback() catch {};
try users.add(...);
try users.add(...);
try users.commit();
```

当前 `Collection` 暂未暴露事务方法；先使用底层 `Database` 是高级用法。
日常单条 CRUD 不需要关心事务。

## 适合什么场景

适合：

- 应用设置、最近文件、收藏、草稿
- 本地列表数据
- 简单的增删改查

不适合：

- 复杂联表查询
- 多写者/连接池
- 复杂 schema 迁移

需要这些能力时再使用底层 SQLite API。
