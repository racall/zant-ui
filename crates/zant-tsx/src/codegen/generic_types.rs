//! Generic type declarations: `interface Box<T> { value: T }`,
//! `type Pair<A, B = A> = { a: A; b: B }`, `type Maybe<T> = T | null`.
//!
//! Object types become one Rust generic struct (`pub struct Box<T>`), so the
//! same instantiation is the same Rust type in every file; each use
//! (`Box<number>`) is checked with its arguments substituted, so field reads,
//! formatting and equality see concrete types. Any other alias body is
//! substituted where it is used (`Maybe<string>` is `Option<zant::Str>`).
//! Arguments follow TypeScript: defaults may refer to earlier parameters and
//! constraints (`T extends { id: number }`) are checked per use.

use crate::ast;

use super::generate;
use super::model::{self, Info, InfoKind};
use super::rust_text;
use super::types::{self, Type};
use super::{Generator, Result, fail};

/// One use of a generic declaration with its type arguments.
#[derive(Clone, Debug)]
pub struct Instance {
    pub decl: usize,
    pub args: Vec<Type>,
    pub ty: Type,
}

/// The declaration index when `name` is a generic declaration of this file.
pub fn generic_decl(g: &Generator<'_>, name: &[u8]) -> Option<usize> {
    g.type_decls
        .iter()
        .position(|decl| decl.name.name == name && !decl.type_params.is_empty())
}

/// The type a type parameter currently stands for.
pub fn bound(g: &Generator<'_>, name: &[u8]) -> Option<Type> {
    g.type_env
        .iter()
        .rev()
        .find(|(param, _)| param == name)
        .map(|(_, ty)| *ty)
}

/// Runs `f` with `params` bound (`T` → `number`).
pub fn with_env<T>(
    g: &mut Generator<'_>,
    params: Vec<(Vec<u8>, Type)>,
    f: impl FnOnce(&mut Generator<'_>) -> Result<T>,
) -> Result<T> {
    let depth = g.type_env.len();
    g.type_env.extend(params);
    let result = f(g);
    g.type_env.truncate(depth);
    result
}

/// `Box<number>`: the type of a generic declaration with arguments.
pub fn instantiate(g: &mut Generator<'_>, reference: &ast::TypeRef, index: usize) -> Result<Type> {
    let decl = g.type_decls[index].clone();
    let name = String::from_utf8_lossy(&decl.name.name).into_owned();
    if reference.args.len() > decl.type_params.len() {
        return fail(
            g,
            reference.loc,
            format!("`{name}` takes {} type arguments", decl.type_params.len()),
        );
    }
    let bindings = arguments(g, &name, &decl.type_params, &reference.args, reference.loc)?;
    let args: Vec<Type> = bindings.iter().map(|(_, ty)| *ty).collect();
    if let Some(instance) = g
        .generic_instances
        .iter()
        .find(|instance| instance.decl == index && instance.args == args)
    {
        return Ok(instance.ty);
    }
    let ty = if is_object(&decl.ty) {
        declare(g, index)?;
        let members = with_env(g, bindings, |g| members(g, &decl.ty))?;
        let rust = format!(
            "{}<{}>",
            rust_text::id_at(g, &decl.name.name, decl.name.loc)?,
            args.iter()
                .map(|ty| ty.rust(g).to_owned())
                .collect::<Vec<_>>()
                .join(", ")
        );
        model::add(
            g,
            Info {
                kind: InfoKind::Object,
                rust,
                fields: members,
                ..Info::default()
            },
        )?
    } else {
        with_env(g, bindings, |g| types::type_ref(g, &decl.ty, false))?
    };
    g.generic_instances.push(Instance {
        decl: index,
        args,
        ty,
    });
    Ok(ty)
}

/// The arguments bound to `params`: given, else the default (which may use
/// earlier parameters); constraints checked.
pub fn arguments(
    g: &mut Generator<'_>,
    owner: &str,
    params: &[ast::TypeParam],
    given: &[ast::TypeRef],
    loc: ast::Loc,
) -> Result<Vec<(Vec<u8>, Type)>> {
    let mut bindings: Vec<(Vec<u8>, Type)> = Vec::new();
    for (position, param) in params.iter().enumerate() {
        let ty = match (given.get(position), &param.default) {
            (Some(arg), _) => types::type_ref(g, arg, false)?,
            (None, Some(default)) => {
                with_env(g, bindings.clone(), |g| types::type_ref(g, default, false))?
            }
            (None, None) => {
                return fail(
                    g,
                    loc,
                    format!(
                        "`{owner}` needs a type argument for `{}`: `{owner}<…>`",
                        String::from_utf8_lossy(&param.name.name)
                    ),
                );
            }
        };
        bindings.push((param.name.name.clone(), ty));
        check_constraint(g, param, &bindings, ty, loc)?;
    }
    Ok(bindings)
}

/// `T extends C`: the argument must be assignable to the constraint.
pub fn check_constraint(
    g: &mut Generator<'_>,
    param: &ast::TypeParam,
    bindings: &[(Vec<u8>, Type)],
    ty: Type,
    loc: ast::Loc,
) -> Result<()> {
    let Some(constraint) = &param.constraint else {
        return Ok(());
    };
    if matches!(constraint.name.as_slice(), b"unknown" | b"any") && constraint.detail.is_none() {
        return Ok(());
    }
    let expected = with_env(g, bindings.to_vec(), |g| {
        types::type_ref(g, constraint, false)
    })?;
    if assignable(g, ty, expected) {
        return Ok(());
    }
    let actual = ty.rust(g).to_owned();
    let wanted = spelling(constraint);
    fail(
        g,
        loc,
        format!(
            "`{actual}` does not satisfy `{} extends {wanted}`; pass a type with those fields",
            String::from_utf8_lossy(&param.name.name)
        ),
    )
}

/// The TypeScript spelling of a type annotation (for messages).
pub fn spelling(reference: &ast::TypeRef) -> String {
    let named = || {
        let name = String::from_utf8_lossy(&reference.name).into_owned();
        if reference.args.is_empty() {
            name
        } else {
            let args: Vec<String> = reference.args.iter().map(spelling).collect();
            format!("{name}<{}>", args.join(", "))
        }
    };
    let join = |members: &[ast::TypeRef], sep: &str| {
        members.iter().map(spelling).collect::<Vec<_>>().join(sep)
    };
    match reference.detail.as_deref() {
        None => named(),
        Some(ast::TypeDetail::Array(item)) => format!("{}[]", spelling(item)),
        Some(ast::TypeDetail::Union(members)) => join(members, " | "),
        Some(ast::TypeDetail::Intersection(members)) => join(members, " & "),
        Some(ast::TypeDetail::StringLit(text)) => format!("{:?}", String::from_utf8_lossy(text)),
        Some(ast::TypeDetail::Null) => "null".to_owned(),
        Some(ast::TypeDetail::Func(_)) => "(…) => …".to_owned(),
        Some(ast::TypeDetail::Object(fields)) => {
            let fields: Vec<String> = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}{}: {}",
                        String::from_utf8_lossy(&field.name.name),
                        if field.optional { "?" } else { "" },
                        spelling(&field.ty)
                    )
                })
                .collect();
            format!("{{ {} }}", fields.join("; "))
        }
    }
}

