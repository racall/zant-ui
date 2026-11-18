//! Object types: interface members, intersections, tuples and the struct a literal or a declaration interns.

use super::*;

/// A field of an object type being declared, already resolved.
pub(in crate::codegen) struct Member {
    pub name: Vec<u8>,
    pub loc: ast::Loc,
    pub ty: Type,
    pub optional: bool,
}

pub(super) fn object(
    g: &mut Generator<'_>,
    fields: &[ast::TypeField],
    name: Option<&[u8]>,
) -> Result<Type> {
    object_with(g, name, |g| object_members(g, fields))
}

pub(in crate::codegen) fn object_members(
    g: &mut Generator<'_>,
    fields: &[ast::TypeField],
) -> Result<Vec<Member>> {
    let mut result: Vec<Member> = Vec::new();
    for field in fields {
        if result
            .iter()
            .any(|previous| previous.name == field.name.name)
        {
            return fail(
                g,
                field.name.loc,
                format!(
                    "duplicate field `{}`",
                    String::from_utf8_lossy(&field.name.name)
                ),
            );
        }
        let mut ty = types::type_ref(g, &field.ty, false)?;
        if field.optional {
            ty = optional(g, ty)?;
        }
        result.push(Member {
            name: field.name.name.clone(),
            loc: field.name.loc,
            ty,
            optional: field.optional,
        });
    }
    Ok(result)
}

/// `A & B` / `interface B extends A`: one struct with every member's fields,
/// as TypeScript's structural types see it. A field both sides declare must
/// have the same type; it is optional only when optional on every side.
pub(super) fn intersection(
    g: &mut Generator<'_>,
    members: &[ast::TypeRef],
    name: Option<&[u8]>,
) -> Result<Type> {
    object_with(g, name, |g| intersection_members(g, members))
}

pub(in crate::codegen) fn intersection_members(
    g: &mut Generator<'_>,
    members: &[ast::TypeRef],
) -> Result<Vec<Member>> {
    let mut result: Vec<Member> = Vec::new();
    for member in members {
        let ty = types::type_ref(g, member, false)?;
        let fields = match info(g, ty) {
            // another native spec's type is opaque here: its fields are
            // not known, so they cannot be copied
            Some(entry)
                if entry.kind == InfoKind::Object
                    && entry.fields.is_empty()
                    && entry.declaration.is_none() =>
            {
                return fail(
                    g,
                    member.loc,
                    "this type's fields are not known in this file; declare the combined type next to it",
                );
            }
            Some(entry) if entry.kind == InfoKind::Object => entry.fields,
            _ => {
                return fail(
                    g,
                    member.loc,
                    "`&` and `extends` combine object types (interfaces or `{ .. }` types)",
                );
            }
        };
        for field in fields {
            let required = optional_child(g, field.ty).filter(|_| field.optional);
            match result
                .iter_mut()
                .find(|previous| previous.name == field.name)
            {
                None => result.push(Member {
                    name: field.name,
                    loc: member.loc,
                    ty: field.ty,
                    optional: field.optional,
                }),
                Some(previous) => {
                    let previous_required =
                        optional_child(g, previous.ty).filter(|_| previous.optional);
                    let same = previous.ty == field.ty
                        || previous_required.unwrap_or(previous.ty) == required.unwrap_or(field.ty);
                    if !same {
                        let name = String::from_utf8_lossy(&field.name).into_owned();
                        return fail(
                            g,
                            member.loc,
                            format!(
                                "field `{name}` has different types in the combined types; declare it with one type"
                            ),
                        );
                    }
                    if previous.optional && !field.optional {
                        previous.ty = required.unwrap_or(field.ty);
                        previous.optional = false;
                    }
                }
            }
        }
    }
    Ok(result)
}

/// `[A, B]` as a Rust tuple `(A, B)` (interned by element types).
pub fn tuple(g: &mut Generator<'_>, items: Vec<Type>) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == InfoKind::Tuple && entry.params == items {
            return Ok(Type::Compound(index as u32));
        }
    }
    let spelled: Vec<String> = items.iter().map(|item| item.rust(g).to_owned()).collect();
    let rust = if spelled.len() == 1 {
        format!("({},)", spelled[0])
    } else {
        format!("({})", spelled.join(", "))
    };
    let copy = items.iter().all(|item| item.copy(g));
    add(
        g,
        Info {
            kind: InfoKind::Tuple,
            rust,
            params: items,
            copy,
            ..Info::default()
        },
    )
}

/// The element types when `ty` is a tuple.
pub fn tuple_items(g: &Generator<'_>, ty: Type) -> Option<Vec<Type>> {
    info(g, ty)
        .filter(|entry| entry.kind == InfoKind::Tuple)
        .map(|entry| entry.params)
}

/// The struct of an untyped object literal (`const p = { x: 1 }`): one
/// anonymous struct per shape, so literals of the same shape share a type.
pub(in crate::codegen) fn literal_object(
    g: &mut Generator<'_>,
    members: Vec<Member>,
) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        let same = entry.kind == InfoKind::Object
            && entry.rust.starts_with("_Object")
            && entry.declaration.is_some()
            && entry.fields.len() == members.len()
            && entry
                .fields
                .iter()
                .zip(&members)
                .all(|(field, member)| field.name == member.name && field.ty == member.ty);
        if same {
            return Ok(Type::Compound(index as u32));
        }
    }
    object_with(g, None, |_| Ok(members))
}

