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
//! It compiles `src/**/*.tsx` (and `.ts`, plus hand-written `.rs` files that
//! TSX imports), the `native/*.ts` specs and the project configuration with
//! `zant-tsx` and writes into `$OUT_DIR`:
//!
//! - one `<Stem>.rs` per TSX module, and `main.rs` declaring them
//!   (`#[path = "App.rs"] pub mod app;`). The app's `src/main.rs` includes the
//!   declarations at its crate root:
//!
//!   ```rust,ignore
//!   include!(concat!(env!("OUT_DIR"), "/main.rs"));
//!   ```
//!
//!   `#[path]` inside the generated file resolves relative to the generated
//!   file's own directory (`$OUT_DIR`), so the `<Stem>.rs` modules are found
//!   next to it. Hand-written modules (`src/format.rs`, `native/…`) are not
//!   declared: the app's crate root does that itself.
//! - the assets imported from TSX (`import icon from "../assets/x.png"`) under
//!   `_assets/`, referenced by `include_bytes!` in the generated modules.
//! - `icons.rs`: one `icon_<name>` byte constant per `assets/icons/*.png`
//!   (`-` → `_`). Include it where the bytes are
//!   needed: `mod icons { include!(concat!(env!("OUT_DIR"), "/icons.rs")); }`.
//! - `app.d`, the dep file listing every source and asset the build read.
//!
//! It prints `cargo:rerun-if-changed` for the project inputs so editing a
//! `.tsx`, `.ts`, `.rs`, `native/` file, an icon/asset or `app.config.json`
//! rebuilds the app.
//!
//! On Windows (host) it additionally configures the executable: the `.ico`
//! built from `assets/icons/app-windows.png`, the per-monitor-DPI /
//! Common-Controls manifest (`crates/zant-sys/native/platform/windows/zant.manifest`)
//! and the GUI
//! subsystem for release builds are embedded with `embed-resource`, and
//! `pdfium.dll` is copied next to the executable. macOS is a no-op: `zant
//! package` turns `assets/icons/app-macos.png` into `AppIcon.icns`.
//!
//! Failures panic with an actionable message (a build script's only way to fail
//! the build); [`tsx_app_in`] is the testable variant that returns a
//! [`Error`] and also lets tests pass their own app root and output directory.

#![forbid(unsafe_code)]

mod icons;
mod png;
#[cfg(test)]
mod tests;

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
use zant_tsx::{codegen, entry};

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
    let report = tsx_app_in(&root, &out_dir)?;
    print_cargo_directives(&report, &root);
    platform::apply(&root, &out_dir)?;
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
    /// Every compiler input (`.tsx` / `.ts` / `.rs` under `src/`, the
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
    /// The parsed `app.config.json` (the Rust entry reads the generated
    /// `src/app_config.rs` instead, but the build validates the JSON it came
    /// from and keeps it in the rerun set).
    pub config: Option<AppConfig>,
    /// `$OUT_DIR/app.d` (Makefile-style dep file: sources and assets).
    pub dep_file: PathBuf,
    /// `<app_root>/native/spec.rs`: the generated module tree of the
    /// `native/*.ts` specs, written into the app so
    /// `native/lib.rs` can declare `pub mod spec;`. `None` when the app has
    /// no native specs.
    pub spec_module: Option<PathBuf>,
}

/// Compile the app at `app_root` into `out_dir` — the testable core of
/// [`tsx_app`] (no Cargo directives, no platform resource embedding).
///
/// The inputs are `<app_root>/src/**` (`.tsx` / `.ts` / `.rs`), the
/// `<app_root>/native/*.ts` specs, `assets/icons/*.png` and
/// `<app_root>/app.config.json`. `out_dir` receives the generated modules,
/// `main.rs`, `_assets/`, `_icons/`, `icons.rs` and `app.d`.
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
    validate_root(&root_path, &root_source)?;

    // The compiler inputs: every `.tsx` / `.ts` / `.rs` under `src` (the
    // hand-written Rust files are parsed for the signatures TSX imports), then
    // the native module specs.
    // Paths are app-relative (`src/App.tsx`, `native/system.ts`): the
    // diagnostics read like the sources on disk, the dep file names files the
    // way `zant-tsx --dep` does, and the asset hashes (which include the
    // importing path) do not change with the checkout directory.
    let mut files: Vec<SourceFile> = Vec::new();
    for path in entry::collect(&path_string(&src_dir))? {
        let source = read_text(Path::new(&path))?;
        files.push(SourceFile::tsx(&project_relative(app_root, &path), &source));
    }
    let native_dir = app_root.join("native");
    if native_dir.is_dir() {
        for path in entry::collect_specs(&path_string(&native_dir))? {
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

    let generated = compile::compile_project(&files).map_err(|error| {
        let file = &files[error.file];
        Error::Compile(zant_tsx::error::render_diag(
            &file.path,
            &file.src,
            &error.diag,
        ))
    })?;

    std::fs::create_dir_all(out_dir)?;
    let outputs = write_modules(&files, &generated, out_dir)?;
    let spec_module = write_spec_module(app_root, &generated)?;
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
    })
}

