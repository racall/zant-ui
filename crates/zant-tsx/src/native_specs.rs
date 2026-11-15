//! Native module specs: `native/<name>.ts` compiled to a Rust trait.
//!
//! A spec declares the data types and functions `native/<name>.rs` gives the
//! app, like a React Native TurboModule spec:
//!
//! ```ts
//! import type { i64 } from "zant";
//! import type { Kind } from "./kinds";
//! export interface ListEntry { row: i64; kind: Kind; preview: string }
//! export type CopyResult = "ok" | "not_found";
//! export declare function entries(query: string): ListEntry[];
//! export declare function copy(row: i64): CopyResult;
//! export declare function clear(): void;
//! ```
//!
//! The compiler writes one generated file, `native/spec.rs`, holding a module
//! per spec:
//!
//! ```rust,ignore
//! pub mod clips {
//!     use zant::tsx;
//!
//!     #[derive(Clone, PartialEq, Default)]
//!     pub struct ListEntry { pub row: i64, pub kind: super::kinds::Kind, /* .. */ }
//!
//!     pub trait ClipsSpec {
//!         fn entries(query: &str) -> tsx::List<ListEntry>;
//!         // ..
//!     }
//!
//!     pub const NAME: &str = "clips";
//! }
//! ```
//!
//! The app writes `native/clips.rs` as `impl crate::native::spec::clips::ClipSpec
//! for Native`, and a missing or mistyped function is a rustc error at the
//! `impl`: the trait is the compile-time conformance check. The generated
//! entry performs the optional registry registration
//! (`zant::tsx::native::register_named(NAME, Native)`); generated call sites
//! (`<Native as ClipsSpec>::entries(..)`) bypass the registry. See
//! `docs/tsx-native.md`.
//!
//! The application code generator calls: [`module_name`] to recognise
//! `import … from "native/<name>"`, [`spec_for`] / [`find`] to resolve it,
//! [`import`] to classify an imported name (and to get the `native/<name>.ts`
//! diagnostics), and [`spec_root`] to emit the generated `native/spec.rs`. Type
//! resolution of a spec type inside app code stays in the import scope
//! machinery (`codegen::imports`); this module supplies the declarations.
//!
//! A spec's types and functions are emitted together as one trait
//! (`ClipsSpec`) so rustc checks the implementation, and `native/spec.rs` is a
//! single generated file with a Rust module per spec so the app declares one
//! module.

use crate::ast;
use crate::error::Diag;

use crate::codegen::{
    self, Generator, Result, TypeAlias, fail, functions, model, rust_text, types,
};

/// A parsed spec `native/<name>.ts` of the project.
#[derive(Clone, Copy, Debug)]
pub struct Spec<'a> {
    pub name: &'a str,
    pub module: &'a ast::Module,
}

/// `native/clips` -> `Some("clips")`; `None` for any other import source.
pub fn module_name(source: &str) -> Option<&str> {
    let name = source.strip_prefix("native/")?;
    if name.is_empty() || name.chars().any(|ch| matches!(ch, '/' | '\\' | '.')) {
        return None;
    }
    Some(name)
}

/// `./components/kinds` / `./kinds.ts` -> `kinds`.
fn file_stem(source: &str) -> &str {
    let base = source.rsplit(['/', '\\']).next().unwrap_or(source);
    base.strip_suffix(".ts").unwrap_or(base)
}

/// The spec `name` of the project.
pub fn find<'a>(specs: &'a [Spec<'a>], name: &str) -> Option<&'a Spec<'a>> {
    specs.iter().find(|spec| spec.name == name)
}

/// The spec an import source names, or the "add the spec" diagnostic:
/// `import … from "native/files"` / `import type … from "./kinds"`.
pub fn spec_for<'a>(
    specs: &'a [Spec<'a>],
    source: &str,
    loc: ast::Loc,
) -> std::result::Result<&'a Spec<'a>, Diag> {
    let name = module_name(source).unwrap_or_else(|| file_stem(source));
    find(specs, name).ok_or_else(|| Diag {
        loc,
        msg: format!(
            "cannot find `{source}`: add native/{name}.ts (the spec) and native/{name}.rs"
        ),
    })
}

/// The type `name` the spec exports, if any.
pub fn exported_type<'a>(module: &'a ast::Module, name: &[u8]) -> Option<&'a ast::TypeDecl> {
    module
        .types
        .iter()
        .find(|decl| decl.exported && decl.name.name == name)
}

