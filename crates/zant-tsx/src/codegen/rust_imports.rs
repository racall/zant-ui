//! Public functions and types of hand-written Rust files, used from TSX
//! (`import { plural } from "./format.rs"`, `import type { Peer } from
//! "./sync.rs"`). The files are parsed with `syn`.
//!
//! A project file ending in `.rs` is copied next to the generated modules and
//! declared as a Rust module by the generated entry, so its `pub fn`s are
//! callable as `crate::format::plural(..)` and its `pub struct` / `pub enum`
//! types are named `crate::format::Peer`.
//!
//! Only the small typed boundary documented in `docs/tsx.md` is mapped:
//! `bool`, the numbers, `String` / `Str` / `&str`, `Option<T>`, `Vec<T>` /
//! `List<T>` / `&[T]`, `tsx::Bytes`, structs and enums of the same file.
//! Anything else is a diagnostic naming the offending signature.

use quote::ToTokens;

use crate::ast;

use super::app_state;
use super::expr;
use super::model;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// A hand-written project file: its parsed `syn` tree lives in
/// [`app_state::RustSource`].
pub use super::app_state::RustSource as Source;

/// An imported function, after its signature has been checked and mapped to
/// TSX types.
#[derive(Clone, Debug)]
pub struct Function {
    pub local: String,
    pub target: String,
    pub params: Vec<Type>,
    pub ret: Type,
}

/// A type imported by name into this file.
#[derive(Clone, Debug)]
pub struct ImportedType {
    pub local: String,
    pub ty: Type,
}

/// Register named imports.
pub fn process(g: &mut Generator<'_>, import: &ast::Import, source: &str) -> Result<()> {
    let Some(project) = app_state::project() else {
        return fail(
            g,
            import.source_loc,
            "Rust imports need a project (zant-tsx --out / zant_build::tsx_app)",
        );
    };
    let Some(module) = project.find_rust(source) else {
        return fail(
            g,
            import.source_loc,
            format!("cannot find `{source}` among the project's .rs files"),
        );
    };
    let module_name = super::imports::module_name(&module.name);
    let alias = format!("crate::{module_name}");
    let file = module.file.clone();
    for n in &import.names {
        let imported = String::from_utf8_lossy(&n.imported.name).into_owned();
        let local = String::from_utf8_lossy(&n.local.name).into_owned();
        if n.type_only || import.type_only {
            match resolve_type(g, &alias, &file, &imported)? {
                Some(ty) => app_state::with_module(|state| {
                    state.rust_types.push(ImportedType {
                        local: local.clone(),
                        ty,
                    })
                }),
                None => {
                    return fail(
                        g,
                        n.imported.loc,
                        format!("`{imported}` is not a pub struct or pub enum of `{source}`"),
                    );
                }
            }
            continue;
        }
        let Some(function) = find(&file, &imported) else {
            if let Some(near) = closest(&file, &imported) {
                return fail(
                    g,
                    n.imported.loc,
                    format!("`{imported}` is not a pub fn of `{source}`; did you mean `{near}`?"),
                );
            }
            return fail(
                g,
                n.imported.loc,
                format!("`{imported}` is not a pub fn of `{source}`"),
            );
        };
        let signature = signature(g, &alias, &file, &function, n.imported.loc)?;
        app_state::with_module(|state| {
            state.rust_functions.push(Function {
                local: local.clone(),
                ..signature
            })
        });
    }
    Ok(())
}

/// `name` is one of this file's imported Rust types.
pub fn named(_g: &mut Generator<'_>, name: &str) -> Result<Option<Type>> {
    Ok(app_state::with_module(|state| {
        state
            .rust_types
            .iter()
            .find(|entry| entry.local == name)
            .map(|entry| entry.ty)
    }))
}

