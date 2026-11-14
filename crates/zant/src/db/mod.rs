//! Port of `src/db/mod.zig` — WP-3.10.
//!
//! Persistence for Zant UI apps: one [`Store`] for key/value, JSON and object
//! collections, with no SQL surface for business code.
//!
//! ```ignore
//! let store = zant::db::open("app.data")?;
//! store.set_string("theme", "dark")?;
//! let theme = store.get_string("theme")?.unwrap_or_else(|| "light".to_string());
//! ```

use std::marker::PhantomData;

use crate::db::sqlite::Database;

pub mod sqlite;
pub mod table;

pub use sqlite::DbError as Error;
pub use table::{SqlEnum, Table, TableModel};

/// Object collection for one model (Zig `Collection(T)`).
///
/// Zig freed rows with `free` / `freeAll`; Rust values own their strings and
/// drop them, so those methods are not ported.
pub struct Collection<'a, T: TableModel> {
    db: &'a Database,
    model: PhantomData<T>,
}

impl<'a, T: TableModel> Collection<'a, T> {
    fn table(&self) -> Table<'a, T> {
        self.db.table::<T>()
    }

    pub fn add(&self, value: &T) -> Result<i64, Error> {
        self.table().insert(value)
    }

    pub fn find(&self, id: i64) -> Result<Option<T>, Error> {
        self.table().get(id)
    }

    pub fn all(&self) -> Result<Vec<T>, Error> {
        self.table().all()
    }

    pub fn update(&self, value: &T) -> Result<(), Error> {
        self.table().update(value)
    }

    pub fn remove(&self, id: i64) -> Result<(), Error> {
        self.table().delete(id)
    }

    pub fn count(&self) -> Result<i64, Error> {
        self.table().count()
    }

    /// Removes every object.
    pub fn clear(&self) -> Result<(), Error> {
        self.table().delete_all()
    }
}

/// ZIP of the scalar types Zig's generic `Store.set` accepted.
///
/// It also covers the wire format: the `kind` column stores 0 = string,
/// 1 = int, 2 = bool, 3 = float, 4 = JSON.
#[derive(Clone, Debug, PartialEq)]
pub enum StoreValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl From<bool> for StoreValue {
    fn from(value: bool) -> Self {
        StoreValue::Bool(value)
    }
}

macro_rules! impl_from_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for StoreValue {
                fn from(value: $ty) -> Self {
                    StoreValue::Int(i64::from(value))
                }
            }
        )*
    };
}

impl_from_int!(i8, i16, i32, i64, u8, u16, u32);

impl From<u64> for StoreValue {
    fn from(value: u64) -> Self {
        StoreValue::Int(i64::try_from(value).expect("int cast"))
    }
}

impl From<f32> for StoreValue {
    fn from(value: f32) -> Self {
        StoreValue::Float(f64::from(value))
    }
}

impl From<f64> for StoreValue {
    fn from(value: f64) -> Self {
        StoreValue::Float(value)
    }
}

impl From<&str> for StoreValue {
    fn from(value: &str) -> Self {
        StoreValue::Text(value.to_owned())
    }
}

impl From<String> for StoreValue {
    fn from(value: String) -> Self {
        StoreValue::Text(value)
    }
}

impl From<&String> for StoreValue {
    fn from(value: &String) -> Self {
        StoreValue::Text(value.clone())
    }
}

/// The store of one application data file (Zig `Store`).
pub struct Store {
    db: Database,
}

impl Store {
    /// Opens the database and prepares it for small writes (Zig `Store.open`).
    pub fn open(path: &str) -> Result<Store, Error> {
        let db = Database::open(path)?;
        // write-ahead log: small writes (one per user action) without a full
        // sync each, and readers never block the writer
        let _ = db.exec("PRAGMA journal_mode=WAL");
        let _ = db.exec("PRAGMA synchronous=NORMAL");
        let _ = db.exec("PRAGMA busy_timeout=2000");
        // a file that is not a database (damaged) fails here
        db.exec("CREATE TABLE IF NOT EXISTS _zant_kv (key TEXT PRIMARY KEY, kind INTEGER NOT NULL, value TEXT)")?;
        Ok(Store { db })
    }

    /// Closes the store; dropping has the same effect (Zig `close`).
    pub fn close(self) {}

