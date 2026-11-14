//! `zant::db`: the key/value store, object collections and `#[derive(Table)]`.
//!
//! Every test that needs a database file uses its own temporary directory
//! instead of a shared path.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use zant::db;
use zant::db::SqlEnum;
use zant::db::sqlite::TableModel;
use zant_macros::Table;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct Settings {
    dark: bool,
    count: i32,
}

#[derive(Table)]
#[table(name = "users")]
struct User {
    id: i64,
    name: String,
    age: i32,
    active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    Image,
}

impl SqlEnum for Kind {
    fn as_str(&self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Image => "image",
        }
    }

    fn from_str(name: &str) -> Option<Self> {
        match name {
            "text" => Some(Kind::Text),
            "image" => Some(Kind::Image),
            _ => None,
        }
    }

    const FIRST: Self = Kind::Text;
}

#[derive(Table)]
#[table(name = "clips")]
struct ClipV1 {
    id: i64,
    body: String,
}

#[derive(Table)]
#[table(name = "clips")]
struct ClipV2 {
    id: i64,
    #[table(default = "image")]
    kind: Kind,
    body: String,
    pinned: bool,
}

/// No `#[table(name = ..)]`: the Rust type name is the table name.
#[derive(Table)]
struct Message {
    id: i64,
    from: String,
    order: i64,
}

#[derive(Table)]
#[table(name = "notes")]
struct Note {
    id: i64,
    title: String,
    body: Option<String>,
    rank: Option<i32>,
}

