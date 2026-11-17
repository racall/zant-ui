//! `RegExp`: literals (`/a+/g`, parsed as `new RegExp("a+", "g")`),
//! `new RegExp(text, flags)`, `test` / `exec`, the match arrays and the
//! string methods that take a pattern (`match`, `matchAll`, `search`,
//! `replace`, `replaceAll`, `split`). The engine is `regress` (ECMAScript
//! semantics; `zant::tsx::regexp`).
//!
//! A literal pattern is checked when compiling (a bad one is a compile
//! error) and compiled once per site, like JS engines cache literals: each
//! evaluation is a new object (its own `lastIndex`) sharing the program.
//! `new RegExp(text)` with a computed pattern can throw a `SyntaxError`.
//!
//! Differences from JavaScript, all from typing: `m[i]` is a `string` (as
//! TypeScript types it) and an unmatched capture reads as `""`; a global
//! `match` result has no `index` (NaN). `matchAll` / `replaceAll` with a
//! non-global pattern held in a variable report the TypeError as uncaught
//! and yield nothing (a literal without `g` is a compile error).

use crate::ast;

use super::aggregate;
use super::expr;
use super::model::{self, Info, InfoKind};
use super::rust_text::{self, Precedence};
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

const REGEXP: &str = "zant::tsx::RegExp";
const MATCH: &str = "zant::tsx::regexp::MatchArray";

fn find(g: &Generator<'_>, rust: &str) -> Option<Type> {
    g.compounds
        .iter()
        .position(|entry| entry.rust == rust)
        .map(|index| Type::Compound(index as u32))
}

fn opaque(g: &mut Generator<'_>, rust: &str) -> Result<Type> {
    if let Some(ty) = find(g, rust) {
        return Ok(ty);
    }
    model::add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: rust.to_owned(),
            ..Info::default()
        },
    )
}

pub fn regexp_type(g: &mut Generator<'_>) -> Result<Type> {
    opaque(g, REGEXP)
}

pub fn match_type(g: &mut Generator<'_>) -> Result<Type> {
    opaque(g, MATCH)
}

pub fn is_regexp(g: &Generator<'_>, ty: Type) -> bool {
    find(g, REGEXP) == Some(ty)
}

pub fn is_match(g: &Generator<'_>, ty: Type) -> bool {
    find(g, MATCH) == Some(ty)
}

/// `RegExp` / `RegExpMatchArray` / `RegExpExecArray` in a type position.
pub fn named_type(g: &mut Generator<'_>, name: &[u8]) -> Result<Option<Type>> {
    if g.type_decls.iter().any(|decl| decl.name.name == name) {
        return Ok(None);
    }
    match name {
        b"RegExp" => regexp_type(g).map(Some),
        b"RegExpMatchArray" | b"RegExpExecArray" => match_type(g).map(Some),
        _ => Ok(None),
    }
}

/// The literal text of a string argument.
fn literal(e: &ast::Expr) -> Option<String> {
    match &e.data {
        ast::ExprData::String(bytes) => String::from_utf8(bytes.clone()).ok(),
        _ => None,
    }
}

/// `new RegExp(pattern, flags)` whose arguments are all string literals
/// (a regex literal): `Some((pattern, flags))`.
pub fn literal_regexp(call: &ast::Call) -> Option<(String, String)> {
    if !matches!(&call.callee.data, ast::ExprData::Ident(n) if n == b"RegExp") {
        return None;
    }
    let pattern = literal(call.args.first()?)?;
    let flags = match call.args.get(1) {
        Some(arg) => literal(arg)?,
        None => String::new(),
    };
    (call.args.len() <= 2).then_some((pattern, flags))
}

/// Whether `call` builds a RegExp from a computed pattern (it can throw a
/// `SyntaxError`; `throws` asks).
pub fn computed_construction(call: &ast::Call) -> bool {
    matches!(&call.callee.data, ast::ExprData::Ident(n) if n == b"RegExp")
        && literal_regexp(call).is_none()
}

/// A literal pattern, checked now and compiled once per site.
fn cached(g: &mut Generator<'_>, loc: ast::Loc, pattern: &str, flags: &str) -> Result<Value> {
    // the runtime's `RegExp` constructor compiles with the same check
    if let Err(message) = zant_base::regexp::compile(pattern, flags) {
        return fail(g, loc, format!("SyntaxError: {message}"));
    }
    let ty = regexp_type(g)?;
    Ok(Value::new(
        ty,
        format!(
            "{{ thread_local! {{ static _REGEXP: {REGEXP} = {REGEXP}::new({pattern:?}, {flags:?}).expect(\"checked when compiled\"); }} _REGEXP.with({REGEXP}::fresh) }}"
        ),
    ))
}

