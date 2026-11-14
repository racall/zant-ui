//! String calls use typed runtime helpers, preserving receiver / argument
//! evaluation order.
//!
//! The helpers are `zant::tsx::strings` (JS code-point semantics); the
//! generated call names are the snake_case Rust spellings of the JS methods
//! ([`rust_name`]). String results are `zant::Str` (runtime values are
//! copied).

use crate::ast;
use crate::string_method_names;

use super::aggregate;
use super::expr;
use super::model;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

#[derive(Clone, Copy, Debug)]
enum ResultKind {
    String,
    Boolean,
    Index,
    Array,
    Optional,
    Number,
    OptionalNumber,
}

#[derive(Clone, Copy, Debug)]
struct Method {
    min: usize,
    max: usize,
    numeric: bool,
    result: ResultKind,
}

const fn method(min: usize, max: usize, result: ResultKind) -> Method {
    Method {
        min,
        max,
        numeric: false,
        result,
    }
}

/// Every supported method with its arity.
fn lookup(name: &str) -> Option<Method> {
    let entry = match name {
        "includes" => method(1, 1, ResultKind::Boolean),
        "startsWith" => method(1, 1, ResultKind::Boolean),
        "endsWith" => method(1, 1, ResultKind::Boolean),
        "indexOf" => method(1, 1, ResultKind::Index),
        "lastIndexOf" => method(1, 1, ResultKind::Index),
        "trim" => method(0, 0, ResultKind::String),
        "trimStart" => method(0, 0, ResultKind::String),
        "trimEnd" => method(0, 0, ResultKind::String),
        "toLowerCase" => method(0, 0, ResultKind::String),
        "toUpperCase" => method(0, 0, ResultKind::String),
        "slice" => Method {
            min: 1,
            max: 2,
            numeric: true,
            result: ResultKind::String,
        },
        "substring" => Method {
            min: 1,
            max: 2,
            numeric: true,
            result: ResultKind::String,
        },
        "padStart" => Method {
            min: 1,
            max: 2,
            numeric: true,
            result: ResultKind::String,
        },
        "padEnd" => Method {
            min: 1,
            max: 2,
            numeric: true,
            result: ResultKind::String,
        },
        "repeat" => Method {
            min: 1,
            max: 1,
            numeric: true,
            result: ResultKind::String,
        },
        "replace" => method(2, 2, ResultKind::String),
        "replaceAll" => method(2, 2, ResultKind::String),
        "split" => method(1, 1, ResultKind::Array),
        "charAt" => Method {
            min: 1,
            max: 1,
            numeric: true,
            result: ResultKind::String,
        },
        "at" => Method {
            min: 1,
            max: 1,
            numeric: true,
            result: ResultKind::Optional,
        },
        "charCodeAt" => Method {
            min: 1,
            max: 1,
            numeric: true,
            result: ResultKind::Number,
        },
        "codePointAt" => Method {
            min: 1,
            max: 1,
            numeric: true,
            result: ResultKind::OptionalNumber,
        },
        _ => return None,
    };
    Some(entry)
}

/// The Rust helper name of a JS string method (`startsWith` →
/// `starts_with`), the contract with `zant::tsx::strings`.
pub fn rust_name(name: &str) -> String {
    match name {
        "toLowerCase" => return "to_lower_case".to_owned(),
        "toUpperCase" => return "to_upper_case".to_owned(),
        _ => {}
    }
    let mut out = String::with_capacity(name.len() + 4);
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Lower one known-string receiver, evaluating it and each argument exactly
/// once in source order.
pub fn emit(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    receiver: Value,
) -> Result<Value> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        unreachable!("string method call")
    };
    let name = String::from_utf8_lossy(&member.name.name).into_owned();
    let Some(method) = lookup(&name) else {
        return fail(
            g,
            member.name.loc,
            format!(
                "unknown string method `{name}`; supported: {}",
                string_method_names::SUPPORTED
            ),
        );
    };
    if !call.type_args.is_empty() {
        return fail(g, loc, "string methods do not take type arguments");
    }
    if c.constant {
        return fail(g, loc, "string methods cannot run in a top-level constant");
    }
    if call.args.len() < method.min || call.args.len() > method.max {
        return fail(
            g,
            loc,
            format!(
                "string method `{name}` expects {}..{} arguments",
                method.min, method.max
            ),
        );
    }
    let label = aggregate::temporary(g, c, b"_string")?;
    let source = aggregate::temporary(g, c, b"_text")?;
    let mut setup: Vec<String> = Vec::new();
    let mut args: Vec<String> = Vec::new();
    setup.push(format!("let {source}: zant::Str = {};", receiver.code));
    args.push(format!("&{source}"));
    let endpoints = name == "slice" || name == "substring";
    for (index, arg) in call.args.iter().enumerate() {
        let numeric = method.numeric && (index == 0 || endpoints);
        let ty = if numeric { Type::F64 } else { Type::String };
        let value = expr::emit(g, c, arg, Some(ty), false)?;
        let value = types::coerce(g, value, ty, arg.loc)?;
        let temp = aggregate::temporary(g, c, b"_arg")?;
        setup.push(format!("let {temp}: {} = {};", ty.rust(g), value.code));
        args.push(if ty == Type::String {
            format!("&{temp}")
        } else if endpoints && index == 1 {
            // `slice(start, end)` / `substring(start, end)`: the runtime's
            // end is `Option<f64>` (`None` when the argument is omitted)
            format!("Some({temp})")
        } else {
            temp
        });
    }
    if call.args.len() == 1 && method.max == 2 {
        // the second argument's `undefined` default: `None` for the slicing
        // endpoints, a single space for padding
        args.push(if endpoints {
            "None".to_owned()
        } else {
            "&zant::Str::Static(\" \")".to_owned()
        });
    }
    let ty = match method.result {
        ResultKind::String => Type::String,
        ResultKind::Boolean => Type::Boolean,
        ResultKind::Index => Type::Isize,
        ResultKind::Array => model::array(g, Type::String)?,
        ResultKind::Optional => model::optional(g, Type::String)?,
        ResultKind::Number => Type::F64,
        ResultKind::OptionalNumber => model::optional(g, Type::F64)?,
    };
    let helper = rust_name(&name);
    // a labeled block: `'label: { … break 'label value; }`
    let code = format!(
        "'{label}: {{\n{}\nbreak '{label} tsx::strings::{helper}({});\n}}",
        setup.join("\n"),
        args.join(", ")
    );
    let _ = loc;
    Ok(Value {
        ty,
        code,
        ..Value::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The helper-name contract with `zant::tsx::strings` is pinned here.
    #[test]
    fn helper_names_are_snake_case() {
        assert_eq!("starts_with", rust_name("startsWith"));
        assert_eq!("last_index_of", rust_name("lastIndexOf"));
        assert_eq!("to_lower_case", rust_name("toLowerCase"));
        assert_eq!("code_point_at", rust_name("codePointAt"));
        assert_eq!("replace_all", rust_name("replaceAll"));
        assert_eq!("trim", rust_name("trim"));
    }

    #[test]
    fn arities_match_the_method_table() {
        assert_eq!(1, lookup("includes").expect("includes").min);
        assert_eq!(2, lookup("replace").expect("replace").max);
        assert!(lookup("padStart").expect("padStart").numeric);
        assert!(lookup("concat").is_none());
    }
}
