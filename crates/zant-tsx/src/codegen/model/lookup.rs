//! Looking types up by name, field types with spelling suggestions, callback types and the emitted declarations.

use super::*;

/// Lazily resolve named declarations, permitting references to later
/// declarations.
pub fn named(g: &mut Generator<'_>, name: &[u8]) -> Result<Option<Type>> {
    // a type parameter of the generic declaration being instantiated
    if let Some(ty) = crate::codegen::generic_types::bound(g, name) {
        return Ok(Some(ty));
    }
    if g.type_scope {
        return match g.extensions.hooks.imported_type {
            Some(hook) => hook(g, &String::from_utf8_lossy(name)),
            None => Ok(None),
        };
    }
    if let Some(alias) = g
        .type_aliases
        .iter()
        .find(|alias| alias.name.as_bytes() == name)
    {
        return Ok(Some(alias.ty));
    }
    if let Some(hook) = g.extensions.hooks.imported_type
        && let Some(ty) = hook(g, &String::from_utf8_lossy(name))?
    {
        return Ok(Some(ty));
    }
    if let Some(hook) = g.extensions.hooks.rust_type
        && let Some(ty) = hook(g, &String::from_utf8_lossy(name))?
    {
        return Ok(Some(ty));
    }
    let decl = g.type_decls.iter().position(|decl| decl.name.name == name);
    let Some(index) = decl else {
        return Ok(None);
    };
    if !g.type_decls[index].type_params.is_empty() {
        // `Box` alone: every parameter must have a default
        let reference = ast::TypeRef {
            name: name.to_vec(),
            loc: g.type_decls[index].name.loc,
            detail: None,
            args: Vec::new(),
        };
        return crate::codegen::generic_types::instantiate(g, &reference, index).map(Some);
    }
    if g.resolving[index] {
        return fail(
            g,
            g.type_decls[index].name.loc,
            "recursive types are not supported yet",
        );
    }
    g.resolving[index] = true;
    // Resolve the declaration *in place*: `types::type_ref` caches resolutions
    // by the address of a type detail, so resolving a clone would record
    // pointers into a temporary that is freed right after (`testdata/arrays`
    // hit that as a stale cache entry for `Box[]`).
    let placeholder = ast::TypeDecl {
        type_params: Vec::new(),
        // keep the name: a reference to the same type while resolving it must
        // find this declaration and report the recursion, not "unsupported"
        name: g.type_decls[index].name.clone(),
        ty: ast::TypeRef {
            args: Vec::new(),
            name: Vec::new(),
            loc: 0,
            detail: None,
        },
        exported: false,
    };
    let declaration = std::mem::replace(&mut g.type_decls[index], placeholder);
    let resolved = resolve(g, &declaration.ty, Some(name));
    g.type_decls[index] = declaration;
    g.resolving[index] = false;
    let ty = resolved?;
    if let Some(detail) = g.type_decls[index].ty.detail.as_deref() {
        g.resolved_types.push(crate::codegen::ResolvedType {
            detail: detail.clone(),
            ty,
        });
    }
    if let Some(alias) = g
        .type_aliases
        .iter()
        .find(|alias| alias.name.as_bytes() == name)
    {
        return Ok(Some(alias.ty));
    }
    g.type_aliases.push(crate::codegen::TypeAlias {
        name: String::from_utf8_lossy(name).into_owned(),
        ty,
    });
    Ok(Some(ty))
}

pub fn field_type(g: &mut Generator<'_>, ty: Type, name: &[u8], loc: ast::Loc) -> Result<Type> {
    let spelling = ty.rust(g).to_owned();
    let Some(entry) = info(g, ty) else {
        return fail(
            g,
            loc,
            format!(
                "{spelling} has no field `{}`",
                String::from_utf8_lossy(name)
            ),
        );
    };
    if entry.kind == InfoKind::Object {
        for field in &entry.fields {
            if field.name == name {
                return Ok(field.ty);
            }
        }
    }
    if let Some(near) = closest_field(&entry.fields, name) {
        return fail(
            g,
            loc,
            format!(
                "{spelling} has no field `{}`; did you mean `{}`?",
                String::from_utf8_lossy(name),
                String::from_utf8_lossy(&near)
            ),
        );
    }
    fail(
        g,
        loc,
        format!(
            "{spelling} has no field `{}`",
            String::from_utf8_lossy(name)
        ),
    )
}

