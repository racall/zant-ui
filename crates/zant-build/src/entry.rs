//! Generated application shell, shared by executable and hot library builds.
use std::path::Path;

use crate::{Error, config, read_text, write_file};
use zant_tsx::compile::GeneratedProject;

pub(crate) fn write(
    root: &Path,
    out: &Path,
    project: &GeneratedProject,
    declarations: &str,
) -> Result<Vec<std::path::PathBuf>, Error> {
    let path = root.join("app.config.json");
    let cfg = if path.is_file() {
        config::parse(&read_text(&path)?, config::Os::current()).map_err(|error| Error::Config {
            path,
            message: error.to_string(),
        })?
    } else {
        config::AppConfig::default()
    };
    let config = config::source(&cfg).replace("#![allow(dead_code)]", "");
    let mut app = format!(
        "#[allow(dead_code, non_upper_case_globals)]\npub mod app_config {{\n{config}\n}}\npub mod native {{\n"
    );
    let native = root.join("native");
    let mut modules = Vec::new();
    if native.is_dir() {
        for item in std::fs::read_dir(&native)? {
            let path = item?.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
                let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
                // editor / archive leftovers ("._core.rs", "core copy.rs") are not modules
                let identifier = stem.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                    && stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if identifier && stem != "main" && stem != "build" {
                    modules.push((stem, path.canonicalize()?));
                }
            }
        }
    }
    modules.sort();
    for (name, path) in &modules {
        app.push_str(&format!(
            "#[path = {:?}]\npub mod {name};\n",
            path.to_string_lossy()
        ));
    }
    let spec = project.spec_root.as_deref().unwrap_or("");
    app.push_str(&format!(
        "pub mod spec {{\n{spec}\n}}\npub fn register() {{\n"
    ));
    for (name, _) in &modules {
        if native.join(format!("{name}.ts")).is_file() {
            app.push_str(&format!("zant::tsx::native::register({name}::Native);\n"));
        }
    }
    app.push_str("}\n}\n");
    app.push_str(&declarations.replace(
        "#[path =",
        "#[allow(clippy::all, non_upper_case_globals)]\n#[path =",
    ));
    let root_source = read_text(&root.join("src/App.tsx"))?;
    let mut parser = zant_tsx::parser::Parser::init(&root_source);
    // `useClipboardChange` in the root or in a custom hook it calls
    let watch_clipboard = parser.parse_module().is_ok_and(|module| {
        zant_tsx::codegen::app_hooks::watches_clipboard(&module)
            || zant_tsx::codegen::app_hooks::watches_clipboard_via_hooks(&module, &project_modules(&root.join("src")))
    });
    let config = "zant::tsx::app::Config { title: app_config::title.to_string(), window: app_config::window, bundle_id: app_config::bundle_id.to_string(), single_instance: app_config::single_instance, remember: app_config::remember, watch_clipboard: WATCH_CLIPBOARD }".replace("WATCH_CLIPBOARD", if watch_clipboard { "true" } else { "false" });
    let main = format!(
        "fn main() -> zant::Result<()> {{ native::register(); zant::tsx::app::run::<crate::app::App>({config}) }}\n"
    );
    let hot = format!(
        "zant::hot_entry!({{ native::register(); zant::tsx::app::prepare::<crate::app::App>({config}) }});\n"
    );
    let mut outputs = Vec::new();
    for (name, source) in [
        ("zant_app.rs", app),
        ("zant_main.rs", main),
        ("zant_hot.rs", hot),
    ] {
        let path = out.join(name);
        let parsed = syn::parse_file(&source)
            .map_err(|error| Error::Message(format!("generated {name}: {error}")))?;
        let formatted = prettyplease::unparse(&parsed);
        write_file(&path, formatted.as_bytes())?;
        outputs.push(path);
    }
    Ok(outputs)
}

/// Preserve default exports with other names while giving the shell a fixed entry.
pub(crate) fn root_alias(source: &str) -> String {
    let mut parser = zant_tsx::parser::Parser::init(source);
    if let Ok(module) = parser.parse_module()
        && let Some(root) = module.functions.iter().find(|top| top.default)
    {
        let name = String::from_utf8_lossy(&root.func.name);
        if name != "App" {
            return format!("\n#[allow(unused_imports)]\npub use {name} as App;\n");
        }
    }
    String::new()
}

/// Check all source directories, including hidden ones the TSX collector skips.
pub(crate) fn reject_rust_sources(directory: &Path) -> Result<(), Error> {
    for item in std::fs::read_dir(directory)? {
        let path = item?.path();
        if path.is_dir() {
            reject_rust_sources(&path)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            return Err(Error::Message(format!(
                "`{}`: src/ contains only TSX/TS; move Rust code into native/ with a .ts spec",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Checks that `src/App.tsx` really is the app entry: a default exported
/// component with no props (the project compiler runs with
/// `require_root = false` and would otherwise emit a module without `App`).
pub(crate) fn validate_root(path: &Path, source: &str) -> Result<(), Error> {
    let mut parser = zant_tsx::parser::Parser::init(source);
    let Ok(module) = parser.parse_module() else {
        return Ok(());
    };
    let Some(root) = module.functions.iter().find(|top| top.default) else {
        return Err(Error::NoRootExport {
            path: path.to_path_buf(),
        });
    };
    if !root.func.params.is_empty() {
        return Err(Error::RootTakesProps {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

/// Every parsable `.ts` / `.tsx` module under `src`.
fn project_modules(src: &Path) -> Vec<zant_tsx::ast::Module> {
    let mut out = Vec::new();
    let mut dirs = vec![src.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let script = path.extension().is_some_and(|e| e == "ts" || e == "tsx");
            if !script {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path)
                && let Ok(module) = zant_tsx::parser::Parser::init(&text).parse_module()
            {
                out.push(module);
            }
        }
    }
    out
}
