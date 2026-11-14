//! Procedural macros for Zant UI.
//!
//! - `#[derive(TsxProps)]`: exposes a plain Rust options struct to the TSX
//!   compiler (field names / types / defaults). See docs/rust-port/tsx-codegen.md.
//! - `#[derive(Table)]`: generates the SQLite schema of `db/sqlite.zig`.
//!   See docs/rust-port/plan.md WP-3.10.
use proc_macro::TokenStream;
use syn::parse_macro_input;

mod table;

/// Placeholder: real derive lands in WP-5.2.
#[proc_macro_derive(TsxProps)]
pub fn derive_tsx_props(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

/// `#[derive(Table)]`: the comptime ORM of `src/db/sqlite.zig` (`Table(T)`) as
/// a derive macro.
///
/// It generates an `impl zant::db::sqlite::TableModel for T` with the table
/// name, the exact SQL text Zig generated (`CREATE TABLE`, `SELECT`, `INSERT`,
/// `UPDATE`, `DELETE`, `COUNT`, and the `ALTER TABLE ... ADD COLUMN` list used
/// to migrate older table files), plus the parameter binding and row reading
/// code.
///
/// Struct attributes:
/// - `#[table(name = "users")]` — SQL table name; mirrors Zig's
///   `pub const table_name`. Without it the Rust type name is used. Zig's
///   `@typeName(T)` fallback is the module root name plus the type name (for
///   the original test file, `"db_test.Message"`), so models with existing
///   table files should always spell the name out.
///
/// Field attributes:
/// - `#[table(default = "image")]` — the enum variant used for rows written by
///   an older app version (and by `ALTER TABLE ... DEFAULT`), mirroring Zig's
///   field default (`kind: Kind = .image`). Without it the enum's first variant
///   (`SqlEnum::FIRST`) is used, like Zig's `defaultEnum` fallback.
///
/// The model must be a plain struct with named fields. `id` is the
/// auto-increment primary key (`INTEGER PRIMARY KEY AUTOINCREMENT`); an
/// `Option<i64>` `id` is a plain nullable column instead, like Zig.
#[proc_macro_derive(Table, attributes(table))]
pub fn derive_table(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match table::expand_table(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