/// Structural assignability as constraints use it: equal types, or an object
/// with every field the target object requires (same field types).
pub fn assignable(g: &Generator<'_>, from: Type, to: Type) -> bool {
    if from == to || from.concrete() == to {
        return true;
    }
    let (Some(source), Some(target)) = (model::info(g, from), model::info(g, to)) else {
        return false;
    };
    if source.kind != InfoKind::Object || target.kind != InfoKind::Object {
        return false;
    }
    target.fields.iter().all(|field| {
        source.fields.iter().any(|candidate| {
            candidate.name == field.name
                && (candidate.ty == field.ty || assignable(g, candidate.ty, field.ty))
        }) || field.optional
    })
}

fn is_object(reference: &ast::TypeRef) -> bool {
    matches!(
        reference.detail.as_deref(),
        Some(ast::TypeDetail::Object(_) | ast::TypeDetail::Intersection(_))
    )
}

fn members(g: &mut Generator<'_>, reference: &ast::TypeRef) -> Result<Vec<model::Field>> {
    let resolved = match reference.detail.as_deref() {
        Some(ast::TypeDetail::Object(fields)) => model::object_members(g, fields)?,
        Some(ast::TypeDetail::Intersection(members)) => model::intersection_members(g, members)?,
        _ => unreachable!("object type"),
    };
    Ok(resolved
        .into_iter()
        .map(|member| model::Field {
            name: member.name,
            ty: member.ty,
            optional: member.optional,
            rust: None,
        })
        .collect())
}

/// Emits `pub struct Box<T> { … }` once, with each parameter as a Rust type
/// parameter of the same name.
fn declare(g: &mut Generator<'_>, index: usize) -> Result<()> {
    let decl = g.type_decls[index].clone();
    let name = rust_text::id_at(g, &decl.name.name, decl.name.loc)?;
    let params: Vec<String> = decl
        .type_params
        .iter()
        .map(|param| rust_text::id_at(g, &param.name.name, param.name.loc))
        .collect::<Result<_>>()?;
    let generic = format!("{name}<{}>", params.join(", "));
    if g.compounds.iter().any(|entry| entry.rust == generic) {
        return Ok(());
    }
    // each parameter as an opaque Rust type of its own name
    let mut bindings = Vec::new();
    for (param, rust) in decl.type_params.iter().zip(&params) {
        let ty = model::add(
            g,
            Info {
                kind: InfoKind::Object,
                rust: rust.clone(),
                ..Info::default()
            },
        )?;
        bindings.push((param.name.name.clone(), ty));
    }
    let fields = with_env(g, bindings, |g| members(g, &decl.ty))?;
    let mut code = String::new();
    for field in &fields {
        if model::info(g, field.ty).is_some_and(|entry| entry.kind == InfoKind::Callback) {
            return fail(
                g,
                decl.name.loc,
                "generic types with function fields are not supported yet",
            );
        }
        code.push_str(&format!(
            "    pub {}: {},\n",
            rust_text::id_at(g, &field.name, decl.name.loc)?,
            field.ty.rust(g)
        ));
    }
    let bounds = params
        .iter()
        .map(|param| format!("{param}: tsx::hot::Hot + Clone + Default"))
        .collect::<Vec<_>>()
        .join(", ");
    let hot = generate::hot_impl(&generic, &code.replace("pub ", ""), true).replacen(
        "impl tsx::hot::Hot for",
        &format!("impl<{bounds}> tsx::hot::Hot for"),
        1,
    );
    let declaration =
        format!("#[derive(Clone, PartialEq, Default)]\npub struct {generic} {{\n{code}}}\n\n{hot}");
    model::add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: generic,
            declaration: Some(declaration),
            ..Info::default()
        },
    )?;
    Ok(())
}
