//! TSX source → generated Rust, and project-level compilation. The generated
//! text is validated and formatted with `syn` + `prettyplease` inside
//! [`crate::codegen::generate`].
//!
//! - [`compile`] compiles one `.tsx` file (no project imports).
//! - [`compile_project`] compiles the files of a project: relative imports
//!   resolve between them, one Rust module is generated per `.tsx` file and
//!   hand-written `.rs` files are parsed for their public signatures.
//! - [`compile_project_root`] is the build integration used by
//!   `zant_build::tsx_app`: it reads `<root>/src/**`, the `native/`
//!   specs and `<root>/app.config.json`, and returns the generated modules,
//!   assets and the app entry.
//!
//! A diagnostic is a byte offset into the source plus a message, rendered by
//! [`crate::error::render_diag`] as `path:line:col: error: msg` with a caret.

use std::cell::RefCell;
use std::rc::Rc;

use crate::ast;
use crate::codegen::app_state::{self, ModuleFile, ProjectData, RustSource};
use crate::codegen::{self, generate};
use crate::entry;
use crate::hooks::{self, resolve::Unit};
use crate::parser::Parser;

pub use crate::error::Diag;

/// One input file of a compilation.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// Path as given on the command line (diagnostics use it).
    pub path: String,
    pub src: String,
    /// A native module spec (`native/<name>.ts`, compiled by `native_specs`).
    pub native: bool,
}

impl SourceFile {
    pub fn tsx(path: &str, src: &str) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            src: src.to_owned(),
            native: false,
        }
    }

    pub fn rust(path: &str, src: &str) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            src: src.to_owned(),
            native: false,
        }
    }

    pub fn native_spec(path: &str, src: &str) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            src: src.to_owned(),
            native: true,
        }
    }

    /// Hand-written Rust is copied beside the generated modules.
    pub fn is_rust(&self) -> bool {
        self.path.ends_with(".rs")
    }

    pub fn is_tsx(&self) -> bool {
        !self.is_rust()
    }

    /// The file stem: `src/components/TodoRow.tsx` → `TodoRow`.
    pub fn stem(&self) -> String {
        let base = self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path);
        for extension in [".tsx", ".ts", ".rs"] {
            if let Some(stem) = base.strip_suffix(extension) {
                return stem.to_owned();
            }
        }
        base.to_owned()
    }

    /// Both `App.tsx` and `App.rs` produce `App.rs` in the output directory.
    pub fn output_name(&self) -> String {
        let base = self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path);
        if self.native {
            return format!("_spec_{}.rs", self.stem());
        }
        if self.is_rust() {
            return base.to_owned();
        }
        format!("{}.rs", self.stem())
    }
}

/// A problem in one input file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectError {
    pub file: usize,
    pub diag: Diag,
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", self.diag.msg)
    }
}

impl std::error::Error for ProjectError {}

/// The result of compiling a project: one source per input (the generated
/// module, or a hand-written `.rs` file verbatim) and the asset imports.
#[derive(Clone, Debug, Default)]
pub struct GeneratedProject {
    pub outputs: Vec<String>,
    pub assets: Vec<crate::assets::Asset>,
    /// The combined `native/spec.rs`: one `pub mod` per
    /// `native/*.ts` spec, or `None` when the project has no native specs.
    /// The build writes it next to the app's `native/` modules; its module
    /// path is `crate::native::spec::<name>`.
    pub spec_root: Option<String>,
}

/// Parse, expand custom hooks and generate one file.
pub fn compile(src: &str, source_name: &str) -> Result<String, Diag> {
    let module = parse_module(src)?;
    let mut units = [Unit {
        name: source_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(source_name)
            .trim_end_matches(".tsx")
            .as_bytes()
            .to_vec(),
        path: source_name.as_bytes().to_vec(),
        module,
        added: Vec::new(),
        exported: Vec::new(),
    }];
    let mut failure: Option<hooks::expand::Failure> = None;
    if !hooks::expand::expand(&mut units, &mut failure) {
        let failure = failure.expect("hook expansion failure");
        return Err(Diag {
            loc: failure.loc,
            msg: failure.msg,
        });
    }
    let module = std::mem::replace(
        &mut units[0].module,
        ast::Module {
            imports: Vec::new(),
            functions: Vec::new(),
            types: Vec::new(),
            consts: Vec::new(),
            declared: Vec::new(),
        },
    );
    let options = generate::Options {
        source_name,
        ..generate::Options::default()
    };
    match generate::generate(source_name, &module, &options) {
        Ok(generated) => Ok(generated.source),
        Err(_) => Err(generate::last_diag().unwrap_or(Diag {
            loc: 0,
            msg: "code generation error".to_owned(),
        })),
    }
}

