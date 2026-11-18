//! Converting values between types: coercion, explicit casts, concrete types, formatting, literal code and unification.

use super::*;

/// Allow matching types, contextual literals, integer-to-float, and
/// float-width conversions.
pub fn coerce(g: &mut Generator<'_>, v: Value, t: Type, loc: ast::Loc) -> Result<Value> {
    if v.ty == t {
        return Ok(v);
    }
    // string unions are structural (`keyof Labels` where `keyof typeof
    // labels` is expected): member by member
    if let (Some(from), Some(to)) = (model::info(g, v.ty), model::info(g, t))
        && from.kind == model::InfoKind::Enumeration
        && to.kind == model::InfoKind::Enumeration
        && !from.members.is_empty()
        && from
            .members
            .iter()
            .all(|member| to.members.contains(member))
    {
        let (from_rust, to_rust) = (from.rust.clone(), to.rust.clone());
        let mut arms = String::new();
        for member in &from.members {
            let Some(variant) = rust_text::enum_variant(member) else {
                return fail(g, loc, "enum member is not valid UTF-8");
            };
            arms.push_str(&format!(
                "{from_rust}::{variant} => {to_rust}::{variant},\n"
            ));
        }
        return Ok(Value::new(t, format!("match {} {{\n{arms}}}", v.code)));
    }
    // `Promise<any>` (`res.json()`) where a `Promise<T>` is expected: decoded
    if let Some(decoded) = crate::codegen::promises::cast(g, &v, t)? {
        return Ok(decoded);
    }
    // a member of a union becomes the union (`U::Num(v)`)
    if let Some(wrapped) = crate::codegen::unions::wrap(g, &v, t, loc)? {
        return Ok(wrapped);
    }
    if t == Type::String && model::enum_type(g, v.ty) {
        return Ok(Value {
            ty: Type::String,
            code: format!("zant::Str::Static({}.as_str())", concrete(g, v)?.code),
            ..Value::default()
        });
    }
    if let Some(child) = model::optional_child(g, t) {
        if v.ty == Type::Null {
            return Ok(Value {
                ty: t,
                code: "None".to_owned(),
                ..Value::default()
            });
        }
        let mut result = coerce(g, v, child, loc)?;
        result.ty = t;
        result.code = format!("Some({})", result.code);
        result.precedence = Precedence::Suffix;
        return Ok(result);
    }
    if model::optional_child(g, v.ty).is_some() {
        return fail(
            g,
            loc,
            "optional value needs a fallback with ?? or a non-null assertion !",
        );
    }
    if let Some(converted) = crate::codegen::structural::convert(g, &v, t, loc)? {
        return Ok(converted);
    }
    if model::enum_type(g, t)
        && v.ty == Type::String
        && let Some(text) = v.text.as_ref()
    {
        let members = model::info(g, t)
            .map(|info| info.members)
            .unwrap_or_default();
        for member in &members {
            if member == text {
                let Some(variant) = rust_text::enum_variant(member) else {
                    return fail(g, loc, "enum member is not valid UTF-8");
                };
                return Ok(Value {
                    ty: t,
                    code: format!("{}::{variant}", t.rust(g)),
                    inference: Inference::ResultLocation,
                    ..Value::default()
                });
            }
        }
        let expected = members
            .iter()
            .map(|member| String::from_utf8_lossy(member).into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        return fail(
            g,
            loc,
            format!(
                "unknown {} member `{}`; expected: {expected}",
                t.rust(g),
                String::from_utf8_lossy(text)
            ),
        );
    }
    if (v.ty == Type::IntLiteral && t.numeric()) || (v.ty == Type::FloatLiteral && t.float()) {
        let mut result = v;
        result.code = literal_code(&result.code, t);
        result.ty = t;
        return Ok(result);
    }
    if v.ty.integer() && t.integer() && Type::numeric_peer(v.ty, t) == Some(t) {
        // widening (`u8` → `u32`): Rust needs an explicit `as` cast
        return Ok(Value {
            ty: t,
            code: format!(
                "({} as {})",
                rust_text::operand(&concrete(g, v)?, Precedence::Cast, false),
                t.rust(g)
            ),
            ..Value::default()
        });
    }
    if v.ty.integer() && t.float() {
        return Ok(Value {
            ty: t,
            code: format!(
                "({} as {})",
                rust_text::operand(&concrete(g, v)?, Precedence::Cast, false),
                t.rust(g)
            ),
            inference: Inference::ResultLocation,
            ..Value::default()
        });
    }
    if v.ty.float() && t.float() {
        return Ok(Value {
            ty: t,
            code: format!(
                "({} as {})",
                rust_text::operand(&concrete(g, v)?, Precedence::Cast, false),
                t.rust(g)
            ),
            inference: Inference::ResultLocation,
            ..Value::default()
        });
    }
    // `return null` from a component (React renders nothing): an empty node,
    // which takes no space in its parent's layout
    if v.ty == Type::Null && t == Type::Node {
        return Ok(Value::new(t, "ui::Node::default()"));
    }
    if (v.ty == Type::Node || v.ty == Type::Null) && t == Type::OptionalNode {
        if v.ty == Type::Null {
            return Ok(Value::new(t, "None"));
        }
        let mut result = v;
        result.ty = t;
        result.code = format!("Some({})", result.code);
        result.precedence = Precedence::Suffix;
        return Ok(result);
    }
    if (v.ty.float() && t.integer()) || (v.ty == Type::FloatLiteral && t.integer()) {
        return fail(
            g,
            loc,
            format!(
                "cannot convert {} to {}; float-to-integer conversion is not implicit; use an integer value",
                v.ty.rust(g),
                t.rust(g)
            ),
        );
    }
    // `Uint8Array` is a `&'static [u8]`: an unannotated byte literal lowered to
    // a `tsx::List<u8>` cannot become a static slice after the fact.
    if model::is_static_bytes(g, t) {
        return fail(
            g,
            loc,
            format!(
                "cannot convert {} to {}; annotate byte literals with `const name: Uint8Array = [..]` (or import the bytes)",
                v.ty.rust(g),
                t.rust(g)
            ),
        );
    }
    fail(
        g,
        loc,
        format!(
            "cannot convert {} to {}; use an explicit matching type",
            v.ty.rust(g),
            t.rust(g)
        ),
    )
}

/// Explicit assertions allow numeric conversions that implicit coercion
/// rejects.
pub fn explicit_cast(
    g: &mut Generator<'_>,
    value: Value,
    target: Type,
    loc: ast::Loc,
) -> Result<Value> {
    let mut source = value;
    if source.ty.literal() {
        let concrete_ty = source.ty.concrete();
        source = coerce(g, source, concrete_ty, loc)?;
    }
    if source.ty == target {
        return Ok(source);
    }
    if !source.ty.numeric() || !target.numeric() {
        return fail(
            g,
            loc,
            format!(
                "cannot cast {} to {}; only numeric conversions are supported",
                source.ty.rust(g),
                target.rust(g)
            ),
        );
    }
    let source = concrete(g, source)?;
    Ok(Value {
        ty: target,
        code: format!(
            "({} as {})",
            rust_text::operand(&source, Precedence::Cast, false),
            target.rust(g)
        ),
        inference: Inference::ResultLocation,
        ..Value::default()
    })
}

/// Materialize a contextual literal or conversion into a typed expression.
/// Conversions already carry their `as` cast, so only literals need the
/// typed spelling here.
pub fn concrete(g: &mut Generator<'_>, v: Value) -> Result<Value> {
    if v.inference == Inference::Known || (!v.ty.numeric() && !model::enum_type(g, v.ty)) {
        return Ok(v);
    }
    let mut result = v;
    if result.ty.literal() {
        result.ty = result.ty.concrete();
        result.code = literal_code(&result.code, result.ty);
    }
    result.inference = Inference::Known;
    result.precedence = Precedence::Primary;
    Ok(result)
}

/// Rust literals are polymorphic, so [`concrete`] decides the final type of a
/// literal before it is formatted.
pub fn format_value(g: &mut Generator<'_>, v: Value) -> Result<Value> {
    if v.inference == Inference::Literal || v.inference == Inference::ResultLocation {
        return concrete(g, v);
    }
    Ok(v)
}

/// The Rust spelling of a numeric literal for target type `t`.
///
/// Integer targets get a type suffix (`1u32`); float targets need a decimal
/// point (`1.0f64`, `0.5f32`). Negative literals keep their sign, and
/// hexadecimal/octal/binary integer literals become `as` casts for floats.
pub fn literal_code(raw: &str, t: Type) -> String {
    let (sign, rest) = if let Some(rest) = raw.strip_prefix('-') {
        ("-", rest)
    } else if let Some(rest) = raw.strip_prefix('+') {
        ("", rest)
    } else {
        ("", raw)
    };
    let suffix = match t {
        Type::U8 => "u8",
        Type::U16 => "u16",
        Type::U32 => "u32",
        Type::U64 => "u64",
        Type::I8 => "i8",
        Type::I16 => "i16",
        Type::I32 => "i32",
        Type::I64 => "i64",
        Type::Usize => "usize",
        Type::Isize => "isize",
        Type::F32 => "f32",
        Type::F64 => "f64",
        _ => return raw.to_owned(),
    };
    if t.float() {
        let prefixed = rest.starts_with("0x") || rest.starts_with("0X");
        if prefixed {
            return format!("({sign}{rest} as {suffix})");
        }
        if rest.contains('.') || rest.contains('e') || rest.contains('E') {
            return format!("{sign}{rest}{suffix}");
        }
        return format!("{sign}{rest}.0{suffix}");
    }
    format!("{sign}{rest}{suffix}")
}

/// Unify one operand type with the other.
pub fn unify(g: &mut Generator<'_>, a: Type, b: Type, loc: ast::Loc) -> Result<Type> {
    if a == b {
        return Ok(a);
    }
    if a.numeric() && b.numeric() {
        return match Type::numeric_peer(a, b) {
            Some(t) => Ok(t),
            None => fail(
                g,
                loc,
                format!(
                    "different integer types {} and {}; use the same integer type",
                    a.rust(g),
                    b.rust(g)
                ),
            ),
        };
    }
    if a == Type::Null {
        return model::optional(g, b);
    }
    if b == Type::Null {
        return model::optional(g, a);
    }
    if model::optional_child(g, a) == Some(b) {
        return Ok(a);
    }
    if model::optional_child(g, b) == Some(a) {
        return Ok(b);
    }
    let a_node = matches!(a, Type::Node | Type::OptionalNode | Type::Null);
    let b_node = matches!(b, Type::Node | Type::OptionalNode | Type::Null);
    if a_node && b_node {
        return Ok(Type::OptionalNode);
    }
    fail(
        g,
        loc,
        format!("incompatible types {} and {}", a.rust(g), b.rust(g)),
    )
}
