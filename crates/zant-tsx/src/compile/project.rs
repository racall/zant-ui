//! Compiling the files of a project together: shared types, hooks, windows and every module's generation.

use super::*;

/// [`compile_project_with`] for an app with translations: `t(…)` calls are
/// checked against `i18n` (the compiled `locales/*.json`, see
/// [`crate::i18n::emit`]).
pub fn compile_project_i18n(
    files: &[SourceFile],
    install_extra: Option<fn(&mut codegen::Generator<'_>)>,
    i18n: Option<codegen::i18n_calls::Index>,
) -> Result<GeneratedProject, ProjectError> {
    // case-insensitive output collision check (macOS and Windows file systems
    // are by default case-insensitive)
    let mut names: Vec<String> = Vec::with_capacity(files.len());
    for (index, file) in files.iter().enumerate() {
        let name = file.output_name();
        if names
            .iter()
            .any(|previous| previous.eq_ignore_ascii_case(&name))
        {
            return Err(ProjectError {
                file: index,
                diag: Diag {
                    loc: 0,
                    msg: format!("project output collision: `{name}`"),
                },
                more: Vec::new(),
            });
        }
        names.push(name);
    }
    // hand-written Rust: validate the syntax, keep the parsed signatures
    let mut rust_sources: Vec<RustSource> = Vec::new();
    for (index, file) in files.iter().enumerate() {
        if !file.is_rust() {
            continue;
        }
        let parsed = syn::parse_file(&file.src).map_err(|error| ProjectError {
            file: index,
            diag: Diag {
                loc: 0,
                msg: error.to_string(),
            },
            more: Vec::new(),
        })?;
        rust_sources.push(RustSource {
            name: file.stem(),
            path: file.path.clone(),
            source: file.src.clone(),
            file: parsed,
        });
    }
    // parse the TSX files
    let mut modules: Vec<ModuleFile> = Vec::new();
    let mut file_of_module: Vec<usize> = Vec::new();
    for (index, file) in files.iter().enumerate() {
        if file.is_rust() {
            continue;
        }
        let module = parse_module(&file.src).map_err(|diag| ProjectError {
            file: index,
            diag,
            more: Vec::new(),
        })?;
        codegen::generics::validate(&module).map_err(|diag| ProjectError {
            file: index,
            diag,
            more: Vec::new(),
        })?;
        modules.push(ModuleFile {
            name: file.stem(),
            path: file.path.clone(),
            native: file.native,
            source: file.src.clone(),
            module,
        });
        file_of_module.push(index);
    }
    // custom hooks are inlined into the components calling them
    let mut units: Vec<Unit> = Vec::new();
    let mut unit_files: Vec<usize> = Vec::new();
    for (module_index, module) in modules.iter_mut().enumerate() {
        if module.native {
            continue;
        }
        units.push(Unit {
            name: module.name.as_bytes().to_vec(),
            path: module.path.as_bytes().to_vec(),
            module: std::mem::replace(
                &mut module.module,
                ast::Module {
                    imports: Vec::new(),
                    functions: Vec::new(),
                    types: Vec::new(),
                    consts: Vec::new(),
                    declared: Vec::new(),
                    enums: Vec::new(),
                },
            ),
            added: Vec::new(),
            exported: Vec::new(),
        });
        unit_files.push(module_index);
    }
    let mut failure: Option<hooks::expand::Failure> = None;
    if !hooks::expand::expand(&mut units, &mut failure) {
        let failure = failure.expect("hook expansion failure");
        return Err(ProjectError {
            file: file_of_module[unit_files[failure.unit]],
            diag: Diag {
                loc: failure.loc,
                msg: failure.msg,
            },
            more: Vec::new(),
        });
    }
    for (unit_index, unit) in units.into_iter().enumerate() {
        modules[unit_files[unit_index]].module = unit.module;
    }
    let window_components = window_components(&modules);
    let project = Rc::new(ProjectData {
        modules,
        rust: rust_sources,
        assets: RefCell::new(Vec::new()),
        root: String::new(),
        config: None,
        window_components,
        i18n: i18n.map(Rc::new),
    });
    let _guard = app_state::set_project(Some(Rc::clone(&project)));
    let module_sources = project.module_sources();
    let mut outputs: Vec<String> = files.iter().map(|file| file.src.clone()).collect();
    // specs first: their errors are the ones to fix before the code using them
    let mut failures: Vec<(usize, Diag)> = Vec::new();
    for natives in [true, false] {
        for (module_index, module) in project.modules.iter().enumerate() {
            if module.native != natives {
                continue;
            }
            let index = file_of_module[module_index];
            let file = &files[index];
            let options = generate::Options {
                source_name: basename(&file.path),
                project: &module_sources,
                require_root: false,
                source_path: Some(&file.path),
                native: file.native,
            };
            match generate::generate_with(
                basename(&file.path),
                &module.module,
                &options,
                install_extra,
            ) {
                Ok(generated) => outputs[index] = generated.source,
                Err(_) => {
                    // every file is generated: independent errors are
                    // reported together
                    for diag in generate::last_diags() {
                        // code inlined from another file's hook is reported there
                        let located = match unit_files.iter().position(|file| *file == module_index)
                        {
                            Some(unit) => {
                                let (unit_index, loc) = hooks::expand::decode(unit, diag.loc);
                                (
                                    file_of_module[unit_files[unit_index]],
                                    Diag { loc, msg: diag.msg },
                                )
                            }
                            None => (index, diag),
                        };
                        failures.push(located);
                    }
                }
            }
        }
    }
    if !failures.is_empty() {
        let (file, diag) = failures.remove(0);
        return Err(ProjectError {
            file,
            diag,
            more: failures,
        });
    }
    // The combined `native/spec.rs` root of the project's specs. The
    // per-spec outputs above stay available, but the app consumes this one
    // file as the `native::spec` module.
    let specs: Vec<crate::native_specs::Spec<'_>> = project
        .modules
        .iter()
        .filter(|module| module.native)
        .map(|module| crate::native_specs::Spec {
            name: module.name.as_str(),
            module: &module.module,
        })
        .collect();
    let spec_root = if specs.is_empty() {
        None
    } else {
        let mut spec_diag: Option<Diag> = None;
        match crate::native_specs::spec_root(&specs, &mut spec_diag) {
            Ok(source) => Some(source),
            Err(_) => {
                let diag = spec_diag.unwrap_or(Diag {
                    loc: 0,
                    msg: "native module spec error".to_owned(),
                });
                let file = project
                    .modules
                    .iter()
                    .position(|module| module.native)
                    .map(|module_index| file_of_module[module_index])
                    .unwrap_or(0);
                return Err(ProjectError {
                    file,
                    diag,
                    more: Vec::new(),
                });
            }
        }
    };
    let assets = project.assets.borrow().clone();
    Ok(GeneratedProject {
        outputs,
        assets,
        spec_root,
    })
}

/// The component names of every `createWindow(<Component>, …)` of the project
/// (the defining module emits the window wrapper, wherever the const lives).
pub(super) fn window_components(modules: &[app_state::ModuleFile]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for module in modules {
        for constant in &module.module.consts {
            if !crate::codegen::windows::is_create(&constant.value) {
                continue;
            }
            let ast::ExprData::Call(call) = &constant.value.data else {
                continue;
            };
            if call.args.is_empty() {
                continue;
            }
            let ast::ExprData::Ident(name) = &call.args[0].data else {
                continue;
            };
            let name = String::from_utf8_lossy(name).into_owned();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}
