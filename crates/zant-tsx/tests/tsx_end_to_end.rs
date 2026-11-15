//! End-to-end project tests: the fixture projects compiled through the project
//! API ([`compile::compile_project_root_with`], the entry point
//! `zant_build::tsx_app` uses).
//!
//! The fixtures are the projects under `crates/zant-tsx/tests/fixtures/`; the
//! hand-written Rust modules they import are `data.rs`, `thumbs.rs`,
//! `devices.rs` and `jobs.rs`. The assertions drive the whole pipeline: file
//! collection, parsing, cross-file resolution (components, types, stores and
//! hand-written Rust signatures), custom-hook expansion, assets, and code
//! generation of every module.

use std::path::{Path, PathBuf};

use zant_tsx::compile::{self, BuildOutput, ProjectOptions};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

/// Copies `<fixture>/**` into `<target>/tsx-e2e/<name>/src/` (keeping the
/// subdirectories, so relative imports resolve) plus `assets/` for asset
/// imports (`../assets/dot.png`).
fn project(fixture: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    let root = target_dir()
        .join("tsx-e2e")
        .join(format!("{fixture}-{unique}"));
    let _ = std::fs::remove_dir_all(&root);
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create project src");
    copy_tree(&fixtures_dir().join(fixture), &src);
    // assets are imported by media/App.tsx as `../assets/dot.png`
    let assets = fixtures_dir().join("assets");
    if assets.is_dir() {
        copy_all(&assets, &root.join("assets"));
    }
    root
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in
        std::fs::read_dir(from).unwrap_or_else(|error| panic!("read {}: {error}", from.display()))
    {
        let entry = entry.expect("directory entry");
        let name = entry.file_name();
        let file_type = entry.file_type().expect("file type");
        let destination = to.join(&name);
        if file_type.is_dir() {
            std::fs::create_dir_all(&destination).expect("create directory");
            copy_tree(&entry.path(), &destination);
        } else if is_source(&name.to_string_lossy()) {
            std::fs::copy(entry.path(), &destination).expect("copy source file");
        }
    }
}

/// Copies every file (asset bytes included), keeping subdirectories.
fn copy_all(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create directory");
    for entry in
        std::fs::read_dir(from).unwrap_or_else(|error| panic!("read {}: {error}", from.display()))
    {
        let entry = entry.expect("directory entry");
        let file_type = entry.file_type().expect("file type");
        let destination = to.join(entry.file_name());
        if file_type.is_dir() {
            std::fs::create_dir_all(&destination).expect("create directory");
            copy_all(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), &destination).expect("copy file");
        }
    }
}

fn is_source(name: &str) -> bool {
    name.ends_with(".tsx") || name.ends_with(".ts") || name.ends_with(".rs")
}

/// Compiles the fixture and writes the output into the project's `out/`.
fn build(fixture: &str) -> (PathBuf, BuildOutput) {
    let root = project(fixture);
    let out = root.join("out");
    let options = ProjectOptions {
        root: root.to_string_lossy().into_owned(),
        out: out.to_string_lossy().into_owned(),
        src: root.join("src").to_string_lossy().into_owned(),
        native: root.join("native").to_string_lossy().into_owned(),
        main: None,
        app_config: None,
        dep: None,
    };
    let output = match compile::compile_project_root_with(&options, None) {
        Ok(output) => output,
        Err(error) => panic!("{fixture} does not compile: {error}"),
    };
    output.write(&options.out, None).expect("write output");
    (root, output)
}

fn module<'a>(output: &'a BuildOutput, name: &str) -> &'a str {
    output
        .files
        .iter()
        .find(|file| file.name == name)
        .unwrap_or_else(|| {
            panic!(
                "module `{name}` is missing; generated: {:?}",
                output
                    .files
                    .iter()
                    .map(|file| file.name.as_str())
                    .collect::<Vec<_>>()
            )
        })
        .source
        .as_str()
}

