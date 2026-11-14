//! Index of the `zant` crate sources, replacing the comptime reflection of
//! `tools/tsx/manifest/reflect.zig`.
//!
//! The Zig generator saw the `ui.*` namespace through `@typeInfo`; Rust has no
//! module reflection, so the generator walks `crates/zant/src/**/*.rs` with
//! `syn`:
//!
//! - `pub fn ... -> Node` builders of `ui/builders/` (name, options type,
//!   text-argument / instance-id shape);
//! - `zant::theme::tokens::Theme` for the `useTheme()` props;
//! - every `pub enum` / `pub struct` and `use` alias, so a field type such as
//!   `flex::Wrap` can be resolved to the enum whose variants become TSX
//!   spellings (`reflect.zig:kind` read them through `@typeInfo` instead).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use syn::{Item, PathArguments, UseTree};

/// A `pub fn ... -> Node` builder of `ui/builders/`.
#[derive(Clone, Debug)]
pub struct Builder {
    /// Rust function name with the raw-identifier prefix stripped (`switch`).
    pub name: String,
    /// `r#switch`: the Zig table spelled this builder `ui.@"switch"`.
    pub raw_ident: bool,
    /// The options struct of the last parameter.
    pub options: String,
    /// `text(value, options)`: the text is the first argument (`ui.text`).
    pub text_arg: bool,
}

/// One field of `zant::theme::tokens::Theme`.
#[derive(Clone, Debug)]
pub struct ThemeField {
    pub name: String,
    /// The Rust type ident (`u32` / `f32`).
    pub ty: String,
}

/// What a resolved type path names.
#[derive(Clone, Debug)]
pub enum Resolved {
    /// Enum variants in declaration order.
    Enum(Vec<String>),
    /// A struct; `true` when it derives `TsxProps` (an options struct).
    Struct,
    /// A module (only useful for intermediate path segments).
    Module,
}

pub struct Index {
    modules: HashMap<Vec<String>, Module>,
    pub builders: Vec<Builder>,
    pub theme: Vec<ThemeField>,
}

#[derive(Default)]
struct Module {
    enums: HashMap<String, Vec<String>>,
    structs: HashSet<String>,
    children: HashSet<String>,
    /// `use a::b::Name` → absolute `["crate", "a", "b", "Name"]`.
    aliases: HashMap<String, Vec<String>>,
    /// `use a::b::*` → absolute `["crate", "a", "b"]`.
    globs: Vec<Vec<String>>,
}

impl Index {
    /// Parse every `.rs` file of `zant/src`.
    pub fn load(src_dir: &Path) -> Index {
        let mut index = Index {
            modules: HashMap::new(),
            builders: Vec::new(),
            theme: Vec::new(),
        };
        let mut files = Vec::new();
        collect_rust_files(src_dir, &mut files);
        files.sort();
        for file in &files {
            index.parse_file(file);
        }
        index
            .builders
            .sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.options.cmp(&b.options)));
        index
    }

    /// Resolve a type path as written in `module` (`["crate", "ui", "options"]`).
    pub fn resolve(&self, module: &[String], path: &[String]) -> Option<Resolved> {
        self.resolve_inner(module, path, &mut HashSet::new())
    }

    fn resolve_inner(
        &self,
        module: &[String],
        path: &[String],
        visited: &mut HashSet<Vec<String>>,
    ) -> Option<Resolved> {
        let first = path.first()?;
        match first.as_str() {
            "crate" => self.resolve_at(&["crate".to_string()], &path[1..], visited),
            "self" => self.resolve_at(module, &path[1..], visited),
            "super" => {
                let mut base = module.to_vec();
                base.pop();
                self.resolve_at(&base, &path[1..], visited)
            }
            _ => {
                let current = self.modules.get(module)?;
                if let Some(alias) = current.aliases.get(first) {
                    let mut target = alias.clone();
                    target.extend_from_slice(&path[1..]);
                    return self.resolve_inner(&["crate".to_string()], &target, visited);
                }
                if current.children.contains(first) {
                    let mut child = module.to_vec();
                    child.push(first.clone());
                    return self.resolve_at(&child, &path[1..], visited);
                }
                // Plain path: an item of this module (`AlertType`), a
                // crate-level module (`ui::foo`) or an external crate (`std`);
                // only the first two are in the index.
                if let Some(resolved) = self.resolve_at(module, path, visited) {
                    return Some(resolved);
                }
                let root_key = vec!["crate".to_string()];
                let root = self.modules.get(&root_key)?;
                if root.children.contains(first) {
                    return self.resolve_at(&["crate".to_string()], path, visited);
                }
                None
            }
        }
    }

    fn resolve_at(
        &self,
        module: &[String],
        rest: &[String],
        visited: &mut HashSet<Vec<String>>,
    ) -> Option<Resolved> {
        let Some(seg) = rest.first() else {
            return Some(Resolved::Module);
        };
        let tail = &rest[1..];
        let current = self.modules.get(module)?;
        if tail.is_empty() {
            if let Some(variants) = current.enums.get(seg) {
                return Some(Resolved::Enum(variants.clone()));
            }
            if current.structs.contains(seg) {
                return Some(Resolved::Struct);
            }
            if current.children.contains(seg) {
                return Some(Resolved::Module);
            }
            if let Some(alias) = current.aliases.get(seg) {
                return self.resolve_alias(alias, visited);
            }
            for glob in &current.globs {
                let mut target = glob.clone();
                target.push(seg.clone());
                if let Some(resolved) = self.resolve_alias(&target, visited) {
                    return Some(resolved);
                }
            }
            return None;
        }
        if current.children.contains(seg) {
            let mut child = module.to_vec();
            child.push(seg.clone());
            return self.resolve_at(&child, tail, visited);
        }
        if let Some(alias) = current.aliases.get(seg) {
            let mut target = alias.clone();
            target.extend_from_slice(tail);
            return self.resolve_alias(&target, visited);
        }
        None
    }

    /// Follow an absolute path, guarding against `use` cycles.
    fn resolve_alias(
        &self,
        path: &[String],
        visited: &mut HashSet<Vec<String>>,
    ) -> Option<Resolved> {
        if !visited.insert(path.to_vec()) {
            return None;
        }
        let (first, rest) = path.split_first()?;
        if first == "crate" {
            self.resolve_at(&["crate".to_string()], rest, visited)
        } else {
            None
        }
    }

    fn parse_file(&mut self, path: &Path) {
        let Ok(source) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(file) = syn::parse_file(&source) else {
            // Generated / cfg-gated sources may be unparsable; skip like the
            // Zig tool skipped non-ui modules.
            return;
        };
        let module = module_path_for(path);
        let entry = self.modules.entry(module.clone()).or_default();
        for item in &file.items {
            match item {
                Item::Mod(item_mod) => {
                    entry.children.insert(item_mod.ident.to_string());
                }
                Item::Enum(item_enum) => {
                    let variants = item_enum
                        .variants
                        .iter()
                        .map(|variant| variant.ident.to_string())
                        .collect();
                    entry.enums.insert(item_enum.ident.to_string(), variants);
                }
                Item::Struct(item_struct) => {
                    entry.structs.insert(item_struct.ident.to_string());
                }
                Item::Use(item_use) => {
                    walk_use_tree(&item_use.tree, &mut Vec::new(), &module, entry);
                }
                Item::Fn(item_fn) if is_builder_module(&module) => {
                    if let Some(builder) = builder_of(item_fn) {
                        self.builders.push(builder);
                    }
                }
                _ => {}
            }
        }
        if module == theme_module() {
            self.theme = theme_fields(&file);
        }
    }
}

