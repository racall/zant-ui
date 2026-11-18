//! Build-script helpers for a Zant application.
//!
//! An application's `build.rs` calls [`tsx_app`]:
//!
//! ```rust,ignore
//! fn main() {
//!     zant_build::tsx_app();
//! }
//! ```
//!
//! `native/build.rs` compiles TSX and native specs into `$OUT_DIR`. Applications
//! use `zant::app!()` in `native/main.rs`; configuration, native registration
//! and both normal/hot entry points are generated without writing source files.
//!
#![forbid(unsafe_code)]

pub mod config;
mod entry;
pub mod i18n;
mod icons;
pub mod png;
#[cfg(test)]
mod tests;
#[cfg_attr(not(windows), allow(dead_code))]
mod version_info;

#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
mod windows_stub;

#[cfg(windows)]
use windows as platform;
#[cfg(not(windows))]
use windows_stub as platform;

use std::path::{Path, PathBuf};

use zant_tsx::compile::{self, GeneratedProject, SourceFile};
use zant_tsx::{codegen, entry as tsx_entry};

pub use icons::{Icon, enumerate as app_icons, module_source as icon_module_source};
pub use zant_tsx::compile::ConfigSpec as AppConfig;

/// The root component's file stem (`src/App.tsx`).
pub const ENTRY: &str = "App";

/// A build-script failure (`build.rs` prints it and panics, like Cargo).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "the app root `{root}` has no `src/` directory; \
         `tsx_app()` must run from the app's build.rs (CARGO_MANIFEST_DIR)"
    )]
    NoSourceDir { root: PathBuf },
    #[error(
        "`{path}` is missing: a TSX app's root component is `src/{ENTRY}.tsx` \
         (with `export default function {ENTRY}()`)"
    )]
    NoRootComponent { path: PathBuf },
    #[error(
        "`{path}` has no default exported component; the app's root is \
         `export default function {ENTRY}() {{ … }}`"
    )]
    NoRootExport { path: PathBuf },
    #[error("the root component of `{path}` takes no props (it is the app's state)")]
    RootTakesProps { path: PathBuf },
    #[error(
        "`{path}` generates `main.rs`; rename it (the entry file is generated as OUT_DIR/main.rs)"
    )]
    EntryCollision { path: String },
    #[error("cannot read `{path}`: {message}")]
    Read { path: PathBuf, message: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("cannot write `{path}`: {message}")]
    Write { path: PathBuf, message: String },
    #[error("`{path}`: {message}")]
    Config { path: PathBuf, message: String },
    #[error("{0}")]
    Compile(String),
    #[error("{path}: {message}")]
    Icon { path: PathBuf, message: String },
    #[error("{0}")]
    Message(String),
}

/// The build-script entry point: compiles the app's TSX project into
/// `$OUT_DIR`, prints the Cargo directives and applies the platform extras.
///
/// # Panics
///
/// On any error, with the message of [`Error`] prefixed by `zant-build:`.
pub fn tsx_app() {
    if let Err(error) = run_build_script() {
        panic!("zant-build: {error}");
    }
}

/// [`tsx_app`] without the panic: the result, for callers that want to handle
/// the failure themselves.
pub fn try_tsx_app() -> Result<BuildReport, Error> {
    run_build_script()
}

fn run_build_script() -> Result<BuildReport, Error> {
    let root = environment_path("CARGO_MANIFEST_DIR")?;
    let out_dir = environment_path("OUT_DIR")?;
    let report = hot_app(&root)?;
    platform::apply(&root, &out_dir)?;
    Ok(report)
}