/// The spec's `export declare function` `name`.
pub fn declared<'a>(module: &'a ast::Module, name: &[u8]) -> Option<&'a ast::Function> {
    module
        .declared
        .iter()
        .find(|top| top.exported && top.func.name == name)
        .map(|top| &*top.func)
}

/// What `import { … } from "native/<name>"` brings into a `.tsx` file.
#[derive(Clone, Copy, Debug)]
pub enum Imported<'a> {
    /// A type the spec exports (it has the same shape in generated Rust).
    Type(&'a ast::TypeDecl),
    /// A declared function: generated code calls it as a trait method.
    Function(&'a ast::Function),
}

/// Classifies one imported name; `Err` is the diagnostic for a name the
/// spec does not export or for a function imported as a type (or vice versa).
pub fn import<'a>(
    spec: &Spec<'a>,
    name: &ast::ImportName,
    type_only: bool,
) -> std::result::Result<Imported<'a>, Diag> {
    if let Some(decl) = exported_type(spec.module, &name.imported.name) {
        return Ok(Imported::Type(decl));
    }
    if let Some(function) = declared(spec.module, &name.imported.name) {
        if type_only {
            return Err(Diag {
                loc: name.imported.loc,
                msg: format!(
                    "`{}` is a function",
                    String::from_utf8_lossy(&name.imported.name)
                ),
            });
        }
        return Ok(Imported::Function(function));
    }
    Err(Diag {
        loc: name.imported.loc,
        msg: format!(
            "native/{}.ts declares no exported `{}`",
            spec.name,
            String::from_utf8_lossy(&name.imported.name)
        ),
    })
}

/// The Rust module of a spec inside the generated `native/spec.rs`:
/// `clips` -> `clips` (escaped like every generated identifier).
pub fn module_ident(name: &str) -> String {
    rust_text::id(name.as_bytes()).unwrap_or_else(|| "_".to_owned())
}

/// The generated trait of a spec: `clips` -> `ClipsSpec`.
pub fn trait_name(name: &str) -> String {
    let mut name = rust_text::enum_variant(name.as_bytes()).unwrap_or_else(|| "_".to_owned());
    name.push_str("Spec");
    name
}

/// The generated module path of a spec: `clips` -> `crate::native::spec::clips`.
pub fn app_spec_path(spec_name: &str) -> String {
    format!("crate::native::spec::{}", module_ident(spec_name))
}

/// The generated trait path of a spec: `crate::native::spec::clips::ClipsSpec`.
pub fn app_trait_path(spec_name: &str) -> String {
    format!("{}::{}", app_spec_path(spec_name), trait_name(spec_name))
}

/// The app's implementation path of a spec: `crate::native::clips::Native`
/// (the `native/<name>.rs` module; call sites name it).
pub fn app_impl_path(spec_name: &str) -> String {
    format!("crate::native::{}::Native", module_ident(spec_name))
}

/// The app's spelling of a spec type in generated code:
/// `crate::native::spec::clips::ListEntry`.
pub fn app_type_path(spec_name: &str, type_name: &str) -> String {
    format!(
        "{}::{}",
        app_spec_path(spec_name),
        rust_text::id(type_name.as_bytes()).unwrap_or_else(|| "_".to_owned())
    )
}

/// A spec has types and declared functions only; the implementation is Rust.
/// Object and string-union types must be named so both sides can refer to
/// them.
pub fn validate(g: &mut Generator<'_>, module: &ast::Module, name: &str) -> Result<()> {
    if let Some(top) = module.functions.first() {
        return fail(
            g,
            top.func.loc,
            format!(
                "a spec declares functions (`export declare function {}(…): …;`); write them in native/{name}.rs",
                String::from_utf8_lossy(&top.func.name)
            ),
        );
    }
    if let Some(constant) = module.consts.first() {
        return fail(
            g,
            constant.name.loc,
            format!(
                "a spec has types and declared functions only; constants belong in native/{name}.rs"
            ),
        );
    }
    for top in &module.declared {
        if !top.exported {
            return fail(
                g,
                top.func.loc,
                format!(
                    "export the declared function: `export declare function {}`",
                    String::from_utf8_lossy(&top.func.name)
                ),
            );
        }
    }
    // Rust names every type the implementation uses: no inline objects or
    // enums (arrays and `| null` of named types are fine)
    for decl in &module.types {
        let Some(detail) = decl.ty.detail.as_deref() else {
            continue;
        };
        match detail {
            ast::TypeDetail::Object(fields) => {
                for field in fields {
                    require_named(g, &field.ty)?;
                }
            }
            ast::TypeDetail::Union(members) => {
                for member in members {
                    let Some(detail) = member.detail.as_deref() else {
                        continue;
                    };
                    match detail {
                        ast::TypeDetail::StringLit(_) | ast::TypeDetail::Null => {}
                        _ => require_named(g, member)?,
                    }
                }
            }
            _ => require_named(g, &decl.ty)?,
        }
    }
    for top in &module.declared {
        for param in &top.func.params {
            if let Some(ty) = param.ty.as_ref() {
                require_named(g, ty)?;
            }
        }
        if let Some(ty) = top.func.ret.as_ref() {
            require_named(g, ty)?;
        }
    }
    Ok(())
}