/// Resolve `name` in the Rust file `source` (a type another project file
/// imported through this file).
pub fn named_in(g: &mut Generator<'_>, source: &str, name: &str) -> Result<Option<Type>> {
    let Some(project) = app_state::project() else {
        return Ok(None);
    };
    let Some(module) = project.find_rust(source) else {
        return Ok(None);
    };
    let module_name = super::imports::module_name(&module.name);
    let file = module.file.clone();
    resolve_type(g, &format!("crate::{module_name}"), &file, name)
}

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
fn find(file: &syn::File, name: &str) -> Option<syn::ItemFn> {
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
fn closest(file: &syn::File, name: &str) -> Option<String> {
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
fn signature(
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

fn unsupported(
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

fn quote_type(ty: &syn::Type) -> String {
    quote_tokens(ty).unwrap_or_else(|| "…".to_owned())
}

fn quote_tokens(tokens: impl ToTokens) -> Option<String> {
    Some(tokens.to_token_stream().to_string().replace(' ', ""))
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Diag;

    fn file(source: &str) -> syn::File {
        syn::parse_file(source).expect("test Rust source parses")
    }

    /// Runs `f` on a fresh generator (the diagnostic slot is leaked: the
    /// generator borrows it for its lifetime, and these tests are tiny).
    fn with_generator<R>(f: impl FnOnce(&mut Generator<'static>) -> R) -> (Option<Diag>, R) {
        let diag: &'static mut Option<Diag> = Box::leak(Box::new(None));
        let mut g = Generator::new(diag);
        let result = f(&mut g);
        let diag = g.diag.clone();
        (diag, result)
    }

    fn signature_of(source: &str, name: &str) -> (Option<Diag>, Result<Function>) {
        let file = file(source);
        let function = find(&file, name).expect("fn");
        with_generator(|g| signature(g, "crate::format", &file, &function, 0))
    }

    #[test]
    fn signatures_map_primitive_optional_and_slice_types() {
        let source = "\
fn hidden() {}
pub const value: u8 = 1;
pub fn plural(n: usize, word: &str) -> zant::Str { word.into() }
pub fn all(a: bool, b: Option<i32>, c: Vec<f64>, d: Vec<zant::Str>) -> Option<Vec<bool>> { None }
pub fn nothing() {}
";
        let module_file = file(source);
        assert!(find(&module_file, "hidden").is_none());
        assert!(find(&module_file, "value").is_none());
        with_generator(|g| {
            let plural = find(&module_file, "plural").expect("plural");
            let plural = signature(g, "crate::format", &module_file, &plural, 0).expect("plural");
            assert_eq!(vec![Type::Usize, Type::String], plural.params);
            assert_eq!(Type::String, plural.ret);
            let all = find(&module_file, "all").expect("all");
            let all = signature(g, "crate::format", &module_file, &all, 0).expect("all");
            assert_eq!(Type::Boolean, all.params[0]);
            assert_eq!(
                Type::I32,
                model::optional_child(g, all.params[1]).expect("optional")
            );
            assert_eq!(
                Type::F64,
                model::element(g, all.params[2]).expect("element")
            );
            assert_eq!(
                Type::String,
                model::element(g, all.params[3]).expect("element")
            );
            let inner = model::optional_child(g, all.ret).expect("optional");
            assert_eq!(Type::Boolean, model::element(g, inner).expect("element"));
            let nothing = find(&module_file, "nothing").expect("nothing");
            let nothing =
                signature(g, "crate::format", &module_file, &nothing, 0).expect("nothing");
            assert_eq!(Type::Void, nothing.ret);
            for (name, expected) in [
                ("u8", Type::U8),
                ("u16", Type::U16),
                ("u32", Type::U32),
                ("u64", Type::U64),
                ("i8", Type::I8),
                ("i16", Type::I16),
                ("i32", Type::I32),
                ("i64", Type::I64),
                ("usize", Type::Usize),
                ("isize", Type::Isize),
                ("f32", Type::F32),
                ("f64", Type::F64),
            ] {
                let source = format!("pub fn f(x: {name}) -> {name} {{ x }}");
                let numeric = file(&source);
                let function = find(&numeric, "f").expect("f");
                let function =
                    signature(g, "crate::format", &numeric, &function, 0).expect("numeric");
                assert_eq!(expected, function.params[0], "{name}");
                assert_eq!(expected, function.ret, "{name}");
            }
            // plain (non-generic) references are followed to the file's types
            let references = file("pub fn name(p: Peer) -> Peer { p }");
            let function = find(&references, "name").expect("name");
            assert!(
                signature(g, "crate::devices", &references, &function, 0).is_err(),
                "unknown types are diagnostics"
            );
        });
    }

    #[test]
    fn unsupported_signatures_report_the_import_name_and_offending_type() {
        for parameter in [
            "x: Foo",
            "x: *const u8",
            "x: impl Iterator<Item = u8>",
            "x: [u8; 4]",
            "x: (u8, u8)",
        ] {
            let source = format!("pub fn f({parameter}) {{}}");
            let (diag, result) = signature_of(&source, "f");
            assert!(result.is_err(), "{parameter} should be unsupported");
            let diag = diag.expect("diagnostic");
            assert_eq!(0, diag.loc);
            assert!(diag.msg.starts_with("`f` takes `"), "{}", diag.msg);
        }
        for ret in ["!", "Result<u8, ()>", "Foo", "Box<u8>"] {
            let source = format!("pub fn f() -> {ret} {{ todo!() }}");
            let (diag, result) = signature_of(&source, "f");
            assert!(result.is_err(), "{ret} should be unsupported");
            assert!(
                diag.expect("diagnostic").msg.starts_with("`f` returns `"),
                "ret {ret}"
            );
        }
    }

    #[test]
    fn resolve_type_maps_pub_structs_and_enums() {
        let file = file(
            "pub struct Peer { pub name: String, pub online: bool, not_tsx: Box<u8> }\n\
             pub enum Kind { Laptop, Desktop }\n\
             struct Private { id: u8 }\n",
        );
        let (_, ()) = with_generator(|g| {
            let peer = resolve_type(g, "crate::devices", &file, "Peer")
                .expect("resolves")
                .expect("pub struct");
            let info = model::info(g, peer).expect("object");
            assert_eq!("crate::devices::Peer", info.rust);
            assert_eq!(2, info.fields.len());
            assert_eq!(Type::String, info.fields[0].ty);
            assert_eq!(Type::Boolean, info.fields[1].ty);
            let kind = resolve_type(g, "crate::devices", &file, "Kind")
                .expect("resolves")
                .expect("pub enum");
            assert!(model::enum_type(g, kind));
            assert!(
                resolve_type(g, "crate::devices", &file, "Private")
                    .expect("resolves")
                    .is_none()
            );
        });
    }
}
