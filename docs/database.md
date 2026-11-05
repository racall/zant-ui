# SQLite 持久化

`zant.db` 是 Zant UI 内置的轻量 SQLite 封装，使用系统 `libsqlite3`，无需额外安装。

## 打开数据库

```zig
var db = try zant.db.Database.open(allocator, "app.sqlite");
defer db.close();

// 测试/临时数据
var memory = try zant.db.Database.openMemory(allocator);
defer memory.close();
```

## 建表与写入

```zig
try db.exec(
    \\CREATE TABLE IF NOT EXISTS notes (
    \\  id INTEGER PRIMARY KEY AUTOINCREMENT,
    \\  body TEXT NOT NULL,
    \\  score REAL DEFAULT 0
    \\)
);

try db.run(
    "INSERT INTO notes (body, score) VALUES (?, ?)",
    .{ "hello", 1.5 },
);
const id = db.lastInsertId();
```

参数支持：`null`、整数、浮点、`bool`、字符串切片、字符串字面量、可选值。
SQL 必须是 sentinel 字符串（普通字符串字面量即可）。

## 查询

```zig
var rows = try db.query("SELECT id, body, score FROM notes ORDER BY id", .{});
defer rows.deinit();

while (try rows.next()) |row| {
    const id = row.int(0);
    const body = row.text(1);
    const score = row.float(2);
    // row.value(0) 返回 Value union
}
```

`row.isNull(index)` 判断 NULL；`row.blob(index)` 读取二进制。

## 单值查询

```zig
const count = (try db.queryOne(i64, "SELECT COUNT(*) FROM notes", .{})).?;
```

支持 `i64`、`f64`、`[]const u8`。

## 事务

```zig
try db.begin();
errdefer db.rollback() catch {};
try db.run("INSERT ...", .{...});
try db.commit();
```

或使用自动回滚的封装：

```zig
fn save(d: *zant.db.Database) !void {
    try d.run("INSERT ...", .{...});
}

try db.transaction(save, .{});
```

## 设计原则

- 不隐藏 SQL；复杂查询仍可直接写 SQLite SQL。
- 默认使用 prepared statement，参数绑定避免注入。
- 没有 ORM、迁移框架或连接池；需要时可在上层自行封装。
- 数据库句柄和语句都是显式生命周期，便于测试和资源控制。
