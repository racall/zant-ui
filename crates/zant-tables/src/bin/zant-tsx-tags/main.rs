//! `zant-tsx-tags [--check] <path>` writes the reflected tag table; `--check`
//! verifies the file is up to date.
//!
//! Regenerate `crates/zant-tsx/src/ui_tags.rs` with
//!
//! ```text
//! cargo run -p zant-tables --bin zant-tsx-tags -- crates/zant-tsx/src/ui_tags.rs
//! cargo run -p zant-tables --bin zant-tsx-tags -- --check crates/zant-tsx/src/ui_tags.rs
//! ```
//!
//! The `zant` crate directory comes from `$ZANT_HOME` (zant-ui root or the
//! crate itself) or the checkout this binary was built from.

mod emit;
mod pending;
mod source;
mod table;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (check, path) = match args.as_slice() {
        [path] if !path.starts_with("--") => (false, path.clone()),
        [flag, path] if flag == "--check" => (true, path.clone()),
        _ => {
            eprintln!("usage: zant-tsx-tags [--check] <path>");
            return ExitCode::from(2);
        }
    };
    let output = emit::source(&table::generate());
    if check {
        match std::fs::read_to_string(&path) {
            Ok(existing) if existing == output => ExitCode::SUCCESS,
            Ok(_) => outdated(&path),
            Err(error) => {
                eprintln!("zant-tsx-tags: {path}: {error}");
                ExitCode::from(1)
            }
        }
    } else if let Err(error) = std::fs::write(&path, output) {
        eprintln!("zant-tsx-tags: {path}: {error}");
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn outdated(path: &str) -> ExitCode {
    eprintln!(
        "zant-tsx-tags: {path} is out of date; run \
         `cargo run -p zant-tables --bin zant-tsx-tags -- {path}`"
    );
    ExitCode::from(1)
}
