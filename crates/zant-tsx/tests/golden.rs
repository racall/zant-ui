//! WP-5.9 golden cases: the 31 Rust outputs of the TSX corpus, and
//! the first real proof that the generated code compiles against the runtime.
//!
//! - [`golden_outputs_match_the_compiler`] regenerates every `.rs` with
//!   [`compile::compile`] and compares it to the committed
//!   `testdata/<name>.rs` byte for byte (the Zig `golden_test.zig` contract;
//!   the files were generated once and human-reviewed against the design doc
//!   and the Zig golden, see `docs/rust-port/status.md`).
//! - [`generated_goldens_compile_against_the_runtime`] writes all goldens into
//!   one temporary, workspace-external crate whose `zant` dependency is this
//!   repository's `crates/zant`, and runs `cargo check`. All goldens go into a
//!   single compile (one crate, one `cargo check`), so the test stays fast.
//!   `desktop` is included: `Uint8Array` is `&'static [u8]` (the tray icon and
//!   imported assets are static slices), so `TrayOptions::icon` accepts the
//!   generated literal.
//!
//! One corpus file cannot be a golden yet; the test documents why instead of
//! silently skipping it:
//! - `entities`: `zant::Str` is UTF-8 and cannot hold an unpaired surrogate
//!   (`crates/zant-tsx/testdata/tsx/entities.tsx` has `&#xD800;`; the Zig
//!   compiler used
//!   WTF-8 bytes); replacing it with U+FFFD would change the data, so the
//!   compiler reports a diagnostic and the golden stays excluded.

use std::path::{Path, PathBuf};
use std::process::Command;

use zant_tsx::compile;

/// The 32 corpus names in the old Zig golden order; the generated ones are
/// below. The corpus moved from `tools/tsx/testdata/` at M8.
const CORPUS: &[&str] = &[
    "counter",
    "layout",
    "text",
    "events",
    "conditional",
    "components",
    "todos",
    "arrays",
    "children",
    "theme",
    "list",
    "local_state",
    "hooks",
    "strings",
    "store",
    "nesting",
    "widgets",
    "functions",
    "loops",
    "spread",
    "app_hooks",
    "builtins",
    "windows",
    "desktop",
    "react_compat",
    "entities",
    "platform",
    "named_handlers",
    "narrowing",
    "defaults",
    "timers",
    "list_scroll",
];

/// Corpus files that have no `.rs` golden: rejected by the Rust compiler for
/// the reason in the module doc (`entities`: lone surrogate in a `zant::Str`).
const NO_GOLDEN: &[&str] = &["entities"];

/// Goldens excluded from the cargo compile check (framework APIs missing).
const NOT_COMPILED: &[&str] = &[];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn corpus_dir() -> PathBuf {
    manifest_dir().join("testdata/tsx")
}

fn testdata_dir() -> PathBuf {
    manifest_dir().join("testdata")
}