/// A pattern made from a string literal (`s.match("a.b")`: JavaScript
/// treats the text as a pattern).
pub fn from_text(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    pattern: &str,
    flags: &str,
) -> Result<Value> {
    cached(g, loc, pattern, flags)
}

/// `new RegExp(…)` / `RegExp(…)`; `None` for other calls.
pub fn construct(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return Ok(None);
    };
    if name != b"RegExp" || scope::local(c, name).is_some() {
        return Ok(None);
    }
    if call.args.is_empty() || call.args.len() > 2 {
        return fail(g, loc, "`new RegExp(pattern, flags?)`");
    }
    if let Some((pattern, flags)) = literal_regexp(call) {
        return cached(g, loc, &pattern, &flags).map(Some);
    }
    let mut parts = Vec::new();
    for arg in &call.args {
        let value = expr::emit(g, c, arg, Some(Type::String), false)?;
        if is_regexp(g, value.ty) {
            return fail(
                g,
                arg.loc,
                "copy a RegExp with `new RegExp(re.source, re.flags)`",
            );
        }
        let value = types::coerce(g, value, Type::String, arg.loc)?;
        parts.push(format!(
            "{}.as_str()",
            rust_text::operand(&value, Precedence::Suffix, false)
        ));
    }
    if parts.len() == 1 {
        parts.push("\"\"".to_owned());
    }
    let ty = regexp_type(g)?;
    let value = Value::new(ty, format!("{REGEXP}::new({})", parts.join(", ")));
    super::exceptions::checked_call(g, c, value, loc).map(Some)
}

/// `re.source`, `m.index`, `m.groups`, …; `None` for other types.
pub fn member_read(
    g: &mut Generator<'_>,
    ty: Type,
    operand: &str,
    name: &ast::Name,
) -> Result<Option<Value>> {
    let field = String::from_utf8_lossy(&name.name).into_owned();
    if is_regexp(g, ty) {
        let (ty, method) = match field.as_str() {
            "source" => (Type::String, "source"),
            "flags" => (Type::String, "flags"),
            "global" => (Type::Boolean, "global"),
            "ignoreCase" => (Type::Boolean, "ignore_case"),
            "multiline" => (Type::Boolean, "multiline"),
            "sticky" => (Type::Boolean, "sticky"),
            "unicode" => (Type::Boolean, "unicode"),
            "dotAll" => (Type::Boolean, "dot_all"),
            "hasIndices" => (Type::Boolean, "has_indices"),
            "lastIndex" => (Type::F64, "last_index"),
            _ => {
                return fail(
                    g,
                    name.loc,
                    format!(
                        "RegExp has no `{field}` (source, flags, global, ignoreCase, multiline, sticky, unicode, dotAll, hasIndices, lastIndex)"
                    ),
                );
            }
        };
        return Ok(Some(Value::new(ty, format!("{operand}.{method}()"))));
    }
    if is_match(g, ty) {
        return match field.as_str() {
            "index" => Ok(Some(Value::new(Type::F64, format!("{operand}.index()")))),
            "length" => Ok(Some(Value::new(Type::F64, format!("{operand}.len()")))),
            "input" => Ok(Some(Value::new(Type::String, format!("{operand}.input()")))),
            "groups" => {
                let record = super::dicts::record_of(g, Type::String, Type::String)?;
                let ty = model::optional(g, record)?;
                Ok(Some(Value::new(ty, format!("{operand}.groups()"))))
            }
            _ => fail(
                g,
                name.loc,
                format!("a match has no `{field}` (index, input, groups, length, m[i])"),
            ),
        };
    }
    Ok(None)
}

/// `m[i]` on a match: a `string` (an unmatched capture reads as "").
pub fn index(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    object: &Value,
    position: &ast::Expr,
) -> Result<Option<Value>> {
    if !is_match(g, object.ty) {
        return Ok(None);
    }
    let at = expr::emit(g, c, position, Some(Type::F64), false)?;
    let at = types::coerce(g, at, Type::F64, position.loc)?;
    Ok(Some(Value::new(
        Type::String,
        format!(
            "{}.get({}).unwrap_or_default()",
            rust_text::operand(object, Precedence::Suffix, false),
            at.code
        ),
    )))
}

