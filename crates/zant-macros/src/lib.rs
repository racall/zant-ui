//! Procedural macros for Zant UI.
//!
//! - `#[derive(TsxProps)]`: exposes a plain Rust options struct to the TSX
//!   compiler (field names / types / defaults). See docs/rust-port/tsx-codegen.md.
//! - `#[derive(Table)]`: generates the SQLite schema of `db/sqlite.zig`.
//!   See docs/rust-port/plan.md WP-3.10.
use proc_macro::TokenStream;
use syn::parse_macro_input;

mod table;
mod tsx_props;

/// `#[derive(TsxProps)]`: compile-time reflection of a `ui` option struct for
/// the TSX compiler (`docs/rust-port/tsx-codegen.md` §5, WP-5.2).
///
/// It emits `impl zant::ui::options::tsx_props::TsxProps for T` with the struct
/// name, doc comment, module path and every field in declaration order. Field
/// types are emitted as token strings; the `zant-tsx-tags` generator resolves
/// them against the `zant` sources and turns them into the TSX prop table
/// (`crates/zant-tsx/src/ui_tags.rs`), exactly like `tools/tsx/manifest/`
/// reflected the Zig `ui.*` structs at comptime.
///
/// Field attributes:
/// - `#[tsx(required)]` — the TSX prop must be supplied. In the Zig structs
///   those were the fields without a default value; `impl Default` hides that
///   in Rust, so it is spelled out. Also marks unsupported required fields, so
///   the generator can skip the whole tag like `reflect.zig` did.
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