/// Build an app rooted outside the current Cargo package as a hot UI library.
/// Generates the same app shell as [`tsx_app`], emits absolute Cargo inputs,
/// and leaves executable-only platform resources to the host.
pub fn hot_app(root: &Path) -> Result<BuildReport, Error> {
    let out_dir = environment_path("OUT_DIR")?;
    let started = std::time::Instant::now();
    let report = tsx_app_in(root, &out_dir)?;
    print_cargo_directives(&report, root);
    println!("cargo:rerun-if-env-changed=ZANT_HOT_TRACE");
    if std::env::var_os("ZANT_HOT_TRACE").is_some() {
        println!(
            "cargo:warning=[zant-hot] codegen_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    if let Ok(output) = std::process::Command::new(rustc).arg("--version").output() {
        let line = String::from_utf8_lossy(&output.stdout);
        println!(
            "cargo:rustc-env=RUSTC_VERSION={}",
            line.trim().strip_prefix("rustc ").unwrap_or(line.trim())
        );
    }
    Ok(report)
}

fn environment_path(name: &str) -> Result<PathBuf, Error> {
    match std::env::var_os(name) {
        Some(value) if !value.is_empty() => Ok(PathBuf::from(value)),
        _ => Err(Error::Message(format!(
            "{name} is not set: zant_build::tsx_app() must be called from the \
             app's build.rs (cargo provides {name} there)"
        ))),
    }
}

/// The result of a successful [`tsx_app_in`]: everything the build script
/// needs to print Cargo directives for, and tests to assert on.
#[derive(Clone, Debug)]
pub struct BuildReport {
    /// Every compiler input (`.tsx` / `.ts` under `src/`, the
    /// `native/*.ts` specs) — for `cargo:rerun-if-changed`.
    pub sources: Vec<PathBuf>,
    /// The asset files imported from TSX (copied to `_assets/`).
    pub assets: Vec<PathBuf>,
    /// The generated module files and `main.rs`.
    pub outputs: Vec<PathBuf>,
    /// `assets/icons/*.png` as `icon_<name>`.
    pub icons: Vec<Icon>,
    /// The generated `$OUT_DIR/icons.rs`.
    pub icons_module: PathBuf,
    /// `app.config.json`, when the app has one.
    pub config_path: Option<PathBuf>,
    /// Parsed compiler configuration (kept for compatibility).
    pub config: Option<AppConfig>,
    /// `$OUT_DIR/app.d` (Makefile-style dep file: sources and assets).
    pub dep_file: PathBuf,
    /// The generated `zant_app.rs` containing native specs, when present.
    pub spec_module: Option<PathBuf>,
    /// `locales/*.json` (rerun-if-changed).
    pub locales: Vec<PathBuf>,
    /// Missing translations, reported as Cargo warnings.
    pub warnings: Vec<String>,
}

/// Compile the app at `app_root` into `out_dir` — the testable core of
/// [`tsx_app`] (no Cargo directives, no platform resource embedding).
///
/// The inputs are `<app_root>/src/**` (`.tsx` / `.ts`), the
/// `<app_root>/native/*.ts` specs, `assets/icons/*.png` and
/// `<app_root>/app.config.json`. `out_dir` receives the generated modules,
/// `main.rs`, `zant_app.rs`, `zant_main.rs`, `zant_hot.rs`, assets, icons and `app.d`.
pub fn tsx_app_in(app_root: &Path, out_dir: &Path) -> Result<BuildReport, Error> {
    let src_dir = app_root.join("src");
    if !src_dir.is_dir() {
        return Err(Error::NoSourceDir {
            root: app_root.to_path_buf(),
        });
    }
    let root_path = src_dir.join(format!("{ENTRY}.tsx"));
    if !root_path.is_file() {
        return Err(Error::NoRootComponent { path: root_path });
    }
    let root_source = read_text(&root_path)?;
    entry::validate_root(&root_path, &root_source)?;
    entry::reject_rust_sources(&src_dir)?;

    let mut files: Vec<SourceFile> = Vec::new();
    for path in tsx_entry::collect(&path_string(&src_dir))? {
        let source = read_text(Path::new(&path))?;
        files.push(SourceFile::tsx(&project_relative(app_root, &path), &source));
    }
    let native_dir = app_root.join("native");
    if native_dir.is_dir() {
        for path in tsx_entry::collect_specs(&path_string(&native_dir))? {
            let source = read_text(Path::new(&path))?;
            files.push(SourceFile::native_spec(
                &project_relative(app_root, &path),
                &source,
            ));
        }
    }
    if files.is_empty() {
        return Err(Error::Message(format!(
            "`{}` has no .tsx files; the TSX compiler needs at least src/{ENTRY}.tsx",
            src_dir.display()
        )));
    }

    let config_path = app_root.join("app.config.json");
    let config = if config_path.is_file() {
        Some(
            compile::read_config(&path_string(&config_path)).map_err(|message| Error::Config {
                path: config_path.clone(),
                message,
            })?,
        )
    } else {
        None
    };

    let translations = i18n::load(app_root)?;
    let index = translations.as_ref().map(i18n::AppI18n::index);
    // the app's `zant` features (its build script, `links = "zant"`): a tag
    // of a feature that is off becomes a TSX error
    codegen::app_state::set_framework_features(framework_features(
        std::env::var("DEP_ZANT_FEATURES").ok().as_deref(),
    ));
    let generated = compile::compile_project_i18n(&files, None, index)
        .map_err(|error| Error::Compile(error.render(&files)))?;

    std::fs::create_dir_all(out_dir)?;
    let mut outputs = write_modules(&files, &generated, out_dir)?;
    let spec_module = generated
        .spec_root
        .as_ref()
        .map(|_| out_dir.join("zant_app.rs"));
    outputs.extend(entry::write(
        app_root,
        out_dir,
        &generated,
        &module_declarations(&files),
        translations.as_ref(),
    )?);
    let assets = write_assets(app_root, &generated, out_dir)?;
    let dep_file = write_dep_file(&files, &generated, out_dir)?;

    let icons = icons::enumerate(app_root)?;
    let (icons_module, _copies) = icons::write(out_dir, &icons)?;

    Ok(BuildReport {
        sources: files.iter().map(|file| PathBuf::from(&file.path)).collect(),
        assets,
        outputs,
        icons,
        icons_module,
        config_path: config_path.is_file().then_some(config_path),
        config,
        dep_file,
        spec_module,
        locales: translations
            .as_ref()
            .map(|translations| translations.files.clone())
            .unwrap_or_default(),
        warnings: translations
            .map(|translations| translations.compiled.warnings)
            .unwrap_or_default(),
    })
}

/// Writes the generated modules and the declaration file `main.rs`, returning
/// every written path.
fn write_modules(
    files: &[SourceFile],
    generated: &GeneratedProject,
    out_dir: &Path,
) -> Result<Vec<PathBuf>, Error> {
    let mut outputs = Vec::new();
    for (index, file) in files.iter().enumerate() {
        if file.is_rust() {
            continue;
        }
        if file.native {
            continue;
        }
        let name = file.output_name();
        if name.eq_ignore_ascii_case("main.rs") {
            return Err(Error::EntryCollision {
                path: file.path.clone(),
            });
        }
        let destination = out_dir.join(&name);
        let mut source = generated.outputs[index].clone();
        if file.stem() == ENTRY {
            source.push_str(&entry::root_alias(&file.src));
        }
        write_file(&destination, source.as_bytes())?;
        outputs.push(destination);
    }
    let entry_path = out_dir.join("main.rs");
    let declarations = module_declarations(files);
    write_file(&entry_path, declarations.as_bytes())?;
    outputs.push(entry_path);
    Ok(outputs)
}

/// The generated `main.rs`: a plain list of `#[path] pub mod …;` declarations
fn module_declarations(files: &[SourceFile]) -> String {
    let mut out = String::from("// Generated TSX module declarations.\n");
    for file in files {
        if file.is_rust() || file.native {
            continue;
        }
        out.push_str(&format!(
            "#[path = \"{}\"]\npub mod {};\n",
            file.output_name(),
            codegen::imports::module_name(&file.stem())
        ));
    }
    out
}

/// Copies the TSX asset imports to `_assets/<key>` (the generated modules'
/// `include_bytes!` names them relative to their own file in `out_dir`).
fn write_assets(
    app_root: &Path,
    generated: &GeneratedProject,
    out_dir: &Path,
) -> Result<Vec<PathBuf>, Error> {
    let mut assets = Vec::with_capacity(generated.assets.len());
    for asset in &generated.assets {
        let relative = Path::new(&asset.from);
        let source_buf = if relative.is_absolute() {
            relative.to_path_buf()
        } else {
            app_root.join(relative)
        };
        let source = source_buf.as_path();
        let destination = out_dir.join(&asset.key);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = std::fs::read(source).map_err(|error| Error::Read {
            path: source.to_path_buf(),
            message: error.to_string(),
        })?;
        write_file(&destination, &data)?;
        assets.push(source.to_path_buf());
    }
    Ok(assets)
}

/// Writes `$OUT_DIR/app.d`: every source read and asset embedded
/// (`assets::dep_file`, the same dep file `zant-tsx --dep` writes).
fn write_dep_file(
    files: &[SourceFile],
    generated: &GeneratedProject,
    out_dir: &Path,
) -> Result<PathBuf, Error> {
    let sources: Vec<String> = files.iter().map(|file| file.path.clone()).collect();
    let contents = zant_tsx::assets::dep_file("main.rs", &sources, &generated.assets);
    let path = out_dir.join("app.d");
    write_file(&path, contents.as_bytes())?;
    Ok(path)
}

/// The Windows `.ico` (16, 20, 24, 32, 40, 48, 64, 256) built from
/// `assets/icons/app-windows.png`.
/// `None` when the app has no Windows icon; an invalid PNG is an error (the
/// executable's icon must not silently disappear).
pub fn windows_icon(app_root: &Path) -> Result<Option<Vec<u8>>, Error> {
    let path = app_root.join("assets/icons/app-windows.png");
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(Error::Read {
                path,
                message: error.to_string(),
            });
        }
    };
    let image = png::decode(&data).map_err(|error| Error::Icon {
        path: path.clone(),
        message: format!("{error}; needs an 8-bit, non-interlaced PNG"),
    })?;
    if image.width != image.height || image.width < 256 {
        return Err(Error::Icon {
            path,
            message: format!(
                "is {}×{}; the Windows app icon must be square and at least 256×256",
                image.width, image.height
            ),
        });
    }
    let ico = png::build_ico(&image).map_err(|error| Error::Icon {
        path,
        message: error.to_string(),
    })?;
    Ok(Some(ico))
}

