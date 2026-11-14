//! Port of `src/db/sqlite.zig` — WP-3.10.
//!
//! A small, high-level SQLite wrapper for Zant UI apps. `rusqlite` replaces the
//! hand-written C declarations of `src/db/sqlite_c.zig`; the public surface
//! mirrors the Zig module:
//!
//! ```ignore
//! let db = Database::open("app.sqlite")?;
//! db.exec("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, body TEXT)")?;
//! db.run("INSERT INTO notes (body) VALUES (?)", ("hello",))?;
//! let mut rows = db.query("SELECT id, body FROM notes ORDER BY id", ())?;
//! while let Some(row) = rows.next()? {
//!     let id = row.int(0);
//!     let body = row.text(1);
//! }
//! ```
//!
//! There is no `Statement` wrapper: Zig's `Statement` was a thin façade over
//! `sqlite3_stmt`, so [`Database::prepare`] hands out rusqlite's own
//! `Statement`. The error type is [`DbError`]; it has to live here because the
//! crate root does not define `zant::Error` yet (see docs/rust-port/plan.md §4).

use crate::db::table::{Table, row_value};

// The ORM helpers live next to the schema code in `db::table`, but the Zig
// module exposed them all from `sqlite.zig`; re-export so the generated code
// (and applications) can keep using the `db::sqlite` path.
pub use crate::db::table::{
    SqlEnum, TableModel, column_bool, column_enum, column_float, column_int, column_opt_bool,
    column_opt_float, column_opt_int, column_opt_text, column_text,
};
use rusqlite::types::FromSql;
use rusqlite::{Connection, Params};

pub use rusqlite::types::Value;
pub use rusqlite::{Error as SqliteError, Result as SqliteResult, Row as SqliteRow};

