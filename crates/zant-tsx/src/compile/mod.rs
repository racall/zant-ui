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

mod config;
mod project;
mod root;

pub use self::config::*;
pub use self::project::*;
pub use self::root::*;

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
    /// Further independent errors (other components, other files), after
    /// the first: `(file index, diagnostic)`.
    pub more: Vec<(usize, Diag)>,
}

impl ProjectError {
    /// The first error and the others, in report order.
    pub fn all(&self) -> impl Iterator<Item = (usize, &Diag)> {
        std::iter::once((self.file, &self.diag))
            .chain(self.more.iter().map(|(file, diag)| (*file, diag)))
    }

    /// Every error rendered as `path:line:col: error: msg` with its source
    /// line and caret ([`crate::error::render_diag`]), one after another.
    pub fn render(&self, files: &[SourceFile]) -> String {
        let mut out = String::new();
        for (index, diag) in self.all() {
            match files.get(index) {
                Some(file) => out.push_str(&crate::error::render_diag(&file.path, &file.src, diag)),
                None => {
                    out.push_str(&diag.msg);
                    out.push('\n');
                }
            }
        }
        out
    }
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
    codegen::generics::validate(&module)?;
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
            enums: Vec::new(),
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
    compile_project_i18n(files, install_extra, None)
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
