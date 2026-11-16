//! Generic functions (`function first<T>(xs: T[]): T | null`), compiled by
//! monomorphization as AssemblyScript and C++ templates do: a generic
//! declaration is a template, and each distinct list of type arguments a
//! call needs becomes one concrete Rust function (`first__f64`), generated
//! with the parameters bound so its body sees concrete types.
//!
//! Type arguments are explicit (`first<number>(xs)`) or inferred like tsc:
//! each parameter type is matched against the type of its argument
//! (`T`, `T[]`, `T | null`, `Box<T>`, object fields), unmatched parameters
//! take their default, and `extends` constraints are checked. Arguments
//! whose type depends on the context (`{ … }`, `[]`, `null`, arrows) do not
//! take part in inference; when nothing else fixes a parameter the error
//! asks for explicit type arguments.

use crate::ast;

use super::expr;
use super::functions::{self, Function};
use super::generic_types::{self, with_env};
use super::model::{self, InfoKind};
use super::rust_text;
use super::scope;
use super::types::{Type, Value};
use super::{Generator, Result, fail};

/// Instances one template may have before the instantiation is considered
/// runaway (polymorphic recursion such as `f<T>(x: T) { f([x]) }`).
const MAX_INSTANCES: usize = 64;

#[derive(Clone, Debug)]
pub struct Template {
    pub name: Vec<u8>,
    pub func: Box<ast::Function>,
    /// (type arguments, index into `Generator::functions`)
    pub instances: Vec<(Vec<Type>, usize)>,
}

/// Registers a generic declaration (`functions::collect`).
pub fn register(g: &mut Generator<'_>, func: &ast::Function) -> Result<()> {
    if g.generic_functions
        .iter()
        .any(|template| template.name == func.name)
        || functions::find_index(g, &func.name).is_some()
    {
        return fail(
            g,
            func.loc,
            format!(
                "duplicate function `{}`",
                String::from_utf8_lossy(&func.name)
            ),
        );
    }
    g.generic_functions.push(Template {
        name: func.name.clone(),
        func: Box::new(func.clone()),
        instances: Vec::new(),
    });
    Ok(())
}

pub fn find(g: &Generator<'_>, name: &[u8]) -> Option<usize> {
    g.generic_functions
        .iter()
        .position(|template| template.name == name)
}

/// A call of template `index`: its type arguments, its instance, the call.
pub fn call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    index: usize,
) -> Result<Value> {
    let func = g.generic_functions[index].func.clone();
    let name = String::from_utf8_lossy(&func.name).into_owned();
    if call.type_args.len() > func.type_params.len() {
        return fail(
            g,
            loc,
            format!("`{name}` takes {} type arguments", func.type_params.len()),
        );
    }
    let mut bound: Vec<Option<Type>> = vec![None; func.type_params.len()];
    for (position, arg) in call.type_args.iter().enumerate() {
        bound[position] = Some(super::types::type_ref(g, arg, false)?);
    }
    let mut emitted: Vec<Option<Value>> = vec![None; call.args.len()];
    if call.type_args.is_empty() {
        infer(g, c, &func, call, &mut bound, &mut emitted)?;
    }
    // unresolved parameters take their defaults; constraints are checked
    let mut bindings: Vec<(Vec<u8>, Type)> = Vec::new();
    for (position, param) in func.type_params.iter().enumerate() {
        let ty = match (bound[position], &param.default) {
            (Some(ty), _) => ty,
            (None, Some(default)) => with_env(g, bindings.clone(), |g| {
                super::types::type_ref(g, default, false)
            })?,
            (None, None) => {
                let param = String::from_utf8_lossy(&param.name.name).into_owned();
                return fail(
                    g,
                    loc,
                    format!(
                        "cannot infer `{param}` of `{name}` from these arguments; pass it explicitly: `{name}<…>(…)`"
                    ),
                );
            }
        };
        bindings.push((param.name.name.clone(), ty));
        generic_types::check_constraint(g, param, &bindings, ty, loc)?;
    }
    let function = instance(g, index, bindings, loc)?;
    functions::call_with(g, c, loc, call, &function, emitted)
}

/// Binds type parameters from the arguments whose types do not depend on
/// the context, keeping their generated values for the call.
fn infer(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    func: &ast::Function,
    call: &ast::Call,
    bound: &mut [Option<Type>],
    emitted: &mut [Option<Value>],
) -> Result<()> {
    for (position, param) in func.params.iter().enumerate() {
        let Some(reference) = param.ty.as_ref() else {
            continue;
        };
        if param.rest {
            // `...items: T[]`: each further argument is one `T`
            let item = match reference.detail.as_deref() {
                Some(ast::TypeDetail::Array(item)) => item,
                _ => continue,
            };
            for arg in call.args.iter().skip(position) {
                if !self_typed(arg) {
                    continue;
                }
                let (target, arg) = match &arg.data {
                    ast::ExprData::Spread(inner) => (reference, &**inner),
                    _ => (item, &**arg),
                };
                let value = expr::emit(g, c, arg, None, false)?;
                unify(g, &func.type_params, target, value.ty, bound);
            }
            break;
        }
        let Some(arg) = call.args.get(position) else {
            break;
        };
        if !self_typed(arg) || !mentions(reference, &func.type_params) {
            continue;
        }
        let value = expr::emit(g, c, arg, None, false)?;
        unify(g, &func.type_params, reference, value.ty, bound);
        emitted[position] = Some(value);
    }
    Ok(())
}

/// Whether `e` has a type of its own (not one the context gives it).
fn self_typed(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Object(_) | ast::ExprData::NullLit | ast::ExprData::Arrow(_) => false,
        ast::ExprData::Array(items) => {
            !items.is_empty() && items.iter().all(|item| self_typed(item))
        }
        ast::ExprData::Spread(inner) => self_typed(inner),
        _ => true,
    }
}

