//! Error values: the `Error` / `Thrown` types, `new Error(...)`, `instanceof`, narrowing facts and `typeof` / text of a thrown value.

use super::*;

pub(super) fn find(g: &Generator<'_>, rust: &str) -> Option<Type> {
    g.compounds
        .iter()
        .position(|entry| entry.rust == rust)
        .map(|index| Type::Compound(index as u32))
}

/// `Error` (any error kind): `{ name: string; message: string }`.
pub fn error_type(g: &mut Generator<'_>) -> Result<Type> {
    if let Some(ty) = find(g, ERROR) {
        return Ok(ty);
    }
    let field = |name: &str| Field {
        name: name.as_bytes().to_vec(),
        ty: Type::String,
        optional: false,
        rust: None,
    };
    model::add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: ERROR.to_owned(),
            fields: vec![field("name"), field("message")],
            ..Info::default()
        },
    )
}

/// What a `catch` binding holds (TypeScript's `unknown`).
pub fn thrown_type(g: &mut Generator<'_>) -> Result<Type> {
    if let Some(ty) = find(g, THROWN) {
        return Ok(ty);
    }
    model::add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: THROWN.to_owned(),
            ..Info::default()
        },
    )
}

pub fn is_thrown(g: &Generator<'_>, ty: Type) -> bool {
    find(g, THROWN) == Some(ty)
}

pub fn is_error(g: &Generator<'_>, ty: Type) -> bool {
    find(g, ERROR) == Some(ty)
}

/// `Error` / `TypeError` / … in a type position (unless the file declares
/// its own type of that name).
pub fn named_type(g: &mut Generator<'_>, name: &[u8]) -> Result<Option<Type>> {
    if g.type_decls.iter().any(|decl| decl.name.name == name) {
        return Ok(None);
    }
    if ERROR_NAMES.iter().any(|known| known.as_bytes() == name) {
        return error_type(g).map(Some);
    }
    Ok(None)
}

// ── values ───────────────────────────────────────────────────────────

/// `new Error(message?)` / `TypeError(message?)`; `None` for other calls.
pub fn construct(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return Ok(None);
    };
    let Some(kind) = ERROR_NAMES.iter().find(|known| known.as_bytes() == name) else {
        return Ok(None);
    };
    if scope::local(c, name).is_some() || crate::codegen::functions::find_index(g, name).is_some() {
        return Ok(None);
    }
    if call.args.len() > 2 {
        return fail(g, loc, format!("`{kind}` takes a message"));
    }
    let message = match call.args.first() {
        Some(arg) => {
            let value = expr::emit(g, c, arg, Some(Type::String), false)?;
            crate::codegen::types::coerce(g, value, Type::String, arg.loc)?.code
        }
        None => "zant::Str::Static(\"\")".to_owned(),
    };
    let ty = error_type(g)?;
    Ok(Some(Value::new(
        ty,
        format!("{ERROR}::new({kind:?}, {message})"),
    )))
}

/// The value of `throw e` as a `Thrown`.
pub(super) fn thrown(g: &mut Generator<'_>, value: Value, loc: ast::Loc) -> Result<String> {
    let operand = rust_text::operand(&value, Precedence::Suffix, false);
    if is_thrown(g, value.ty) {
        return Ok(value.code);
    }
    if is_error(g, value.ty) {
        return Ok(format!("{THROWN}::error({})", value.code));
    }
    match value.ty {
        Type::String => Ok(format!("{THROWN}::String({})", value.code)),
        Type::Boolean => Ok(format!("{THROWN}::Boolean({})", value.code)),
        Type::Null => Ok(format!("{THROWN}::Null")),
        ty if ty.numeric() => Ok(format!("{THROWN}::Number(({operand}) as f64)")),
        _ => fail(
            g,
            loc,
            "throw an Error (`throw new Error(\"…\")`), a string or a number",
        ),
    }
}

