//! `JSON.parse` / `JSON.stringify`, typed the way Swift's `Codable` or
//! Rust's serde are: every generated struct, string enum and union
//! implements `tsx::json::Json`, and
//!
//! - `JSON.stringify(value, null?, indent?)` writes exactly what Node writes
//!   (field order, number format, escapes, indentation);
//! - `JSON.parse(text)` decodes into the type its context gives —
//!   `JSON.parse(text) as Settings`, `const s: Settings = JSON.parse(text)`,
//!   an argument or return position. Invalid JSON throws a `SyntaxError`
//!   like JavaScript; JSON of the wrong shape throws a `TypeError` (where
//!   JavaScript would hand back an object that does not match its type).
//!
//! A TS optional field (`x?: T`) is left out when absent; `T | null` writes
//! `null`. A `Map` / `Set` (JavaScript writes `{}`), functions and callbacks
//! are compile errors with the conversion to use.

use crate::ast;

use super::expr;
use super::model::{self, InfoKind};
use super::rust_text;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// A field of a generated struct: its TS name, Rust name, TS-optional flag.
pub struct JsonField {
    pub name: String,
    pub rust: String,
    pub optional: bool,
}

/// `impl Json` for a generated struct (`generics`: `<T: Json, …>` bounds).
pub fn struct_impl(rust: &str, generics: &str, fields: &[JsonField]) -> String {
    let mut write = String::from("tsx::json::object_builder()");
    let mut read = String::new();
    for field in fields {
        let name = format!("{:?}", field.name);
        let method = if field.optional { "optional" } else { "field" };
        write.push_str(&format!(".{method}({name}, &self.{})", field.rust));
        let reader = if field.optional {
            "read_optional"
        } else {
            "read_field"
        };
        read.push_str(&format!(
            "{}: tsx::json::{reader}(value, {name}, path)?,\n",
            field.rust
        ));
    }
    let (params, bounds) = split_generics(generics);
    super::parse_and_format(&format!(
        "impl{bounds} tsx::json::Json for {rust}{params} {{\n\
         fn to_json(&self) -> Option<tsx::json::JsonValue> {{\n\
         Some({write}.build())\n}}\n\
         fn from_json(value: &tsx::json::JsonValue, path: &mut tsx::json::JsonPath) -> Result<Self, String> {{\n\
         if !matches!(value, tsx::json::JsonValue::Object(_)) {{\n\
         return Err(path.expected(\"an object\", value));\n\
         }}\n\
         #[allow(clippy::needless_update)]\n\
         Ok(Self {{\n{read}..Default::default()\n}})\n}}\n}}\n\n"
    ))
}

/// `<T, U>` plus `<T: tsx::json::Json + Default + Clone, …>` from a list of
/// type parameter names (`"T, U"`; empty for a plain struct).
fn split_generics(generics: &str) -> (String, String) {
    if generics.trim().is_empty() {
        return (String::new(), String::new());
    }
    let names: Vec<&str> = generics.split(',').map(str::trim).collect();
    let bounds: Vec<String> = names
        .iter()
        .map(|name| format!("{name}: tsx::json::Json + Default + Clone"))
        .collect();
    (
        format!("<{}>", names.join(", ")),
        format!("<{}>", bounds.join(", ")),
    )
}

/// `impl Json` for a string-literal enum (`"a" | "b"`).
pub fn enum_impl(rust: &str, members: &[(String, String)]) -> String {
    let mut cases = String::new();
    for (text, variant) in members {
        cases.push_str(&format!("{text:?} => Ok(Self::{variant}),\n"));
    }
    super::parse_and_format(&format!(
        "impl tsx::json::Json for {rust} {{\n\
         fn to_json(&self) -> Option<tsx::json::JsonValue> {{\n\
         Some(tsx::json::JsonValue::String(zant::Str::Static(self.as_str())))\n}}\n\
         fn from_json(value: &tsx::json::JsonValue, path: &mut tsx::json::JsonPath) -> Result<Self, String> {{\n\
         let text: zant::Str = tsx::json::Json::from_json(value, path)?;\n\
         match text.as_str() {{\n{cases}_ => Err(path.expected(\"one of the type's strings\", value)),\n}}\n}}\n}}\n\n"
    ))
}

