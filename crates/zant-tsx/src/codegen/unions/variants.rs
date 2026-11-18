//! Union variants: discriminant fields, literal members and variant names.

use super::*;

/// One member of a union.
#[derive(Clone, Debug)]
pub struct Variant {
    pub rust: String,
    pub ty: Type,
    /// The discriminant's literal (`"circle"`) for object members.
    pub tag: Option<Vec<u8>>,
    /// What `typeof` says for it.
    pub type_of: &'static str,
}

pub(super) fn type_of_name(g: &Generator<'_>, ty: Type) -> &'static str {
    if ty == Type::String {
        "string"
    } else if ty.numeric() {
        "number"
    } else if ty == Type::Boolean {
        "boolean"
    } else if model::enum_type(g, ty) {
        "string"
    } else {
        "object"
    }
}

/// The single literal of a one-member string enum (`kind: "circle"`).
pub(super) fn literal(g: &Generator<'_>, ty: Type) -> Option<Vec<u8>> {
    let info = model::info(g, ty)?;
    (info.kind == InfoKind::Enumeration && info.members.len() == 1).then(|| info.members[0].clone())
}

/// A discriminant field and each member's tag (`None` for non-objects).
pub(super) type Discriminant = (Vec<u8>, Vec<Option<Vec<u8>>>);

/// The discriminant field and each object member's tag.
pub(super) fn discriminant(g: &Generator<'_>, types: &[Type]) -> Option<Discriminant> {
    let objects: Vec<Info> = types
        .iter()
        .filter_map(|ty| model::info(g, *ty).filter(|info| info.kind == InfoKind::Object))
        .collect();
    if objects.len() < 2 {
        return None;
    }
    for field in &objects[0].fields {
        let mut tags: Vec<Vec<u8>> = Vec::new();
        let all = objects.iter().all(|object| {
            object
                .fields
                .iter()
                .find(|candidate| candidate.name == field.name && !candidate.optional)
                .and_then(|candidate| literal(g, candidate.ty))
                .is_some_and(|tag| {
                    let fresh = !tags.contains(&tag);
                    tags.push(tag);
                    fresh
                })
        });
        if all {
            let per_type = types
                .iter()
                .map(|ty| {
                    model::info(g, *ty)
                        .filter(|info| info.kind == InfoKind::Object)
                        .and_then(|info| {
                            info.fields
                                .iter()
                                .find(|f| f.name == field.name)
                                .map(|f| f.ty)
                        })
                        .and_then(|ty| literal(g, ty))
                })
                .collect();
            return Some((field.name.clone(), per_type));
        }
    }
    None
}

/// The members of union `ty`.
pub fn variants(g: &Generator<'_>, ty: Type) -> Option<Vec<Variant>> {
    let info = model::info(g, ty).filter(|info| info.kind == InfoKind::Union)?;
    let tags = discriminant(g, &info.params).map(|(_, tags)| tags);
    Some(
        info.params
            .iter()
            .zip(&info.members)
            .enumerate()
            .map(|(index, (ty, name))| Variant {
                rust: String::from_utf8_lossy(name).into_owned(),
                ty: *ty,
                tag: tags.as_ref().and_then(|tags| tags[index].clone()),
                type_of: type_of_name(g, *ty),
            })
            .collect(),
    )
}

/// The discriminant field's name of union `ty`, if it has one.
pub fn discriminant_field(g: &Generator<'_>, ty: Type) -> Option<Vec<u8>> {
    model::info(g, ty)
        .filter(|info| info.kind == InfoKind::Union)
        .and_then(|info| info.fields.first().map(|field| field.name.clone()))
}

pub(super) fn pascal(text: &[u8]) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in String::from_utf8_lossy(text).chars() {
        if c.is_ascii_alphanumeric() {
            if upper {
                out.extend(c.to_uppercase());
            } else {
                out.push(c);
            }
            upper = false;
        } else {
            upper = true;
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'V');
    }
    out
}
