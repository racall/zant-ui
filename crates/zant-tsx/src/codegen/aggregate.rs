//! Immutable array/object values, spreads, indexing, and optional member
//! access.
//!
//! Arrays become `tsx::List::from_vec(..)`, with a `Vec` block for spreads;
//! `??` and optional member access become `unwrap_or_else` / `.map(..)`.

use crate::ast;

use super::builtins;
use super::expr;
use super::model;
use super::rust_text::{self, Precedence};
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// General arrays become `tsx::List` values.
pub fn array(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    items: &[Box<ast::Expr>],
    expected: Option<Type>,
) -> Result<Value> {
    if let Some(items_ty) = expected.and_then(|ty| model::tuple_items(g, unwrap(g, ty))) {
        return tuple(g, c, loc, items, expected.unwrap(), &items_ty);
    }
    let mut child: Option<Type> = match expected {
        Some(ty) => model::element(g, unwrap(g, ty)),
        None => None,
    };
    let mut values: Vec<Value> = Vec::new();
    for item in items {
        let spread = matches!(item.data, ast::ExprData::Spread(_));
        let context = if expected.is_some() {
            child
        } else {
            child.filter(|ty| {
                !(ty.numeric() || *ty == Type::Null || matches!(item.data, ast::ExprData::NullLit))
            })
        };
        let target: &ast::Expr = match &item.data {
            ast::ExprData::Spread(inner) => inner,
            _ => item,
        };
        // `[...xs]` copies the array: a later change of `xs` is not seen
        // here unless the items themselves are shared objects
        if spread {
            super::place::project(c, target);
        }
        let value = expr::emit(g, c, target, if spread { None } else { context }, false)?;
        if spread
            && let ast::ExprData::Ident(name) = &target.data
            && let Some(index) = scope::local_index(c, name)
            && model::element(g, value.ty).is_some_and(|item| model::owned(g, item))
        {
            c.bindings[index].escaped = true;
        }
        if spread && model::optional_child(g, value.ty).is_some() {
            return fail(g, item.loc, "optional array needs ?? or ! before spread");
        }
        let element = if spread {
            match model::element(g, value.ty) {
                Some(element) => element,
                None => return fail(g, item.loc, "spread needs an array"),
            }
        } else {
            value.ty
        };
        child = match child {
            Some(ty) => Some(types::unify(g, ty, element, item.loc)?),
            None => Some(element.concrete()),
        };
        values.push(value);
    }
    let Some(element) = child else {
        return fail(g, loc, "empty array needs a type annotation");
    };
    let element = element.concrete();
    let ty = model::array(g, element)?;
    let element_rust = element.rust(g).to_owned();
    // in source order: `[...xs, x]` appends, `[x, ...xs]` prepends
    let mut parts: Vec<(bool, String)> = Vec::new();
    for (item, raw) in items.iter().zip(values) {
        if matches!(item.data, ast::ExprData::Spread(_)) {
            if model::element(g, raw.ty) != Some(element) {
                return fail(g, item.loc, "spread array element type does not match");
            }
            parts.push((
                true,
                rust_text::operand(&raw, Precedence::Suffix, false).to_string(),
            ));
        } else {
            let value = types::coerce(g, raw, element, item.loc)?;
            parts.push((false, value.code));
        }
    }
    if parts.iter().all(|(spread, _)| !spread) {
        let parts: Vec<String> = parts.into_iter().map(|(_, code)| code).collect();
        let code = if parts.is_empty() {
            // an empty `vec![]` would need the type from context; name it
            format!("tsx::List::from_vec(Vec::<{element_rust}>::new())")
        } else {
            format!("tsx::List::from_vec(vec![{}])", parts.join(", "))
        };
        return Ok(Value {
            ty,
            code,
            ..Value::default()
        });
    }
    let temp = temporary(g, c, b"_items")?;
    let mut block = format!("{{\n    let mut {temp}: Vec<{element_rust}> = Vec::new();\n");
    for (spread, code) in parts {
        if spread {
            block.push_str(&format!("    {temp}.extend({code}.iter().cloned());\n"));
        } else {
            block.push_str(&format!("    {temp}.push({code});\n"));
        }
    }
    block.push_str(&format!("    tsx::List::from_vec({temp})\n}}"));
    Ok(Value {
        ty,
        code: block,
        precedence: Precedence::Conditional,
        ..Value::default()
    })
}