/// `app.config.json` of the app at `app_root`, or `None` when it has none.
pub fn app_config(app_root: &Path) -> Result<Option<AppConfig>, Error> {
    let path = app_root.join("app.config.json");
    if !path.is_file() {
        return Ok(None);
    }
    compile::read_config(&path_string(&path))
        .map(Some)
        .map_err(|message| Error::Config { path, message })
}

/// The Zant UI checkout / installed `zant-ui/` directory this build script was
/// compiled against (`crates/zant-build`'s grandparent): `third_party/` and the
/// native sources live there.
pub fn zant_ui_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The tree whose `third_party/` holds the binary assets (PDFium, Skia…).
/// A Cargo `git` dependency carries only tracked sources, so it falls back to
/// the installed release, `$ZANT_HOME/zant-ui` (default `~/.zant` /
/// `%LOCALAPPDATA%\zant`) — the same rule as `zant-sys`'s build script.
pub fn zant_ui_assets_root() -> PathBuf {
    let root = zant_ui_root();
    if root.join("third_party").is_dir() {
        return root;
    }
    let home = std::env::var_os("ZANT_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            if cfg!(windows) {
                std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("zant"))
            } else {
                std::env::var_os("HOME").map(|dir| PathBuf::from(dir).join(".zant"))
            }
        });
    match home.map(|home| home.join("zant-ui")) {
        Some(installed) if installed.join("third_party").is_dir() => installed,
        _ => root,
    }
}

