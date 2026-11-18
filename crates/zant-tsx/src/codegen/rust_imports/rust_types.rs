//! Mapping Rust types of imported functions to TSX types.

use super::*;

/// The TSX type of the Rust type `name` declared in `file`.
pub fn resolve_type(
    g: &mut Generator<'_>,
    alias: &str,
    file: &syn::File,
    name: &str,
) -> Result<Option<Type>> {
    let rust = format!("{alias}::{name}");
    if let Some(alias) = g.type_aliases.iter().find(|alias| alias.name == rust) {
        return Ok(Some(alias.ty));
    }
    let Some(item) = file.items.iter().find(|item| {
        let (is_public, ident) = match item {
            syn::Item::Struct(item) => {
                (matches!(item.vis, syn::Visibility::Public(_)), &item.ident)
            }
            syn::Item::Enum(item) => (matches!(item.vis, syn::Visibility::Public(_)), &item.ident),
            _ => return false,
        };
        is_public && ident == name
    }) else {
        return Ok(None);
    };
    match item {
        syn::Item::Enum(item) => {
            let mut members: Vec<Vec<u8>> = Vec::new();
            for variant in &item.variants {
                if !matches!(variant.fields, syn::Fields::Unit) {
                    continue;
                }
                members.push(variant.ident.to_string().into_bytes());
            }
            let ty = model::add(
                g,
                model::Info {
                    kind: model::InfoKind::Enumeration,
                    rust: rust.clone(),
                    members,
                    copy: true,
                    ..model::Info::default()
                },
            )?;
            g.type_aliases
                .push(crate::codegen::TypeAlias { name: rust, ty });
            Ok(Some(ty))
        }
        syn::Item::Struct(item) => {
            let mut fields: Vec<model::Field> = Vec::new();
            if let syn::Fields::Named(named) = &item.fields {
                for field in &named.named {
                    let Some(ident) = field.ident.as_ref() else {
                        continue;
                    };
                    let Some(ty) = map_type(g, alias, file, &field.ty, false).unwrap_or(None)
                    else {
                        continue;
                    };
                    fields.push(model::Field {
                        name: ident.to_string().into_bytes(),
                        ty,
                        optional: false,
                        rust: None,
                    });
                }
            }
            let ty = model::add(
                g,
                model::Info {
                    kind: model::InfoKind::Object,
                    rust: rust.clone(),
                    fields,
                    ..model::Info::default()
                },
            )?;
            g.type_aliases
                .push(crate::codegen::TypeAlias { name: rust, ty });
            Ok(Some(ty))
        }
        _ => Ok(None),
    }
}

/// A Rust type as a TSX type.
pub fn map_type(
    g: &mut Generator<'_>,
    alias: &str,
    file: &syn::File,
    ty: &syn::Type,
    allow_void: bool,
) -> Result<Option<Type>> {
    match ty {
        syn::Type::Paren(inner) => map_type(g, alias, file, &inner.elem, allow_void),
        syn::Type::Group(inner) => map_type(g, alias, file, &inner.elem, allow_void),
        syn::Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(allow_void.then_some(Type::Void)),
        syn::Type::Tuple(_) => Ok(None),
        syn::Type::Never(_) => Ok(None),
        syn::Type::Reference(reference) => {
            // `&'static [u8]`: imported asset bytes (`rust_imports::bytes`)
            let is_static = reference
                .lifetime
                .as_ref()
                .is_some_and(|lifetime| lifetime.ident == "static");
            if is_static
                && let syn::Type::Slice(slice) = &*reference.elem
                && matches!(&*slice.elem, syn::Type::Path(path)
                    if path.path.is_ident("u8"))
            {
                return Ok(Some(bytes(g)?));
            }
            map_type(g, alias, file, &reference.elem, allow_void)
        }
        syn::Type::Slice(slice) => {
            let Some(child) = map_type(g, alias, file, &slice.elem, false)? else {
                return Ok(None);
            };
            if child == Type::U8 {
                return Ok(Some(Type::String));
            }
            Ok(Some(model::array(g, child)?))
        }
        syn::Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return Ok(None);
            };
            let name = segment.ident.to_string();
            if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
                let mut inner: Vec<syn::Type> = Vec::new();
                for argument in &arguments.args {
                    if let syn::GenericArgument::Type(ty) = argument {
                        inner.push(ty.clone());
                    }
                }
                if name == "Option" && inner.len() == 1 {
                    let Some(child) = map_type(g, alias, file, &inner[0], false)? else {
                        return Ok(None);
                    };
                    if model::optional_child(g, child).is_some() {
                        return Ok(None);
                    }
                    return Ok(Some(model::optional(g, child)?));
                }
                if (name == "Vec" || name == "List") && inner.len() == 1 {
                    let Some(child) = map_type(g, alias, file, &inner[0], false)? else {
                        return Ok(None);
                    };
                    return Ok(Some(model::array(g, child)?));
                }
                return Ok(None);
            }
            if name == "bool" {
                return Ok(Some(Type::Boolean));
            }
            if allow_void && name == "void" {
                return Ok(Some(Type::Void));
            }
            if let Some(ty) = types::numeric_name(name.as_bytes()) {
                return Ok(Some(ty));
            }
            if name == "String" || name == "Str" || name == "str" {
                return Ok(Some(Type::String));
            }
            if name == "Bytes" || name == "Image" {
                if name == "Image" {
                    return match g.extensions.hooks.image_type {
                        Some(hook) => Ok(Some(hook(g)?)),
                        None => Ok(None),
                    };
                }
                return Ok(Some(bytes(g)?));
            }
            resolve_type(g, alias, file, &name)
        }
        _ => Ok(None),
    }
}

/// `Uint8Array` / `tsx::Bytes` / asset bytes: static PNG / file bytes.
///
/// The runtime takes imported assets as `&'static [u8]`
/// (`tsx::platform::asset_image`, `tsx::desktop::TrayOptions::icon`), so the
/// type keeps that spelling instead of `tsx::List<u8>`.
pub fn bytes(g: &mut Generator<'_>) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.rust == "&'static [u8]" {
            return Ok(Type::Compound(index as u32));
        }
    }
    model::add(
        g,
        model::Info {
            kind: model::InfoKind::Array,
            rust: "&'static [u8]".to_owned(),
            child: Type::U8,
            copy: true,
            ..model::Info::default()
        },
    )
}