/// `[a, b]` where a tuple `[A, B]` is expected.
fn tuple(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    items: &[Box<ast::Expr>],
    expected: Type,
    types_of: &[Type],
) -> Result<Value> {
    if items.len() != types_of.len()
        || items
            .iter()
            .any(|item| matches!(item.data, ast::ExprData::Spread(_)))
    {
        return fail(
            g,
            loc,
            format!("this tuple has {} elements", types_of.len()),
        );
    }
    let mut parts = Vec::new();
    for (item, ty) in items.iter().zip(types_of) {
        let value = expr::emit(g, c, item, Some(*ty), false)?;
        parts.push(types::coerce(g, value, *ty, item.loc)?.code);
    }
    let tuple_ty = unwrap(g, expected);
    let code = if parts.len() == 1 {
        format!("({},)", parts[0])
    } else {
        format!("({})", parts.join(", "))
    };
    Ok(Value::new(tuple_ty, code))
}

/// The type of an object literal without a contextual type, as TypeScript
/// infers it: each property's (widened) value type, in source order. A lone
/// spread base (`{ ...todo, done: true }`) keeps the base's type.
fn infer_object(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    properties: &[ast::Property],
) -> Result<Type> {
    let spreads: Vec<&ast::Property> = properties.iter().filter(|p| p.spread).collect();
    if let [base] = spreads.as_slice() {
        let value = expr::emit(g, c, &base.value, None, false)?;
        if model::info(g, value.ty).is_some_and(|info| info.kind == model::InfoKind::Object) {
            return Ok(value.ty);
        }
        return fail(g, base.value.loc, "spread needs an object");
    }
    if !spreads.is_empty() {
        return fail(
            g,
            loc,
            "an object with several spreads needs a type annotation (`const x: T = { … }`)",
        );
    }
    let mut members: Vec<model::Member> = Vec::new();
    for property in properties {
        if matches!(property.value.data, ast::ExprData::NullLit) {
            return fail(
                g,
                property.value.loc,
                format!(
                    "`{}` is null: give the object a type so the field's type is known (`const x: T = {{ … }}`)",
                    String::from_utf8_lossy(&property.key.name)
                ),
            );
        }
        let value = expr::emit(g, c, &property.value, None, false)?;
        if value.ty == Type::Void || value.ty == Type::Node || value.ty == Type::OptionalNode {
            return fail(
                g,
                property.value.loc,
                "this value cannot be an object field",
            );
        }
        let ty = value.ty.concrete();
        // a repeated key: the last one wins, as in JavaScript
        members.retain(|member| member.name != property.key.name);
        members.push(model::Member {
            name: property.key.name.clone(),
            loc: property.key.loc,
            ty,
            optional: false,
        });
    }
    model::literal_object(g, members)
}