/// Compile the files of a project.
pub fn compile_project(files: &[SourceFile]) -> Result<GeneratedProject, ProjectError> {
    compile_project_with(files, None)
}

/// [`compile_project`] with one extra hook-installation step per module, run
/// after `generate::install` (the end-to-end tests inject their stubs through
/// this).
pub fn compile_project_with(
    files: &[SourceFile],
    install_extra: Option<fn(&mut codegen::Generator<'_>)>,
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
        let module = parse_module(&file.src).map_err(|diag| ProjectError { file: index, diag })?;
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
    });
    let _guard = app_state::set_project(Some(Rc::clone(&project)));
    let module_sources = project.module_sources();
    let mut outputs: Vec<String> = files.iter().map(|file| file.src.clone()).collect();
    // specs first: their errors are the ones to fix before the code using them
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
                    let diag = generate::last_diag().unwrap_or(Diag {
                        loc: 0,
                        msg: "code generation error".to_owned(),
                    });
                    // code inlined from another file's hook is reported there
                    if let Some(unit) = unit_files.iter().position(|file| *file == module_index) {
                        let (unit_index, loc) = hooks::expand::decode(unit, diag.loc);
                        let where_file = unit_files[unit_index];
                        return Err(ProjectError {
                            file: file_of_module[where_file],
                            diag: Diag { loc, msg: diag.msg },
                        });
                    }
                    return Err(ProjectError { file: index, diag });
                }
            }
        }
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
                return Err(ProjectError { file, diag });
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

/// The output file name of an input (used by the build and the CLI).
pub fn output_name(file: &SourceFile) -> String {
    file.output_name()
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn parse_module(src: &str) -> Result<ast::Module, Diag> {
    let mut parser = Parser::init(src);
    match parser.parse_module() {
        Ok(module) => Ok(module),
        Err(_) => Err(parser.diag.unwrap_or(Diag {
            loc: 0,
            msg: "syntax error".to_owned(),
        })),
    }
}

// ── the project-root build integration ───────────────────────────────

/// The app configuration from `app.config.json` (the subset generated Rust
/// needs).
#[derive(Clone, Debug)]
pub struct ConfigSpec {
    pub title: String,
    pub bundle_id: String,
    pub single_instance: bool,
    pub window: WindowConfig,
}

impl Default for ConfigSpec {
    fn default() -> Self {
        ConfigSpec {
            title: "Zant App".to_owned(),
            bundle_id: "dev.zant.app".to_owned(),
            single_instance: false,
            window: WindowConfig::default(),
        }
    }
}

/// The window section of `app.config.json`, with its defaults.
#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub style: String,
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
    pub min_height: f32,
    pub background: u32,
    pub resizable: bool,
    pub always_on_top: bool,
    pub movable_by_background: bool,
    pub center: bool,
    pub titlebar_height: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        WindowConfig {
            style: "native".to_owned(),
            width: 560.0,
            height: 420.0,
            min_width: 480.0,
            min_height: 320.0,
            background: 0xfff5_f5f5,
            resizable: true,
            always_on_top: false,
            movable_by_background: false,
            center: true,
            titlebar_height: 0.0,
        }
    }
}

