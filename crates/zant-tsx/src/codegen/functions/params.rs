//! Parameters: types, required counts, defaults (including callback defaults) and missing arguments.

use super::*;

/// The default as a value of `ty`.
pub fn default_value(g: &mut Generator<'_>, e: &ast::Expr, ty: Type) -> Result<Value> {
    let mut c = scope::Context {
        render: true,
        ret: Type::Void,
        constant: true,
        ..scope::Context::default()
    };
    let value = expr::emit(g, &mut c, e, Some(ty), false)?;
    types::coerce(g, value, ty, e.loc)
}

/// Defaults are independent closures: they cannot borrow a component's render.
pub(super) fn callback_default(
    g: &mut Generator<'_>,
    e: &ast::Expr,
    ty: Type,
    params: &[Type],
    owner: &ast::Function,
) -> Result<Value> {
    let function = match &e.data {
        ast::ExprData::Arrow(function) => (**function).clone(),
        ast::ExprData::Ident(name) => {
            let Some(index) = find_index(g, name) else {
                return fail(
                    g,
                    e.loc,
                    "callback defaults must be an arrow or a file/imported function; they cannot read props, state or render locals",
                );
            };
            let target = &g.functions[index];
            if target.ret != Type::Void
                || target.params.len() > params.len()
                || target.params != params[..target.params.len()]
            {
                return fail(g, e.loc, "callback default has a different signature");
            }
            let count = target.params.len();
            let arrow = crate::codegen::events::forward_arrow(e, count);
            let ast::ExprData::Arrow(function) = arrow.data else {
                unreachable!("forward arrow")
            };
            *function
        }
        _ => {
            return fail(
                g,
                e.loc,
                "callback defaults must be an arrow or a file/imported function",
            );
        }
    };
    if function.params.len() > params.len() {
        return fail(g, e.loc, "callback default has too many parameters");
    }
    if let Some(ret) = &function.ret
        && types::type_ref(g, ret, true)? != Type::Void
    {
        return fail(g, e.loc, "callback defaults must return void");
    }

    // Reuse the lexical walker so an arrow's own parameters and block locals
    // are allowed, including inside array callbacks; only free reads are checked.
    struct FreeNames(Vec<Vec<u8>>);
    impl crate::hooks::rewrite::Free for FreeNames {
        fn resolve(
            &mut self,
            name: &[u8],
            loc: ast::Loc,
            kind: crate::hooks::rewrite::Kind,
        ) -> std::result::Result<Vec<u8>, crate::hooks::rewrite::Failure> {
            if kind != crate::hooks::rewrite::Kind::Value || self.0.iter().any(|n| n == name) {
                return Ok(name.to_vec());
            }
            Err(crate::hooks::rewrite::Failure {
                loc,
                msg: format!(
                    "callback defaults cannot read props, state or render locals (`{}`)",
                    String::from_utf8_lossy(name)
                ),
            })
        }
    }
    let mut free = FreeNames(
        g.functions
            .iter()
            .map(|f| f.name.as_bytes().to_vec())
            .chain(g.constants.iter().map(|c| c.name.as_bytes().to_vec()))
            .chain(
                [
                    "Math",
                    "Number",
                    "Date",
                    "performance",
                    "String",
                    "parseInt",
                    "parseFloat",
                    "isNaN",
                    "isFinite",
                    "console",
                ]
                .into_iter()
                .map(|name| name.as_bytes().to_vec()),
            )
            .filter(|name| {
                !owner.params.iter().any(|param| {
                    if param.destructure.is_empty() {
                        param.name.name == *name
                    } else {
                        param
                            .destructure
                            .iter()
                            .any(|prop| prop.local.name == *name)
                    }
                })
            })
            .collect(),
    );
    let mut walker = crate::hooks::rewrite::Rewriter {
        env: Vec::new(),
        type_names: Vec::new(),
        prefix: None,
        loc_tag: 0,
        free: Some(&mut free),
    };
    if let Err(error) = walker.function(&function) {
        return fail(g, error.loc, error.msg);
    }

    let mut c = scope::Context {
        no_self: true,
        ret: Type::Void,
        cx_var: Some("_cx".to_owned()),
        ..scope::Context::default()
    };
    let mut arguments = vec!["_cx: &mut ui::ActionCx<'_>".to_owned()];
    let mut prelude = String::new();
    let text_action = components::action_kind(params) == Some(crate::tags::Kind::TextAction);
    for (index, param) in params.iter().enumerate() {
        let Some(source) = function.params.get(index) else {
            arguments.push(format!(
                "_: {}",
                if text_action { "&str" } else { param.rust(g) }
            ));
            continue;
        };
        if !source.destructure.is_empty() || source.optional || source.default.is_some() {
            return fail(
                g,
                source.name.loc,
                "callback default parameters must be plain required names",
            );
        }
        if let Some(reference) = &source.ty
            && types::type_ref(g, reference, false)? != *param
        {
            return fail(
                g,
                source.name.loc,
                "callback default has a different signature",
            );
        }
        let binding = scope::bind(g, &mut c, &source.name.name, *param, false)?;
        c.params.push(binding);
        let name = &c.bindings[binding].rust;
        if text_action {
            arguments.push(format!("{name}: &str"));
            prelude.push_str(&format!("let {name} = zant::Str::copy({name});\n"));
        } else {
            arguments.push(format!("{name}: {}", param.rust(g)));
        }
    }
    // No render bindings are visible here. In particular, a module function
    // keeps its lexical scope even if a component prop uses the same name.
    let component = g.current_component.take();
    let states = std::mem::take(&mut g.states);
    let methods = std::mem::take(&mut g.methods);
    let result = (|| {
        let body = match &function.body {
            ast::Body::Block(ss) => stmt::statements(g, &mut c, ss, false)?,
            ast::Body::Expr(e) => stmt::expression_statement(g, &mut c, e)?,
        };
        scope::finish(g, &c, format!("{prelude}{body}"))
    })();
    g.current_component = component;
    g.states = states;
    g.methods = methods;
    let body = result?;
    let constructor = components::action_kind(params)
        .map(|kind| format!("{}::new", components::action_type(kind)))
        .unwrap_or_else(|| "std::rc::Rc::new".to_owned());
    Ok(Value::new(
        ty,
        format!("{constructor}(|{}| {{\n{body}}})", arguments.join(", ")),
    ))
}

