//! Procedural macros for Zant UI.
//!
//! - `#[derive(TsxProps)]`: exposes a plain Rust options struct to the TSX
//!   compiler (field names / types / defaults).
//! - `#[derive(Table)]`: generates the SQLite schema, parameter binding and
//!   row reading of a model struct for `zant::db::sqlite`.
use proc_macro::TokenStream;
use syn::parse_macro_input;

mod table;
mod tsx_props;

/// `#[derive(TsxProps)]`: compile-time reflection of a `ui` option struct for
/// the TSX compiler.
///
/// It emits `impl zant::ui::options::tsx_props::TsxProps for T` with the struct
/// name, doc comment, module path and every field in declaration order. Field
/// types are emitted as token strings; the `zant-tsx-tags` generator resolves
/// them against the `zant` sources and turns them into the TSX prop table
/// (`crates/zant-tsx/src/ui_tags.rs`).
///
/// Field attributes:
/// - `#[tsx(required)]` — the TSX prop must be supplied. `impl Default` gives
///   every field a value, so a field without a meaningful default is spelled
///   out. Also marks unsupported required fields, so the generator can skip
///   the whole tag.
/// - `#[tsx(default = "..")]` — the default expression as written (defaults to
///   the struct's `Default::default()`).
#[proc_macro_derive(TsxProps, attributes(tsx))]
pub fn derive_tsx_props(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match tsx_props::expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// `#[derive(Table)]`: the compile-time ORM behind `zant::db::sqlite::Table<T>`.
///
/// It generates an `impl zant::db::sqlite::TableModel for T` with the table
/// name, the SQL text (`CREATE TABLE`, `SELECT`, `INSERT`, `UPDATE`, `DELETE`,
/// `COUNT`, and the `ALTER TABLE ... ADD COLUMN` list used to migrate older
/// table files), plus the parameter binding and row reading code. The SQL text
/// is stable so existing databases keep opening.
///
/// Struct attributes:
/// - `#[table(name = "users")]` — SQL table name. Without it the Rust type
///   name is used, so models with existing table files should always spell
///   the name out (renaming the type would otherwise rename the table).
///
/// Field attributes:
/// - `#[table(default = "image")]` — the enum variant used for rows written by
///   an older app version (and by `ALTER TABLE ... DEFAULT`). Without it the
///   enum's first variant (`SqlEnum::FIRST`) is used.
///
/// The model must be a plain struct with named fields. `id` is the
/// auto-increment primary key (`INTEGER PRIMARY KEY AUTOINCREMENT`); an
/// `Option<i64>` `id` is a plain nullable column instead.
#[proc_macro_derive(Table, attributes(table))]
pub fn derive_table(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match table::expand_table(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