/// Reads `app.config.json` (the fields generated Rust uses; malformed values
/// are an error, unknown fields are ignored).
pub fn read_config(path: &str) -> Result<ConfigSpec, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let mut config = ConfigSpec::default();
    let Some(object) = value.as_object() else {
        return Err("not a JSON object".to_owned());
    };
    if let Some(title) = object.get("title").and_then(|value| value.as_str()) {
        config.title = title.to_owned();
    }
    if let Some(bundle) = object.get("bundle_id").and_then(|value| value.as_str()) {
        config.bundle_id = bundle.to_owned();
    }
    if let Some(flag) = object
        .get("single_instance")
        .and_then(|value| value.as_bool())
    {
        config.single_instance = flag;
    }
    if let Some(window) = object.get("window").and_then(|value| value.as_object()) {
        let window_config = &mut config.window;
        if let Some(style) = window.get("style").and_then(|value| value.as_str()) {
            window_config.style = style.to_owned();
        }
        if let Some(value) = window.get("width").and_then(|value| value.as_f64()) {
            window_config.width = value as f32;
        }
        if let Some(value) = window.get("height").and_then(|value| value.as_f64()) {
            window_config.height = value as f32;
        }
        if let Some(value) = window.get("min_width").and_then(|value| value.as_f64()) {
            window_config.min_width = value as f32;
        }
        if let Some(value) = window.get("min_height").and_then(|value| value.as_f64()) {
            window_config.min_height = value as f32;
        }
        if let Some(value) = window
            .get("titlebar_height")
            .and_then(|value| value.as_f64())
        {
            window_config.titlebar_height = value as f32;
        }
        if let Some(background) = window.get("background").and_then(|value| value.as_str()) {
            window_config.background = parse_color(background)?;
        }
        if let Some(flag) = window.get("resizable").and_then(|value| value.as_bool()) {
            window_config.resizable = flag;
        }
        if let Some(flag) = window
            .get("always_on_top")
            .and_then(|value| value.as_bool())
        {
            window_config.always_on_top = flag;
        }
        if let Some(flag) = window
            .get("movable_by_background")
            .and_then(|value| value.as_bool())
        {
            window_config.movable_by_background = flag;
        }
        if let Some(flag) = window.get("center").and_then(|value| value.as_bool()) {
            window_config.center = flag;
        }
    }
    Ok(config)
}

fn parse_color(text: &str) -> Result<u32, String> {
    let value = text.strip_prefix('#').unwrap_or(text);
    match value.len() {
        6 => u32::from_str_radix(value, 16)
            .map(|rgb| 0xff00_0000 | rgb)
            .map_err(|_| format!("invalid color `{text}`")),
        8 => u32::from_str_radix(value, 16).map_err(|_| format!("invalid color `{text}`")),
        _ => Err(format!("invalid color `{text}`")),
    }
}

/// The options of a project build (`zant-tsx --out`): the project root, the
/// output directory and the optional entry file.
#[derive(Clone, Debug)]
pub struct ProjectOptions {
    pub root: String,
    pub out: String,
    pub src: String,
    pub native: String,
    /// The entry `.tsx` stem (`--main App`).
    pub main: Option<String>,
    /// `app.config.json` (read by `--app-config` or found in the root).
    pub app_config: Option<String>,
    /// The dep file (`--dep`).
    pub dep: Option<String>,
}

impl ProjectOptions {
    pub fn new(root: &str, out: &str) -> ProjectOptions {
        ProjectOptions {
            root: root.to_owned(),
            out: out.to_owned(),
            src: format!("{root}/src"),
            native: format!("{root}/native"),
            main: None,
            app_config: None,
            dep: None,
        }
    }
}

/// Compile a project root: `<src>/**/*.tsx`, hand-written `.rs` files and the
/// `native/*.ts` specs, with `app.config.json` for the entry.
pub fn compile_project_root(options: &ProjectOptions) -> Result<BuildOutput, BuildError> {
    compile_project_root_with(options, None)
}

