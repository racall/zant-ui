//! Compiling a project directory (`zant-tsx --out`): reading the sources and writing the outputs and the dep file.

use super::*;

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

pub(super) fn sources(files: &[SourceFile]) -> Vec<String> {
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
