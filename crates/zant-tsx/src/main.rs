//! `zant-tsx`: compiles a `.tsx` component file into Rust (docs/tsx.md).
//!
//! ```text
//! 用法: zant-tsx <in.tsx> <out.rs>
//!       zant-tsx --out <dir> [--main <App>] [--src <dir>] [--native <dir>]
//!               [--dep <file>] [--app-config <file>] [<a.tsx> <b.rs>...]
//! ```
//!
//! Run by the build (`zant_build::tsx_app` / `zant-tsx-tags`); errors print as
//! `file:line:col: error: …` and exit with status 1.
//!
//! Flags: `--out`, `--main`, `--src`, `--native`, `--dep`, `--app-config`,
//! `--help` / `-h`. The generated entry carries a `zant::tsx::app::Config`
//! literal, so no platform-specific config file is generated. AST dumps are a
//! library helper (`dump::dump_program`), not a CLI switch.

use std::process::ExitCode;

use zant_tsx::compile::{self, ConfigSpec, GeneratedProject, SourceFile};
use zant_tsx::entry;

const USAGE: &str = "用法: zant-tsx <in.tsx> <out.rs>\n      zant-tsx --out <dir> [--main <App>] [--src <dir>] [--native <dir>] [--dep <file>] [--app-config <file>] [<a.tsx> <b.rs>...]\n";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.first().is_some_and(|argument| argument == "--out") {
        if args.len() < 3 {
            print!("{USAGE}");
            return ExitCode::from(2);
        }
        return project(&args[1], &args[2..]);
    }
    if args.len() != 2 {
        print!("{USAGE}");
        return ExitCode::from(2);
    }
    single_file(&args[0], &args[1])
}

/// `zant-tsx <in.tsx> <out.rs>`: one file, no project imports.
fn single_file(input: &str, output: &str) -> ExitCode {
    let source = match read(input) {
        Some(source) => source,
        None => return ExitCode::FAILURE,
    };
    match compile::compile(&source, input) {
        Ok(rust) => match std::fs::write(output, rust) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("zant-tsx: 不能写入 {output}（{error}）");
                ExitCode::FAILURE
            }
        },
        Err(diag) => {
            eprint!("{}", zant_tsx::error::render_diag(input, &source, &diag));
            ExitCode::FAILURE
        }
    }
}

/// `--out <dir> [flags] files…`: compile TSX and copy Rust into one output
/// directory; `--src` adds every `.tsx` / `.rs` under a directory; `--main App`
/// also writes the app's entry `main.rs`; `--native` adds the `native/*.ts`
/// specs; imported assets are copied to `_assets/` and listed in the `--dep`
/// file.
fn project(out_dir: &str, rest: &[String]) -> ExitCode {
    let mut paths: Vec<String> = Vec::new();
    let mut main_entry: Option<String> = None;
    let mut dep_file: Option<String> = None;
    let mut app_config: Option<String> = None;
    let mut native_dir: Option<String> = None;
    let mut index = 0;
    while index < rest.len() {
        let argument = rest[index].as_str();
        match argument {
            "--dep" if index + 1 < rest.len() => {
                index += 1;
                dep_file = Some(rest[index].clone());
            }
            "--app-config" if index + 1 < rest.len() => {
                index += 1;
                app_config = Some(rest[index].clone());
            }
            "--native" if index + 1 < rest.len() => {
                index += 1;
                native_dir = Some(rest[index].clone());
            }
            "--main" if index + 1 < rest.len() => {
                index += 1;
                main_entry = Some(rest[index].clone());
            }
            "--src" if index + 1 < rest.len() => {
                index += 1;
                match entry::collect(&rest[index]) {
                    Ok(files) => paths.extend(files),
                    Err(error) => {
                        eprintln!("zant-tsx: 不能读取 {}（{error}）", rest[index]);
                        return ExitCode::FAILURE;
                    }
                }
            }
            _ => paths.push(rest[index].clone()),
        }
        index += 1;
    }
    if let Some(native) = &native_dir {
        match entry::collect_specs(native) {
            Ok(specs) => paths.extend(specs),
            Err(error) => {
                eprintln!("zant-tsx: 不能读取 {native}（{error}）");
                return ExitCode::FAILURE;
            }
        }
    }
    // the app.config.json given replaces a stale generated config file
    if app_config.is_some() {
        paths.retain(|path| !path.ends_with("app_config.rs"));
    }
    let native_dir_present = native_dir.is_some();
    let mut files: Vec<SourceFile> = Vec::new();
    for path in &paths {
        let Some(source) = read(path) else {
            return ExitCode::FAILURE;
        };
        let native = native_dir_present
            && native_dir
                .as_deref()
                .is_some_and(|native| path.starts_with(native));
        files.push(if native {
            SourceFile::native_spec(path, &source)
        } else if path.ends_with(".rs") {
            SourceFile::rust(path, &source)
        } else {
            SourceFile::tsx(path, &source)
        });
    }
    let compiled: GeneratedProject = match compile::compile_project(&files) {
        Ok(compiled) => compiled,
        Err(error) => {
            let file = &files[error.file];
            eprint!(
                "{}",
                zant_tsx::error::render_diag(&file.path, &file.src, &error.diag)
            );
            return ExitCode::FAILURE;
        }
    };
    let config: Option<ConfigSpec> = match &app_config {
        Some(path) => match compile::read_config(path) {
            Ok(config) => Some(config),
            Err(error) => {
                eprintln!("{path}: error: {error}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };
    let dep_source = dep_file.as_ref().map(|_| {
        let sources: Vec<String> = files.iter().map(|file| file.path.clone()).collect();
        zant_tsx::assets::dep_file(out_dir, &sources, &compiled.assets)
    });
    let mut output = compile::BuildOutput {
        files: Vec::new(),
        entry: None,
        assets: compiled.assets,
        dep_file: dep_source,
        // `native/*.ts` specs: one combined `spec.rs` in the output directory.
        spec_root: compiled.spec_root,
    };
    for (index, file) in files.iter().enumerate() {
        if file.is_rust() || file.native {
            continue;
        }
        output.files.push(compile::OutputFile {
            name: file.output_name(),
            source: compiled.outputs[index].clone(),
        });
    }
    if let Some(name) = &main_entry {
        match entry::main_source(&files, name, config.as_ref()) {
            Ok(source) => output.entry = Some(source),
            Err(error) => {
                eprintln!("zant-tsx: --main {name}: {}", entry::describe(error));
                return ExitCode::FAILURE;
            }
        }
    }
    match output.write(out_dir, dep_file.as_deref()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zant-tsx: 不能写入 {out_dir}（{error}）");
            ExitCode::FAILURE
        }
    }
}

fn read(path: &str) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(source) => Some(source),
        Err(error) => {
            eprintln!("zant-tsx: 不能读取 {path}（{error}）");
            None
        }
    }
}