/// The Cargo directives the build script must emit for `report`: the rerun set
/// (the project directories so added/removed files are seen, then every file
/// the compiler read) and the fallback warnings.
pub fn cargo_directives(report: &BuildReport, app_root: &Path) -> Vec<String> {
    let mut out = vec!["cargo:rerun-if-changed=native/build.rs".to_owned()];
    for directory in ["src", "native", "assets", "locales"] {
        let path = app_root.join(directory);
        if path.exists() {
            out.push(format!("cargo:rerun-if-changed={}", path.display()));
        }
    }
    if let Some(config) = &report.config_path {
        out.push(format!("cargo:rerun-if-changed={}", config.display()));
    }
    let mut seen = std::collections::HashSet::new();
    for path in report
        .sources
        .iter()
        .chain(report.assets.iter())
        .chain(report.icons.iter().map(|icon| &icon.path))
        .chain(report.locales.iter())
    {
        if seen.insert(path.to_string_lossy().into_owned()) {
            out.push(format!("cargo:rerun-if-changed={}", path.display()));
        }
    }
    for warning in &report.warnings {
        out.push(format!("cargo:warning=zant-build: {warning}"));
    }
    if report.config.is_none() {
        out.push(format!(
            "cargo:warning=zant-build: {} has no app.config.json; the app uses the defaults",
            app_root.display()
        ));
    }
    if report.icons.is_empty() {
        out.push(format!(
            "cargo:warning=zant-build: {} has no assets/icons/*.png; the app has no icons",
            app_root.display()
        ));
    }
    out
}

/// [`cargo_directives`] on stdout, for the build script.
fn print_cargo_directives(report: &BuildReport, app_root: &Path) {
    for directive in cargo_directives(report, app_root) {
        println!("{directive}");
    }
}

fn read_text(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|error| Error::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn write_file(path: &Path, data: &[u8]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, data).map_err(|error| Error::Write {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `path` relative to the app root (`/a/b/src/App.tsx` → `src/App.tsx`); the
/// form the compiler and the dep file use. Paths outside the root are kept
/// as-is.
fn project_relative(app_root: &Path, path: &str) -> String {
    Path::new(path)
        .strip_prefix(app_root)
        .map(|rest| rest.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_owned())
}

/// `DEP_ZANT_FEATURES` (`"markdown,pdf"`) as a list; `None` when the build
/// script cannot see it (no direct `zant` dependency): everything allowed.
fn framework_features(value: Option<&str>) -> Option<Vec<String>> {
    value.map(|value| {
        value
            .split(',')
            .filter(|feature| !feature.is_empty())
            .map(str::to_owned)
            .collect()
    })
}