/// A parameter's type: its annotation, else its default's.
pub fn param_type(g: &mut Generator<'_>, p: &ast::Param, owner: &[u8]) -> Result<Option<Type>> {
    if let Some(reference) = p.ty.as_ref() {
        return Ok(Some(types::type_ref(g, reference, false)?));
    }
    let Some(default) = p.default.as_deref() else {
        return Ok(None);
    };
    let mut c = scope::Context {
        render: true,
        ret: Type::Void,
        constant: true,
        ..scope::Context::default()
    };
    let value = expr::emit(g, &mut c, default, None, false)?;
    if value.ty == Type::Null || value.ty == Type::Void {
        return fail(
            g,
            default.loc,
            format!(
                "parameter `{}` of `{}` needs a type for this default",
                String::from_utf8_lossy(&p.name.name),
                String::from_utf8_lossy(owner)
            ),
        );
    }
    Ok(Some(value.ty.concrete()))
}

/// Required arguments: those before the first optional or defaulted one.
pub fn required(params: &[ast::Param]) -> usize {
    for (index, param) in params.iter().enumerate() {
        if param.optional || param.default.is_some() || param.rest {
            return index;
        }
    }
    params.len()
}

/// The argument code for a parameter the call left out.
pub fn missing(g: &mut Generator<'_>, p: &ast::Param, ty: Type) -> Result<String> {
    let Some(default) = p.default.as_deref() else {
        // an optional parameter
        return Ok("None".to_owned());
    };
    Ok(default_value(g, default, ty)?.code)
}

/// `{ size = 14 }`: reading `size` gives `props.size.clone().unwrap_or(14)`.
pub fn prop(
    g: &mut Generator<'_>,
    default: &ast::Expr,
    ty: Type,
    code: &str,
    owner: &ast::Function,
) -> Result<Value> {
    let base = model::optional_child(g, ty).unwrap_or(ty);
    let fallback = if let Some(entry) = components::callback_info(g, base) {
        callback_default(g, default, base, &entry.params, owner)?
    } else {
        default_value(g, default, base)?
    };
    let Some(child) = model::optional_child(g, ty) else {
        // TypeScript permits defaults on required properties. Their values
        // are already defined, so only validate the fallback's type.
        return Ok(scope::owned_read(g, ty, code.to_owned()));
    };
    Ok(Value {
        ty: child,
        // clone: the prop is read through `&self` (an `Option<Str>` cannot move out)
        code: if components::callback_info(g, child).is_some() {
            format!("({code}).clone().unwrap_or_else(|| {})", fallback.code)
        } else {
            format!("({code}).clone().unwrap_or({})", fallback.code)
        },
        ..Value::default()
    })
}

// ── functions ────────────────────────────────────────────────────────