    /// Groups many changes into one write (and makes them all-or-nothing).
    pub fn begin(&self) -> Result<(), Error> {
        self.db.begin()
    }

    pub fn commit(&self) -> Result<(), Error> {
        self.db.commit()
    }

    pub fn rollback(&self) {
        let _ = self.db.rollback();
    }

    /// Creates the model's table if needed and returns its collection.
    pub fn collection<T: TableModel>(&self) -> Result<Collection<'_, T>, Error> {
        self.db.table::<T>().create()?;
        Ok(Collection {
            db: &self.db,
            model: PhantomData,
        })
    }

    fn kv_put(&self, key: &str, kind: i64, value: &str) -> Result<(), Error> {
        self.db.run(
            "INSERT INTO _zant_kv(key, kind, value) VALUES(?, ?, ?) ON CONFLICT(key) DO UPDATE SET kind=excluded.kind, value=excluded.value",
            (key, kind, value),
        )
    }

    pub fn set_string(&self, key: &str, value: &str) -> Result<(), Error> {
        self.kv_put(key, 0, value)
    }

    pub fn set_int(&self, key: &str, value: i64) -> Result<(), Error> {
        self.kv_put(key, 1, &format!("{value}"))
    }

    pub fn set_bool(&self, key: &str, value: bool) -> Result<(), Error> {
        self.kv_put(key, 2, if value { "1" } else { "0" })
    }

    pub fn set_float(&self, key: &str, value: f64) -> Result<(), Error> {
        self.kv_put(key, 3, &format!("{value}"))
    }

    pub fn set_json<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<(), Error> {
        let text = serde_json::to_string(value).map_err(Error::InvalidJson)?;
        self.kv_put(key, 4, &text)
    }

    /// Writes a scalar, picking the wire format from its type (Zig `Store.set`).
    pub fn set<V: Into<StoreValue>>(&self, key: &str, value: V) -> Result<(), Error> {
        match value.into() {
            StoreValue::Bool(value) => self.set_bool(key, value),
            StoreValue::Int(value) => self.set_int(key, value),
            StoreValue::Float(value) => self.set_float(key, value),
            StoreValue::Text(value) => self.set_string(key, &value),
        }
    }

    fn kv_get(&self, key: &str) -> Result<Option<(i64, String)>, Error> {
        self.db.query_row_optional(
            "SELECT kind, value FROM _zant_kv WHERE key = ? LIMIT 1",
            (key,),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    }

    pub fn get_string(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self.kv_get(key)?.map(|(_kind, text)| text))
    }

    pub fn get_int(&self, key: &str, default: i64) -> i64 {
        match self.kv_get(key) {
            Ok(Some((_kind, text))) => text.parse().unwrap_or(default),
            _ => default,
        }
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.kv_get(key) {
            Ok(Some((_kind, text))) => text == "1",
            _ => default,
        }
    }

    pub fn get_float(&self, key: &str, default: f64) -> f64 {
        match self.kv_get(key) {
            Ok(Some((_kind, text))) => text.parse().unwrap_or(default),
            _ => default,
        }
    }

    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        // Zig read JSON through a 64 KB stack buffer and rejected larger values.
        const JSON_MAX_BYTES: usize = 64 * 1024;
        let Some((_kind, text)) = self.kv_get(key)? else {
            return Ok(None);
        };
        if text.len() + 1 > JSON_MAX_BYTES {
            return Err(Error::ValueTooLarge);
        }
        serde_json::from_str(&text)
            .map(Some)
            .map_err(Error::InvalidJson)
    }

    /// The key exists (whatever the size of its value).
    pub fn has(&self, key: &str) -> bool {
        match self
            .db
            .query("SELECT 1 FROM _zant_kv WHERE key = ? LIMIT 1", (key,))
        {
            Ok(mut rows) => matches!(rows.next(), Ok(Some(_))),
            Err(_) => false,
        }
    }

    pub fn remove(&self, key: &str) -> Result<(), Error> {
        self.db.run("DELETE FROM _zant_kv WHERE key = ?", (key,))
    }

    pub fn clear(&self) -> Result<(), Error> {
        self.db.exec("DELETE FROM _zant_kv")
    }
}

/// Opens an application data file (Zig `db.open`).
pub fn open(path: &str) -> Result<Store, Error> {
    Store::open(path)
}