/// Declares the struct for an object type whose fields `members` resolves
/// (after the type is registered, so a field may refer back to it by name).
pub(super) fn object_with(
    g: &mut Generator<'_>,
    name: Option<&[u8]>,
    members: impl FnOnce(&mut Generator<'_>) -> Result<Vec<Member>>,
) -> Result<Type> {
    let external = match name {
        Some(name) => match g.extensions.hooks.external_name {
            Some(hook) => hook(g, &String::from_utf8_lossy(name))?,
            None => None,
        },
        None => None,
    };
    if name.is_none() && g.type_scope {
        return anonymous_external(g);
    }
    let is_external = external.is_some();
    let rust = match external {
        Some(external) => external,
        None => match name {
            Some(name) => rust_text::id_at(g, name, 0)?,
            None => format!("_Object{}", g.compounds.len()),
        },
    };
    let ty = add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: rust.clone(),
            ..Info::default()
        },
    )?;
    // another file's type keeps its alias key so it never shadows a type of
    // this file
    if let Some(name) = name {
        let alias = if rust != String::from_utf8_lossy(name) {
            rust.clone()
        } else {
            String::from_utf8_lossy(name).into_owned()
        };
        g.type_aliases
            .push(crate::codegen::TypeAlias { name: alias, ty });
    }
    let mut result: Vec<Field> = Vec::new();
    let mut code = String::new();
    let mut has_callback = false;
    let mut json_fields: Vec<crate::codegen::json::JsonField> = Vec::new();
    for member in members(g)? {
        // `manager: User | null` inside `User` needs a pointer in Rust; an
        // array (`reports: User[]`) already is one
        if contains_inline(g, member.ty, ty, 0) {
            return fail(
                g,
                member.loc,
                format!(
                    "`{}` contains its own type directly; recursive data goes through an array (`children: {}[]`)",
                    String::from_utf8_lossy(&member.name),
                    String::from_utf8_lossy(name.unwrap_or(b"T"))
                ),
            );
        }
        json_fields.push(crate::codegen::json::JsonField {
            name: String::from_utf8_lossy(&member.name).into_owned(),
            rust: rust_text::id_at(g, &member.name, member.loc)?,
            optional: member.optional,
        });
        has_callback = has_callback
            || callback_params(g, member.ty).is_some()
            || crate::codegen::closures::signature(
                g,
                optional_child(g, member.ty).unwrap_or(member.ty),
            )
            .is_some();
        code.push_str(&format!(
            "    pub {}: {},\n",
            rust_text::id_at(g, &member.name, member.loc)?,
            member.ty.rust(g)
        ));
        result.push(Field {
            name: member.name,
            ty: member.ty,
            optional: member.optional,
            rust: None,
        });
    }
    // `Rc<dyn Fn(..)>` implements neither `Default` nor `PartialEq`, so structs
    // that hold callback props derive `Clone` only and get a hand-written
    // `Default` with a no-op closure.
    let mut declaration = if has_callback {
        format!("#[derive(Clone)]\npub struct {rust} {{\n{code}}}\n\n")
    } else {
        format!("#[derive(Clone, PartialEq, Default)]\npub struct {rust} {{\n{code}}}\n\n")
    };
    if has_callback {
        let mut defaults = String::new();
        for field in &result {
            let field_name = rust_text::id_at(g, &field.name, 0)?;
            let function = crate::codegen::closures::signature(g, field.ty).is_some();
            let default = match callback_params(g, field.ty) {
                _ if function => crate::codegen::closures::default_code(g, field.ty),
                Some(params) if model_optional_child_is_none(g, field.ty) => {
                    let _ = params;
                    "None".to_owned()
                }
                Some(params) => match crate::codegen::components::action_kind(&params) {
                    // ui-event callbacks are the runtime action types
                    Some(kind) if !matches!(kind, tags::Kind::ShortcutAction) => {
                        let action = crate::codegen::components::action_type(kind);
                        let extra = match kind {
                            tags::Kind::BoolAction => ", _: bool",
                            tags::Kind::TextAction => ", _: &str",
                            tags::Kind::IndexAction => ", _: usize",
                            _ => "",
                        };
                        format!("{action}::new(|_cx{extra}| ())")
                    }
                    _ => {
                        let args = std::iter::once("_: &mut ui::ActionCx<'_>".to_owned())
                            .chain(params.iter().map(|ty| format!("_: {}", ty.rust(g))))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("std::rc::Rc::new(|{args}| ())")
                    }
                },
                None => "Default::default()".to_owned(),
            };
            defaults.push_str(&format!("            {field_name}: {default},\n"));
        }
        declaration.push_str(&format!(
            "impl Default for {rust} {{\n    fn default() -> Self {{\n        Self {{\n{defaults}        }}\n    }}\n}}\n\n"
        ));
    }
    let hot_fields = code.replace("pub ", "");
    declaration.push_str(&crate::codegen::generate::hot_impl(
        &rust,
        &hot_fields,
        true,
    ));
    // functions and elements are not data: a struct holding one has no
    // JSON form
    if !has_callback
        && result
            .iter()
            .all(|field| crate::codegen::json::check(g, field.ty).is_ok())
    {
        declaration.push_str(&crate::codegen::json::struct_impl(&rust, "", &json_fields));
    }
    let info = info_mut(g, ty);
    info.fields = result;
    // declared by the other file
    info.declaration = if is_external { None } else { Some(declaration) };
    Ok(ty)
}

/// Inline object / union types inside another file's declarations would be
/// separate Rust types in each file.
pub(super) fn anonymous_external<T>(g: &mut Generator<'_>) -> Result<T> {
    fail(
        g,
        0,
        "types shared between files must be named: declare nested object and union types as interfaces or type aliases",
    )
}