/// `ty` names its object and string-union types.
fn require_named(g: &mut Generator<'_>, ty: &ast::TypeRef) -> Result<()> {
    let Some(detail) = ty.detail.as_deref() else {
        return Ok(());
    };
    match detail {
        ast::TypeDetail::Array(item) => require_named(g, item),
        ast::TypeDetail::Union(members) => {
            let mut others = 0usize;
            for member in members {
                if matches!(member.detail.as_deref(), Some(ast::TypeDetail::Null)) {
                    continue;
                }
                others += 1;
                require_named(g, member)?;
            }
            if others > 1 {
                return unnamed(g, ty, "a union");
            }
            Ok(())
        }
        ast::TypeDetail::StringLit(_) => unnamed(g, ty, "a union"),
        ast::TypeDetail::Object(_) => unnamed(g, ty, "an object type"),
        ast::TypeDetail::Func(_) => fail(
            g,
            ty.loc,
            "native functions take and return data, not callbacks",
        ),
        ast::TypeDetail::Null => Ok(()),
    }
}

fn unnamed<T>(g: &mut Generator<'_>, ty: &ast::TypeRef, what: &str) -> Result<T> {
    fail(
        g,
        ty.loc,
        format!(
            "name {what} the Rust side uses: `export type Name = …;` / `export interface Name {{ … }}`"
        ),
    )
}

