# 本地数据存储

`zant.db` 的目标是让业务代码感觉不到 SQLite：一个 Store，存键值、JSON 和
对象集合，不写 SQL，不管理表、连接、statement 或结果集。

## 打开

```zig
var store = try zant.db.open(allocator, "app.data");
defer store.close();
```

`path` 是 UTF-8、以 0 结尾的路径（`[:0]const u8`），相对路径按当前目录解析；`":memory:"` 可用于测试。
正式应用把文件放在应用数据目录（macOS `~/Library/Application Support/<id>`，Windows `%APPDATA%\<id>`）：

```zig
const dir = (try zant.paths.appDataDir(allocator, init.environ_map, cfg.bundle_id)).?; // 不会自动创建
// 创建 dir 后：
const path = try std.fs.path.joinZ(allocator, &.{ dir, "app.data" });
var store = try zant.db.open(allocator, path);
```

使用系统 SQLite：macOS 链接 `libsqlite3`，Windows 链接系统自带的 `winsqlite3.dll`，不需要额外分发。

## 键值

```zig
try store.setString("theme", "dark");
try store.setInt("launches", 3);
try store.setBool("onboarded", true);
try store.setFloat("scale", 1.25);

var buf: [64]u8 = undefined;
const theme = (try store.getString("theme", &buf)) orelse "light";
const launches = store.getInt("launches", 0);
const onboarded = store.getBool("onboarded", false);
const scale = store.getFloat("scale", 1.0);

try store.remove("theme");
const exists = store.has("onboarded");
```

也可以使用通用 `set` 写入标量：

```zig
try store.set("count", 42);
try store.set("enabled", true);
try store.setString("name", "Ada");
```

## JSON 对象

```zig
const Settings = struct {
    dark: bool = false,
    notifications: bool = true,
    accent: []const u8 = "#1677ff",
};

try store.setJson("settings", Settings{
    .dark = true,
    .notifications = false,
    .accent = "#ff4d4f",
});

var arena = std.heap.ArenaAllocator.init(allocator);
defer arena.deinit();

const settings = (try store.getJson(Settings, "settings", arena.allocator()))
    orelse Settings{};
```

JSON 里的字符串由传入的 allocator 分配，推荐用 arena 管理生命周期。

## 对象集合

```zig
const User = struct {
    id: i64 = 0,
    name: []const u8,
    age: i32,
    active: bool,

    pub const table_name = "users"; // 可选
};

var users = try store.collection(User);

const id = try users.add(.{ .name = "Ada", .age = 36, .active = true });
const user = (try users.find(id)).?;
defer users.free(user);

var changed = user;
changed.age = 37;
try users.update(changed);

const all_users = try users.all();
defer users.freeAll(all_users);

try users.remove(id);
const count = try users.count();
```

集合会自动建表、映射字段、生成自增 id；不需要写 `CREATE TABLE` 或任何
`INSERT/SELECT/UPDATE/DELETE`。

- 字段类型：整数、浮点、布尔、字符串（`[]const u8`）、枚举（按名字存，读到不认识的值时用字段默认值），
  以及它们的可选类型。
- 应用新版本给结构体加了字段：打开集合时自动加列，旧数据取字段默认值（可选字段为 null）；字段名
  可以是 SQL 关键字（`from`、`order`）。
- `clear()` 删除集合里的全部对象。
- 很多次修改合成一次写入（也是“要么全部成功要么全部不做”）：

```zig
try store.begin();
errdefer store.rollback();
for (ids) |id| try users.remove(id);
try store.commit();
```

数据库用 WAL 日志模式（每次小的修改不必整盘同步，读不阻塞写），旁边会有 `-wal` / `-shm` 文件，
属于正常现象。文件损坏时 `open` 返回错误，应用可以把它改名备份后重新创建（ClipHop 的
`storage.zig` 是参考写法）。

## 开发者需要记住的 API

| 场景 | API |
|---|---|
| 打开 | `zant.db.open(allocator, path)` |
| 字符串 | `setString` / `getString` |
| 整数 | `setInt` / `getInt` |
| 布尔 | `setBool` / `getBool` |
| 浮点 | `setFloat` / `getFloat` |
| JSON | `setJson` / `getJson`（单个值最大 64 KB；更大的数据用对象集合或文件） |
| 删除键 | `remove(key)`；清空所有键值 `clear()` |
| 是否存在 | `has(key)` |
| 对象集合 | `store.collection(T)` |
| 集合 CRUD | `add/find/all/update/remove/count/clear` |
| 批量写入 | `begin` / `commit` / `rollback` |

底层 SQLite 完全由库管理；业务代码不需要接触。