/// Port of Zig `sqlite.Error`. The rusqlite error is kept as the source (Zig
/// only had the error tag), and maps to the same tags the Zig version used.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("failed to open the database: {0}")]
    OpenFailed(#[source] SqliteError),
    #[error("failed to prepare the SQL statement: {0}")]
    PrepareFailed(#[source] SqliteError),
    #[error("failed to bind a SQL parameter: {0}")]
    BindFailed(#[source] SqliteError),
    #[error("failed to step the SQL statement: {0}")]
    StepFailed(#[source] SqliteError),
    #[error("column index out of range")]
    ColumnOutOfRange,
    #[error("column type mismatch")]
    TypeMismatch,
    #[error("value too large")]
    ValueTooLarge,
    #[error("invalid JSON: {0}")]
    InvalidJson(#[source] serde_json::Error),
    #[error("database is busy")]
    Busy,
}

impl From<SqliteError> for DbError {
    fn from(error: SqliteError) -> Self {
        match &error {
            SqliteError::InvalidParameterName(_) | SqliteError::InvalidParameterCount(..) => {
                DbError::BindFailed(error)
            }
            _ if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy) => {
                DbError::Busy
            }
            _ => DbError::StepFailed(error),
        }
    }
}

/// A single result row (Zig `sqlite.Row`).
///
/// Zig's row borrowed a live `sqlite3_stmt`, which safe Rust cannot store next
/// to the statement that owns it. The wrapper therefore snapshots the column
/// values; the accessors keep the Zig names and the SQLite-style coercion.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    values: Vec<Value>,
}

impl Row {
    fn from_sqlite(row: &SqliteRow<'_>, column_count: usize) -> SqliteResult<Row> {
        let mut values = Vec::with_capacity(column_count);
        for index in 0..column_count {
            values.push(row_value(row, index)?);
        }
        Ok(Row { values })
    }

    /// Number of columns (Zig `count`).
    pub fn count(&self) -> usize {
        self.values.len()
    }

    /// True for SQL NULL (and for an out-of-range column), like Zig `isNull`.
    pub fn is_null(&self, index: usize) -> bool {
        matches!(self.values.get(index), None | Some(Value::Null))
    }

    /// Integer column, coerced like Zig's `sqlite3_column_int64`: NULL and
    /// blobs read as 0, real values are truncated, text is parsed.
    pub fn int(&self, index: usize) -> i64 {
        match self.values.get(index) {
            Some(Value::Integer(value)) => *value,
            Some(Value::Real(value)) => *value as i64,
            Some(Value::Text(text)) => text.trim().parse().unwrap_or(0),
            _ => 0,
        }
    }

    /// Real column, coerced like Zig's `sqlite3_column_double`.
    pub fn float(&self, index: usize) -> f64 {
        match self.values.get(index) {
            Some(Value::Real(value)) => *value,
            Some(Value::Integer(value)) => *value as f64,
            Some(Value::Text(text)) => text.trim().parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    /// Text column; NULL reads as "" like Zig `row.text`.
    pub fn text(&self, index: usize) -> &str {
        match self.values.get(index) {
            Some(Value::Text(text)) => text,
            Some(Value::Blob(bytes)) => std::str::from_utf8(bytes).unwrap_or(""),
            _ => "",
        }
    }

    /// Blob column; NULL reads as "" like Zig `row.blob`.
    pub fn blob(&self, index: usize) -> &[u8] {
        match self.values.get(index) {
            Some(Value::Blob(bytes)) => bytes,
            Some(Value::Text(text)) => text.as_bytes(),
            _ => &[],
        }
    }

    /// The dynamic value of a column (Zig `value`).
    pub fn value(&self, index: usize) -> Value {
        self.values.get(index).cloned().unwrap_or(Value::Null)
    }
}

/// Query result cursor (Zig `sqlite.Rows`).
///
/// `Database::query` fetches the rows; `next` then mirrors the Zig cursor. (A
/// lazily stepping wrapper would have to own rusqlite's `Rows` together with
/// its `Statement`, i.e. be self-referential, which safe Rust forbids.)
pub struct Rows {
    rows: Vec<Row>,
    current: Option<usize>,
    next_index: usize,
    done: bool,
}

impl Rows {
    fn new(rows: Vec<Row>) -> Rows {
        Rows {
            rows,
            current: None,
            next_index: 0,
            done: false,
        }
    }

    /// Returns true when a row is available, false when done (Zig `next`).
    #[allow(clippy::should_implement_trait)] // Zig shape: `try rows.next()`
    pub fn next(&mut self) -> Result<Option<&Row>, DbError> {
        if self.done {
            return Ok(None);
        }
        if self.next_index < self.rows.len() {
            let index = self.next_index;
            self.next_index += 1;
            self.current = Some(index);
            return Ok(Some(&self.rows[index]));
        }
        self.done = true;
        Ok(None)
    }

    /// The row last returned by [`Rows::next`] (Zig `rows.stmt.row()`).
    pub fn row(&self) -> &Row {
        let index = self.current.expect("Rows::row called before Rows::next");
        &self.rows[index]
    }

    /// No-op: Rust drops the rows automatically (Zig `deinit`).
    pub fn deinit(self) {}
}

/// An opened SQLite database (Zig `sqlite.Database`).
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens (or creates) the database at `path`; `":memory:"` is a scratch
    /// database (Zig `open`).
    pub fn open(path: &str) -> Result<Database, DbError> {
        let conn = Connection::open(path).map_err(DbError::OpenFailed)?;
        Ok(Database { conn })
    }

    /// Opens an in-memory database, useful for tests and scratch data.
    pub fn open_memory() -> Result<Database, DbError> {
        let conn = Connection::open_in_memory().map_err(DbError::OpenFailed)?;
        Ok(Database { conn })
    }

    /// Closes the database; dropping has the same effect (Zig `close`).
    pub fn close(self) {}

    pub fn last_insert_id(&self) -> i64 {
        self.conn.last_insert_rowid()
    }

    pub fn changes(&self) -> u64 {
        self.conn.changes()
    }

    /// Runs one or more statements, ignoring results (Zig `exec`).
    pub fn exec(&self, sql: &str) -> Result<(), DbError> {
        self.conn.execute_batch(sql).map_err(DbError::StepFailed)
    }

    /// Prepares a statement; rusqlite's `Statement` takes the place of Zig's
    /// `sqlite.Statement`.
    pub fn prepare(&self, sql: &str) -> Result<rusqlite::Statement<'_>, DbError> {
        self.conn.prepare(sql).map_err(DbError::PrepareFailed)
    }

    /// Executes a statement with positional `?` parameters (Zig `run`).
    pub fn run<P: Params>(&self, sql: &str, params: P) -> Result<(), DbError> {
        let mut stmt = self.prepare(sql)?;
        let mut rows = stmt.query(params).map_err(DbError::BindFailed)?;
        let _ = rows.next().map_err(DbError::from)?;
        Ok(())
    }

    /// Runs a query and returns its rows (Zig `query`).
    pub fn query<P: Params>(&self, sql: &str, params: P) -> Result<Rows, DbError> {
        let mut stmt = self.prepare(sql)?;
        let column_count = stmt.column_count();
        let mut raw_rows = stmt.query(params).map_err(DbError::BindFailed)?;
        let mut rows = Vec::new();
        while let Some(row) = raw_rows.next().map_err(DbError::from)? {
            rows.push(Row::from_sqlite(row, column_count).map_err(DbError::from)?);
        }
        Ok(Rows::new(rows))
    }

    /// Single scalar result; an empty result returns `None` (Zig `queryOne`).
    pub fn query_one<T: FromSql, P: Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Option<T>, DbError> {
        let mut stmt = self.prepare(sql)?;
        let mut rows = stmt.query(params).map_err(DbError::BindFailed)?;
        match rows.next().map_err(DbError::from)? {
            Some(row) => Ok(Some(row.get(0).map_err(DbError::from)?)),
            None => Ok(None),
        }
    }

    pub fn begin(&self) -> Result<(), DbError> {
        self.exec("BEGIN")
    }

    pub fn commit(&self) -> Result<(), DbError> {
        self.exec("COMMIT")
    }

    pub fn rollback(&self) -> Result<(), DbError> {
        self.exec("ROLLBACK")
    }

    /// Runs `callback` inside a transaction, rolling back when it (or the
    /// commit) fails (Zig `transaction`).
    pub fn transaction<E, F>(&self, callback: F) -> Result<(), E>
    where
        E: From<DbError>,
        F: FnOnce(&Database) -> Result<(), E>,
    {
        self.begin().map_err(E::from)?;
        let result = match callback(self) {
            Ok(()) => self.commit().map_err(E::from),
            Err(error) => Err(error),
        };
        if result.is_err() {
            let _ = self.rollback();
        }
        result
    }

    /// CRUD facade for one model (Zig `table`).
    pub fn table<T: TableModel>(&self) -> Table<'_, T> {
        Table::new(self)
    }

    /// `query_row` convenience used by `db::Store` (Zig returned `Rows` here).
    pub(crate) fn query_row_optional<T, P, F>(
        &self,
        sql: &str,
        params: P,
        map: F,
    ) -> Result<Option<T>, DbError>
    where
        P: Params,
        F: FnOnce(&SqliteRow<'_>) -> SqliteResult<T>,
    {
        let mut stmt = self.prepare(sql)?;
        let mut rows = stmt.query(params).map_err(DbError::BindFailed)?;
        match rows.next().map_err(DbError::from)? {
            Some(row) => Ok(Some(map(row)?)),
            None => Ok(None),
        }
    }
}

impl std::fmt::Debug for Database {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Database")
            .field("path", &self.conn.path())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_crud_and_typed_rows() {
        let db = Database::open_memory().unwrap();
        db.exec("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT, score REAL)")
            .unwrap();
        db.run(
            "INSERT INTO notes (body, score) VALUES (?, ?)",
            ("hello", 1.5),
        )
        .unwrap();
        assert_eq!(1, db.last_insert_id());
        let mut rows = db.query("SELECT id, body, score FROM notes", ()).unwrap();
        assert!(rows.next().unwrap().is_some());
        let row = rows.row();
        assert_eq!(1, row.int(0));
        assert_eq!("hello", row.text(1));
        assert!((row.float(2) - 1.5).abs() < 0.001);
        assert!(rows.next().unwrap().is_none());
    }

    #[test]
    fn sqlite_transaction_rolls_back() {
        let db = Database::open_memory().unwrap();
        db.exec("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT)")
            .unwrap();

        #[derive(Debug)]
        #[allow(dead_code)] // the payload keeps `?` from hiding a DB failure
        enum Fail {
            Boom,
            Db(DbError),
        }

        impl From<DbError> for Fail {
            fn from(error: DbError) -> Self {
                Fail::Db(error)
            }
        }

        fn write(db: &Database) -> Result<(), Fail> {
            db.run("INSERT INTO notes (body) VALUES (?)", ("bad",))?;
            Err(Fail::Boom)
        }

        assert!(matches!(db.transaction(write), Err(Fail::Boom)));
        let count = db
            .query_one::<i64, _>("SELECT COUNT(*) FROM notes", ())
            .unwrap()
            .unwrap();
        assert_eq!(0, count);
    }
}
