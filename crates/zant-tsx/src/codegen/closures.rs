//! Functions as values: a function type that returns a value
//! (`(x: number) => number`) is `Rc<dyn Fn(f64) -> f64>`, so functions can be
//! stored in variables, fields and arrays, passed to and returned from
//! functions, and called (`f(x)`, `op.run(x)`, `ops[0].run(x)`).
//!
//! An arrow becomes a `move` closure over **copies** of what it reads from
//! its surroundings: locals, state and props, taken when the arrow is
//! created — React's per-render closure semantics. A `let` that may change
//! later cannot be captured (TypeScript would see the change), so that is a
//! diagnostic naming the fix. A plain function is a value too
//! (`items.map(double)` where a function is expected).
//!
//! Function types returning `void` stay UI callbacks (`components`).

use crate::ast;

use super::expr;
use super::free_names;
use super::functions;
use super::model::{self, Info, InfoKind};
use super::rust_text;
use super::scope;
use super::stmt;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// `(A, B) => R` (R not void).
pub fn function_type(g: &mut Generator<'_>, params: Vec<Type>, ret: Type) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == InfoKind::Function && entry.params == params && entry.child == ret {
            return Ok(Type::Compound(index as u32));
        }
    }
    let spelled: Vec<String> = params.iter().map(|ty| ty.rust(g).to_owned()).collect();
    let rust = format!(
        "std::rc::Rc<dyn Fn({}) -> {}>",
        spelled.join(", "),
        ret.rust(g)
    );
    model::add(
        g,
        Info {
            kind: InfoKind::Function,
            rust,
            child: ret,
            params,
            ..Info::default()
        },
    )
}

/// The parameter and return types when `ty` is a function value type.
pub fn signature(g: &Generator<'_>, ty: Type) -> Option<(Vec<Type>, Type)> {
    model::info(g, ty)
        .filter(|info| info.kind == InfoKind::Function)
        .map(|info| (info.params, info.child))
}

/// A closure that ignores its arguments and returns the default (the
/// `Default` of a struct holding a function field).
pub fn default_code(g: &Generator<'_>, ty: Type) -> String {
    let (params, ret) = signature(g, ty).unwrap_or_default();
    let args: Vec<String> = params
        .iter()
        .map(|ty| format!("_: {}", ty.rust(g)))
        .collect();
    format!(
        "std::rc::Rc::new(|{}| -> {} {{ Default::default() }})",
        args.join(", "),
        ret.rust(g)
    )
}

/// The type of an arrow whose parameters (and return, or expression body)
/// are annotated: `(x: number): number => …`.
pub fn arrow_type(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    arrow: &ast::Function,
) -> Result<Option<Type>> {
    let mut params = Vec::new();
    for param in &arrow.params {
        let Some(reference) = param.ty.as_ref() else {
            return Ok(None);
        };
        params.push(types::type_ref(g, reference, false)?);
    }
    let ret = match (&arrow.ret, &arrow.body) {
        (Some(reference), _) => types::type_ref(g, reference, false)?,
        (None, ast::Body::Expr(body)) => {
            // the body's type, read with the parameters bound
            let mut inner = scope::Context {
                ret: Type::Void,
                no_self: c.no_self,
                render: false,
                ..scope::Context::default()
            };
            for (param, ty) in arrow.params.iter().zip(&params) {
                scope::bind(g, &mut inner, &param.name.name, *ty, false)?;
            }
            for index in c.active.clone() {
                let binding = c.bindings[index].clone();
                let local = scope::bind(g, &mut inner, binding.name.as_bytes(), binding.ty, false)?;
                inner.bindings[local].rust = binding.rust.clone();
            }
            let value = expr::emit(g, &mut inner, body, None, false)?;
            value.ty.concrete()
        }
        _ => return Ok(None),
    };
    if ret == Type::Void || ret == Type::Node || ret == Type::OptionalNode {
        return Ok(None);
    }
    function_type(g, params, ret).map(Some)
}

