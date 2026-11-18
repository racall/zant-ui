//! String methods taking a regexp (`match`, `replace`, `split`, …) and replacer callbacks.

use super::*;

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
pub(super) fn replacer(
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
        crate::codegen::callbacks::closure(g, c, with, &arguments, &pattern, Some(Type::String))?;
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
pub(super) fn capture_count(pattern: &str) -> usize {
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