/// `re.test(s)` / `re.exec(s)`, and array methods on a match
/// (`m.map(…)`, `m.slice(1)`); `None` for other receivers.
pub fn method(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    receiver: &Value,
) -> Result<Option<Value>> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        return Ok(None);
    };
    let name = member.name.name.as_slice();
    if is_match(g, receiver.ty) {
        // a match is an array of strings
        let list = model::array(g, Type::String)?;
        let as_list = Value::new(
            list,
            format!(
                "{}.to_list()",
                rust_text::operand(receiver, Precedence::Suffix, false)
            ),
        );
        return super::array_methods::emit_on(g, c, loc, call, None, as_list).map(Some);
    }
    if !is_regexp(g, receiver.ty) {
        return Ok(None);
    }
    if call.args.len() != 1 || !matches!(name, b"test" | b"exec") {
        return fail(
            g,
            loc,
            "RegExp methods: `re.test(text)`, `re.exec(text)`; strings have match / matchAll / replace / split / search",
        );
    }
    let text = expr::emit(g, c, &call.args[0], Some(Type::String), false)?;
    let text = types::coerce(g, text, Type::String, call.args[0].loc)?;
    let re = rust_text::operand(receiver, Precedence::Suffix, false);
    let text = rust_text::operand(&text, Precedence::Suffix, false);
    if name == b"test" {
        return Ok(Some(Value::new(
            Type::Boolean,
            format!("{re}.test(&{text})"),
        )));
    }
    let found = match_type(g)?;
    let ty = model::optional(g, found)?;
    Ok(Some(Value::new(
        ty,
        format!("zant::tsx::regexp::exec(&{re}, &{text})"),
    )))
}

/// `re.lastIndex = n`: the one writable property; `None` for other targets.
pub fn assign(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    target: &ast::Expr,
    value: &ast::Expr,
) -> Result<Option<Value>> {
    let ast::ExprData::Member(member) = &target.data else {
        return Ok(None);
    };
    if member.name.name != b"lastIndex" {
        return Ok(None);
    }
    super::place::project(c, &member.object);
    let object = expr::emit(g, c, &member.object, None, false)?;
    if !is_regexp(g, object.ty) {
        return Ok(None);
    }
    let index = expr::emit(g, c, value, Some(Type::F64), false)?;
    let index = types::coerce(g, index, Type::F64, value.loc)?;
    Ok(Some(Value::new(
        Type::Void,
        format!(
            "{}.set_last_index({})",
            rust_text::operand(&object, Precedence::Suffix, false),
            index.code
        ),
    )))
}

/// The string methods that take a pattern.
pub fn takes_pattern(name: &str) -> bool {
    matches!(
        name,
        "match" | "matchAll" | "search" | "replace" | "replaceAll" | "split"
    )
}

/// `s.match(re)` and the other pattern methods, once the first argument
/// is known to be a RegExp (or a literal pattern for `match` / `matchAll` /
/// `search`). `text` is the receiver, `re` the evaluated pattern.
pub fn string_method(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    name: &str,
    text: Value,
    re: Value,
) -> Result<Value> {
    let label = aggregate::temporary(g, c, b"_string")?;
    let source = aggregate::temporary(g, c, b"_text")?;
    let pattern = aggregate::temporary(g, c, b"_pattern")?;
    let mut setup = format!(
        "let {source}: zant::Str = {};\nlet {pattern}: {REGEXP} = {};\n",
        text.code, re.code
    );
    let global_literal = call
        .args
        .first()
        .and_then(|arg| match &arg.data {
            ast::ExprData::Call(inner) => literal_regexp(inner),
            _ => None,
        })
        .map(|(_, flags)| flags.contains('g'));
    let (ty, body) = match name {
        "match" => {
            let found = match_type(g)?;
            (
                model::optional(g, found)?,
                format!("zant::tsx::regexp::match_array(&{source}, &{pattern})"),
            )
        }
        "matchAll" => {
            if global_literal == Some(false) {
                return fail(g, loc, "matchAll needs a global pattern: add the `g` flag");
            }
            let found = match_type(g)?;
            (
                model::array(g, found)?,
                format!(
                    "zant::tsx::regexp::match_all_arrays(&{source}, &{pattern}).unwrap_or_else(|thrown| {{ zant::tsx::error::uncaught(&thrown); Default::default() }})"
                ),
            )
        }
        "search" => (
            Type::F64,
            format!("zant::tsx::regexp::search(&{source}, &{pattern})"),
        ),
        "split" => {
            let limit = match call.args.get(1) {
                Some(arg) => {
                    let value = expr::emit(g, c, arg, Some(Type::F64), false)?;
                    let value = types::coerce(g, value, Type::F64, arg.loc)?;
                    format!("Some({})", value.code)
                }
                None => "None".to_owned(),
            };
            (
                model::array(g, Type::String)?,
                format!("zant::tsx::regexp::split(&{source}, &{pattern}, {limit})"),
            )
        }
        "replace" | "replaceAll" => {
            let all = name == "replaceAll";
            if all && global_literal == Some(false) {
                return fail(
                    g,
                    loc,
                    "replaceAll needs a global pattern: add the `g` flag",
                );
            }
            let Some(with) = call.args.get(1) else {
                return fail(g, loc, format!("`{name}(pattern, replacement)`"));
            };
            let (helper, argument) = if matches!(with.data, ast::ExprData::Arrow(_)) {
                let closure = replacer(g, c, call, with)?;
                (
                    if all {
                        "replace_all_with"
                    } else {
                        "replace_with"
                    },
                    closure,
                )
            } else {
                let value = expr::emit(g, c, with, Some(Type::String), false)?;
                let value = types::coerce(g, value, Type::String, with.loc)?;
                let temp = aggregate::temporary(g, c, b"_with")?;
                setup.push_str(&format!("let {temp}: zant::Str = {};\n", value.code));
                (
                    if all { "replace_all" } else { "replace" },
                    format!("{temp}.as_str()"),
                )
            };
            let mut code =
                format!("zant::tsx::regexp::{helper}(&{source}, &{pattern}, {argument})");
            if all {
                code.push_str(".unwrap_or_else(|thrown| { zant::tsx::error::uncaught(&thrown); Default::default() })");
            }
            (Type::String, code)
        }
        _ => return fail(g, loc, format!("`{name}` does not take a pattern")),
    };
    Ok(Value::new(
        ty,
        format!("'{label}: {{\n{setup}break '{label} {body};\n}}"),
    ))
}

