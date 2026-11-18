//! Typed JS built-ins, isolated from JSX and the general expression emitter.
//! The runtime helpers are `tsx::builtins`.
//!
//! The operation is passed as an enum value:
//! `Math.floor(x)` → `tsx::builtins::unary(Unary::Floor, x)`.

use crate::ast;

use super::expr;
use super::model;
use super::rust_text::Precedence;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod math;

use self::math::*;

const MATH_NAMES: &str = "floor, ceil, round, trunc, abs, sign, sqrt, cbrt, pow, exp, log, log2, log10, \
sin, cos, tan, atan, atan2, hypot, min, max, random, PI, E";

const NUMBER_NAMES: &str = "isNaN, isFinite, isInteger, MAX_SAFE_INTEGER, MIN_SAFE_INTEGER, EPSILON, MAX_VALUE, MIN_VALUE, NaN, POSITIVE_INFINITY, NEGATIVE_INFINITY";

const UNARY_NAMES: [&str; 16] = [
    "floor", "ceil", "round", "trunc", "abs", "sign", "sqrt", "cbrt", "exp", "log", "log2",
    "log10", "sin", "cos", "tan", "atan",
];

pub fn shadowed(g: &mut Generator<'_>, c: &mut scope::Context, name: &str) -> bool {
    if scope::local(c, name.as_bytes()).is_some()
        || scope::state_index(g, name.as_bytes()).is_some()
        || scope::global_name(g, name.as_bytes())
    {
        return true;
    }
    if let Some(parent) = c.capture_source.clone()
        && capture_source_shadowed(&parent, name)
    {
        return true;
    }
    (g.extensions.component_shadow)(g, name)
}

fn capture_source_shadowed(source: &scope::CaptureSource, name: &str) -> bool {
    if source.names.iter().any(|local| local == name) {
        return true;
    }
    match source.parent.as_ref() {
        Some(parent) => capture_source_shadowed(parent, name),
        None => false,
    }
}

fn object_name(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    object: &ast::Expr,
) -> Option<String> {
    let ast::ExprData::Ident(name) = &object.data else {
        return None;
    };
    let name = String::from_utf8_lossy(name).into_owned();
    if shadowed(g, c, &name) {
        return None;
    }
    for known in ["Math", "Number", "Date", "performance"] {
        if name == known {
            return Some(name);
        }
    }
    None
}

fn unknown<T>(g: &mut Generator<'_>, object: &str, name: &ast::Name) -> Result<T> {
    let supported = if object == "Math" {
        MATH_NAMES
    } else if object == "Number" {
        NUMBER_NAMES
    } else {
        "now (full Date objects are out of scope)"
    };
    fail(
        g,
        name.loc,
        format!(
            "unknown {object} member `{}`; supported: {supported}",
            String::from_utf8_lossy(&name.name)
        ),
    )
}

/// Resolve numeric constants without treating the built-in object as a
/// runtime value.
pub fn member(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    m: &ast::Member,
) -> Result<Option<Value>> {
    let Some(object) = object_name(g, c, &m.object) else {
        return Ok(None);
    };
    let name = String::from_utf8_lossy(&m.name.name).into_owned();
    let code: Option<&str> = if object == "Math" {
        match name.as_str() {
            "PI" => Some("std::f64::consts::PI"),
            "E" => Some("std::f64::consts::E"),
            _ => None,
        }
    } else if object == "Number" {
        match name.as_str() {
            "MAX_SAFE_INTEGER" => Some("9_007_199_254_740_991f64"),
            "MIN_SAFE_INTEGER" => Some("-9_007_199_254_740_991f64"),
            "EPSILON" => Some("f64::EPSILON"),
            "MAX_VALUE" => Some("f64::MAX"),
            "MIN_VALUE" => Some("5e-324f64"),
            "NaN" => Some("f64::NAN"),
            "POSITIVE_INFINITY" => Some("f64::INFINITY"),
            "NEGATIVE_INFINITY" => Some("f64::NEG_INFINITY"),
            _ => None,
        }
    } else {
        None
    };
    match code {
        Some(code) => Ok(Some(Value::new(Type::F64, code))),
        None => unknown(g, &object, &m.name),
    }
}