/// Whitespace-insensitive containment: `prettyplease` prints the tokens of a
/// macro invocation (`nodes![…]`) without re-formatting them, so the same
/// expression may read `crate ::row` there and `crate::row` elsewhere.
#[track_caller]
fn contains(source: &str, expected: &str) {
    let squeeze =
        |text: &str| -> String { text.chars().filter(|ch| !ch.is_whitespace()).collect() };
    let squeezed_source = squeeze(source);
    let squeezed_expected = squeeze(expected);
    assert!(
        squeezed_source.contains(&squeezed_expected),
        "missing `{expected}` in:\n{source}"
    );
}

/// Every generated module must be valid Rust (the project entry point also
/// formats it, but the test makes the contract explicit).
fn assert_modules_parse(output: &BuildOutput) {
    for file in &output.files {
        syn::parse_file(&file.source).unwrap_or_else(|error| {
            panic!("{} does not parse: {error}\n{}", file.name, file.source)
        });
    }
}

#[test]
fn custom_hooks_are_inlined_into_their_callers() {
    let (_, output) = build("hooks");
    assert_modules_parse(&output);
    let app = module(&output, "App.rs");
    // `useCounter(2)` / `useToggle()` are expanded into App's state fields and
    // handler methods; the hook modules themselves compile to empty modules.
    contains(app, "pub struct App {");
    contains(app, "impl ui::App for App");
    contains(app, "fn _on_click");
    contains(app, "useCounter_count");
    contains(app, "crate::config::step()");
    // the child component from the same file keeps its own local-state slot
    contains(app, "ui::local_state::local_state_hot::<");
    let counter = module(&output, "useCounter.rs");
    assert!(
        !counter.contains("fn useCounter"),
        "a custom hook is inlined into its callers:\n{counter}"
    );
}

#[test]
fn cross_file_components_and_rust_types_resolve() {
    let (_, output) = build("crossfile");
    assert_modules_parse(&output);
    let app = module(&output, "App.rs");
    // the components live in other modules (`use crate::row::Row;`)
    contains(app, "crate::row::Row");
    contains(app, "crate::rows::Rows");
    // the Rust module's enum/struct come through `import type { … }`
    let row = module(&output, "Row.rs");
    contains(row, "crate::data::Data");
    contains(row, "crate::data::Kind");
    // and the Rust functions are called with their real signatures
    contains(app, "crate::data::sample()");
    contains(row, "crate::data::");
}

#[test]
fn rust_type_fixtures_use_structs_enums_and_background_wakes() {
    let (_, output) = build("rust_types");
    assert_modules_parse(&output);
    let app = module(&output, "App.rs");
    contains(app, "crate::devices::Peer");
    contains(app, "crate::devices::peers()");
    contains(app, "crate::devices::firstOnline");
    contains(app, "crate::devices::label");
    let worker = module(&output, "Worker.rs");
    contains(worker, "crate::jobs::start(");
    contains(worker, "crate::jobs::take()");
    // `useWake` becomes an `App::on_wake` hook
    contains(worker, "fn _root_on_wake(&mut self)");
    contains(worker, "fn on_wake(&mut self)");
}

#[test]
fn media_assets_and_in_memory_images_compile() {
    let (_, output) = build("media");
    assert_modules_parse(&output);
    let app = module(&output, "App.rs");
    // the imported PNG becomes a `&'static [u8]` asset, copied by `write`
    assert_eq!(1, output.assets.len(), "one asset import");
    contains(app, "include_bytes!(\"_assets/");
    contains(app, "crate::thumbs::thumbnail(");
    contains(app, "crate::thumbs::pngWidth(");
    // the hotkey recorder maps its shortcut callbacks
    contains(app, "tsx::desktop::shortcut_action(");
}

#[test]
fn a_project_build_is_deterministic() {
    let (_, first) = build("hooks");
    let (_, second) = build("hooks");
    assert_eq!(
        first.files.len(),
        second.files.len(),
        "same number of modules"
    );
    for (a, b) in first.files.iter().zip(&second.files) {
        assert_eq!(a.name, b.name, "same module order");
        assert_eq!(a.source, b.source, "same generated source for {}", a.name);
    }
}