/// [`compile_project_root`] with the test-only hook installation of
/// [`compile_project_with`].
pub fn compile_project_root_with(
    options: &ProjectOptions,
    install_extra: Option<fn(&mut codegen::Generator<'_>)>,
) -> Result<BuildOutput, BuildError> {
    let mut files: Vec<SourceFile> = Vec::new();
    for path in entry::collect(&options.src)? {
        let src = std::fs::read_to_string(&path)?;
        files.push(SourceFile::tsx(&path, &src));
    }
    if std::path::Path::new(&options.native).is_dir() {
        for path in entry::collect_specs(&options.native)? {
            let src = std::fs::read_to_string(&path)?;
            files.push(SourceFile::native_spec(&path, &src));
        }
    }
    let config = match &options.app_config {
        Some(path) => Some(read_config(path).map_err(BuildError::Config)?),
        None => {
            let path = format!("{}/app.config.json", options.root);
            if std::path::Path::new(&path).is_file() {
                Some(read_config(&path).map_err(BuildError::Config)?)
            } else {
                None
            }
        }
    };
    let compiled = compile_project_with(&files, install_extra).map_err(BuildError::Compile)?;
    let dep_file = options
        .dep
        .as_ref()
        .map(|_| crate::assets::dep_file(&options.out, &sources(&files), &compiled.assets));
    let mut output = Vec::new();
    for (index, file) in files.iter().enumerate() {
        if file.is_rust() {
            // the app's crate declares its own modules (`mod format;`); the
            // copy would collide with them
            continue;
        }
        if file.native {
            // the specs are one generated `spec.rs` (`BuildOutput::write`),
            // not one file each
            continue;
        }
        output.push(OutputFile {
            name: file.output_name(),
            source: compiled.outputs[index].clone(),
        });
    }
    let entry = match &options.main {
        Some(name) => {
            Some(entry::main_source(&files, name, config.as_ref()).map_err(BuildError::Entry)?)
        }
        None => None,
    };
    Ok(BuildOutput {
        files: output,
        entry,
        assets: compiled.assets,
        dep_file,
        spec_root: compiled.spec_root,
    })
}

fn sources(files: &[SourceFile]) -> Vec<String> {
    files.iter().map(|file| file.path.clone()).collect()
}

/// One generated file of a project build.
#[derive(Clone, Debug)]
pub struct OutputFile {
    pub name: String,
    pub source: String,
}

/// The generated project: modules, the entry (if `--main`) and the dep file.
#[derive(Clone, Debug, Default)]
pub struct BuildOutput {
    pub files: Vec<OutputFile>,
    pub entry: Option<String>,
    /// Asset imports: copy each `from` to `<out>/<key>`.
    pub assets: Vec<crate::assets::Asset>,
    pub dep_file: Option<String>,
    /// The combined `native/spec.rs` of the project's `native/*.ts` specs.
    /// Written as `spec.rs` in the output directory; the app's `native/lib.rs`
    /// consumes the file that `zant-build` writes into its `native/` directory
    /// instead.
    pub spec_root: Option<String>,
}

impl BuildOutput {
    /// Writes the modules, the entry, the assets and the dep file (`zant-tsx
    /// --out`).
    pub fn write(&self, out: &str, dep: Option<&str>) -> std::io::Result<()> {
        std::fs::create_dir_all(out)?;
        for file in &self.files {
            std::fs::write(format!("{out}/{}", file.name), &file.source)?;
        }
        for asset in &self.assets {
            let destination = format!("{out}/{}", asset.key);
            if let Some(parent) = std::path::Path::new(&destination).parent() {
                std::fs::create_dir_all(parent)?;
            }
            let data = std::fs::read(&asset.from)?;
            std::fs::write(destination, data)?;
        }
        if let Some(entry) = &self.entry {
            std::fs::write(format!("{out}/main.rs"), entry)?;
        }
        if let Some(spec_root) = &self.spec_root {
            std::fs::write(format!("{out}/spec.rs"), spec_root)?;
        }
        if let (Some(dep), Some(dep_file)) = (dep, &self.dep_file) {
            if let Some(parent) = std::path::Path::new(dep).parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dep, dep_file)?;
        }
        Ok(())
    }
}

/// A project build failure.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Compile(ProjectError),
    #[error("{0}")]
    Entry(entry::Error),
}

/// The component names of every `createWindow(<Component>, …)` of the project
/// (the defining module emits the window wrapper, wherever the const lives).
fn window_components(modules: &[app_state::ModuleFile]) -> Vec<String> {
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
