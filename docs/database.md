# 本地数据存储

`zant::db` 的目标是让业务代码感觉不到 SQLite：一个 Store，存键值、JSON 和
对象集合，不写 SQL，不管理表、连接、statement 或结果集。底层用 `rusqlite`（bundled SQLite），
不需要额外分发系统库。TSX 是应用界面的推荐写法；数据库操作写在 `native/` 的 Rust 模块，
通过 `.ts` 规范暴露业务方法给界面，见 [原生模块](tsx-native.md)。下文为原生模块中的 Rust API。

## 打开

```rust
let store = zant::db::open("app.data")?;
// 用完自动关闭（Drop）；也可以显式 store.close()
```

`path` 是 UTF-8 路径，相对路径按当前目录解析；`":memory:"` 可用于测试。
正式应用把文件放在应用数据目录（macOS `~/Library/Application Support/<id>`，Windows `%APPDATA%\<id>`）：

```rust
let dir = zant::core::paths::app_data_dir(crate::app_config::bundle_id).expect("data dir"); // 不会自动创建
std::fs::create_dir_all(&dir)?;
let path = dir.join("app.data");
let store = zant::db::open(&path.to_string_lossy())?;
```

打开时尝试设置 WAL 日志模式、`synchronous=NORMAL` 和 2000 ms 忙等待；文件数据库旁可能有 `-wal` / `-shm` 文件，
属于正常现象。无法打开或初始化数据库时 `open` 返回错误；确认文件损坏后，可由应用提供备份并重建的恢复流程。

## 键值

```rust
store.set_string("theme", "dark")?;
store.set_int("launches", 3)?;
store.set_bool("onboarded", true)?;
store.set_float("scale", 1.25)?;

let theme = store.get_string("theme")?.unwrap_or_else(|| "light".to_string());
let launches = store.get_int("launches", 0);
let onboarded = store.get_bool("onboarded", false);
let scale = store.get_float("scale", 1.0);

store.remove("theme")?;
let exists = store.has("onboarded");
```

也可以使用通用 `set` 写入标量（按 Rust 类型选落库格式：`bool` / 整数 / 浮点 / 字符串）：

```rust
store.set("count", 42)?;
store.set("enabled", true)?;
store.set_string("name", "Ada")?;
```

`get_int` / `get_float` 在缺键、查询失败或解析失败时返回给定默认值；`get_bool` 在缺键或查询失败时
返回默认值，已有值只有文本 `"1"` 视为 true；`has` 查询失败时返回 false。需要处理读取错误时使用
返回 `Result` 的 `get_string` / `get_json`。

## JSON 对象

```rust
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Settings {
    dark: bool,
    notifications: bool,
    accent: String,
}

store.set_json("settings", &Settings {
    dark: true,
    notifications: false,
    accent: "#ff4d4f".into(),
})?;

let settings: Settings = store.get_json("settings")?.unwrap_or_default();
```

`set_json` 本身不限制长度；`get_json` 要求序列化文本的 UTF-8 字节数加 1 不超过 64 × 1024，
超出返回 `ValueTooLarge`。更大的数据用对象集合或文件。反序列化结果拥有自己的字符串。

## 对象集合

模型用 `#[derive(zant_macros::Table)]` 生成 SQL（建表、增删改查、`ALTER TABLE` 迁移），
不需要写 `CREATE TABLE` 或任何 `INSERT/SELECT/UPDATE/DELETE`：

```rust
use zant_macros::Table;

#[derive(Table)]
#[table(name = "users")]      // 可选；默认用类型名（已有库的表名不同时显式写上）
struct User {
    id: i64,                  // 自增主键（INSERT/UPDATE 时跳过）
    name: String,
    age: i32,
    active: bool,
}

let users = store.collection::<User>()?;
let id = users.add(&User { id: 0, name: "Ada".into(), age: 36, active: true })?;
let user = users.find(id)?.unwrap();
let mut changed = user;
changed.age = 37;
users.update(&changed)?;
let all = users.all()?;
users.remove(id)?;
let count = users.count()?;
```

- 字段类型：整数、浮点（`f32` / `f64`）、布尔、字符串（`String`）、枚举（按名字存 TEXT），以及它们的 `Option<T>`。
- 枚举字段要实现 `zant::db::SqlEnum`（`as_str` / `from_str` / `FIRST`），见 `crates/zant/tests/db_test.rs`；
  非可选枚举读到不认识的值时用字段默认值（`FIRST`，或用 `#[table(default = "image")]` 指定）；`Option<枚举>` 读到未知值时为 `None`。
- 应用新版本给结构体加了字段：打开集合时自动加列，旧数据取字段默认值（可选字段为 null）；字段名
  可以是 SQL 关键字（`from`、`order`）。
- `users.clear()` 删除集合里的全部对象。
- 很多次修改合成一次写入（也是“要么全部成功要么全部不做”）：

```rust
store.begin()?;
for id in ids {
    if let Err(error) = users.remove(*id) {
        store.rollback();       // 放弃这一批；rollback 本身不返回错误
        return Err(error);
    }
}
store.commit()?;
```

自动迁移只补缺失列，不会替你重命名、删除列或转换字段类型。集合借用 Store，不能比 Store 活得久。

查询结果不需要手动释放：值自己拥有字符串，离开作用域即释放。

## 开发者需要记住的 API

| 场景 | API |
|---|---|
| 打开 | `zant::db::open(path)` |
| 字符串 | `set_string` / `get_string` |
| 整数 | `set_int` / `get_int` |
| 布尔 | `set_bool` / `get_bool` |
| 浮点 | `set_float` / `get_float` |
| JSON | `set_json` / `get_json`（读取有 64 KB 限制，见上文） |
| 删除键 | `remove(key)`；清空所有键值 `clear()` |
| 是否存在 | `has(key)` |
| 对象集合 | `store.collection::<T>()`（`T: TableModel`，由 `#[derive(Table)]` 实现） |
| 集合 CRUD | `add / find / all / update / remove / count / clear` |
| 批量写入 | `begin` / `commit` / `rollback` |
| 应用数据目录 | `zant::core::paths::app_data_dir(bundle_id)`（不会自动创建） |

底层 SQLite 完全由库管理；业务代码不需要接触。需要自定义查询时用 `zant::db::sqlite::Database`
（`query` / `query_row_optional` / `run` / `exec`），但应用代码优先用上面的 Store / Collection。