/// The field whose name is within a couple of edits of `name` (typos).
pub(super) fn closest_field(fields: &[Field], name: &[u8]) -> Option<Vec<u8>> {
    let mut best: Option<Vec<u8>> = None;
    let mut best_distance = 3usize;
    for field in fields {
        let distance = edit_distance(&field.name, name);
        if distance < best_distance {
            best = Some(field.name.clone());
            best_distance = distance;
        }
    }
    best
}

/// Levenshtein distance for short identifiers (longer ones count as far).
pub fn edit_distance(a: &[u8], b: &[u8]) -> usize {
    if a.len() > 64 || b.len() > 64 {
        return usize::MAX;
    }
    let mut row = [0usize; 65];
    for (index, slot) in row.iter_mut().enumerate().take(b.len() + 1) {
        *slot = index;
    }
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

/// `(id: number) => void` as a callback type (interned by signature): the
/// `Rc<dyn Fn(..)>` by default. The component layer maps UI event signatures
/// to `ui::Action` through [`Generator::extensions`].
pub fn callback_type(g: &mut Generator<'_>, loc: ast::Loc, f: &ast::FuncType) -> Result<Type> {
    if !f.type_params.is_empty() {
        return fail(g, loc, "generic functions are not supported yet");
    }
    // a function that returns a value is a function value (`closures`)
    let ret = types::type_ref(g, &f.ret, true)?;
    if ret != Type::Void && ret != Type::Node && ret != Type::OptionalNode {
        let mut params = Vec::new();
        for param in &f.params {
            let Some(reference) = param.ty.as_ref() else {
                return fail(
                    g,
                    param.name.loc,
                    format!(
                        "parameter `{}` needs a type",
                        String::from_utf8_lossy(&param.name.name)
                    ),
                );
            };
            params.push(types::type_ref(g, reference, false)?);
        }
        return crate::codegen::closures::function_type(g, params, ret);
    }
    if let Some(hook) = g.extensions.hooks.callback_type {
        return hook(g, loc, f);
    }
    default_callback_type(g, loc, f)
}

/// The generic closure shape; used when no action-aware mapping is
/// installed.
pub fn default_callback_type(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    f: &ast::FuncType,
) -> Result<Type> {
    let ret = types::type_ref(g, &f.ret, true)?;
    if ret != Type::Void {
        return fail(g, loc, "callback props must return void");
    }
    let mut params: Vec<Type> = Vec::new();
    for param in &f.params {
        let Some(reference) = param.ty.as_ref() else {
            return fail(
                g,
                param.name.loc,
                format!(
                    "callback parameter `{}` needs a type",
                    String::from_utf8_lossy(&param.name.name)
                ),
            );
        };
        params.push(types::type_ref(g, reference, false)?);
    }
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == InfoKind::Callback && entry.params == params {
            return Ok(Type::Compound(index as u32));
        }
    }
    // the runtime context first, like `components::callback_type`: a
    // non-event callback reaches the root state through it
    let list = std::iter::once("&mut ui::ActionCx<'_>".to_owned())
        .chain(params.iter().map(|ty| ty.rust(g).to_owned()))
        .collect::<Vec<_>>()
        .join(", ");
    add(
        g,
        Info {
            kind: InfoKind::Callback,
            rust: format!("std::rc::Rc<dyn Fn({list})>"),
            params,
            ..Info::default()
        },
    )
}

/// The declarations of every compound type that declares one, in interning
/// order. The entry point prepends these to the generated module.
pub fn declarations(g: &Generator<'_>) -> String {
    let mut out = String::new();
    for entry in &g.compounds {
        if let Some(declaration) = entry.declaration.as_ref() {
            out.push_str(declaration);
        }
    }
    out
}

/// Whether a value of `ty` holds a `target` inline (no array, dictionary
/// or set in between): Rust would need a pointer for it.
pub(super) fn contains_inline(g: &Generator<'_>, ty: Type, target: Type, depth: usize) -> bool {
    if ty == target {
        return true;
    }
    if depth > 32 {
        return false;
    }
    let Some(info) = info(g, ty) else {
        return false;
    };
    match info.kind {
        InfoKind::Optional => contains_inline(g, info.child, target, depth + 1),
        InfoKind::Object => info
            .fields
            .iter()
            .any(|field| contains_inline(g, field.ty, target, depth + 1)),
        InfoKind::Tuple | InfoKind::Union => info
            .params
            .iter()
            .any(|param| contains_inline(g, *param, target, depth + 1)),
        _ => false,
    }
}