static TEMP_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Unique temporary directory, removed when the test ends.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "zant-db-test-{}-{sequence}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create test temp dir");
        TempDir(path)
    }

    fn database(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn user(name: &str, age: i32, active: bool) -> User {
    User {
        id: 0,
        name: name.to_string(),
        age,
        active,
    }
}

#[test]
fn store_key_value_json_and_collection() {
    let store = db::open(":memory:").expect("open in-memory store");

    store.set_string("theme", "dark").unwrap();
    store.set_int("launches", 3).unwrap();
    store.set_bool("onboarded", true).unwrap();
    store.set_float("scale", 1.25).unwrap();
    store
        .set_json(
            "settings",
            &Settings {
                dark: true,
                count: 7,
            },
        )
        .unwrap();

    assert_eq!("dark", store.get_string("theme").unwrap().unwrap());
    assert_eq!(3, store.get_int("launches", 0));
    assert!(store.get_bool("onboarded", false));
    assert!((store.get_float("scale", 0.0) - 1.25).abs() < 0.001);
    assert!(store.has("theme"));
    // values longer than any small read buffer exist too
    assert!(store.has("settings"));
    assert!(!store.has("missing"));

    let settings: Settings = store.get_json("settings").unwrap().unwrap();
    assert!(settings.dark);
    assert_eq!(7, settings.count);

    let users = store.collection::<User>().unwrap();
    let id = users.add(&user("Ada", 36, true)).unwrap();
    let found = users.find(id).unwrap().unwrap();
    assert_eq!("Ada", found.name);
    assert_eq!(1, users.count().unwrap());

    store.remove("theme").unwrap();
    assert!(!store.has("theme"));
}

#[test]
fn store_collection_persists_across_reopen() {
    let dir = TempDir::new("persist");
    let path = dir.database("zant-store-test.sqlite");
    let path = path.to_str().unwrap();

    {
        let first = db::open(path).unwrap();
        let users = first.collection::<User>().unwrap();
        users.add(&user("Persisted", 20, false)).unwrap();
        first.close();
    }

    let second = db::open(path).unwrap();
    let users = second.collection::<User>().unwrap();
    let rows = users.all().unwrap();
    assert_eq!(1, rows.len());
    assert_eq!("Persisted", rows[0].name);
}

#[test]
fn store_collection_enums_new_fields_of_a_later_version_clear() {
    let dir = TempDir::new("migrate");
    let path = dir.database("zant-store-migrate.sqlite");
    let path = path.to_str().unwrap();
    {
        let store = db::open(path).unwrap();
        let clips = store.collection::<ClipV1>().unwrap();
        clips
            .add(&ClipV1 {
                id: 0,
                body: "old".to_string(),
            })
            .unwrap();
    }
    let store = db::open(path).unwrap();
    // the table gains kind / pinned; the old row gets their defaults
    let clips = store.collection::<ClipV2>().unwrap();
    store.begin().unwrap();
    clips
        .add(&ClipV2 {
            id: 0,
            kind: Kind::Text,
            body: "new".to_string(),
            pinned: true,
        })
        .unwrap();
    store.commit().unwrap();
    let rows = clips.all().unwrap();
    assert_eq!(2, rows.len());
    assert_eq!(Kind::Image, rows[0].kind);
    assert!(!rows[0].pinned);
    assert_eq!("old", rows[0].body);
    assert_eq!(Kind::Text, rows[1].kind);
    assert!(rows[1].pinned);
    clips.clear().unwrap();
    assert_eq!(0, clips.count().unwrap());
}

#[test]
fn store_collection_sql_keywords_as_names() {
    let store = db::open(":memory:").unwrap();
    let messages = store.collection::<Message>().unwrap();
    messages
        .add(&Message {
            id: 0,
            from: "Ada".to_string(),
            order: 2,
        })
        .unwrap();
    let rows = messages.all().unwrap();
    assert_eq!("Ada", rows[0].from);
    assert_eq!(2, rows[0].order);
}

/// `#[derive(Table)]` must emit exactly this SQL text: it is stable so
/// existing databases keep opening.
#[test]
fn generated_sql_text_is_stable() {
    // `User` has `#[table(name = "users")]`.
    assert_eq!("users", User::TABLE_NAME);
    assert_eq!("\"users\"", User::TABLE_NAME_SQL);
    assert_eq!(User::COLUMNS, ["id", "name", "age", "active"].as_slice());
    assert_eq!("\"id\", \"name\", \"age\", \"active\"", User::COLUMNS_SQL);
    assert_eq!(
        "CREATE TABLE IF NOT EXISTS \"users\" (\"id\" INTEGER PRIMARY KEY AUTOINCREMENT, \"name\" TEXT NOT NULL, \"age\" INTEGER NOT NULL, \"active\" INTEGER NOT NULL)",
        User::CREATE_TABLE_SQL
    );
    assert_eq!(
        "SELECT \"id\", \"name\", \"age\", \"active\" FROM \"users\"",
        User::SELECT_SQL
    );
    assert_eq!(
        "SELECT \"id\", \"name\", \"age\", \"active\" FROM \"users\" WHERE id = ? LIMIT 1",
        User::SELECT_BY_ID_SQL
    );
    assert_eq!(
        "INSERT INTO \"users\" (\"name\", \"age\", \"active\") VALUES (?, ?, ?)",
        User::INSERT_SQL
    );
    assert_eq!(
        "UPDATE \"users\" SET \"name\" = ?, \"age\" = ?, \"active\" = ? WHERE id = ?",
        User::UPDATE_SQL
    );
    assert_eq!("SELECT COUNT(*) FROM \"users\"", User::COUNT_SQL);
    assert_eq!("DELETE FROM \"users\" WHERE id = ?", User::DELETE_SQL);
    assert_eq!("DELETE FROM \"users\"", User::DELETE_ALL_SQL);

    // Fallback when the model has no `#[table(name = ..)]`: the struct name.
    // Models whose existing table files matter should spell the name out with
    // `#[table(name = "..")]`.
    assert_eq!("Message", Message::TABLE_NAME);
    assert_eq!(
        "CREATE TABLE IF NOT EXISTS \"Message\" (\"id\" INTEGER PRIMARY KEY AUTOINCREMENT, \"from\" TEXT NOT NULL, \"order\" INTEGER NOT NULL)",
        Message::CREATE_TABLE_SQL
    );

    // `added_column`: optional fields stay nullable, non-optional ones get a
    // default (`0`, `''`, or the enum field's default tag / first tag).
    assert_eq!(
        Some("TEXT NOT NULL DEFAULT 'image'".to_string()),
        ClipV2::added_column(1)
    );
    assert_eq!(
        Some("TEXT NOT NULL DEFAULT ''".to_string()),
        ClipV2::added_column(2)
    );
    assert_eq!(
        Some("INTEGER NOT NULL DEFAULT 0".to_string()),
        ClipV2::added_column(3)
    );
    assert_eq!(None, ClipV2::added_column(0));
    assert_eq!(Some("TEXT".to_string()), Note::added_column(2));
    assert_eq!(Some("INTEGER".to_string()), Note::added_column(3));
    assert_eq!(None, ClipV1::added_column(0));
}

/// Optional model fields read SQL NULL as `None` and write `None` as NULL.
#[test]
fn table_optional_fields_round_trip() {
    let dir = TempDir::new("optional");
    let path = dir.database("zant-notes.sqlite");
    let store = db::open(path.to_str().unwrap()).unwrap();
    let notes = store.collection::<Note>().unwrap();
    notes
        .add(&Note {
            id: 0,
            title: "empty".to_string(),
            body: None,
            rank: None,
        })
        .unwrap();
    notes
        .add(&Note {
            id: 0,
            title: "full".to_string(),
            body: Some("body".to_string()),
            rank: Some(7),
        })
        .unwrap();

    let rows = notes.all().unwrap();
    assert_eq!(2, rows.len());
    assert_eq!(None, rows[0].body);
    assert_eq!(None, rows[0].rank);
    assert_eq!(Some("body".to_string()), rows[1].body);
    assert_eq!(Some(7), rows[1].rank);
}
