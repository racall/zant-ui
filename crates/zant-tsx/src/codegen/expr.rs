//! Typed expressions, numeric operators, conditionals, strings, calls, and
//! `console.log`.
//!
//! Optionals lower to `Option` combinators and numbers to typed literals;
//! reads of non-`Copy` values clone.

use crate::ast;

use super::aggregate;
use super::builtins;
use super::functions;
use super::model;
use super::narrowing;
use super::rust_text::{self, Format, Precedence};
use super::scope;
use super::types::{self, Inference, Type, Value};
use super::{Generator, Result, fail};

const BOOLEAN_CONDITION_MESSAGE: &str =
    "conditions must be boolean; compare the value explicitly (for example, count > 0)";

pub fn condition(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<Value> {
    let v = emit(g, c, e, None, false)?;
    // JavaScript truthiness (`if (name)`, `!count`)
    super::truthy::test(g, v, e.loc)
}

pub fn emit(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    expected: Option<Type>,
    child: bool,
) -> Result<Value> {
    if let Some(t) = expected {
        types::check_literal(g, e, t)?;
    }
    let v = match &e.data {
        ast::ExprData::Number(n) => number(n),
        ast::ExprData::String(s) => Value {
            ty: Type::String,
            code: format!("zant::Str::from({})", rust_text::quote_at(g, s, e.loc)?),
            text: Some(s.clone()),
            ..Value::default()
        },
        ast::ExprData::Boolean(b) => Value::new(Type::Boolean, if *b { "true" } else { "false" }),
        ast::ExprData::NullLit => Value::new(Type::Null, "None"),
        ast::ExprData::Ident(n) => {
            // a plain function as a value where a function type is expected
            let function = match expected {
                Some(ty) if scope::local(c, n).is_none() => {
                    super::closures::named(g, n, ty, e.loc)?
                }
                _ => None,
            };
            match function {
                Some(value) => value,
                None => scope::read(g, c, n, e.loc)?,
            }
        }
        ast::ExprData::Template(t) => template(g, c, t)?,
        ast::ExprData::Unary(u) => unary(g, c, e.loc, u, expected)?,
        ast::ExprData::Binary(b) => binary(g, c, e.loc, b, expected, child)?,
        ast::ExprData::Cond(cond) => conditional(g, c, e.loc, cond, expected, child)?,
        ast::ExprData::Member(m) => aggregate::member(g, c, m)?,
        ast::ExprData::Call(call) => call_value(g, c, e.loc, call, expected)?,
        ast::ExprData::Jsx(jsx) => match g.extensions.hooks.emit_jsx {
            Some(hook) => hook(g, c, jsx)?,
            None => return fail(g, e.loc, "JSX needs the JSX emitter installed"),
        },
        ast::ExprData::Arrow(arrow) => {
            // a function value where a function type is expected
            match expected.filter(|ty| super::closures::signature(g, *ty).is_some()) {
                Some(ty) => super::closures::arrow(g, c, e.loc, arrow, ty)?,
                None => {
                    return fail(
                        g,
                        e.loc,
                        "an arrow needs a function type here (`const f: (x: number) => number = …`, or annotate its parameters and return type)",
                    );
                }
            }
        }
        ast::ExprData::Object(properties) => aggregate::object(g, c, e.loc, properties, expected)?,
        ast::ExprData::Assign(_) | ast::ExprData::Update(_) => {
            return fail(
                g,
                e.loc,
                "assignments and updates are only supported as statements",
            );
        }
        ast::ExprData::NonNull(inner) => aggregate::non_null(g, c, inner, expected, child)?,
        ast::ExprData::As(cast) => explicit_cast(g, c, e.loc, cast, child)?,
        ast::ExprData::Array(items) => match static_bytes_expected(g, expected) {
            Some(bytes) => byte_slice(g, c, items, bytes)?,
            None => aggregate::array(g, c, e.loc, items, expected)?,
        },
        ast::ExprData::Index(ix) => aggregate::index(g, c, e.loc, ix)?,
        ast::ExprData::Spread(_) => {
            return fail(
                g,
                e.loc,
                "`...` spreads into array literals, object literals and a function's rest parameter (`...items: T[]`)",
            );
        }
    };
    match expected {
        Some(t) => types::coerce(g, v, t, e.loc),
        None => Ok(v),
    }
}

/// Keep a numeric literal contextual until a consumer determines its type.
fn number(n: &[u8]) -> Value {
    let code = String::from_utf8_lossy(n).into_owned();
    Value {
        ty: if types::float_literal(n) {
            Type::FloatLiteral
        } else {
            Type::IntLiteral
        },
        code,
        inference: Inference::Literal,
        ..Value::default()
    }
}

/// The `Uint8Array` type when `expected` is it (optionally wrapped in `?`).
fn static_bytes_expected(g: &Generator<'_>, expected: Option<Type>) -> Option<Type> {
    let ty = expected?;
    let inner = model::optional_child(g, ty).unwrap_or(ty);
    model::is_static_bytes(g, inner).then_some(inner)
}

/// A `Uint8Array` literal becomes a promoted slice, `&[137u8, 80u8, …]`,
/// which coerces to `&'static [u8]`
/// (`const icon: Uint8Array = [137, 80, 78, 71]`). Only integer literals are
/// const-promotable, so anything else is a diagnostic instead of code that
/// would not compile.
fn byte_slice(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    items: &[Box<ast::Expr>],
    ty: Type,
) -> Result<Value> {
    let mut parts: Vec<String> = Vec::new();
    for item in items {
        if !matches!(item.data, ast::ExprData::Number(_)) {
            return fail(
                g,
                item.loc,
                "a Uint8Array literal contains only integer byte literals; build other bytes at runtime",
            );
        }
        let value = emit(g, c, item, Some(Type::U8), false)?;
        parts.push(value.code);
    }
    let code = if parts.is_empty() {
        "&[]".to_owned()
    } else {
        format!("&[{}]", parts.join(", "))
    };
    Ok(Value {
        ty,
        code,
        precedence: Precedence::Unary,
        ..Value::default()
    })
}

/// Check boolean negation or numeric signs without changing the operand type.
pub fn unary(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    u: &ast::Unary,
    expected: Option<Type>,
) -> Result<Value> {
    if u.op == b"delete" {
        return fail(g, loc, "`delete record[key]` is a statement");
    }
    if u.op == b"typeof" {
        return super::unions::type_of(g, c, &u.operand);
    }
    if u.op == b"!" {
        let x = condition(g, c, &u.operand)?;
        return Ok(Value {
            ty: Type::Boolean,
            code: format!("!{}", rust_text::operand(&x, Precedence::Unary, true)),
            precedence: Precedence::Unary,
            ..Value::default()
        });
    }
    let operand_context = if !matches!(u.operand.data, ast::ExprData::Number(_))
        && types::numeric_literals(&u.operand)
    {
        expected
    } else {
        None
    };
    let mut x = emit(g, c, &u.operand, operand_context, false)?;
    if x.inference == Inference::ResultLocation {
        x = types::concrete(g, x)?;
    }
    if model::optional_child(g, x.ty).is_some() {
        return fail(g, loc, "optional operand needs ?? or !");
    }
    if !x.ty.numeric() {
        return fail(
            g,
            loc,
            format!("unary {} requires a number", String::from_utf8_lossy(&u.op)),
        );
    }
    if u.op == b"-" && x.ty.integer() && x.ty.rust(g).starts_with('u') {
        return fail(g, loc, "cannot negate an unsigned value; use a signed type");
    }
    if u.op == b"+" {
        return Ok(x);
    }
    if u.op == b"~" {
        if x.ty.integer() && x.inference != Inference::Literal {
            return Ok(Value {
                ty: x.ty,
                code: format!("!{}", rust_text::operand(&x, Precedence::Unary, true)),
                precedence: Precedence::Unary,
                ..Value::default()
            });
        }
        let x = types::coerce(g, x, Type::F64, u.operand.loc)?;
        return Ok(Value::new(
            Type::F64,
            format!("tsx::builtins::bit_not({})", x.code),
        ));
    }
    Ok(Value {
        ty: x.ty,
        code: format!("-{}", rust_text::operand(&x, Precedence::Unary, true)),
        precedence: Precedence::Unary,
        inference: x.inference,
        ..Value::default()
    })
}

/// Unify branches and retain whether the result needs a concrete type.
pub fn conditional(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    cond: &ast::Cond,
    expected: Option<Type>,
    child: bool,
) -> Result<Value> {
    let test = condition(g, c, &cond.cond)?;
    let mut mark = narrowing::push(g, c, &cond.cond, true)?;
    let mut a = emit(g, c, &cond.then, expected, child)?;
    narrowing::restore(c, mark);
    mark = narrowing::push(g, c, &cond.cond, false)?;
    let mut b = emit(g, c, &cond.else_, expected, child)?;
    narrowing::restore(c, mark);
    let t = match expected {
        Some(t) => t,
        None => types::unify(g, a.ty, b.ty, loc)?.concrete(),
    };
    a = types::coerce(g, a, t, cond.then.loc)?;
    b = types::coerce(g, b, t, cond.else_.loc)?;
    let literal_branches = a.inference == Inference::Literal && b.inference == Inference::Literal;
    let needs_type = a.inference == Inference::ResultLocation
        || b.inference == Inference::ResultLocation
        || literal_branches;
    Ok(Value {
        ty: t,
        code: format!("if {} {{ {} }} else {{ {} }}", test.code, a.code, b.code),
        precedence: Precedence::Conditional,
        inference: if needs_type {
            Inference::ResultLocation
        } else {
            Inference::Known
        },
        ..Value::default()
    })
}

/// Unify numeric peers, reject truthiness, and lower strings and optional
/// children.
pub fn binary(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    b: &ast::Binary,
    expected: Option<Type>,
    child: bool,
) -> Result<Value> {
    if b.op == b"??" {
        return aggregate::coalesce(g, c, loc, b, expected);
    }
    if b.op == b"in" {
        return super::dicts::contains(g, c, &b.lhs, &b.rhs);
    }
    if b.op == b"instanceof" {
        return super::exceptions::instance_of(g, c, loc, b);
    }
    if b.op == b"&&" || b.op == b"||" {
        let lhs = emit(g, c, &b.lhs, None, false)?;
        // `name || "Untitled"`: a value, not a boolean
        if b.op == b"||" && lhs.ty != Type::Boolean {
            return super::truthy::or_value(g, c, b, lhs, expected);
        }
        let a = super::truthy::test(g, lhs.clone(), b.lhs.loc)?;
        // `peer && peer.online`: the right side runs only when the left is
        // true (|| false)
        let mark = narrowing::push(g, c, &b.lhs, b.op == b"&&")?;
        let rhs = emit(g, c, &b.rhs, None, child)?;
        narrowing::restore(c, mark);
        if child && b.op == b"&&" && (rhs.ty == Type::Node || rhs.ty == Type::OptionalNode) {
            return Ok(Value {
                ty: Type::OptionalNode,
                code: format!("if {} {{ Some({}) }} else {{ None }}", a.code, rhs.code),
                precedence: Precedence::Conditional,
                ..Value::default()
            });
        }
        if child && b.op == b"&&" && model::element(g, rhs.ty).is_some() {
            return Ok(Value {
                ty: rhs.ty,
                code: format!(
                    "if {} {{ {} }} else {{ tsx::List::from_vec(Vec::new()) }}",
                    a.code, rhs.code
                ),
                precedence: Precedence::Conditional,
                ..Value::default()
            });
        }
        // `{open && node.children?.map(…)}`: an optional list of elements
        if child
            && b.op == b"&&"
            && model::optional_child(g, rhs.ty)
                .is_some_and(|inner| model::element(g, inner).is_some())
        {
            return Ok(Value {
                ty: rhs.ty,
                code: format!("if {} {{ {} }} else {{ None }}", a.code, rhs.code),
                precedence: Precedence::Conditional,
                ..Value::default()
            });
        }
        if rhs.ty != Type::Boolean {
            // `user && user.name`
            if b.op == b"&&"
                && let Some(value) = super::truthy::and_value(g, &lhs, &a, rhs)?
            {
                return Ok(value);
            }
            return fail(g, b.rhs.loc, BOOLEAN_CONDITION_MESSAGE);
        }
        let op = if b.op == b"&&" { "&&" } else { "||" };
        return infix(g, a, op, rhs, Type::Boolean);
    }
    let numeric_context = expected.filter(|t| t.numeric());
    let lhs_context = if types::numeric_literals(&b.lhs) {
        let rhs_hint = types::numeric_hint(g, c, &b.rhs)?;
        types::numeric_context(g, c, &b.lhs, rhs_hint.or(numeric_context))?
    } else {
        None
    };
    let mut a = emit(g, c, &b.lhs, lhs_context, false)?;
    let rhs_context = if types::numeric_literals(&b.rhs) {
        types::numeric_context(
            g,
            c,
            &b.rhs,
            if a.ty.numeric() && !a.ty.literal() {
                Some(a.ty)
            } else {
                numeric_context
            },
        )?
    } else {
        None
    };
    let mut z = emit(g, c, &b.rhs, rhs_context, false)?;
    if equality_operator(&b.op) {
        if model::enum_type(g, a.ty) && z.ty == Type::String {
            z = types::coerce(g, z, a.ty, b.rhs.loc)?;
        }
        if model::enum_type(g, z.ty) && a.ty == Type::String {
            a = types::coerce(g, a, z.ty, b.lhs.loc)?;
        }
        if a.ty == Type::Null || z.ty == Type::Null {
            let other = if a.ty == Type::Null { &z } else { &a };
            if other.ty == Type::Null {
                let negated = b.op == b"!==" || b.op == b"!=";
                return Ok(Value::new(
                    Type::Boolean,
                    if negated { "false" } else { "true" },
                ));
            }
            if model::optional_child(g, other.ty).is_none() {
                return fail(g, loc, "null comparison needs an optional value");
            }
            let negated = b.op == b"!==" || b.op == b"!=";
            return Ok(Value {
                ty: Type::Boolean,
                code: format!(
                    "{}.{}()",
                    rust_text::operand(other, Precedence::Suffix, false),
                    if negated { "is_some" } else { "is_none" }
                ),
                precedence: Precedence::Suffix,
                ..Value::default()
            });
        }
        if model::optional_child(g, a.ty).is_some() || model::optional_child(g, z.ty).is_some() {
            return fail(g, loc, "optional operand needs ?? or !");
        }
        if model::enum_type(g, a.ty) && a.ty == z.ty {
            let op = if b.op == b"!==" || b.op == b"!=" {
                "!="
            } else {
                "=="
            };
            return infix(g, a, op, z, Type::Boolean);
        }
        if model::info(g, a.ty).is_some() || model::info(g, z.ty).is_some() {
            return fail(
                g,
                loc,
                "equality on structs and arrays is not supported; compare their fields",
            );
        }
    }
    if b.op == b"+" && (a.ty == Type::String || z.ty == Type::String) {
        let mut format = Format {
            compile_time: c.constant,
            ..Format::default()
        };
        format.value(g, a, loc)?;
        format.value(g, z, loc)?;
        return Ok(Value {
            ty: Type::String,
            code: format.finish(g, false)?,
            formatted: Some(std::rc::Rc::new(format)),
            ..Value::default()
        });
    }
    if model::optional_child(g, a.ty).is_some() || model::optional_child(g, z.ty).is_some() {
        return fail(g, loc, "optional operand needs ?? or !");
    }
    let equality = equality_operator(&b.op);
    let not_equal = b.op == b"!==" || b.op == b"!=";
    if equality && a.ty == Type::String && z.ty == Type::String {
        let a = super::loops::explicit_string(g, a)?;
        let z = super::loops::explicit_string(g, z)?;
        return infix(g, a, if not_equal { "!=" } else { "==" }, z, Type::Boolean);
    }
    if equality && a.ty == Type::Boolean && z.ty == Type::Boolean {
        return infix(g, a, if not_equal { "!=" } else { "==" }, z, Type::Boolean);
    }
    if !a.ty.numeric() || !z.ty.numeric() {
        return fail(
            g,
            loc,
            format!(
                "operator `{}` requires matching numbers",
                String::from_utf8_lossy(&b.op)
            ),
        );
    }
    let mut t = types::unify(g, a.ty, z.ty, loc)?.concrete();
    // `items.length - 1` is -1 for an empty list, as JavaScript numbers are
    // signed: unsigned operands subtract as numbers (an unsigned Rust `-`
    // would underflow), unless an integer result is asked for (`let n: u32`)
    if b.op == b"-"
        && t.unsigned()
        && !expected.is_some_and(|e| e.integer())
        && !(a.inference == Inference::Literal && z.inference == Inference::Literal)
    {
        t = Type::F64;
    }
    if let Some(value) = bitwise(g, &b.op, &a, &z, t, b)? {
        return Ok(value);
    }
    a = types::coerce(g, a, t, b.lhs.loc)?;
    z = types::coerce(g, z, t, b.rhs.loc)?;
    // A result location does not flow into binary operands. Conversions and
    // runtime conditionals need their own type.
    if a.inference == Inference::ResultLocation {
        a = types::concrete(g, a)?;
    }
    if z.inference == Inference::ResultLocation {
        z = types::concrete(g, z)?;
    }
    if t.float() && a.inference == Inference::Literal && z.inference == Inference::Literal {
        a = types::concrete(g, a)?;
    }
    let comparison = equality || relational_operator(&b.op);
    let op = if equality {
        if not_equal { "!=" } else { "==" }
    } else {
        std::str::from_utf8(&b.op).unwrap_or("==")
    };
    // Rust's integer `/` and `%` truncate toward zero.
    let result = infix(g, a, op, z, if comparison { Type::Boolean } else { t })?;
    if !comparison && t.integer() && (op == "/" || op == "%") {
        let mut result = result;
        result.inference = if result.inference == Inference::Literal {
            Inference::Literal
        } else {
            Inference::Known
        };
        return Ok(result);
    }
    Ok(result)
}

/// `**`, bitwise and shift operators. On numbers they follow JavaScript
/// (`tsx::builtins::bitwise`: 32-bit operands, shift counts modulo 32);
/// Rust integer values keep Rust's `&`, `|`, `^` and wrapping shifts.
fn bitwise(
    g: &mut Generator<'_>,
    op: &[u8],
    a: &Value,
    z: &Value,
    t: Type,
    b: &ast::Binary,
) -> Result<Option<Value>> {
    let function = match op {
        b"&" => "And",
        b"|" => "Or",
        b"^" => "Xor",
        b"<<" => "Shl",
        b">>" => "Shr",
        b">>>" => "Ushr",
        b"**" => "",
        _ => return Ok(None),
    };
    let both_literal = a.inference == Inference::Literal && z.inference == Inference::Literal;
    if t.integer() && !both_literal && op != b"**" && op != b">>>" {
        let a = types::coerce(g, a.clone(), t, b.lhs.loc)?;
        let z = types::coerce(g, z.clone(), t, b.rhs.loc)?;
        return match op {
            b"<<" | b">>" => {
                let method = if op == b"<<" {
                    "wrapping_shl"
                } else {
                    "wrapping_shr"
                };
                Ok(Some(Value::new(
                    t,
                    format!(
                        "{}.{method}(({}) as u32)",
                        rust_text::operand(&a, Precedence::Suffix, false),
                        z.code
                    ),
                )))
            }
            _ => infix(g, a, std::str::from_utf8(op).unwrap_or("&"), z, t).map(Some),
        };
    }
    let a = types::coerce(g, a.clone(), Type::F64, b.lhs.loc)?;
    let z = types::coerce(g, z.clone(), Type::F64, b.rhs.loc)?;
    let code = if function.is_empty() {
        format!("tsx::builtins::pow({}, {})", a.code, z.code)
    } else {
        format!(
            "tsx::builtins::bitwise(tsx::builtins::Bitwise::{function}, {}, {})",
            a.code, z.code
        )
    };
    Ok(Some(Value::new(Type::F64, code)))
}

/// Preserve the source grouping, including floating-point right operands and
/// non-associative comparisons, without wrapping complete initializers.
pub fn infix(g: &mut Generator<'_>, a: Value, op: &str, b: Value, t: Type) -> Result<Value> {
    let precedence = match op {
        "||" => Precedence::LogicalOr,
        "&&" => Precedence::LogicalAnd,
        "+" | "-" => Precedence::Sum,
        "*" | "/" | "%" => Precedence::Product,
        "|" => Precedence::BitOr,
        "^" => Precedence::BitXor,
        "&" => Precedence::BitAnd,
        _ => Precedence::Comparison,
    };
    let lhs = rust_text::operand(&a, precedence, precedence == Precedence::Comparison);
    let rhs = rust_text::operand(
        &b,
        precedence,
        precedence != Precedence::LogicalAnd && precedence != Precedence::LogicalOr,
    );
    let literal =
        t.integer() && a.inference == Inference::Literal && b.inference == Inference::Literal;
    let _ = g;
    Ok(Value {
        ty: t,
        code: format!("{lhs} {op} {rhs}"),
        precedence,
        inference: if literal {
            Inference::Literal
        } else {
            Inference::Known
        },
        ..Value::default()
    })
}

pub fn template(g: &mut Generator<'_>, c: &mut scope::Context, t: &ast::Template) -> Result<Value> {
    if t.exprs.is_empty() {
        let text = t.quasis.first().cloned().unwrap_or_default();
        return Ok(Value {
            ty: Type::String,
            code: format!("zant::Str::from({})", rust_text::quote_at(g, &text, 0)?),
            text: Some(text),
            ..Value::default()
        });
    }
    let mut format = Format {
        compile_time: c.constant,
        ..Format::default()
    };
    for (index, quasi) in t.quasis.iter().enumerate() {
        format.text(quasi);
        if let Some(expr) = t.exprs.get(index) {
            let value = emit(g, c, expr, None, false)?;
            format.value(g, value, expr.loc)?;
        }
    }
    let code = format.finish(g, false)?;
    Ok(Value {
        ty: Type::String,
        code,
        formatted: Some(std::rc::Rc::new(format)),
        ..Value::default()
    })
}

/// Reject explicit type arguments on a call with a concrete signature.
pub(crate) fn no_type_args(g: &mut Generator<'_>, loc: ast::Loc, call: &ast::Call) -> Result<()> {
    if !call.type_args.is_empty() {
        let name = match &call.callee.data {
            ast::ExprData::Ident(name) => name,
            ast::ExprData::Member(m) => &m.name.name,
            _ => b"function".as_slice(),
        };
        return fail(
            g,
            loc,
            format!(
                "`{}` takes no type arguments",
                String::from_utf8_lossy(name)
            ),
        );
    }
    Ok(())
}

pub fn call_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    expected: Option<Type>,
) -> Result<Value> {
    if let Some(error) = super::exceptions::construct(g, c, loc, call)? {
        return Ok(error);
    }
    if let Some(regexp) = super::regexps::construct(g, c, loc, call)? {
        return Ok(regexp);
    }
    if let Some(value) = super::promises::call(g, c, loc, call)? {
        return Ok(value);
    }
    if call.is_new {
        return super::dicts::construct(g, c, loc, call, expected);
    }
    if let Some(value) = super::closures::call_name(g, c, loc, call)? {
        return Ok(value);
    }
    for hook in g.extensions.call_before_functions.clone() {
        if let Some(value) = hook(g, c, loc, call)? {
            return Ok(value);
        }
    }
    if let Some(value) = functions::try_call(g, c, loc, call)? {
        return Ok(value);
    }
    for hook in g.extensions.call_before_builtins.clone() {
        if let Some(value) = hook(g, c, loc, call)? {
            return Ok(value);
        }
    }
    // the callback-prop emitter above handles `onSave?.()`; nothing else
    // here can be absent
    if call.optional {
        return fail(
            g,
            loc,
            "optional calls (`f?.()`) work on optional callback props (`onSave?.()`); call this function directly",
        );
    }
    if let Some(value) = super::json::call(g, c, loc, call, expected)? {
        return Ok(value);
    }
    if let Some(value) = super::dicts::object_call(g, c, loc, call)? {
        return Ok(value);
    }
    if let Some(value) = builtins::try_call(g, c, loc, call)? {
        return Ok(value);
    }
    no_type_args(g, loc, call)?;
    if matches!(call.callee.data, ast::ExprData::Member(_)) {
        return match g.extensions.array_methods {
            Some(hook) => hook(g, c, loc, call, expected),
            None => fail(
                g,
                loc,
                "array and string methods need the method emitter installed",
            ),
        };
    }
    let ast::ExprData::Ident(name) = &call.callee.data else {
        // `make(1)(2)`, `fns[0](x)`: a function value
        let callee = emit(g, c, &call.callee, None, false)?;
        if super::closures::signature(g, callee.ty).is_some() {
            return super::closures::call_value(g, c, loc, callee, &call.args);
        }
        return fail(
            g,
            loc,
            "only functions can be called; console.log is a statement",
        );
    };
    let name = String::from_utf8_lossy(name).into_owned();
    if scope::local(c, name.as_bytes()).is_none() && name == "useTheme" {
        if !c.render {
            return fail(
                g,
                loc,
                "useTheme() is read while rendering; capture the value in handlers",
            );
        }
        if !call.args.is_empty() {
            return fail(g, loc, "useTheme() takes no arguments");
        }
        return Ok(Value::new(model::theme_type(g)?, "ui::current_theme()"));
    }
    if scope::local(c, name.as_bytes()).is_some() {
        return fail(
            g,
            loc,
            format!("`{name}` is a local value, not a component function"),
        );
    }
    if super::hook_name(g, &name) {
        return fail(
            g,
            loc,
            "hooks must be called at the top level of the component",
        );
    }
    if let Some(value) = super::local_functions::call(g, c, loc, name.as_bytes(), call)? {
        return Ok(value);
    }
    if scope::setter_index(g, name.as_bytes()).is_some() {
        return fail(
            g,
            loc,
            if c.render {
                "state can only be set in event handlers"
            } else {
                "setter calls must be statements"
            },
        );
    }
    let Some(index) = scope::source_method(g, name.as_bytes()) else {
        return fail(g, loc, format!("unknown function `{name}`"));
    };
    let method = g.methods[index].clone();
    let mut receiver = "self".to_owned();
    let mut target = self_method_target(g, &name)?;
    if method.inline_only {
        let Some(hook) = g.extensions.hooks.nested else {
            return fail(
                g,
                loc,
                "component functions that read render locals need the component emitter installed",
            );
        };
        let nested = hook(g, c, loc, index)?;
        receiver = nested.receiver;
        target = nested.target;
    }
    let Some(function) = method.func.as_ref() else {
        return fail(g, loc, format!("`{name}` is not a function"));
    };
    let function = function.clone();
    // a called function that calls a callback prop takes the runtime context;
    // only a handler (which has one) can call it
    let cx = if super::action_cx::needs_cx(g, &function) {
        match c.cx_var.clone() {
            Some(cx) => Some(cx),
            None => {
                return fail(
                    g,
                    loc,
                    format!(
                        "`{name}` calls a callback prop, so it only runs from an event handler"
                    ),
                );
            }
        }
    } else {
        None
    };
    let required = functions::required(&function.params);
    if call.args.len() < required || call.args.len() > function.params.len() {
        return fail(
            g,
            loc,
            format!("`{name}` expects {} arguments", function.params.len()),
        );
    }
    let mut arguments: Vec<String> = Vec::new();
    if method.inline_only {
        // `self.method(&_capture, …)`: the receiver is the instance, not the
        // capture struct
        arguments.push("self".to_owned());
        if receiver != "self" {
            arguments.push(format!("&({receiver})"));
        }
    }
    if let Some(cx) = &cx {
        arguments.push(cx.clone());
    }
    for (param_index, param) in function.params.iter().enumerate() {
        let parameter_type = match g.extensions.hooks.method_param {
            Some(hook) => hook(g, &method, param_index, param)?,
            None => match param.ty.as_ref() {
                Some(reference) => types::type_ref(g, reference, false)?,
                None => {
                    return fail(
                        g,
                        param.name.loc,
                        format!(
                            "parameter `{}` needs a type",
                            String::from_utf8_lossy(&param.name.name)
                        ),
                    );
                }
            },
        };
        if param_index >= call.args.len() {
            arguments.push(functions::missing(g, param, parameter_type)?);
            continue;
        }
        let value = emit(g, c, &call.args[param_index], Some(parameter_type), false)?;
        arguments.push(value.code);
    }
    Ok(Value {
        ty: method.ret,
        code: format!("{target}({})", arguments.join(", ")),
        ..Value::default()
    })
}