/// Copy a spread exactly once, then apply fields in source order (last write
/// wins).
pub fn object(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    properties: &[ast::Property],
    expected: Option<Type>,
) -> Result<Value> {
    let expected = match expected {
        Some(expected) => expected,
        None => infer_object(g, c, loc, properties)?,
    };
    let ty = unwrap(g, expected);
    if super::unions::is_union(g, ty) {
        let value = super::unions::object_literal(g, c, loc, properties, ty)?;
        return Ok(if expected == ty {
            value
        } else {
            Value {
                ty: expected,
                code: format!("Some({})", value.code),
                ..Value::default()
            }
        });
    }
    if super::dicts::kind(g, ty) == Some(model::InfoKind::Record) {
        let value = super::dicts::literal(g, c, loc, properties, ty)?;
        return Ok(Value {
            ty: expected,
            code: if expected == ty {
                value.code
            } else {
                format!("Some({})", value.code)
            },
            ..Value::default()
        });
    }
    let Some(entry) = model::info(g, ty) else {
        return fail(g, loc, "object literal needs a struct type");
    };
    if entry.kind != model::InfoKind::Object {
        return fail(g, loc, "object literal needs a struct type");
    }
    let fields = entry.fields;
    let temp = temporary(g, c, b"_object")?;
    let mut values: Vec<Value> = Vec::new();
    let mut writes: Vec<String> = Vec::new();
    let mut spread = false;
    for property in properties {
        let field_ty = if property.spread {
            ty
        } else {
            model::field_type(g, ty, &property.key.name, property.key.loc)?
        };
        let value = expr::emit(g, c, &property.value, Some(field_ty), false)?;
        let value_code = value.code.clone();
        if property.spread {
            spread = true;
            writes.push(format!("{temp} = {value_code};"));
        } else {
            writes.push(format!(
                "{temp}.{} = {value_code};",
                rust_text::id_at(g, &property.key.name, property.key.loc)?
            ));
        }
        values.push(value);
    }
    if spread {
        let ty_rust = ty.rust(g).to_owned();
        let path = rust_text::expression_path(&ty_rust);
        let mut block = format!("{{\n    let mut {temp}: {ty_rust} = {path}::default();\n");
        for write in &writes {
            block.push_str(&format!("    {write}\n"));
        }
        block.push_str(&format!("    {temp}\n}}"));
        return Ok(Value {
            ty,
            code: block,
            precedence: Precedence::Conditional,
            ..Value::default()
        });
    }
    for field in &fields {
        let present = properties
            .iter()
            .any(|property| property.key.name == field.name);
        if !present && !field.optional {
            return fail(
                g,
                loc,
                format!(
                    "{} is missing field `{}`",
                    ty.rust(g),
                    String::from_utf8_lossy(&field.name)
                ),
            );
        }
    }
    // Source order matters when field values call component functions.
    let mut initial: Vec<String> = Vec::new();
    for (index, property) in properties.iter().enumerate() {
        let replaced = properties[index + 1..]
            .iter()
            .any(|later| later.key.name == property.key.name);
        if replaced {
            continue;
        }
        initial.push(format!(
            "    {}: {},",
            rust_text::id_at(g, &property.key.name, property.key.loc)?,
            values[index].code
        ));
    }
    let all_present = fields.iter().all(|field| {
        properties
            .iter()
            .any(|property| property.key.name == field.name)
    });
    if !all_present {
        initial.push("    ..Default::default()".to_owned());
    }
    Ok(Value {
        ty,
        code: format!(
            "{} {{\n{}\n}}",
            rust_text::expression_path(ty.rust(g)),
            initial.join("\n")
        ),
        ..Value::default()
    })
}

pub fn unwrap(g: &Generator<'_>, ty: Type) -> Type {
    model::optional_child(g, ty).unwrap_or(ty)
}

/// Compiler temporaries are ordinary bindings, so they cannot shadow source
/// locals.
pub fn temporary(g: &mut Generator<'_>, c: &mut scope::Context, base: &[u8]) -> Result<String> {
    let binding = scope::bind(g, c, base, Type::Void, false)?;
    c.bindings[binding].name.clear();
    Ok(c.bindings[binding].rust.clone())
}

