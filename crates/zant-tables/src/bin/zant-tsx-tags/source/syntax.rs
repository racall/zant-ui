//! Reading `syn` items: builder functions, type helpers, the theme struct and `use` trees.

use super::*;

/// `pub fn name(options: X) -> Node`, `pub fn text(value: impl Into<Str>, options: X) -> Node`.
pub(super) fn builder_of(item_fn: &syn::ItemFn) -> Option<Builder> {
    if !matches!(item_fn.vis, syn::Visibility::Public(_)) {
        return None;
    }
    let return_type = match &item_fn.sig.output {
        syn::ReturnType::Type(_, ty) => ty,
        syn::ReturnType::Default => return None,
    };
    if last_type_ident(return_type).as_deref() != Some("Node") {
        return None;
    }
    let params: Vec<&syn::Type> = item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(typed) => Some(&*typed.ty),
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    let options = last_type_ident(params.last()? as &syn::Type)?;
    let text_arg = params.len() == 2 && matches!(params[0], syn::Type::ImplTrait(_));
    let raw = item_fn.sig.ident.to_string();
    let name = raw.strip_prefix("r#").unwrap_or(&raw).to_string();
    Some(Builder {
        raw_ident: raw.starts_with("r#"),
        name,
        options,
        text_arg,
    })
}

/// Last path segment of a type (`BoxOptions`, `f32`, `Node`).
pub fn last_type_ident(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => last_type_ident(&reference.elem),
        _ => None,
    }
}

/// Type arguments of `Option<..>` / `Vec<..>` / `Rc<..>`.
pub fn type_arguments(ty: &syn::Type) -> Vec<&syn::Type> {
    let syn::Type::Path(path) = ty else {
        return Vec::new();
    };
    let Some(segment) = path.path.segments.last() else {
        return Vec::new();
    };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Vec::new();
    };
    arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect()
}

/// Path segments of a type path (`flex::Wrap` → `["flex", "Wrap"]`).
pub fn type_segments(ty: &syn::Type) -> Option<Vec<String>> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    Some(
        path.path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect(),
    )
}

pub(super) fn theme_fields(file: &syn::File) -> Vec<ThemeField> {
    for item in &file.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        if item_struct.ident != "Theme" {
            continue;
        }
        let syn::Fields::Named(named) = &item_struct.fields else {
            continue;
        };
        return named
            .named
            .iter()
            .filter_map(|field| {
                Some(ThemeField {
                    name: field.ident.as_ref()?.to_string(),
                    ty: last_type_ident(&field.ty)?,
                })
            })
            .collect();
    }
    Vec::new()
}

pub(super) fn walk_use_tree(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    module: &[String],
    entry: &mut Module,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            walk_use_tree(&path.tree, prefix, module, entry);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let local = name.ident.to_string();
            if let Some(absolute) = absolute_use_path(module, prefix, &local) {
                entry.aliases.insert(local, absolute);
            }
        }
        UseTree::Rename(rename) => {
            let local = rename.rename.to_string();
            let original = rename.ident.to_string();
            if let Some(absolute) = absolute_use_path(module, prefix, &original) {
                entry.aliases.insert(local, absolute);
            }
        }
        UseTree::Glob(_) => {
            if let Some(absolute) = absolute_use_path(module, prefix, "") {
                entry.globs.push(absolute);
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                walk_use_tree(item, prefix, module, entry);
            }
        }
    }
}

/// Turn `crate::a::b` / `crate::c::D` into an absolute crate path; `None` for
/// external crates, which the table never needs.
pub(super) fn absolute_use_path(
    module: &[String],
    prefix: &[String],
    name: &str,
) -> Option<Vec<String>> {
    let Some(first) = prefix.first() else {
        // `use Name;` resolves inside the current module.
        let mut path = module.to_vec();
        path.push(name.to_string());
        return Some(path);
    };
    let (mut absolute, rest) = match first.as_str() {
        "crate" => (vec!["crate".to_string()], &prefix[1..]),
        "self" => (module.to_vec(), &prefix[1..]),
        "super" => {
            let mut base = module.to_vec();
            let mut index = 0;
            while index < prefix.len() && prefix[index] == "super" {
                base.pop();
                index += 1;
            }
            (base, &prefix[index..])
        }
        // A plain path is module-relative in the `zant` sources
        // (`pub use data::{Icon}` in `core/node/mod.rs`); external crates
        // simply fail to resolve later.
        _ => (module.to_vec(), prefix),
    };
    absolute.extend(rest.iter().cloned());
    if !name.is_empty() {
        absolute.push(name.to_string());
    }
    Some(absolute)
}