/// `impl Json` for a union enum: written as its member; read by trying the
/// members in order (a discriminated member only matches its own tag).
pub fn union_impl(rust: &str, variants: &[(String, String)]) -> String {
    let mut write = String::new();
    let mut read = String::new();
    for (variant, spelled) in variants {
        write.push_str(&format!("{rust}::{variant}(value) => value.to_json(),\n"));
        read.push_str(&format!(
            "if let Ok(member) = <{spelled} as tsx::json::Json>::from_json(value, path) {{\nreturn Ok({rust}::{variant}(member));\n}}\n"
        ));
    }
    super::parse_and_format(&format!(
        "impl tsx::json::Json for {rust} {{\n\
         fn to_json(&self) -> Option<tsx::json::JsonValue> {{\n\
         use tsx::json::Json as _;\n\
         match self {{\n{write}}}\n}}\n\
         fn from_json(value: &tsx::json::JsonValue, path: &mut tsx::json::JsonPath) -> Result<Self, String> {{\n\
         {read}Err(path.expected(\"a value of one of the union's members\", value))\n}}\n}}\n\n"
    ))
}

/// Whether values of `ty` can go through JSON; the error names the fix.
pub fn check(g: &Generator<'_>, ty: Type) -> std::result::Result<(), String> {
    check_in(g, ty, &mut Vec::new())
}

/// `seen`: compound types being checked (recursive types: `Tree.children`).
fn check_in(g: &Generator<'_>, ty: Type, seen: &mut Vec<Type>) -> std::result::Result<(), String> {
    if seen.contains(&ty) {
        return Ok(());
    }
    if let Type::Compound(_) = ty {
        seen.push(ty);
    }
    if ty == Type::String || ty == Type::Boolean || ty == Type::Null || ty.numeric() {
        return Ok(());
    }
    if matches!(ty, Type::Void | Type::Node | Type::OptionalNode) {
        return Err("elements and `void` cannot be written as JSON".to_owned());
    }
    if super::exceptions::is_thrown(g, ty) || super::exceptions::is_error(g, ty) {
        return Ok(());
    }
    let Some(info) = model::info(g, ty) else {
        return Err("this type cannot be written as JSON".to_owned());
    };
    match info.kind {
        InfoKind::Array | InfoKind::Optional => check_in(g, info.child, seen),
        InfoKind::Enumeration => Ok(()),
        InfoKind::Tuple | InfoKind::Union => info
            .params
            .iter()
            .try_for_each(|member| check_in(g, *member, seen)),
        InfoKind::Record => check_in(g, info.child, seen),
        InfoKind::Object => {
            if info.rust == "zant::theme::tokens::Theme" {
                return Err("the theme cannot be written as JSON".to_owned());
            }
            if info.rust.starts_with("zant::tsx::Promise") || info.rust.starts_with("zant::tsx::fetch") {
                return Err("promises, responses and fetch objects are not data: write the values they hold".to_owned());
            }
            if info.rust.starts_with("zant::tsx::RegExp") || info.rust.contains("regexp::MatchArray") {
                return Err("JavaScript writes a RegExp as `{}`; store its `source` and `flags`".to_owned());
            }
            info.fields.iter().try_for_each(|field| {
                check_in(g, field.ty, seen).map_err(|why| {
                    format!("field `{}`: {why}", String::from_utf8_lossy(&field.name))
                })
            })
        }
        InfoKind::Map => Err(
            "JavaScript writes a Map as `{}`; convert it first: `Object.fromEntries(map)` (and `new Map(Object.entries(…))` back)"
                .to_owned(),
        ),
        InfoKind::Set => Err(
            "JavaScript writes a Set as `{}`; convert it first: `[...set]` (and `new Set(…)` back)"
                .to_owned(),
        ),
        InfoKind::Callback | InfoKind::Function => {
            Err("functions are left out of JSON; keep data and functions apart".to_owned())
        }
    }
}