fn theme_module() -> Vec<String> {
    vec![
        "crate".to_string(),
        "theme".to_string(),
        "tokens".to_string(),
    ]
}

fn is_builder_module(module: &[String]) -> bool {
    module.len() >= 3 && module[0] == "crate" && module[1] == "ui" && module[2] == "builders"
}

/// `src/ui/builders/mod.rs` → `["crate", "ui", "builders"]`;
/// `src/layout/flex.rs` → `["crate", "layout", "flex"]`.
fn module_path_for(path: &Path) -> Vec<String> {
    let mut segments = vec!["crate".to_string()];
    let mut parts: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    let Some(crate_index) = parts.iter().position(|part| part == "src") else {
        return segments;
    };
    parts.drain(..=crate_index);
    if let Some(last) = parts.last_mut() {
        let stem = last.strip_suffix(".rs").unwrap_or(last).to_string();
        *last = stem;
    }
    if parts.last().is_some_and(|last| last == "mod") {
        parts.pop();
    }
    // `src/lib.rs` is the crate root.
    if parts == ["lib"] {
        return segments;
    }
    segments.extend(parts);
    segments
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

/// `pub fn name(options: X) -> Node`, `pub fn text(value: impl Into<Str>, options: X) -> Node`.
fn builder_of(item_fn: &syn::ItemFn) -> Option<Builder> {
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

fn theme_fields(file: &syn::File) -> Vec<ThemeField> {
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

fn walk_use_tree(tree: &UseTree, prefix: &mut Vec<String>, module: &[String], entry: &mut Module) {
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

/// Turn `crate::a::b` / `super::c::D` into an absolute crate path; `None` for
/// external crates, which the table never needs.
fn absolute_use_path(module: &[String], prefix: &[String], name: &str) -> Option<Vec<String>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_paths_strip_src_and_the_mod_file() {
        let root = Path::new("/repo/crates/zant/src");
        assert_eq!(
            module_path_for(&root.join("ui/builders/mod.rs")),
            ["crate", "ui", "builders"]
        );
        assert_eq!(
            module_path_for(&root.join("layout/flex.rs")),
            ["crate", "layout", "flex"]
        );
    }

    #[test]
    fn type_helpers_read_generic_types() {
        let ty: syn::Type = syn::parse_quote!(Option<Rc<Image>>);
        assert_eq!(last_type_ident(&ty).as_deref(), Some("Option"));
        assert_eq!(type_arguments(&ty).len(), 1);
        assert_eq!(
            last_type_ident(type_arguments(&ty)[0]).as_deref(),
            Some("Rc")
        );
        let path: syn::Type = syn::parse_quote!(flex::Wrap);
        assert_eq!(type_segments(&path).unwrap(), ["flex", "Wrap"]);
    }
}