pub fn non_null(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    inner: &ast::Expr,
    expected: Option<Type>,
    child: bool,
) -> Result<Value> {
    if types::numeric_literals(inner) {
        return expr::emit(g, c, inner, expected, child);
    }
    let value = expr::emit(g, c, inner, None, child)?;
    if let Some(ty) = model::optional_child(g, value.ty) {
        return Ok(Value {
            ty,
            code: format!(
                "{}.unwrap()",
                rust_text::operand(&value, Precedence::Suffix, false)
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    match expected {
        Some(target) => types::coerce(g, value, target, inner.loc),
        None => Ok(value),
    }
}

pub fn coalesce(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    binary: &ast::Binary,
    expected: Option<Type>,
) -> Result<Value> {
    let value = expr::emit(g, c, &binary.lhs, None, false)?;
    if value.ty == Type::Null {
        let mut fallback = expr::emit(g, c, &binary.rhs, expected, false)?;
        if value.code == "None" {
            return Ok(fallback);
        }
        let target = expected.unwrap_or_else(|| fallback.ty.concrete());
        fallback = types::coerce(g, fallback, target, binary.rhs.loc)?;
        return Ok(Value {
            ty: fallback.ty,
            code: fallback.code,
            precedence: Precedence::LogicalOr,
            ..Value::default()
        });
    }
    let Some(child) = model::optional_child(g, value.ty) else {
        return fail(g, loc, "?? needs an optional value");
    };
    // `find(…) ?? null`: still `T | null`
    if matches!(binary.rhs.data, ast::ExprData::NullLit) {
        return Ok(value);
    }
    // an optional fallback where an optional is expected: `a ?? b` is
    // `T | null` again
    if expected.is_some_and(|ty| model::optional_child(g, ty).is_some()) {
        let fallback = expr::emit(g, c, &binary.rhs, Some(value.ty), false)?;
        let fallback = types::coerce(g, fallback, value.ty, binary.rhs.loc)?;
        return Ok(Value {
            ty: value.ty,
            code: format!(
                "{}.or_else(|| {})",
                rust_text::operand(&value, Precedence::Suffix, false),
                fallback.code
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    let fallback = expr::emit(g, c, &binary.rhs, Some(child), false)?;
    Ok(Value {
        ty: child,
        code: format!(
            "{}.unwrap_or_else(|| {})",
            rust_text::operand(&value, Precedence::Suffix, false),
            fallback.code
        ),
        precedence: Precedence::Suffix,
        ..Value::default()
    })
}

/// Optional access evaluates its receiver once and preserves null through the
/// member.
pub fn member(g: &mut Generator<'_>, c: &mut scope::Context, m: &ast::Member) -> Result<Value> {
    if let Some(value) = super::enums::member(g, c, m)? {
        return Ok(value);
    }
    if !c.narrowed.is_empty() && !m.optional {
        // `s.prompt` narrowed by `s.prompt !== null && …`
        let expression = ast::Expr {
            loc: m.name.loc,
            data: ast::ExprData::Member(m.clone()),
        };
        if let Some(path) = super::narrowing::path(&expression)
            && let Some(value) =
                super::narrowing::find_path(c, &path, &super::narrowing::base(&expression))
        {
            return Ok(value);
        }
    }
    if let Some(value) = builtins::member(g, c, m)? {
        return Ok(value);
    }
    if let Some(hook) = g.extensions.hooks.ref_read
        && let Some(value) = hook(g, c, m)?
    {
        return Ok(value);
    }
    super::place::project(c, &m.object);
    let value = expr::emit(g, c, &m.object, None, false)?;
    if m.optional
        && let Some(child) = model::optional_child(g, value.ty)
    {
        let name = temporary(g, c, b"_value")?;
        let result = member_read(g, child, &name, &m.name)?;
        let ty = model::optional(g, result.ty)?;
        return Ok(Value {
            ty,
            code: format!(
                "{}.map(|{name}| {})",
                rust_text::operand(&value, Precedence::Suffix, false),
                result.code
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    if model::optional_child(g, value.ty).is_some() {
        return fail(
            g,
            m.name.loc,
            "optional value needs ?? or ! before member access",
        );
    }
    if let Some(read) = super::dicts::member(g, &value, &m.name)? {
        return Ok(read);
    }
    if let Some(read) = super::unions::member(g, &value, &m.name)? {
        return Ok(read);
    }
    let operand = rust_text::operand(&value, Precedence::Suffix, false);
    member_read(g, value.ty, &operand, &m.name)
}

/// A member read of a value whose expression is already parenthesized to
/// suffix precedence (`operand`).
fn member_read(
    g: &mut Generator<'_>,
    object_ty: Type,
    operand: &str,
    name: &ast::Name,
) -> Result<Value> {
    let length = name.name == b"length"
        && (object_ty == Type::String || model::element(g, object_ty).is_some());
    if length && object_ty == Type::String {
        // strings count UTF-16 code units, as in TypeScript
        return Ok(Value {
            ty: Type::Usize,
            code: format!("tsx::utf16_length(({operand}).as_str())"),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    if name.name == b"length"
        && let Some(items) = model::tuple_items(g, object_ty)
    {
        return Ok(Value::new(Type::Usize, format!("{}usize", items.len())));
    }
    if length {
        return Ok(Value {
            ty: Type::Usize,
            code: format!("{operand}.len()"),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    let ty = model::field_type(g, object_ty, &name.name, name.loc)?;
    let field = match model::field_rust(g, object_ty, &name.name) {
        Some(field) => field,
        None => rust_text::id_at(g, &name.name, name.loc)?,
    };
    let raw = format!("{operand}.{field}");
    if ty.copy(g) {
        Ok(Value::new(ty, raw))
    } else {
        Ok(Value {
            ty,
            code: format!("{raw}.clone()"),
            precedence: Precedence::Suffix,
            ..Value::default()
        })
    }
}

/// Numeric indexes are checked by `tsx::index` before Rust's own slice bounds
/// check.
pub fn index(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    ix: &ast::Index,
) -> Result<Value> {
    super::place::project(c, &ix.object);
    let value = expr::emit(g, c, &ix.object, None, false)?;
    if super::dicts::kind(g, value.ty).is_some() {
        return super::dicts::index(g, c, value, &ix.index);
    }
    if let Some(read) = super::type_ops::field_by_key(g, c, &value, &ix.index)? {
        return Ok(read);
    }
    if let Some(items) = model::tuple_items(g, value.ty) {
        // `pair[1]`: a literal position
        let position = match &ix.index.data {
            ast::ExprData::Number(n) => std::str::from_utf8(n)
                .ok()
                .and_then(|n| n.parse::<usize>().ok()),
            _ => None,
        };
        let Some(position) = position.filter(|position| *position < items.len()) else {
            return fail(
                g,
                ix.index.loc,
                format!(
                    "index a tuple with a literal position 0..{}",
                    items.len() - 1
                ),
            );
        };
        let item = items[position];
        let operand = rust_text::operand(&value, Precedence::Suffix, false);
        let code = if item.copy(g) {
            format!("{operand}.{position}")
        } else {
            format!("{operand}.{position}.clone()")
        };
        return Ok(Value {
            ty: item,
            code,
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    let ty = unwrap(g, value.ty);
    if !ix.optional && ty != value.ty {
        return fail(g, loc, "optional array needs ?? or !");
    }
    let Some(child) = model::element(g, ty) else {
        return fail(g, loc, "indexing needs an array");
    };
    let position = expr::emit(g, c, &ix.index, None, false)?;
    if model::optional_child(g, position.ty).is_some() {
        return fail(g, ix.index.loc, "optional array index needs ?? or !");
    }
    if !position.ty.numeric() {
        return fail(g, ix.index.loc, "array index must be numeric");
    }
    let at = format!("tsx::index({})", types::concrete(g, position)?.code);
    let child_copy = child.copy(g);
    let access = |code: &str| {
        if child_copy {
            format!("{code}[{at}]")
        } else {
            format!("{code}[{at}].clone()")
        }
    };
    if ty != value.ty {
        let name = temporary(g, c, b"_array")?;
        let body = access(&name);
        return Ok(Value {
            ty: model::optional(g, child)?,
            code: format!(
                "{}.map(|{name}| {body})",
                rust_text::operand(&value, Precedence::Suffix, false)
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    Ok(Value {
        ty: child,
        code: access(&rust_text::operand(&value, Precedence::Suffix, false)),
        precedence: Precedence::Suffix,
        ..Value::default()
    })
}