/// An expression whose type comes from its context (`as T`, an annotation):
/// `JSON.parse(…)`.
pub fn takes_context(e: &ast::Expr) -> bool {
    matches!(&e.data, ast::ExprData::Call(call) if json_member(call) == Some(b"parse".as_slice()))
}

fn json_member(call: &ast::Call) -> Option<&[u8]> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        return None;
    };
    let ast::ExprData::Ident(object) = &member.object.data else {
        return None;
    };
    (object == b"JSON").then_some(member.name.name.as_slice())
}

/// `JSON.stringify(…)` / `JSON.parse(…)`; `None` for other calls.
pub fn call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    expected: Option<Type>,
) -> Result<Option<Value>> {
    let Some(name) = json_member(call) else {
        return Ok(None);
    };
    if scope::local(c, b"JSON").is_some() {
        return Ok(None);
    }
    match name {
        b"stringify" => stringify(g, c, loc, call).map(Some),
        b"parse" => parse(g, c, loc, call, expected).map(Some),
        _ => fail(g, loc, "JSON has `parse` and `stringify`"),
    }
}

fn stringify(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Value> {
    if call.args.is_empty() || call.args.len() > 3 {
        return fail(g, loc, "`JSON.stringify(value, null, indent?)`");
    }
    if let Some(replacer) = call.args.get(1)
        && !matches!(replacer.data, ast::ExprData::NullLit)
        && !matches!(&replacer.data, ast::ExprData::Ident(n) if n == b"undefined")
    {
        return fail(
            g,
            replacer.loc,
            "a replacer is not supported; pass `null` and shape the value before writing it",
        );
    }
    super::place::project(c, &call.args[0]);
    let value = expr::emit(g, c, &call.args[0], None, false)?;
    let value = types::concrete(g, value)?;
    if let Err(why) = check(g, value.ty) {
        return fail(g, call.args[0].loc, why);
    }
    if value.ty == Type::Null {
        return Ok(Value::new(Type::String, "zant::Str::Static(\"null\")"));
    }
    let indent = match call.args.get(2) {
        None => "None".to_owned(),
        Some(arg) => {
            let indent = expr::emit(g, c, arg, None, false)?;
            if indent.ty == Type::String {
                format!(
                    "Some({}.as_str())",
                    rust_text::operand(&indent, rust_text::Precedence::Suffix, false)
                )
            } else if indent.ty.numeric() {
                let count = types::coerce(g, indent, Type::F64, arg.loc)?;
                format!(
                    "Some(\" \".repeat(({}).clamp(0.0, 10.0) as usize).as_str())",
                    count.code
                )
            } else {
                return fail(g, arg.loc, "the indent is a number of spaces or a string");
            }
        }
    };
    Ok(Value::new(
        Type::String,
        format!(
            "tsx::json::stringify_value(&{}, {indent}).unwrap_or_default()",
            rust_text::operand(&value, rust_text::Precedence::Suffix, false)
        ),
    ))
}

fn parse(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    expected: Option<Type>,
) -> Result<Value> {
    if call.args.len() != 1 {
        return fail(
            g,
            loc,
            "`JSON.parse(text)` (a reviver is not supported; convert the value after reading it)",
        );
    }
    let Some(target) = expected.filter(|ty| !matches!(ty, Type::Void | Type::Node)) else {
        return fail(
            g,
            loc,
            "JSON.parse needs the type to read: `JSON.parse(text) as Settings` or `const s: Settings = JSON.parse(text)`",
        );
    };
    let target = target.concrete();
    if let Err(why) = check(g, target) {
        return fail(g, loc, why);
    }
    let text = expr::emit(g, c, &call.args[0], Some(Type::String), false)?;
    let text = types::coerce(g, text, Type::String, call.args[0].loc)?;
    let value = Value::new(
        target,
        format!(
            "tsx::json::parse_as::<{}>({}.as_str())",
            target.rust(g),
            rust_text::operand(&text, rust_text::Precedence::Suffix, false)
        ),
    );
    super::exceptions::checked_call(g, c, value, loc)
}