/// `native/<name>.ts` -> the generated `pub mod <name> { … }` of
/// `native/spec.rs`: the spec's types, and — when it declares functions — its
/// trait and the registry key `NAME`.
///
/// A types-only spec (another spec imports its types, the app never calls it)
/// generates its types alone: there is nothing to implement, so no empty trait
/// has to be written in `native/<name>.rs`.
///
/// Decoding errors are reported through `diag` (the first one, like every
/// other codegen entry point); `specs` is the project's spec list, used to
/// resolve `import type { Kind } from "./kinds"`.
pub fn generate(
    module: &ast::Module,
    name: &str,
    specs: &[Spec<'_>],
    diag: &mut Option<Diag>,
) -> Result<String> {
    let mut g = Generator::new(diag);
    g.name = name.to_owned();
    g.type_decls = module.types.clone();
    g.react_imports = module.imports.clone();
    g.resolving = vec![false; module.types.len()];
    prepare_imports(&mut g, module, name, specs)?;
    validate(&mut g, module, name)?;
    // resolve every declared type first: unused ones still declare in Rust
    let mut resolved: Vec<Option<types::Type>> = Vec::with_capacity(module.types.len());
    for decl in &module.types {
        resolved.push(model::named(&mut g, &decl.name.name)?);
    }
    // the trait's methods, from the declared functions
    let mut methods: Vec<(String, String, String)> = Vec::new();
    for top in &module.declared {
        let rust = rust_text::id_at(&mut g, &top.func.name, top.func.loc)?;
        let function = functions::signature(&mut g, &top.func, rust, false)?;
        let mut params: Vec<String> = Vec::new();
        for (param, ty) in top.func.params.iter().zip(function.params.iter()) {
            let param_name = rust_text::id_at(&mut g, &param.name.name, param.name.loc)?;
            params.push(format!("{param_name}: {}", ty.rust(&g)));
        }
        let ret = if function.ret == types::Type::Void {
            String::new()
        } else {
            format!(" -> {}", function.ret.rust(&g))
        };
        methods.push((function.rust.clone(), params.join(", "), ret));
    }
    let declarations = model::declarations(&g);
    let mut body = indent(&declarations);
    if !body.is_empty() && !body.ends_with("\n\n") {
        body.push('\n');
    }
    // Every exported type keeps a `pub` item in the module, so another spec
    // or the app can name it `super::<name>::<Type>` even when it resolved to
    // an array / optional / alias of another type.
    for (decl, ty) in module.types.iter().zip(resolved.iter()) {
        let Some(ty) = *ty else { continue };
        if !decl.exported {
            continue;
        }
        let type_ident = rust_text::id_at(&mut g, &decl.name.name, decl.name.loc)?;
        if is_declared_item(&g.compounds, ty, &type_ident) {
            continue;
        }
        let spelling = ty.rust(&g).to_owned();
        body.push_str(&format!("    pub type {type_ident} = {spelling};\n"));
    }
    if has_functions(module) {
        if !body.is_empty() && !body.ends_with("\n\n") {
            body.push('\n');
        }
        body.push_str(&format!(
            r"    /// `native/{name}.ts`: the functions `native/{name}.rs` implements.
    ///
    /// A missing or mistyped function is a rustc error at the `impl`.
    /// Methods keep the spec's camelCase names.
    #[allow(non_snake_case)]
    pub trait {} {{
",
            trait_name(name)
        ));
        for (method, params, ret) in &methods {
            body.push_str(&format!("        fn {method}({params}){ret};\n"));
        }
        body.push_str(&format!(
            r#"    }}

    /// The registry key of this spec (`zant::tsx::native::register_named`).
    pub const NAME: &str = "{name}";
"#
        ));
    }
    let description = if has_functions(module) {
        format!("`native/{name}.rs` implements the trait below")
    } else {
        "types shared with other specs".to_owned()
    };
    let mut out = format!(
        "/// The spec `native/{name}.ts`: {description}.\n#[allow(dead_code)]\npub mod {} {{\n",
        module_ident(name)
    );
    // `tsx::` appears in the generated *code* (list types: `tsx::List<T>`),
    // not in the doc comments, so look at the declarations and signatures
    // instead of the assembled body (the trait's doc mentions
    // `zant::tsx::native::register_named`).
    let uses_tsx = declarations.contains("tsx::")
        || resolved
            .iter()
            .flatten()
            .any(|ty| ty.rust(&g).contains("tsx::"))
        || methods
            .iter()
            .any(|(_, params, ret)| params.contains("tsx::") || ret.contains("tsx::"));
    if uses_tsx {
        out.push_str("    use zant::tsx;\n\n");
    }
    while body.ends_with("\n\n") {
        body.pop();
    }
    out.push_str(&body);
    out.push_str("}\n");
    Ok(out)
}

/// Whether a spec declares functions, i.e. generates a trait (`native/<name>.rs`
/// implements it) and a registry key the generated entry registers.
pub fn has_functions(module: &ast::Module) -> bool {
    !module.declared.is_empty()
}

/// The generated `native/spec.rs`: a fixed header and one module per spec in
/// name order (the file is deterministic).
pub fn spec_root(specs: &[Spec<'_>], diag: &mut Option<Diag>) -> Result<String> {
    *diag = None;
    let mut sorted: Vec<&Spec<'_>> = specs.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(b.name));
    for pair in sorted.windows(2) {
        if pair[0].name == pair[1].name {
            *diag = Some(Diag {
                loc: 0,
                msg: format!("duplicate native module spec `{}`", pair[0].name),
            });
            return Err(codegen::Error::Tsx);
        }
    }
    let mut out = String::from(
        "// @generated by zant-tsx from native/*.ts (the native module specs) - do not edit.\n\
         //\n\
         // Each module holds one spec's types (and, when it declares functions, the\n\
         // trait `native/<name>.rs` implements);\n\
         // see docs/tsx-native.md.\n\n",
    );
    for spec in sorted {
        out.push_str(&generate(spec.module, spec.name, specs, diag)?);
        out.push('\n');
    }
    if let Err(error) = syn::parse_file(&out) {
        panic!(
            "zant-tsx: generated native/spec.rs does not parse: {error}\n--- generated source ---\n{out}"
        );
    }
    Ok(out)
}

/// Indents every non-empty line by one module level.
fn indent(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.lines().count() * 4);
    for line in text.lines() {
        if !line.is_empty() {
            out.push_str("    ");
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Whether `ty` already declares an item named `ident` (an interface struct or
/// a string-literal enum); aliases need a `pub type` item of their own.
fn is_declared_item(declarations: &[model::Info], ty: types::Type, ident: &str) -> bool {
    let types::Type::Compound(index) = ty else {
        return false;
    };
    let Some(info) = declarations.get(index as usize) else {
        return false;
    };
    info.declaration.is_some() && info.rust == ident
}

/// Imports of a spec: `import type { i64 } from "zant"` (numeric types) and
/// `import type { Kind } from "./kinds"` (another spec's types, which the
/// generated module names `super::kinds::Kind`). A spec imports types only.
fn prepare_imports(
    g: &mut Generator<'_>,
    module: &ast::Module,
    name: &str,
    specs: &[Spec<'_>],
) -> Result<()> {
    for import in &module.imports {
        let source = String::from_utf8_lossy(&import.source).into_owned();
        if source.starts_with("./") || source.starts_with("../") {
            let target = match spec_for(specs, &source, import.source_loc) {
                Ok(target) => *target,
                Err(diag) => return fail(g, diag.loc, diag.msg),
            };
            for imported in &import.names {
                let Some(decl) = exported_type(target.module, &imported.imported.name) else {
                    return fail(
                        g,
                        imported.imported.loc,
                        format!(
                            "native/{}.ts exports no type `{}` (specs share types, not functions)",
                            target.name,
                            String::from_utf8_lossy(&imported.imported.name)
                        ),
                    );
                };
                // `import type { Kind } from "./clips"` inside clips.ts
                if target.name == name {
                    continue;
                }
                let imported_ident =
                    rust_text::id_at(g, &imported.imported.name, imported.imported.loc)?;
                let local = rust_text::id_at(g, &imported.local.name, imported.local.loc)?;
                let kind = imported_kind(decl);
                let ty = model::add(
                    g,
                    model::Info {
                        rust: format!("super::{}::{imported_ident}", module_ident(target.name)),
                        kind,
                        copy: kind == model::InfoKind::Enumeration,
                        ..model::Info::default()
                    },
                )?;
                g.type_aliases.push(TypeAlias { name: local, ty });
            }
            continue;
        }
        if source == "zant" || source == "react" {
            for imported in &import.names {
                // An in-memory image handle: a spec imports `Image` from
                // "zant", which maps to the JSX hooks' `Rc<zant::image::Image>`
                // (the type `<Image image>` takes).
                if imported.imported.name == b"Image" {
                    let local = rust_text::id_at(g, &imported.local.name, imported.local.loc)?;
                    let ty = model::add(
                        g,
                        model::Info {
                            kind: model::InfoKind::Object,
                            rust: "std::rc::Rc<zant::image::Image>".to_owned(),
                            ..model::Info::default()
                        },
                    )?;
                    g.type_aliases.push(TypeAlias { name: local, ty });
                    continue;
                }
                let Some(ty) = types::numeric_name(&imported.imported.name) else {
                    return fail(
                        g,
                        imported.imported.loc,
                        format!(
                            "unknown import `{}`; this name is not supported by Zant",
                            String::from_utf8_lossy(&imported.imported.name)
                        ),
                    );
                };
                if imported.imported.name != imported.local.name {
                    let local = rust_text::id_at(g, &imported.local.name, imported.local.loc)?;
                    g.type_aliases.push(TypeAlias { name: local, ty });
                }
            }
            continue;
        }
        return fail(
            g,
            import.source_loc,
            "imports come from \"zant\", \"react\", \"native/<module>\" or a relative .tsx / .rs file",
        );
    }
    Ok(())
}

/// The shape of another spec's exported type, for the opaque alias a spec
/// import adds to the generator (`super::<spec>::<Type>`).
fn imported_kind(decl: &ast::TypeDecl) -> model::InfoKind {
    match decl.ty.detail.as_deref() {
        Some(ast::TypeDetail::Object(_)) => model::InfoKind::Object,
        Some(ast::TypeDetail::StringLit(_)) => model::InfoKind::Enumeration,
        Some(ast::TypeDetail::Union(members))
            if members.iter().all(|member| {
                matches!(
                    member.detail.as_deref(),
                    Some(ast::TypeDetail::StringLit(_))
                )
            }) =>
        {
            model::InfoKind::Enumeration
        }
        _ => model::InfoKind::Array,
    }
}