/// `(match, p1, p2, …) => …` as the runtime's `FnMut(&RegExpMatch) -> Str`:
/// each parameter after the first is a capture (as a string).
fn replacer(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    call: &ast::Call,
    with: &ast::Expr,
) -> Result<String> {
    let ast::ExprData::Arrow(function) = &with.data else {
        unreachable!("replacer arrow")
    };
    let captures = call
        .args
        .first()
        .and_then(|arg| match &arg.data {
            ast::ExprData::Call(inner) => literal_regexp(inner),
            _ => None,
        })
        .map(|(pattern, _)| capture_count(&pattern));
    let groups = function.params.len().saturating_sub(1);
    match captures {
        Some(count) if groups > count => {
            return fail(
                g,
                with.loc,
                format!(
                    "the pattern has {count} capture group(s): the replacer takes (match, p1 … p{count}); offset and string arguments are not supported"
                ),
            );
        }
        None if groups > 0 => {
            return fail(
                g,
                with.loc,
                "capture parameters need a literal pattern (`/(\\w+)/g`) so their count is known",
            );
        }
        _ => {}
    }
    let arguments = vec![Type::String; function.params.len()];
    // the closure's parameters are the arrow's names (as `array_methods`)
    let pattern = function
        .params
        .iter()
        .map(|param| format!("{}: zant::Str", String::from_utf8_lossy(&param.name.name)))
        .collect::<Vec<_>>()
        .join(", ");
    let (closure, ty) =
        super::callbacks::closure(g, c, with, &arguments, &pattern, Some(Type::String))?;
    if ty != Type::String {
        return fail(g, with.loc, "the replacer returns a string");
    }
    let reads: Vec<String> = (0..arguments.len())
        .map(|index| format!("_found.get({index}.0).unwrap_or_default()"))
        .collect();
    Ok(format!(
        "|_found: &zant::tsx::RegExpMatch| ({closure})({})",
        reads.join(", ")
    ))
}

/// The number of capture groups of a pattern: `(` that is not `(?:`,
/// `(?=`, `(?!`, `(?<=`, `(?<!` (named groups `(?<name>` count), outside
/// character classes and escapes.
fn capture_count(pattern: &str) -> usize {
    let bytes = pattern.as_bytes();
    let mut count = 0;
    let mut index = 0;
    let mut in_class = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 1,
            b'[' => in_class = true,
            b']' => in_class = false,
            b'(' if !in_class => {
                let rest = &bytes[index + 1..];
                let plain = !rest.starts_with(b"?")
                    || (rest.starts_with(b"?<")
                        && !rest.starts_with(b"?<=")
                        && !rest.starts_with(b"?<!"));
                if plain {
                    count += 1;
                }
            }
            _ => {}
        }
        index += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::capture_count;

    #[test]
    fn capture_groups_are_counted_like_the_engine() {
        assert_eq!(0, capture_count(r"a+b"));
        assert_eq!(2, capture_count(r"(\d+)-(\d+)"));
        assert_eq!(1, capture_count(r"(?:x)(?<name>y)(?=z)(?!w)(?<=v)(?<!u)"));
        assert_eq!(1, capture_count(r"\((a)\)[(]"));
    }
}