/// `self.method` for a plain method.
fn self_method_target(g: &mut Generator<'_>, name: &str) -> Result<String> {
    let id = rust_text::id_at(g, name.as_bytes(), 0)?;
    Ok(format!("self.{id}"))
}

/// `console.log` is accepted only as a statement and emits space-separated
/// values plus a newline.
pub fn call_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<String> {
    let ast::ExprData::Call(call) = &e.data else {
        unreachable!("call statement")
    };
    if let ast::ExprData::Member(member) = &call.callee.data {
        let console = matches!(&member.object.data, ast::ExprData::Ident(n) if n == b"console");
        if console && member.name.name == b"log" {
            no_type_args(g, e.loc, call)?;
            let mut format = Format::default();
            for (index, arg) in call.args.iter().enumerate() {
                if index > 0 {
                    format.text(b" ");
                }
                let value = emit(g, c, arg, None, false)?;
                format.value(g, value, arg.loc)?;
            }
            format.text(b"\n");
            return Ok(format!("{};\n", format.finish(g, true)?));
        }
    }
    let v = emit(g, c, e, None, false)?;
    if v.ty == Type::Void {
        Ok(format!("{};\n", v.code))
    } else {
        Ok(format!("let _ = {};\n", v.code))
    }
}

/// Recognize equality spellings before selecting string or numeric
/// comparison.
fn equality_operator(op: &[u8]) -> bool {
    matches!(op, b"===" | b"!==" | b"==" | b"!=")
}

