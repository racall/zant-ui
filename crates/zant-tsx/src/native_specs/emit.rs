//! Emitting a native spec as a Rust trait module, with its imported types.

use super::*;

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
    // or the app can name it `crate::<name>::<Type>` even when it resolved to
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
pub(super) fn indent(text: &str) -> String {
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
pub(super) fn is_declared_item(declarations: &[model::Info], ty: types::Type, ident: &str) -> bool {
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
/// generated module names `crate::kinds::Kind`). A spec imports types only.
pub(super) fn prepare_imports(
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
/// import adds to the generator (`crate::<spec>::<Type>`).
pub(super) fn imported_kind(decl: &ast::TypeDecl) -> model::InfoKind {
    match decl.ty.detail.as_deref() {
        Some(ast::TypeDetail::Object(_) | ast::TypeDetail::Intersection(_)) => {
            model::InfoKind::Object
        }
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
