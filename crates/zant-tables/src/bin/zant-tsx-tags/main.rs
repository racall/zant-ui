//! `zant-tsx-tags [--check] <dir>` writes the reflected tag table as a
//! directory of modules (one file per tag); `--check` verifies it is up to
//! date (same files, same contents, nothing extra).
//!
//! Regenerate `crates/zant-tsx/src/ui_tags/` with
//!
//! ```text
//! cargo run -p zant-tables --bin zant-tsx-tags -- crates/zant-tsx/src/ui_tags
//! cargo run -p zant-tables --bin zant-tsx-tags -- --check crates/zant-tsx/src/ui_tags
//! ```
//!
//! The `zant` crate directory comes from `$ZANT_HOME` (zant-ui root or the
//! crate itself) or the checkout this binary was built from.

mod emit;
mod pending;
mod source;
mod table;

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (check, dir) = match args.as_slice() {
        [path] if !path.starts_with("--") => (false, path.clone()),
        [flag, path] if flag == "--check" => (true, path.clone()),
        _ => {
            eprintln!("usage: zant-tsx-tags [--check] <dir>");
            return ExitCode::from(2);
        }
    };
    let files = emit::files(&table::generate());
    let dir = Path::new(&dir);
    let result = if check {
        check_dir(dir, &files)
    } else {
        write_dir(dir, &files)
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!(
                "zant-tsx-tags: {} is out of date; run \
                 `cargo run -p zant-tables --bin zant-tsx-tags -- {}`",
                dir.display(),
                dir.display()
            );
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("zant-tsx-tags: {}: {error}", dir.display());
            ExitCode::from(1)
        }
    }
}

/// The `.rs` files directly in `dir` (none when it does not exist).
fn existing(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let name = entry?.file_name().to_string_lossy().into_owned();
                if name.ends_with(".rs") {
                    names.push(name);
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(names)
}

/// Writes the files (only the changed ones) and deletes `.rs` files no tag
/// owns any more.
fn write_dir(dir: &Path, files: &[(String, String)]) -> std::io::Result<bool> {
    std::fs::create_dir_all(dir)?;
    for (name, source) in files {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(source.as_str()) {
            std::fs::write(&path, source)?;
        }
    }
    for name in existing(dir)? {
        if !files.iter().any(|(file, _)| *file == name) {
            std::fs::remove_file(dir.join(name))?;
        }
    }
    Ok(true)
}

/// Every generated file exists with the same contents and no other `.rs`
/// file is left in the directory.
fn check_dir(dir: &Path, files: &[(String, String)]) -> std::io::Result<bool> {
    for (name, source) in files {
        if std::fs::read_to_string(dir.join(name)).ok().as_deref() != Some(source.as_str()) {
            return Ok(false);
        }
    }
    Ok(existing(dir)?.len() == files.len())
}