fn arity(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    call: &ast::Call,
    min: usize,
    max: usize,
) -> Result<()> {
    if !call.type_args.is_empty() {
        return fail(g, loc, "built-ins do not take type arguments");
    }
    if call.args.len() < min || call.args.len() > max {
        return fail(g, loc, format!("built-in expects {min}..{max} arguments"));
    }
    Ok(())
}

fn numeric(g: &mut Generator<'_>, value: &Value, loc: ast::Loc) -> Result<()> {
    if !value.ty.numeric() {
        return fail(g, loc, "built-in requires a number");
    }
    Ok(())
}

/// Typed locals guarantee receiver-first, left-to-right, exactly-once
/// evaluation.
#[derive(Default)]
struct Call {
    setup: Vec<String>,
    args: Vec<String>,
}

impl Call {
    fn add(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        v: Value,
        ty: Type,
        loc: ast::Loc,
    ) -> Result<()> {
        let value = types::coerce(g, v, ty, loc)?;
        let name = super::aggregate::temporary(g, c, b"_arg")?;
        self.setup.push(format!(
            "let {name}: {} = {};",
            ty.rust(g),
            if ty.float() && value.code == "-0" {
                "-0.0".to_owned()
            } else {
                value.code
            }
        ));
        self.args.push(name);
        Ok(())
    }

    fn finish(&self, ty: Type, code: String) -> Value {
        Value {
            ty,
            code: format!("{{\n{}\n{code}\n}}", self.setup.join("\n")),
            precedence: Precedence::Conditional,
            ..Value::default()
        }
    }

    fn joined(&self) -> String {
        self.args.join(", ")
    }
}

/// Dispatch globals, built-in object calls, and numeric prototype methods.
pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    if let ast::ExprData::Ident(name) = &call.callee.data {
        let name = String::from_utf8_lossy(name).into_owned();
        if shadowed(g, c, &name) {
            return Ok(None);
        }
        if name == "Date" {
            return fail(g, loc, "full Date objects are out of scope; use Date.now()");
        }
        if matches!(
            name.as_str(),
            "Number" | "String" | "parseInt" | "parseFloat"
        ) {
            return Ok(Some(global(g, c, loc, call, &name)?));
        }
        return Ok(None);
    }
    let ast::ExprData::Member(m) = &call.callee.data else {
        return Ok(None);
    };
    if let Some(object) = object_name(g, c, &m.object) {
        if object == "Math" {
            return Ok(Some(math(g, c, loc, call)?));
        }
        if object == "Number"
            && let Some(value) = number_predicate(g, c, loc, call, &m.name)?
        {
            return Ok(Some(value));
        }
        if m.name.name == b"now" {
            arity(g, loc, call, 0, 0)?;
            if c.constant {
                return fail(g, loc, "clocks cannot run in a top-level constant");
            }
            return Ok(Some(Value::new(
                Type::F64,
                if object == "Date" {
                    "tsx::builtins::date_now()"
                } else {
                    "tsx::builtins::performance_now()"
                },
            )));
        }
        return unknown(g, &object, &m.name);
    }
    if m.name.name != b"toFixed" && m.name.name != b"toString" {
        return Ok(None);
    }
    let value = expr::emit(g, c, &m.object, None, false)?;
    if model::optional_child(g, value.ty).is_some() {
        return fail(
            g,
            loc,
            "optional value needs ?? or ! before a numeric method",
        );
    }
    numeric(g, &value, m.object.loc)?;
    if m.optional {
        return fail(
            g,
            loc,
            "optional numeric calls are not supported; use ?? or !",
        );
    }
    let fixed = m.name.name == b"toFixed";
    arity(g, loc, call, 0, if fixed { 1 } else { 0 })?;
    if c.constant {
        return fail(g, loc, "numeric strings cannot run in a top-level constant");
    }
    let mut builder = Call::default();
    let receiver_type = if fixed {
        Type::F64
    } else {
        value.ty.concrete()
    };
    builder.add(g, c, value, receiver_type, m.object.loc)?;
    if fixed {
        if let Some(arg) = call.args.first() {
            let digits = expr::emit(g, c, arg, Some(Type::F64), false)?;
            builder.add(g, c, digits, Type::F64, arg.loc)?;
        } else {
            builder.args.push("0.0".to_owned());
        }
    }
    let helper = if fixed { "to_fixed" } else { "string" };
    Ok(Some(builder.finish(
        Type::String,
        format!("tsx::builtins::{helper}({})", builder.joined()),
    )))
}
