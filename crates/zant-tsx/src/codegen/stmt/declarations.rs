//! `const` / `let` declarations, with tuple and object destructuring.

use super::*;

/// Emit a typed local or skip a registered top-level hook or component
/// function.
pub(super) fn declaration(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    top: bool,
) -> Result<String> {
    let Some(e) = d.init.as_deref() else {
        return fail(g, loc, "local declarations need an initializer");
    };
    if crate::codegen::hook(g, e) {
        if let ast::ExprData::Call(call) = &e.data
            && let ast::ExprData::Ident(name) = &call.callee.data
            && (name.as_slice() == b"useMemo" || name.as_slice() == b"useStore")
        {
            return match g.extensions.hooks.hook_declaration {
                Some(hook) => hook(g, c, loc, d, top),
                None => fail(
                    g,
                    loc,
                    "`useMemo` / `useStore` need the hook support installed",
                ),
            };
        }
        if let ast::ExprData::Call(call) = &e.data
            && let ast::ExprData::Ident(name) = &call.callee.data
            && (g.extensions.is_desktop_hook)(&String::from_utf8_lossy(name))
        {
            return match g.extensions.hooks.hook_declaration {
                Some(hook) => hook(g, c, loc, d, top),
                None => fail(g, loc, "desktop hooks need the application hooks installed"),
            };
        }
        if c.render && top {
            return Ok(String::new());
        }
        return fail(
            g,
            e.loc,
            "hooks must be called at the top level of the component",
        );
    }
    if let ast::Pattern::Object(bindings) = &d.pattern {
        return object_declaration(g, c, d, e, bindings);
    }
    if let ast::Pattern::Array(names) = &d.pattern {
        return tuple_declaration(g, c, loc, d, e, names);
    }
    let ast::Pattern::Ident(pattern) = &d.pattern else {
        return fail(g, loc, "destructuring is only supported for useState");
    };
    if let ast::ExprData::Arrow(arrow) = &e.data {
        // at a component's top level a `void` arrow is a handler (a method);
        // one with typed parameters that returns a value is a function value
        let value_type = if c.render && top && d.ty.is_none() {
            let mark = g.diag.is_none();
            match crate::codegen::closures::arrow_type(g, c, arrow) {
                Ok(ty) => ty,
                Err(_) if mark => {
                    *g.diag = None;
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        if c.render && top && value_type.is_none() {
            return Ok(String::new());
        }
        // `const f = (x: number): number => …`: a function value
        let annotated = match (d.ty.as_ref(), value_type) {
            (Some(t), _) => Some(types::type_ref(g, t, false)?),
            (None, Some(ty)) => Some(ty),
            (None, None) => crate::codegen::closures::arrow_type(g, c, arrow)?,
        };
        let Some(ty) = annotated.filter(|ty| crate::codegen::closures::signature(g, *ty).is_some())
        else {
            // a procedure (it sets state, it is async): a local function
            if !c.render && crate::codegen::local_functions::registered(g, arrow) {
                return crate::codegen::local_functions::declare(g, c, e.loc, &pattern.name, arrow);
            }
            return fail(
                g,
                e.loc,
                "a local function needs typed parameters and a return value (`(x: number): number => …`)",
            );
        };
        let value = crate::codegen::closures::arrow(g, c, e.loc, arrow, ty)?;
        let binding = scope::bind(g, c, &pattern.name, ty, !d.is_const)?;
        c.bindings[binding].init = Some(value.code);
        c.bindings[binding].fresh = true;
        return Ok(format!(
            "{}{}{}",
            scope::PLACEHOLDER_START,
            binding,
            scope::PLACEHOLDER_END
        ));
    }
    let target = match d.ty.as_ref() {
        Some(t) => Some(types::type_ref(g, t, false)?),
        None => None,
    };
    let mut v = expr::emit(g, c, e, target, false)?;
    let concrete_ty = target.unwrap_or_else(|| v.ty.concrete());
    v = types::coerce(g, v, concrete_ty, e.loc)?;
    if v.ty == Type::Void || v.ty == Type::Null {
        return fail(g, e.loc, "local must have a value with a concrete type");
    }
    let binding = scope::bind(g, c, &pattern.name, v.ty, !d.is_const)?;
    c.bindings[binding].init = Some(v.code);
    c.bindings[binding].fresh = crate::codegen::place::fresh(e);
    Ok(format!(
        "{}{}{}",
        scope::PLACEHOLDER_START,
        binding,
        scope::PLACEHOLDER_END
    ))
}

/// `const [a, b] = pair`: a tuple's elements, from one evaluation.
pub(super) fn tuple_declaration(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    e: &ast::Expr,
    names: &[Option<ast::Name>],
) -> Result<String> {
    let target =
        d.ty.as_ref()
            .map(|ty| types::type_ref(g, ty, false))
            .transpose()?;
    let mut value = expr::emit(g, c, e, target, false)?;
    if let Some(target) = target {
        value = types::coerce(g, value, target, e.loc)?;
    }
    let Some(items) = crate::codegen::model::tuple_items(g, value.ty) else {
        return fail(
            g,
            loc,
            "array destructuring needs a tuple (`[string, number]`); index arrays instead (`items[0]`)",
        );
    };
    if names.len() > items.len() {
        return fail(g, loc, format!("this tuple has {} elements", items.len()));
    }
    let whole = scope::bind(g, c, b"_tuple", value.ty, false)?;
    c.bindings[whole].init = Some(value.code);
    let whole_rust = c.bindings[whole].rust.clone();
    let mut out = format!(
        "{}{whole}{}",
        scope::PLACEHOLDER_START,
        scope::PLACEHOLDER_END
    );
    for (position, name) in names.iter().enumerate() {
        let Some(name) = name else { continue };
        let ty = items[position];
        let read = if ty.copy(g) {
            format!("{whole_rust}.{position}")
        } else {
            format!("{whole_rust}.{position}.clone()")
        };
        let binding = scope::bind(g, c, &name.name, ty, !d.is_const)?;
        c.bindings[binding].init = Some(read);
        out.push_str(&format!(
            "{}{binding}{}",
            scope::PLACEHOLDER_START,
            scope::PLACEHOLDER_END
        ));
    }
    Ok(out)
}

/// Read a typed object's fields from one evaluation of the initializer.
pub(super) fn object_declaration(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    d: &ast::Decl,
    e: &ast::Expr,
    bindings: &[ast::ObjectBinding],
) -> Result<String> {
    let target =
        d.ty.as_ref()
            .map(|ty| types::type_ref(g, ty, false))
            .transpose()?;
    let mut value = expr::emit(g, c, e, target, false)?;
    if let Some(target) = target {
        value = types::coerce(g, value, target, e.loc)?;
    }
    if model::info(g, value.ty).is_none_or(|info| info.kind != model::InfoKind::Object) {
        return fail(
            g,
            e.loc,
            "object destructuring needs an object with a known type",
        );
    }
    let source = scope::bind(g, c, b"_destructure", value.ty, false)?;
    c.bindings[source].init = Some(value.code);
    // This compiler-generated temporary is not a TSX local.
    c.active.pop();
    let mut out = format!(
        "{}{source}{}",
        scope::PLACEHOLDER_START,
        scope::PLACEHOLDER_END
    );
    for binding in bindings {
        let ty = model::field_type(g, value.ty, &binding.key.name, binding.key.loc)?;
        let field = match model::field_rust(g, value.ty, &binding.key.name) {
            Some(field) => field,
            None => rust_text::id_at(g, &binding.key.name, binding.key.loc)?,
        };
        let code = format!("{}.{}", c.bindings[source].rust, field);
        let read = scope::owned_read(g, ty, code);
        let local = scope::bind(g, c, &binding.local.name, ty, !d.is_const)?;
        c.bindings[local].init = Some(read.code);
        out.push_str(&format!(
            "{}{local}{}",
            scope::PLACEHOLDER_START,
            scope::PLACEHOLDER_END
        ));
    }
    Ok(out)
}
