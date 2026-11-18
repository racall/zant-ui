//! Calling an imported Rust function: finding it in the source and checking its signature.

use super::*;

/// Lower imported calls with exactly the same contextual coercions as
/// component methods.
pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return Ok(None);
    };
    if scope::local(c, name).is_some() {
        return Ok(None);
    }
    let local = String::from_utf8_lossy(name).into_owned();
    let Some(function) = app_state::with_module(|state| {
        state
            .rust_functions
            .iter()
            .find(|function| function.local == local)
            .cloned()
    }) else {
        return Ok(None);
    };
    if !call.type_args.is_empty() {
        return fail(g, loc, "Rust functions do not take TSX type arguments");
    }
    if call.args.len() != function.params.len() {
        return fail(
            g,
            loc,
            format!("`{local}` expects {} arguments", function.params.len()),
        );
    }
    let mut arguments: Vec<String> = Vec::new();
    for (index, ty) in function.params.iter().enumerate() {
        let value = expr::emit(g, c, &call.args[index], Some(*ty), false)?;
        arguments.push(value.code);
    }
    Ok(Some(Value {
        ty: function.ret,
        code: format!("{}({})", function.target, arguments.join(", ")),
        ..Value::default()
    }))
}

/// The `pub fn name` item of `file`.
pub(super) fn find(file: &syn::File, name: &str) -> Option<syn::ItemFn> {
    file.items.iter().find_map(|item| match item {
        syn::Item::Fn(function)
            if matches!(function.vis, syn::Visibility::Public(_)) && function.sig.ident == name =>
        {
            Some(function.clone())
        }
        _ => None,
    })
}

/// The closest public function name, for "did you mean".
pub(super) fn closest(file: &syn::File, name: &str) -> Option<String> {
    let mut best: Option<String> = None;
    let mut distance = 3usize;
    for item in &file.items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        if !matches!(function.vis, syn::Visibility::Public(_)) {
            continue;
        }
        let candidate = function.sig.ident.to_string();
        let d = model::edit_distance(candidate.as_bytes(), name.as_bytes());
        if d < distance {
            best = Some(candidate);
            distance = d;
        }
    }
    best
}

/// The TSX signature of a `pub fn`.
pub(super) fn signature(
    g: &mut Generator<'_>,
    alias: &str,
    file: &syn::File,
    function: &syn::ItemFn,
    loc: ast::Loc,
) -> Result<Function> {
    let n = function.sig.ident.to_string();
    let mut params: Vec<Type> = Vec::new();
    for argument in &function.sig.inputs {
        let syn::FnArg::Typed(typed) = argument else {
            return unsupported(g, &n, "takes", "self", loc);
        };
        match map_type(g, alias, file, &typed.ty, false)? {
            Some(ty) => params.push(ty),
            None => return unsupported(g, &n, "takes", &quote_type(&typed.ty), loc),
        }
    }
    let ret = match &function.sig.output {
        syn::ReturnType::Default => Type::Void,
        syn::ReturnType::Type(_, ty) => match map_type(g, alias, file, ty, true)? {
            Some(ty) => ty,
            None => return unsupported(g, &n, "returns", &quote_type(ty), loc),
        },
    };
    Ok(Function {
        local: String::new(),
        target: format!("{alias}::{}", function.sig.ident),
        params,
        ret,
    })
}

pub(super) fn unsupported(
    g: &mut Generator<'_>,
    n: &str,
    verb: &str,
    spelling: &str,
    loc: ast::Loc,
) -> Result<Function> {
    fail(
        g,
        loc,
        format!(
            "`{n}` {verb} `{spelling}`; Rust functions imported into TSX use bool, string, numbers, optionals and slices of them"
        ),
    )
}

pub(super) fn quote_type(ty: &syn::Type) -> String {
    quote_tokens(ty).unwrap_or_else(|| "…".to_owned())
}

pub(super) fn quote_tokens(tokens: impl ToTokens) -> Option<String> {
    Some(tokens.to_token_stream().to_string().replace(' ', ""))
}
