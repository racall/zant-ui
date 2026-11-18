//! The scan of a child component's JSX before emission: callback props and the arrows forwarded to them.

use super::*;

/// Registers inline arrows passed to callback props (before emission, like ui
/// event handlers) and scans everything else.
pub fn scan(g: &mut Generator<'_>, j: &ast::Jsx, component: &Component) -> Result<()> {
    for attr in spread::literals(g, &j.attrs)? {
        if attr.spread {
            if let Some(value) = attr.value.as_deref() {
                scan_expr_via(g, value)?;
            }
            continue;
        }
        let Some(value) = attr.value.as_deref() else {
            continue;
        };
        let Some(props) = component.props else {
            continue;
        };
        let Some(ty) = field_type(g, props, &attr.name.name) else {
            continue;
        };
        let Some(entry) = callback_info(g, ty) else {
            scan_expr_via(g, value)?;
            continue;
        };
        scan_callback(g, &attr.name.name, value, &entry)?;
    }
    Ok(())
}

/// Registers the handler method of one callback prop value (each branch of a
/// `cond ? a : b`).
pub(super) fn scan_callback(
    g: &mut Generator<'_>,
    attr: &[u8],
    value: &ast::Expr,
    entry: &model::Info,
) -> Result<()> {
    if let ast::ExprData::Cond(cond) = &value.data {
        scan_expr_via(g, &cond.cond)?;
        for branch in [&cond.then, &cond.else_] {
            if !absent_callback(branch) {
                scan_callback(g, attr, branch, entry)?;
            }
        }
        return Ok(());
    }
    {
        let function: Option<Box<ast::Function>> = match &value.data {
            ast::ExprData::Arrow(function) => Some(function.clone()),
            ast::ExprData::Ident(name) => {
                let name = String::from_utf8_lossy(name).into_owned();
                if let Some(function) = inline_function(g, &name) {
                    Some(Box::new(function))
                } else {
                    // a plain function of this file or an imported one, or a
                    // state setter: `(v) => fn(v)`
                    let plain = scope::source_method(g, name.as_bytes()).is_none()
                        && read_prop(g, &name)?.is_none()
                        && (functions::find_index(g, name.as_bytes()).is_some()
                            || scope::setter_index(g, name.as_bytes()).is_some());
                    if !plain {
                        return Ok(());
                    }
                    Some(Box::new(forward_arrow(g, value, entry.params.len())?))
                }
            }
            _ => return Ok(()),
        };
        let Some(function) = function else {
            return Ok(());
        };
        g.counter += 1;
        let name = scope::unique_method(
            g,
            &format!("_{}{}", String::from_utf8_lossy(attr), g.counter),
        )?;
        let capture_name = scope::unique_method(g, &format!("_Capture{}", g.counter))?;
        if function.params.len() > entry.params.len() {
            return fail(
                g,
                value.loc,
                format!("the callback passes {} arguments", entry.params.len()),
            );
        }
        let index = g.methods.len();
        g.methods.push(crate::codegen::Method {
            name,
            func: Some(function.clone()),
            action: action_kind(&entry.params),
            callback_params: Some(entry.params.clone()),
            capture_name: Some(capture_name),
            ..crate::codegen::Method::default()
        });
        g.callback_events.push(CallbackEvent {
            expr: value.clone(),
            method: index,
        });
        scan_body_via(g, &function.body)?;
    }
    Ok(())
}

pub(super) fn scan_expr_via(g: &mut Generator<'_>, e: &ast::Expr) -> Result<()> {
    match g.extensions.hooks.scan_expr {
        Some(hook) => hook(g, e),
        // `events::scan_expr` scans nested handlers; without the hook
        // installed there is nothing to scan.
        None => Ok(()),
    }
}

pub(super) fn scan_body_via(g: &mut Generator<'_>, body: &ast::Body) -> Result<()> {
    if let Some(hook) = g.extensions.hooks.scan_body {
        return hook(g, body);
    }
    Ok(())
}

/// A component function that reads render locals (`codegen.inlineFunction`).
pub(super) fn inline_function(g: &Generator<'_>, name: &str) -> Option<ast::Function> {
    let index = g
        .methods
        .iter()
        .position(|method| method.user && method.name == name && method.inline_only)?;
    g.methods[index].func.as_deref().cloned()
}

pub(super) fn forward_arrow(
    g: &mut Generator<'_>,
    e: &ast::Expr,
    count: usize,
) -> Result<ast::Function> {
    let mut params: Vec<ast::Param> = Vec::new();
    let mut args: Vec<Box<ast::Expr>> = Vec::new();
    for index in 0..count {
        let name = format!("_a{index}");
        params.push(ast::Param {
            name: ast::Name {
                name: name.as_bytes().to_vec(),
                loc: e.loc,
            },
            ty: None,
            optional: false,
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            rest: false,
            default: None,
        });
        args.push(Box::new(ast::Expr {
            loc: e.loc,
            data: ast::ExprData::Ident(name.into_bytes()),
        }));
    }
    let _ = g;
    Ok(ast::Function {
        type_params: Vec::new(),
        loc: e.loc,
        name: Vec::new(),
        params,
        ret: None,
        body: ast::Body::Expr(Box::new(ast::Expr {
            loc: e.loc,
            data: ast::ExprData::Call(ast::Call {
                is_new: false,
                callee: Box::new(e.clone()),
                type_args: Vec::new(),
                args,
                optional: false,
            }),
        })),
    })
}
