//! `useRef`: ref fields, reads, writes and bare refs.

use super::*;

/// `const r = useRef<T>(initial)`: a state entry of kind `.ref`.
pub fn collect_ref(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    d: &ast::Decl,
    call: &ast::Call,
) -> Result<()> {
    let ast::Pattern::Ident(pattern) = &d.pattern else {
        return fail(g, loc, "useRef returns one object: const ref = useRef(…)");
    };
    if !d.is_const {
        return fail(g, loc, "declare useRef with const");
    }
    if call.args.len() != 1 || call.type_args.len() > 1 {
        return fail(
            g,
            loc,
            "useRef takes one literal initial value and at most one type argument",
        );
    }
    let init = &call.args[0];
    let literal = types::is_literal(init)
        || matches!(
            init.data,
            ast::ExprData::Array(_) | ast::ExprData::Object(_) | ast::ExprData::NullLit
        );
    if !literal {
        return fail(g, init.loc, "useRef initial value must be a literal");
    }
    let mut c = scope::Context {
        render: true,
        ret: Type::Node,
        constant: true,
        ..scope::Context::default()
    };
    let target = match call.type_args.first() {
        Some(reference) => Some(types::type_ref(g, reference, false)?),
        None => None,
    };
    let mut value = expr::emit(g, &mut c, init, target, false)?;
    let concrete = target.unwrap_or_else(|| value.ty.concrete());
    value = types::coerce(g, value, concrete, init.loc)?;
    let name = String::from_utf8_lossy(&pattern.name).into_owned();
    if scope::state_index(g, name.as_bytes()).is_some()
        || scope::setter_index(g, name.as_bytes()).is_some()
    {
        return fail(
            g,
            loc,
            format!("duplicate state or ref name; rename `{name}`"),
        );
    }
    g.states.push(crate::codegen::State {
        name,
        setter: String::new(),
        ty: value.ty,
        initial: value.code,
        kind: crate::codegen::StateKind::Ref,
    });
    Ok(())
}

pub(super) fn ref_index(g: &Generator<'_>, c: &scope::Context, e: &ast::Expr) -> Option<usize> {
    let ast::ExprData::Ident(name) = &e.data else {
        return None;
    };
    if scope::local(c, name).is_some() {
        return None;
    }
    let index = scope::state_index(g, name)?;
    if g.states[index].kind == crate::codegen::StateKind::Ref {
        Some(index)
    } else {
        None
    }
}

/// `ref.current` (live value, in render and in handlers), or null when `m` is
/// not a ref access.
pub fn ref_read(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    m: &ast::Member,
) -> Result<Option<Value>> {
    let Some(index) = ref_index(g, c, &m.object) else {
        return Ok(None);
    };
    if m.name.name != b"current" {
        return fail(g, m.name.loc, "a ref only has `.current`");
    }
    let state = g.states[index].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    // non-`Copy` values are cloned at every read (a read never moves out
    // of the state field)
    Ok(Some(Value {
        ty: state.ty,
        code: format!("self.{field}.clone()"),
        ..Value::default()
    }))
}

/// `ref.current = v`, `ref.current += v`, `ref.current++` in handlers and
/// effects; null when the target is not a ref.
pub fn ref_write(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<Option<String>> {
    let target = match &e.data {
        ast::ExprData::Assign(assign) => &assign.target,
        ast::ExprData::Update(update) => &update.target,
        _ => return Ok(None),
    };
    let ast::ExprData::Member(m) = &target.data else {
        return Ok(None);
    };
    let Some(index) = ref_index(g, c, &m.object) else {
        return Ok(None);
    };
    if m.name.name != b"current" {
        return fail(g, m.name.loc, "a ref only has `.current`");
    }
    if c.render {
        return fail(
            g,
            e.loc,
            "refs are written in event handlers and effects, not while rendering",
        );
    }
    let state = g.states[index].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    if let ast::ExprData::Update(update) = &e.data {
        if !state.ty.numeric() {
            return fail(g, e.loc, "++ and -- require a number");
        }
        let op = if update.op == b"++" { "+=" } else { "-=" };
        let one = if state.ty.float() { "1.0" } else { "1" };
        return Ok(Some(format!("self.{field} {op} {one};\n")));
    }
    let ast::ExprData::Assign(assign) = &e.data else {
        unreachable!("assignment")
    };
    let value = if assign.op == b"=" {
        expr::emit(g, c, &assign.value, Some(state.ty), false)?
    } else {
        let binary = ast::Binary {
            op: assign.op[..assign.op.len().saturating_sub(1)].to_vec(),
            lhs: target.clone(),
            rhs: assign.value.clone(),
        };
        let value = expr::binary(g, c, e.loc, &binary, Some(state.ty), false)?;
        types::coerce(g, value, state.ty, e.loc)?
    };
    Ok(Some(format!("self.{field} = {};\n", value.code)))
}

/// A bare `ref` used as a value.
pub fn bare_ref(g: &mut Generator<'_>, index: usize, loc: ast::Loc) -> Result<Value> {
    fail(
        g,
        loc,
        format!(
            "read and write a ref through `{}.current`",
            g.states[index].name
        ),
    )
}

// ── dependency lists ──────────────────────────────────────────────────
