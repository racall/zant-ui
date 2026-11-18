//! Generating one module: the order of declarations, components and functions in the output file ([`generate_inner`]) and the import checks.

use super::*;

pub(super) fn generate_inner(
    source_name: &str,
    module: &ast::Module,
    options: &Options<'_>,
    install_extra: Option<fn(&mut Generator<'_>)>,
    diag: &mut Option<Diag>,
) -> Result<GeneratedFile> {
    if let Err(error) = crate::codegen::generics::validate(module) {
        *diag = Some(error);
        return Err(crate::codegen::Error::Tsx);
    }
    if options.native {
        // `native/<name>.ts`: the spec compiles to `pub mod <name> { … }` of
        // `native/spec.rs`; the project's specs resolve its imports.
        let name = options
            .source_path
            .map(|path| path.rsplit(['/', '\\']).next().unwrap_or(path))
            .and_then(|base| base.strip_suffix(".ts"))
            .unwrap_or("native");
        // keep the project alive: the specs borrow its parsed modules
        let project = crate::codegen::app_state::project();
        let specs: Vec<crate::native_specs::Spec<'_>> = project
            .as_deref()
            .map(|project| {
                project
                    .modules
                    .iter()
                    .filter(|module| module.native)
                    .map(|module| crate::native_specs::Spec {
                        name: module.name.as_str(),
                        module: &module.module,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let source = crate::native_specs::generate(module, name, &specs, diag)?;
        return Ok(GeneratedFile { source });
    }
    let mut g = Generator::new(diag);
    g.name.clear();
    crate::codegen::loops::reset_exhaustive();
    g.type_decls = module.types.clone();
    g.enums = module.enums.clone();
    g.const_decls = module.consts.clone();
    g.react_imports = module.imports.clone();
    g.resolving = vec![false; module.types.len()];
    install(&mut g);
    if let Some(extra) = install_extra {
        extra(&mut g);
    }
    // which functions can throw, across the project's files (before any
    // signature is registered: imports and this file's functions use it)
    g.module_stem = crate::codegen::imports::stem(source_name)
        .trim_end_matches(".tsx")
        .trim_end_matches(".ts")
        .to_owned();
    g.throwing = crate::codegen::throws::analyze(
        options.project,
        stateful::ModuleSource {
            name: &g.module_stem.clone(),
            module,
        },
    );
    validate_imports(&mut g, module)?;
    if let Some(hook) = g.extensions.process_imports {
        hook(&mut g, module)?;
    }
    for declaration in &module.types {
        // a generic declaration is emitted with its first use (`Box<number>`)
        if declaration.type_params.is_empty() {
            let _ = model::named(&mut g, &declaration.name.name)?;
        }
    }
    if let Some(first) = module.declared.first() {
        return fail(
            &mut g,
            first.func.loc,
            "`declare function` belongs in a native module spec (native/<module>.ts)",
        );
    }
    // the root is the default export without props (the app's state); a file of
    // a project may hold child components only
    let mut root: Option<ast::Function> = None;
    for top in &module.functions {
        if !top.default {
            continue;
        }
        if root.is_some() {
            return fail(
                &mut g,
                top.func.loc,
                "only one component can be the default export",
            );
        }
        if !top.func.params.is_empty() && !options.require_root {
            continue;
        }
        root = Some((*top.func).clone());
    }
    if root.is_none() && options.require_root {
        return fail(
            &mut g,
            0,
            "missing default exported component; use export default function App()",
        );
    }
    if let Some(function) = &root {
        if !function.params.is_empty() {
            return fail(
                &mut g,
                function.params[0].name.loc,
                "the root component takes no props; it is the app's state",
            );
        }
        let name = String::from_utf8_lossy(&function.name);
        if reserved_component_name(&name) {
            return fail(
                &mut g,
                function.loc,
                format!("component name `{name}` is reserved; rename it"),
            );
        }
    }
    let is_window = g.extensions.is_window_const.unwrap_or(|_| false);
    let store_code = stores::collect(&mut g, &module.consts)?;
    let plain = stores::plain_constants(&mut g, &module.consts, is_window)?;
    let asset_code = crate::assets::process(&mut g, module, options.source_path)?;
    let globals_code = format!(
        "{asset_code}{}{store_code}",
        globals::collect(&mut g, &plain)?
    );
    functions::collect(&mut g, module)?;
    components::collect(&mut g, module, root.as_ref(), options.project)?;
    let window_code = match g.extensions.collect_windows {
        Some(hook) => hook(&mut g, module)?,
        None => String::new(),
    };
    let mut function_code = functions::emit_all(&mut g)?;
    // children first, then the root; the header needs every compound type
    // a component with an error is recorded and the next one generated, so
    // independent mistakes in several components are reported together
    let mut bodies = String::new();
    let mut failures: Vec<Diag> = Vec::new();
    for index in 0..g.components.len() {
        if g.components[index].external {
            continue;
        }
        let function = (*g.components[index].func).clone();
        match emit_component(&mut g, &function, Some(index)) {
            Ok(body) => {
                bodies.push_str(&body);
                bodies.push('\n');
            }
            Err(_) => failures.extend(g.diag.take()),
        }
    }
    if let Some(function) = &root {
        match emit_component(&mut g, function, None) {
            Ok(body) => bodies.push_str(&body),
            Err(_) => failures.extend(g.diag.take()),
        }
    }
    if !failures.is_empty() {
        failures.sort_by_key(|diag| diag.loc);
        let first = failures.remove(0);
        MORE_DIAGS.with(|slot| *slot.borrow_mut() = failures);
        *g.diag = Some(first);
        return Err(crate::codegen::Error::Tsx);
    }
    // generic function instances the components asked for
    function_code.push_str(&functions::emit_all(&mut g)?);
    let source_name = source_name.replace('\n', " ");
    let declarations = globals::declarations(&g);
    let mut out = String::new();
    out.push_str(HEADER_ALLOWS);
    out.push_str(PREAMBLE);
    out.push_str(&declarations);
    out.push_str(&globals_code);
    out.push_str(&window_code);
    out.push_str(&function_code);
    out.push_str(&bodies);
    let raw = scope::rename_modules(&mut g, out)?;
    // `prettyplease` reformats from the `syn` AST and drops comments: keep the
    // `@generated` marker out of the formatted body and prepend it verbatim.
    let formatted = match crate::codegen::check_and_format(&raw, &source_name) {
        Ok(formatted) => formatted,
        Err(message) => return fail(&mut g, 0, message),
    };
    Ok(GeneratedFile {
        source: format!("// @generated by zant-tsx from {source_name} — do not edit.\n{formatted}"),
    })
}

/// Imports come from `"zant"`, `"react"`, a relative file or a
/// `native/<module>` spec; imported names must be known.
pub(super) fn validate_imports(g: &mut Generator<'_>, module: &ast::Module) -> Result<()> {
    for import in &module.imports {
        let source = String::from_utf8_lossy(&import.source);
        if source.starts_with('.') || source.starts_with("native/") {
            continue;
        }
        if source != "zant" && source != "react" {
            return fail(
                g,
                import.source_loc,
                "imports come from \"zant\", \"react\", \"native/<module>\" or a relative .tsx / .rs file",
            );
        }
        for name in &import.names {
            let imported = String::from_utf8_lossy(&name.imported.name);
            let local = String::from_utf8_lossy(&name.local.name);
            let tag = tags::find(&imported).is_some();
            let known = tag
                || hooks::extra_hook(&imported)
                || crate::codegen::hook_name(g, &imported)
                || types::numeric_name(name.imported.name.as_slice()).is_some()
                || types::node_name(name.imported.name.as_slice())
                || matches!(
                    imported.as_ref(),
                    "useState"
                        // lowered to its function (`react_compat`)
                        | "useCallback"
                        | "useTheme"
                        | "List"
                        | "createStore"
                        | "useStore"
                        | "KeyEvent"
                        | "createWindow"
                        | "Image"
                );
            if !known {
                if let Some(external) = g.extensions.hooks.external_name
                    && external(g, &imported)?.is_some()
                {
                    continue;
                }
                return fail(
                    g,
                    name.imported.loc,
                    format!("unknown import `{imported}`; this name is not supported by Zant"),
                );
            }
            // `import type {u16 as Small} from "zant"`
            // aliases the numeric type; tags cannot be renamed.
            if imported != local {
                if tag {
                    return fail(
                        g,
                        name.local.loc,
                        format!("tag imports cannot be renamed; use `{imported}`"),
                    );
                }
                if imported == "useState" {
                    g.hook_names.push(local.into_owned());
                } else if let Some(ty) = types::numeric_name(name.imported.name.as_slice()) {
                    g.type_aliases.push(crate::codegen::TypeAlias {
                        name: local.into_owned(),
                        ty,
                    });
                } else {
                    return fail(g, name.local.loc, format!("`{imported}` cannot be renamed"));
                }
            }
        }
    }
    Ok(())
}