/// Recognize ordering comparisons without duplicating a long equality chain.
fn relational_operator(op: &[u8]) -> bool {
    matches!(op, b"<" | b">" | b"<=" | b">=")
}

/// Assertions change numeric representation explicitly, without enabling
/// general arrays or optionals.
fn explicit_cast(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    cast: &ast::As,
    child: bool,
) -> Result<Value> {
    let target = types::type_ref(g, &cast.ty, false)?;
    // `satisfies` checks assignability and keeps the value unchanged; `[…]
    // as T` / `{…} as T` give the literal T as its context, as in TS
    let literal = matches!(
        cast.expr.data,
        ast::ExprData::Array(_) | ast::ExprData::Object(_)
    ) || super::json::takes_context(&cast.expr);
    // `{ red: 1 } satisfies Record<string, number>`: the object keeps its
    // own type (`PALETTE.red` is a number), each field checked against the
    // record's value type
    if cast.satisfies
        && matches!(cast.expr.data, ast::ExprData::Object(_))
        && super::dicts::kind(g, target) == Some(model::InfoKind::Record)
    {
        let value = emit(g, c, &cast.expr, None, child)?;
        let value_type = model::info(g, target).map(|info| info.child);
        let fields = model::info(g, value.ty)
            .map(|info| info.fields.clone())
            .unwrap_or_default();
        if let Some(value_type) = value_type {
            for field in fields {
                types::coerce(g, Value::new(field.ty, "_field"), value_type, loc)?;
            }
        }
        return Ok(value);
    }
    if cast.satisfies || literal {
        return emit(g, c, &cast.expr, Some(target), child);
    }
    let value = emit(g, c, &cast.expr, None, child)?;
    // `r.json() as Promise<Item[]>`
    if let Some(decoded) = super::promises::cast(g, &value, target)? {
        return Ok(decoded);
    }
    types::explicit_cast(g, value, target, loc)
}
