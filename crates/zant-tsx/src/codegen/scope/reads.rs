//! Reading a name as a value: locals, captures, states, methods and module items.

use super::*;

/// Handlers read method-entry snapshots.
pub fn read(g: &mut Generator<'_>, c: &mut Context, n: &[u8], loc: ast::Loc) -> Result<Value> {
    let projection = std::mem::take(&mut c.projection);
    if let Some(value) = narrowing::find(c, n) {
        return Ok(value);
    }
    if let Some(index) = local_index(c, n) {
        let ty = c.bindings[index].ty;
        let code = c.bindings[index].rust.clone();
        if !projection && model::owned(g, ty) {
            c.bindings[index].escaped = true;
        }
        return Ok(owned_read(g, ty, code));
    }
    let name = lossy(n);
    if let Some(hook) = g.extensions.hooks.captures_read
        && let Some(value) = hook(g, c, &name)?
    {
        return Ok(value);
    }
    if let Some(hook) = g.extensions.hooks.components_read_prop
        && let Some(value) = hook(g, &name)?
    {
        if c.cleanup {
            return fail(
                g,
                loc,
                "a cleanup runs after the props are gone; copy what it needs into a local",
            );
        }
        return Ok(value);
    }
    if let Some(index) = state_index(g, n) {
        let state = g.states[index].clone();
        if state.kind == crate::codegen::StateKind::Ref {
            return match g.extensions.hooks.bare_ref {
                Some(hook) => hook(g, index, loc),
                None => fail(g, loc, "`useRef` reads need the hook support installed"),
            };
        }
        let field = state_field(g, state.name.as_bytes())?;
        if c.render {
            return Ok(Value {
                ty: state.ty,
                code: if state.ty.copy(g) {
                    format!("self.{field}")
                } else {
                    format!("self.{field}.clone()")
                },
                precedence: rust_text::Precedence::Suffix,
                ..Value::default()
            });
        }
        for snapshot in &c.snapshots {
            if snapshot.state == index {
                return Ok(owned_read(
                    g,
                    state.ty,
                    c.bindings[snapshot.binding].rust.clone(),
                ));
            }
        }
        let binding = bind(g, c, n, state.ty, false)?;
        c.snapshots.push(Snapshot {
            state: index,
            binding,
        });
        return Ok(owned_read(g, state.ty, c.bindings[binding].rust.clone()));
    }
    if let Some(constant) = g
        .constants
        .iter()
        .find(|constant| constant.name.as_bytes() == n)
    {
        // the constant's code is the accessor (`name()`, see globals.rs)
        let ty = constant.value.ty;
        let code = constant.value.code.clone();
        return Ok(owned_read(g, ty, code));
    }
    if let Some(hook) = g.extensions.hooks.zant_api_read
        && let Some(value) = hook(g, c, &name, loc)?
    {
        return Ok(value);
    }
    if !c.render
        && let Some(name) = g
            .render_names
            .iter()
            .find(|render_name| render_name.as_bytes() == n)
    {
        return fail(
            g,
            loc,
            format!(
                "component functions cannot capture render locals; use an inline event handler (`{name}`)"
            ),
        );
    }
    // a function used as a value (`{ save }`, `const f = save`, `[save]`):
    // only handler positions take functions
    if crate::codegen::functions::find_index(g, n).is_some() || source_method(g, n).is_some() {
        return fail(
            g,
            loc,
            format!(
                "`{0}` is a function; functions can't be stored in objects, arrays or variables yet. \
                 Pass it directly where a handler goes (`onClick={{{0}}}`) or call it (`{0}()`)",
                String::from_utf8_lossy(n)
            ),
        );
    }
    // the global number constants (any local of the same name came first)
    match n {
        b"NaN" => return Ok(Value::new(Type::F64, "f64::NAN")),
        b"Infinity" => return Ok(Value::new(Type::F64, "f64::INFINITY")),
        _ => {}
    }
    fail(
        g,
        loc,
        format!("unknown name `{}`", String::from_utf8_lossy(n)),
    )
}

/// A read of a value: non-`Copy` binds clone (`tsx-codegen.md` §3).
pub fn owned_read(g: &Generator<'_>, ty: Type, code: String) -> Value {
    if ty.copy(g) {
        Value::new(ty, code)
    } else {
        Value {
            ty,
            code: format!("{code}.clone()"),
            precedence: rust_text::Precedence::Suffix,
            ..Value::default()
        }
    }
}