fn read_corpus(name: &str) -> String {
    let path = corpus_dir().join(format!("{name}.tsx"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn the_corpus_and_the_goldens_are_the_32_zig_cases() {
    // the 32 inputs are exactly the Zig golden list (nothing silently dropped)
    for name in CORPUS {
        assert!(
            corpus_dir().join(format!("{name}.tsx")).is_file(),
            "missing corpus input `{name}.tsx`"
        );
    }
    let mut on_disk: Vec<String> = std::fs::read_dir(testdata_dir())
        .expect("testdata directory")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".rs").map(str::to_owned))
        .collect();
    on_disk.sort();
    let mut expected: Vec<String> = CORPUS
        .iter()
        .filter(|name| !NO_GOLDEN.contains(name))
        .map(|name| (*name).to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        expected, on_disk,
        "the golden set is the corpus minus NO_GOLDEN"
    );
}

#[test]
fn golden_outputs_match_the_compiler() {
    for name in CORPUS.iter().filter(|name| !NO_GOLDEN.contains(name)) {
        let source = read_corpus(name);
        let generated = compile::compile(&source, &format!("{name}.tsx")).unwrap_or_else(|diag| {
            panic!(
                "{name}.tsx does not compile: {} ({})",
                diag.msg,
                zant_tsx::error::render_diag(&format!("{name}.tsx"), &source, &diag)
            )
        });
        let path = testdata_dir().join(format!("{name}.rs"));
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        if expected != generated {
            // regeneration (like the codegen snapshots): only after a human
            // review of the diff — the goldens are the reviewed contract
            if std::env::var("ZANT_TSX_WRITE_GOLDENS").is_ok() {
                std::fs::write(&path, &generated).expect("write golden");
                continue;
            }
            let line = expected
                .lines()
                .zip(generated.lines())
                .position(|(old, new)| old != new)
                .map(|index| index + 1)
                .unwrap_or(0);
            panic!("golden `{name}.rs` differs at line {line}; regenerate it with the compiler");
        }
    }
}

#[test]
fn the_compiler_is_deterministic() {
    for name in ["counter", "hooks", "timers"] {
        let source = read_corpus(name);
        let first = compile::compile(&source, &format!("{name}.tsx")).expect("first");
        let second = compile::compile(&source, &format!("{name}.tsx")).expect("second");
        assert_eq!(first, second, "{name}.tsx is not deterministic");
    }
}

#[test]
fn entities_reports_its_documented_limitation() {
    // `entities`: no Rust `str` for an unpaired surrogate (`&#xD800;`); unlike
    // the Zig compiler (WTF-8 bytes) the Rust compiler reports a diagnostic.
    let entities = read_corpus("entities");
    let error = compile::compile(&entities, "entities.tsx").expect_err("entities is blocked");
    assert!(error.msg.contains("unpaired surrogate"), "{}", error.msg);
}

#[test]
fn generated_goldens_compile_against_the_runtime() {
    let crate_dir = temporary_crate_dir();
    let src_dir = crate_dir.join("src");
    let _ = std::fs::remove_dir_all(&src_dir);
    std::fs::create_dir_all(&src_dir).expect("create temp crate src");
    let crates_dir = manifest_dir().join("..");
    let zant_dir = crates_dir.join("zant");
    let cargo_toml = format!(
        "[package]\nname = \"zant-tsx-goldens\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nzant = {{ path = {:?} }}\n\n[workspace]\n",
        zant_dir.to_string_lossy()
    );
    std::fs::write(crate_dir.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    let mut modules = String::from("#![allow(dead_code)]\n");
    for name in CORPUS
        .iter()
        .filter(|name| !NO_GOLDEN.contains(name) && !NOT_COMPILED.contains(name))
    {
        let source = std::fs::read_to_string(testdata_dir().join(format!("{name}.rs")))
            .expect("read golden");
        std::fs::write(src_dir.join(format!("{name}.rs")), source).expect("write golden module");
        // one module file per golden: the inner `#![allow]` attributes of the
        // generated file are legal at the top of a module file
        modules.push_str(&format!("#[path = \"{name}.rs\"]\nmod golden_{name};\n"));
    }
    modules.push_str("fn main() {}\n");
    std::fs::write(src_dir.join("main.rs"), modules).expect("write main.rs");

    let output = run_cargo_check(&crate_dir);
    let (success, text) = output;
    assert!(
        success,
        "the generated goldens do not compile against zant:\n{text}"
    );
    assert!(
        !text.contains("error[E"),
        "the generated goldens do not compile against zant:\n{text}"
    );
}

/// One reused, workspace-external crate under the workspace `target/` so its
/// dependency artifacts are cached between test runs.
fn temporary_crate_dir() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir().join("../../target"));
    let dir = target.join("zant-tsx-goldens");
    std::fs::create_dir_all(&dir).expect("create temp crate directory");
    dir
}

fn run_cargo_check(crate_dir: &Path) -> (bool, String) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command
        .current_dir(crate_dir)
        .env("CARGO_TARGET_DIR", crate_dir.join("target"))
        .arg("check")
        .arg("--quiet")
        .arg("--message-format=short")
        // the workspace build already fetched the lockfile's dependencies; a
        // clean machine may still need the registry
        .arg("--offline");
    let output = command.output().expect("run cargo check");
    if output.status.success() {
        return (true, String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let mut retry = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()));
    retry
        .current_dir(crate_dir)
        .env("CARGO_TARGET_DIR", crate_dir.join("target"))
        .arg("check")
        .arg("--quiet")
        .arg("--message-format=short");
    match retry.output() {
        Ok(retry) => (
            retry.status.success(),
            String::from_utf8_lossy(&retry.stderr).into_owned(),
        ),
        Err(error) => (
            false,
            format!(
                "{}\n(and the retry without --offline failed: {error})",
                String::from_utf8_lossy(&output.stderr)
            ),
        ),
    }
}