/// Writes the project's `native/spec.rs` into the app: one `pub mod`
/// per `native/*.ts` spec, which `native/lib.rs` declares as `pub mod spec;`.
///
/// The generated modules call `crate::native::spec::<name>::…` and
/// `<crate::native::<name>::Native as …Spec>`; the file is generated, so the
/// template `.gitignore`s it. The write is skipped when the content is
/// unchanged so `cargo:rerun-if-changed` on the `native/` directory (which
/// contains it) does not rebuild the app in a loop.
fn write_spec_module(
    app_root: &Path,
    generated: &GeneratedProject,
) -> Result<Option<PathBuf>, Error> {
    let Some(source) = &generated.spec_root else {
        return Ok(None);
    };
    let path = app_root.join("native/spec.rs");
    write_if_changed(&path, source.as_bytes())?;
    Ok(Some(path))
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
            // The app crate owns its hand-written modules (`mod format;`).
            continue;
        }
        if file.native {
            // `native/*.ts` specs become the combined `native/spec.rs`
            // (`write_spec_module`), not one generated file each.
            continue;
        }
        let name = file.output_name();
        if name.eq_ignore_ascii_case("main.rs") {
            return Err(Error::EntryCollision {
                path: file.path.clone(),
            });
        }
        let destination = out_dir.join(&name);
        write_file(&destination, generated.outputs[index].as_bytes())?;
        outputs.push(destination);
    }
    let entry_path = out_dir.join("main.rs");
    let declarations = module_declarations(files);
    write_file(&entry_path, declarations.as_bytes())?;
    outputs.push(entry_path);
    Ok(outputs)
}

/// The generated `main.rs`: a plain list of `#[path] pub mod …;` declarations
/// (no inner attributes / doc comments — the app `include!`s it at its crate
/// root, which rejects those).
fn module_declarations(files: &[SourceFile]) -> String {
    let mut out = String::from(
        "// Generated by zant-build from the app's TSX sources. Do not edit.\n\
         //\n\
         // `src/main.rs` includes this file at the crate root:\n\
         //     include!(concat!(env!(\"OUT_DIR\"), \"/main.rs\"));\n\
         // `#[path]` is resolved relative to this file's directory (OUT_DIR).\n",
    );
    for file in files {
        // Native specs are not declared here: they go to `native/spec.rs`
        // and the generated entry registers them.
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
        // `Asset::from` is relative to the app root (the compiler's importing
        // file paths are).
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
    // The target names the generated entry relative to `app.d`'s directory,
    // so the file is identical for every checkout location.
    let contents = zant_tsx::assets::dep_file("main.rs", &sources, &generated.assets);
    let path = out_dir.join("app.d");
    write_file(&path, contents.as_bytes())?;
    Ok(path)
}

/// Checks that `src/App.tsx` really is the app entry: a default exported
/// component with no props (the project compiler runs with
/// `require_root = false` and would otherwise emit a module without `App`).
fn validate_root(path: &Path, source: &str) -> Result<(), Error> {
    let mut parser = zant_tsx::parser::Parser::init(source);
    let Ok(module) = parser.parse_module() else {
        // The compiler reports syntax errors with a rendered diagnostic.
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

/// The Cargo directives the build script must emit for `report`: the rerun set
/// (the project directories so added/removed files are seen, then every file
/// the compiler read) and the fallback warnings.
pub fn cargo_directives(report: &BuildReport, app_root: &Path) -> Vec<String> {
    let mut out = vec!["cargo:rerun-if-changed=build.rs".to_owned()];
    // Directories first: cargo scans them recursively, so a new `.tsx` /
    // `.ts` / `native/` file is noticed even before it is in a source list.
    for directory in ["src", "native", "assets"] {
        let path = app_root.join(directory);
        if path.exists() {
            out.push(format!("cargo:rerun-if-changed={}", path.display()));
        }
    }
    if let Some(config) = &report.config_path {
        out.push(format!("cargo:rerun-if-changed={}", config.display()));
    }
    // The generated `native/spec.rs` is an output, but it lives in the
    // watched `native/` directory: name it so deleting it reruns the script
    // (which rewrites it) instead of failing on `pub mod spec;`.
    if let Some(spec) = &report.spec_module {
        out.push(format!("cargo:rerun-if-changed={}", spec.display()));
    }
    let mut seen = std::collections::HashSet::new();
    for path in report
        .sources
        .iter()
        .chain(report.assets.iter())
        .chain(report.icons.iter().map(|icon| &icon.path))
    {
        // `sources` are app-relative (`src/App.tsx`) while assets and icons are
        // absolute; print each once, the build script's cwd is the app root.
        if seen.insert(path.to_string_lossy().into_owned()) {
            out.push(format!("cargo:rerun-if-changed={}", path.display()));
        }
    }
    if report.config.is_none() {
        out.push(format!(
            "cargo:warning=zant-build: {} has no app.config.json; the app uses the defaults",
            app_root.display()
        ));
    }
    if report.config.is_some() && !app_root.join("src/app_config.rs").is_file() {
        out.push(
            "cargo:warning=zant-build: src/app_config.rs is missing; it is generated from \
             app.config.json — run `zant sync` (or `zant run`) before `cargo build`"
                .to_owned(),
        );
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

/// [`write_file`] only when the content changes: the generated file lives in
/// the app's watched `native/` directory, so rewriting identical bytes would
/// touch its mtime and rebuild the app again.
fn write_if_changed(path: &Path, data: &[u8]) -> Result<(), Error> {
    if std::fs::read(path).is_ok_and(|existing| existing == data) {
        return Ok(());
    }
    write_file(path, data)
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