/// `e instanceof Error` (the right side names an error constructor).
pub fn instance_of(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    b: &ast::Binary,
) -> Result<Value> {
    let Some(kind) = constructor(&b.rhs) else {
        return fail(
            g,
            b.rhs.loc,
            "`instanceof` tests an error kind (`e instanceof Error`); test objects by a field instead",
        );
    };
    // only looked into: a plain local is read in place (no copy)
    crate::codegen::place::project(c, &b.lhs);
    let value = expr::emit(g, c, &b.lhs, None, false)?;
    let operand = match &b.lhs.data {
        ast::ExprData::Ident(name) if narrowing::find(c, name).is_none() => {
            match scope::local_index(c, name) {
                Some(index) => c.bindings[index].rust.clone(),
                None => rust_text::operand(&value, Precedence::Suffix, false),
            }
        }
        _ => rust_text::operand(&value, Precedence::Suffix, false),
    };
    if is_error(g, value.ty) {
        // an `Error` value: every error is an `Error`; a kind tests its name
        let code = if kind == "Error" {
            "true".to_owned()
        } else {
            format!("&*{operand}.name == {kind:?}")
        };
        return Ok(Value::new(Type::Boolean, code));
    }
    if !is_thrown(g, value.ty) {
        return fail(g, loc, "only caught errors can be tested with `instanceof`");
    }
    let code = if kind == "Error" {
        format!("{operand}.is_error()")
    } else {
        format!("{operand}.is_error_named({kind:?})")
    };
    Ok(Value::new(Type::Boolean, code))
}

pub(super) fn constructor(e: &ast::Expr) -> Option<&'static str> {
    let ast::ExprData::Ident(name) = &e.data else {
        return None;
    };
    ERROR_NAMES
        .iter()
        .copied()
        .find(|known| known.as_bytes() == name)
}

/// What `e instanceof Error` / `typeof e === "string"` proves about a caught
/// `e` (pushed onto `c.narrowed`); false when `b` is no such test.
pub fn fact(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    b: &ast::Binary,
    positive: bool,
) -> Result<bool> {
    let (subject, narrowed) = if b.op == b"instanceof" {
        if constructor(&b.rhs).is_none() {
            return Ok(false);
        }
        (&*b.lhs, None)
    } else if (b.op == b"===" || b.op == b"==") && positive {
        let (test, literal) = match (&b.lhs.data, &b.rhs.data) {
            (ast::ExprData::Unary(u), ast::ExprData::String(s)) if u.op == b"typeof" => (u, s),
            (ast::ExprData::String(s), ast::ExprData::Unary(u)) if u.op == b"typeof" => (u, s),
            _ => return Ok(false),
        };
        let ty = match literal.as_slice() {
            b"string" => (Type::String, "as_string"),
            b"number" => (Type::F64, "as_number"),
            _ => return Ok(false),
        };
        (&*test.operand, Some(ty))
    } else {
        return Ok(false);
    };
    if !positive {
        return Ok(b.op == b"instanceof");
    }
    let Some(path) = narrowing::path(subject) else {
        return Ok(false);
    };
    let binding = scope::local_index(c, &narrowing::base(subject));
    let Some(index) = binding else {
        return Ok(false);
    };
    if !is_thrown(g, c.bindings[index].ty) {
        return Ok(false);
    }
    let rust = c.bindings[index].rust.clone();
    let (ty, method) = match narrowed {
        Some(found) => found,
        None => (error_type(g)?, "as_error"),
    };
    c.narrowed.push(Narrow {
        name: String::from_utf8_lossy(&path).into_owned(),
        binding,
        generation: narrowing::generation(c, binding),
        ty,
        code: format!("{rust}.{method}()"),
    });
    Ok(true)
}

/// `typeof e` / `String(e)` of a caught value; `None` for other types.
pub fn type_of(g: &Generator<'_>, value: &Value) -> Option<Value> {
    is_thrown(g, value.ty).then(|| {
        Value::new(
            Type::String,
            format!(
                "{}.type_of()",
                rust_text::operand(value, Precedence::Suffix, false)
            ),
        )
    })
}

/// A caught value or an `Error` shown as text.
pub fn text(g: &Generator<'_>, value: &Value) -> Option<Value> {
    let operand = rust_text::operand(value, Precedence::Suffix, false);
    if is_thrown(g, value.ty) {
        return Some(Value::new(Type::String, format!("{operand}.to_str()")));
    }
    if is_error(g, value.ty) {
        return Some(Value::new(
            Type::String,
            format!("zant::Str::from({operand}.to_string())"),
        ));
    }
    None
}

// ── control flow ─────────────────────────────────────────────────────
