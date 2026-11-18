//! Operators: unary, conditional, binary (arithmetic, comparison, logical, bitwise) and the infix code they emit.

use super::*;

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
        return crate::codegen::unions::type_of(g, c, &u.operand);
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
        return crate::codegen::dicts::contains(g, c, &b.lhs, &b.rhs);
    }
    if b.op == b"instanceof" {
        return crate::codegen::exceptions::instance_of(g, c, loc, b);
    }
    if b.op == b"&&" || b.op == b"||" {
        let lhs = emit(g, c, &b.lhs, None, false)?;
        // `name || "Untitled"`: a value, not a boolean
        if b.op == b"||" && lhs.ty != Type::Boolean {
            return crate::codegen::truthy::or_value(g, c, b, lhs, expected);
        }
        let a = crate::codegen::truthy::test(g, lhs.clone(), b.lhs.loc)?;
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
                && let Some(value) = crate::codegen::truthy::and_value(g, &lhs, &a, rhs)?
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
        let a = crate::codegen::loops::explicit_string(g, a)?;
        let z = crate::codegen::loops::explicit_string(g, z)?;
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
    // and `done.length / items.length` divides as numbers (1/3, not 0):
    // counts are `usize` only for Rust's sake
    if (b.op == b"-" && t.unsigned() || b.op == b"/" && t == Type::Usize)
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
pub(super) fn bitwise(
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

/// Recognize equality spellings before selecting string or numeric
/// comparison.
pub(super) fn equality_operator(op: &[u8]) -> bool {
    matches!(op, b"===" | b"!==" | b"==" | b"!=")
}

/// Recognize ordering comparisons without duplicating a long equality chain.
pub(super) fn relational_operator(op: &[u8]) -> bool {
    matches!(op, b"<" | b">" | b"<=" | b">=")
}
