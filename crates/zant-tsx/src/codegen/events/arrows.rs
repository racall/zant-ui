//! Inline arrows passed to event props: registered as methods, wrapped or forwarded.

use super::*;

/// Item actions use their own schema; JSX nested in any supported prop is
/// scanned too.
pub(super) fn scan_property(
    g: &mut Generator<'_>,
    prop: &tags::Prop,
    value: &ast::Expr,
    loc: ast::Loc,
) -> Result<()> {
    if prop.kind == Kind::ShortcutAction {
        // `(shortcut: string) => void`: a text action behind an adapter
        let mut text_prop = prop.clone();
        text_prop.kind = Kind::TextAction;
        return event(g, value, &text_prop, loc);
    }
    if is_action(prop.kind) {
        event(g, value, prop, loc)?;
    }
    let items: Vec<(String, &ast::Expr)> = match (&value.data, prop.kind) {
        (ast::ExprData::Array(items), Kind::StructList) => items
            .iter()
            .enumerate()
            .map(|(index, item)| (format!("{}[{}]", prop.name, index), &**item))
            .collect(),
        (ast::ExprData::Object(_), Kind::Struct) => vec![(prop.name.to_owned(), value)],
        _ => return scan_expr(g, value),
    };
    let schema = tags::Tag {
        name: "",
        builder: "",
        instance_id: false,
        content: tags::Content::None,
        content_field: "",
        props: prop.item_props.clone(),
    };
    for (owner, item) in items {
        let ast::ExprData::Object(properties) = &item.data else {
            continue;
        };
        for property in properties {
            if property.spread {
                continue;
            }
            if let Some(field) =
                tags::find_prop(&schema, &String::from_utf8_lossy(&property.key.name))
            {
                if let Err(err) = scan_property(g, field, &property.value, property.key.loc) {
                    if err == crate::codegen::Error::Tsx {
                        let prefix =
                            format!("in {owner}.{}", String::from_utf8_lossy(&property.key.name));
                        let _ = crate::codegen::contextualize(g, &prefix);
                    }
                    return Err(err);
                }
            } else {
                scan_expr(g, &property.value)?;
            }
        }
    }
    Ok(())
}

/// Registers an inline arrow as the handler method of `prop`.
pub(super) fn register_arrow(
    g: &mut Generator<'_>,
    prop: &tags::Prop,
    e: &ast::Expr,
) -> Result<usize> {
    let ast::ExprData::Arrow(function) = &e.data else {
        return fail(
            g,
            e.loc,
            "event handler must be a component function, plain function, setter, or inline arrow",
        );
    };
    g.counter += 1;
    let base = format!(
        "_{}{}",
        crate::codegen::jsx::snake_case(prop.name),
        g.counter
    );
    let name = scope::unique_method(g, &base)?;
    let ret = match function.ret.as_ref() {
        Some(reference) => types::type_ref(g, reference, true)?,
        None => Type::Void,
    };
    let capture_name = scope::unique_method(g, &format!("_Capture{}", g.counter))?;
    let method = Method {
        name,
        func: Some(function.clone()),
        action: Some(prop.kind),
        ret,
        capture_name: Some(capture_name),
        ..Method::default()
    };
    signature(g, &method, prop.kind, e.loc, true)?;
    g.methods.push(method);
    Ok(g.methods.len() - 1)
}

/// A component function that reads render locals, as an arrow expression.
pub(super) fn inline_function(g: &Generator<'_>, name: &[u8]) -> Option<Box<ast::Function>> {
    let index = scope::source_method(g, name)?;
    let method = &g.methods[index];
    if method.inline_only {
        method.func.clone()
    } else {
        None
    }
}

/// `fn` passed as a handler, as the arrow `() => fn()` / `(v) => fn(v)`.
pub(super) fn function_arrow(e: &ast::Expr, kind: Kind, pass_value: bool) -> ast::Expr {
    let takes_value = kind != Kind::Action;
    let mut args: Vec<Box<ast::Expr>> = Vec::new();
    let mut params: Vec<ast::Param> = Vec::new();
    if takes_value {
        if pass_value {
            args.push(Box::new(ast::Expr {
                loc: e.loc,
                data: ast::ExprData::Ident(b"_value".to_vec()),
            }));
        }
        params.push(ast::Param {
            name: ast::Name {
                name: b"_value".to_vec(),
                loc: e.loc,
            },
            ty: None,
            optional: false,
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            rest: false,
            default: None,
        });
    }
    let call = ast::Expr {
        loc: e.loc,
        data: ast::ExprData::Call(ast::Call {
            is_new: false,
            callee: Box::new(e.clone()),
            type_args: Vec::new(),
            args,
            optional: false,
        }),
    };
    let function = ast::Function {
        type_params: Vec::new(),
        loc: e.loc,
        name: Vec::new(),
        params,
        ret: None,
        body: ast::Body::Expr(Box::new(call)),
    };
    ast::Expr {
        loc: e.loc,
        data: ast::ExprData::Arrow(Box::new(function)),
    }
}

/// `fn` passed as a child component's callback prop of `count` parameters:
/// `(_a0, …) => fn(_a0, …)` (the prop gives the parameter types). Used by the
/// component layer.
pub fn forward_arrow(e: &ast::Expr, count: usize) -> ast::Expr {
    let mut args: Vec<Box<ast::Expr>> = Vec::new();
    let mut params: Vec<ast::Param> = Vec::new();
    for index in 0..count {
        let name = format!("_a{index}").into_bytes();
        args.push(Box::new(ast::Expr {
            loc: e.loc,
            data: ast::ExprData::Ident(name.clone()),
        }));
        params.push(ast::Param {
            name: ast::Name { name, loc: e.loc },
            ty: None,
            optional: false,
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            rest: false,
            default: None,
        });
    }
    let call = ast::Expr {
        loc: e.loc,
        data: ast::ExprData::Call(ast::Call {
            is_new: false,
            callee: Box::new(e.clone()),
            type_args: Vec::new(),
            args,
            optional: false,
        }),
    };
    let function = ast::Function {
        type_params: Vec::new(),
        loc: e.loc,
        name: Vec::new(),
        params,
        ret: None,
        body: ast::Body::Expr(Box::new(call)),
    };
    ast::Expr {
        loc: e.loc,
        data: ast::ExprData::Arrow(Box::new(function)),
    }
}