/// Whether `reference` names one of `params`.
fn mentions(reference: &ast::TypeRef, params: &[ast::TypeParam]) -> bool {
    if reference.detail.is_none() && params.iter().any(|param| param.name.name == reference.name) {
        return true;
    }
    if reference.args.iter().any(|arg| mentions(arg, params)) {
        return true;
    }
    match reference.detail.as_deref() {
        Some(ast::TypeDetail::Array(item)) => mentions(item, params),
        Some(
            ast::TypeDetail::Union(members)
            | ast::TypeDetail::Intersection(members)
            | ast::TypeDetail::Tuple(members),
        ) => members.iter().any(|member| mentions(member, params)),
        Some(ast::TypeDetail::Object(fields)) => {
            fields.iter().any(|field| mentions(&field.ty, params))
        }
        Some(ast::TypeDetail::Func(f)) => {
            mentions(&f.ret, params)
                || f.params
                    .iter()
                    .any(|p| p.ty.as_ref().is_some_and(|ty| mentions(ty, params)))
        }
        _ => false,
    }
}

/// Matches `reference` (a parameter type) against `ty` (an argument type),
/// binding the type parameters it names; the first binding wins, as tsc's
/// first inference candidate does (a conflicting argument then fails to
/// convert, with its own location).
fn unify(
    g: &Generator<'_>,
    params: &[ast::TypeParam],
    reference: &ast::TypeRef,
    ty: Type,
    bound: &mut [Option<Type>],
) {
    if reference.detail.is_none() && reference.args.is_empty() {
        if let Some(position) = params
            .iter()
            .position(|param| param.name.name == reference.name)
            && bound[position].is_none()
            && ty != Type::Null
            && ty != Type::Void
        {
            bound[position] = Some(ty.concrete());
        }
        return;
    }
    if !reference.args.is_empty() {
        // `Box<T>` against an instance of `Box`
        if let Some(instance) = g.generic_instances.iter().find(|instance| {
            instance.ty == ty && g.type_decls[instance.decl].name.name == reference.name
        }) {
            let args = instance.args.clone();
            for (arg, ty) in reference.args.iter().zip(args) {
                unify(g, params, arg, ty, bound);
            }
        }
        return;
    }
    match reference.detail.as_deref() {
        Some(ast::TypeDetail::Array(item)) => {
            if let Some(child) = model::element(g, ty) {
                unify(g, params, item, child, bound);
            }
        }
        Some(ast::TypeDetail::Union(members)) => {
            // `T | null`: the present value is a `T`
            let present: Vec<&ast::TypeRef> = members
                .iter()
                .filter(|member| !matches!(member.detail.as_deref(), Some(ast::TypeDetail::Null)))
                .collect();
            if let [member] = present.as_slice() {
                let inner = model::optional_child(g, ty).unwrap_or(ty);
                unify(g, params, member, inner, bound);
            }
        }
        Some(ast::TypeDetail::Object(fields)) => {
            let Some(info) = model::info(g, ty).filter(|info| info.kind == InfoKind::Object) else {
                return;
            };
            for field in fields {
                if let Some(actual) = info
                    .fields
                    .iter()
                    .find(|actual| actual.name == field.name.name)
                {
                    let actual_ty = if field.optional {
                        model::optional_child(g, actual.ty).unwrap_or(actual.ty)
                    } else {
                        actual.ty
                    };
                    unify(g, params, &field.ty, actual_ty, bound);
                }
            }
        }
        _ => {}
    }
}

/// The concrete function for template `index` with `bindings`.
fn instance(
    g: &mut Generator<'_>,
    index: usize,
    bindings: Vec<(Vec<u8>, Type)>,
    loc: ast::Loc,
) -> Result<Function> {
    let args: Vec<Type> = bindings.iter().map(|(_, ty)| *ty).collect();
    let template = g.generic_functions[index].clone();
    if let Some((_, function)) = template.instances.iter().find(|(known, _)| *known == args) {
        return Ok(g.functions[*function].clone());
    }
    let name = String::from_utf8_lossy(&template.name).into_owned();
    if template.instances.len() >= MAX_INSTANCES {
        return fail(
            g,
            loc,
            format!(
                "`{name}` is instantiated with ever new type arguments (it calls itself with a growing type); give the recursive call fixed type arguments"
            ),
        );
    }
    let suffix: Vec<String> = args.iter().map(|ty| mangle(ty.rust(g))).collect();
    let rust = format!(
        "{}__{}",
        rust_text::id_at(g, &template.name, template.func.loc)?,
        suffix.join("__")
    );
    let mut func = (*template.func).clone();
    func.type_params.clear();
    let mut function = with_env(g, bindings.clone(), |g| {
        functions::signature(g, &func, rust, false)
    })?;
    function.type_env = bindings;
    function.throws = g.throwing.throws(&g.module_stem, &template.name);
    // instances are not callable by name; the template is
    function.name = format!("{name}<{}>", suffix.join(", "));
    let position = g.functions.len();
    g.functions.push(function.clone());
    g.generic_functions[index].instances.push((args, position));
    Ok(function)
}

/// A Rust type spelling as an identifier fragment (`tsx::List<f64>` →
/// `List_f64`).
fn mangle(rust: &str) -> String {
    let short: String = rust
        .split("::")
        .map(|segment| segment.trim())
        .collect::<Vec<_>>()
        .join("::")
        .replace("tsx::", "")
        .replace("zant::", "");
    let mut out = String::new();
    for c in short.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
}