/// An arrow where function type `ty` is expected.
pub fn arrow(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    arrow: &ast::Function,
    ty: Type,
) -> Result<Value> {
    let Some((params, ret)) = signature(g, ty) else {
        return fail(g, loc, "internal: function type expected");
    };
    if arrow.params.len() > params.len() {
        return fail(
            g,
            loc,
            format!("this function takes {} parameters", params.len()),
        );
    }
    // what it reads from around it, copied now
    let mut setup = String::new();
    let mut captured: Vec<(Vec<u8>, Type, String)> = Vec::new();
    let mut candidates: Vec<(Vec<u8>, Option<usize>)> = c
        .active
        .iter()
        .map(|index| (c.bindings[*index].name.as_bytes().to_vec(), Some(*index)))
        .filter(|(name, _)| !name.is_empty())
        .collect();
    for state in &g.states {
        candidates.push((state.name.as_bytes().to_vec(), None));
    }
    if let Some(component) = g.current_component
        && let Some(props) = g.components[component].props
        && let Some(info) = model::info(g, props)
    {
        for field in info.fields {
            candidates.push((field.name, None));
        }
    }
    for (name, binding) in candidates {
        let text = String::from_utf8_lossy(&name).into_owned();
        if arrow.params.iter().any(|param| param.name.name == name)
            || captured.iter().any(|(known, _, _)| *known == name)
            || !free_names::function(arrow, &[text.as_str()])
        {
            continue;
        }
        if let Some(index) = binding
            && c.bindings[index].mutable
        {
            return fail(
                g,
                loc,
                format!(
                    "this function reads `{text}`, a `let` that may change later; functions keep a copy of what they read, so make it a `const` or pass it as a parameter"
                ),
            );
        }
        let value = match scope::read(g, c, &name, loc) {
            Ok(value) => value,
            Err(_) => {
                *g.diag = None;
                continue;
            }
        };
        let slot = format!("_capture{}", captured.len());
        setup.push_str(&format!(
            "let {slot}: {} = {};\n",
            value.ty.rust(g),
            value.code
        ));
        captured.push((name, value.ty, slot));
    }
    // the body, as its own function
    let mut inner = scope::Context {
        ret,
        no_self: true,
        render: false,
        ..scope::Context::default()
    };
    let mut pattern = Vec::new();
    for (index, ty) in params.iter().enumerate() {
        let name = match arrow.params.get(index) {
            Some(param) => param.name.name.clone(),
            None => format!("_arg{index}").into_bytes(),
        };
        let binding = scope::bind(g, &mut inner, &name, *ty, false)?;
        inner.params.push(binding);
        pattern.push(format!("{}: {}", inner.bindings[binding].rust, ty.rust(g)));
    }
    for (name, ty, slot) in &captured {
        let binding = scope::bind(g, &mut inner, name, *ty, false)?;
        inner.bindings[binding].init = Some(format!("{slot}.clone()"));
    }
    let mut body = String::new();
    for index in inner.params.len()..inner.bindings.len() {
        body.push_str(&format!(
            "{}{index}{}",
            scope::PLACEHOLDER_START,
            scope::PLACEHOLDER_END
        ));
    }
    match &arrow.body {
        ast::Body::Block(statements) => {
            body.push_str(&stmt::statements(g, &mut inner, statements, false)?);
            if !stmt::returns_all(statements) {
                return fail(g, loc, "this function must return a value on every path");
            }
        }
        ast::Body::Expr(e) => {
            let value = expr::emit(g, &mut inner, e, Some(ret), false)?;
            let value = types::coerce(g, value, ret, e.loc)?;
            body.push_str(&format!("return {};\n", value.code));
        }
    }
    let body = scope::finish(g, &inner, body)?;
    Ok(Value::new(
        ty,
        format!(
            "{{\n{setup}let _function: {} = std::rc::Rc::new(move |{}| -> {} {{\n{body}}});\n_function\n}}",
            ty.rust(g),
            pattern.join(", "),
            ret.rust(g)
        ),
    ))
}

/// A plain function named where function type `ty` is expected.
pub fn named(g: &mut Generator<'_>, name: &[u8], ty: Type, loc: ast::Loc) -> Result<Option<Value>> {
    let Some(index) = functions::find_index(g, name) else {
        return Ok(None);
    };
    let function = g.functions[index].clone();
    let Some((params, ret)) = signature(g, ty) else {
        return Ok(None);
    };
    if function.params != params || function.ret != ret {
        return fail(
            g,
            loc,
            format!(
                "`{}` does not have the expected parameter and return types",
                String::from_utf8_lossy(name)
            ),
        );
    }
    let args: Vec<String> = (0..params.len())
        .map(|index| format!("_a{index}"))
        .collect();
    let pattern: Vec<String> = params
        .iter()
        .enumerate()
        .map(|(index, ty)| format!("_a{index}: {}", ty.rust(g)))
        .collect();
    Ok(Some(Value::new(
        ty,
        format!(
            "{{ let _function: {} = std::rc::Rc::new(|{}| {}({})); _function }}",
            ty.rust(g),
            pattern.join(", "),
            function.rust,
            args.join(", ")
        ),
    )))
}

/// `f(args)` on a function value `callee` (already evaluated).
pub fn call_value(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    callee: Value,
    args: &[Box<ast::Expr>],
) -> Result<Value> {
    let Some((params, ret)) = signature(g, callee.ty) else {
        return fail(g, loc, "this value is not a function");
    };
    if args.len() != params.len() {
        return fail(
            g,
            loc,
            format!("this function takes {} arguments", params.len()),
        );
    }
    let mut codes = Vec::new();
    for (arg, ty) in args.iter().zip(&params) {
        let value = expr::emit(g, c, arg, Some(*ty), false)?;
        codes.push(types::coerce(g, value, *ty, arg.loc)?.code);
    }
    Ok(Value::new(
        ret,
        format!(
            "({})({})",
            rust_text::operand(&callee, rust_text::Precedence::Suffix, false),
            codes.join(", ")
        ),
    ))
}

/// `f(args)` where `f` names a local, parameter or prop holding a function
/// value; `None` otherwise (nothing is emitted).
pub fn call_name(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return Ok(None);
    };
    let ty = match scope::local(c, name) {
        Some(binding) => binding.ty,
        None => {
            let prop = g
                .current_component
                .and_then(|component| g.components[component].props);
            let field = prop
                .and_then(|props| model::info(g, props))
                .and_then(|info| info.fields.into_iter().find(|field| field.name == *name));
            match field {
                Some(field) => field.ty,
                None => return Ok(None),
            }
        }
    };
    if signature(g, ty).is_none() {
        return Ok(None);
    }
    let callee = scope::read(g, c, name, loc)?;
    call_value(g, c, loc, callee, &call.args).map(Some)
}
