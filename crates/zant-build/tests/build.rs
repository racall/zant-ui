//! End-to-end tests of the build-script helper: `tsx_app_in` on the fixture app
//! (`tests/fixtures/tsx-app`, the same layout as `scaffold/template-ts`).
//!
//! These run the real TSX pipeline (no stubs), then check what ends up in
//! `OUT_DIR`: the generated modules, the crate-root `main.rs` declarations,
//! the asset copies, `icons.rs` and the dep file. Everything is parsed with
//! `syn`, so a code-generation shape that rustc would reject fails here too.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use zant_build::{BuildReport, Error};

/// The fixture app.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsx-app")
}

/// A fresh, empty directory under the system temp dir.
fn temp_dir(prefix: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let index = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "zant-build-test-{prefix}-{}-{index}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

/// Copies a directory tree (the fixture) so a test can modify it.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("dirs");
    for entry in std::fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy");
        }
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn parse(source: &str, what: &str) -> syn::File {
    syn::parse_file(source)
        .unwrap_or_else(|error| panic!("{what} does not parse: {error}\n{source}"))
}

/// The one asset copy in `<out>/_assets/` for `name`.
fn asset_copy(out: &Path, name: &str) -> PathBuf {
    let directory = out.join("_assets");
    for entry in std::fs::read_dir(&directory).expect("_assets") {
        let path = entry.expect("entry").path();
        if path
            .file_name()
            .and_then(|file| file.to_str())
            .is_some_and(|file| file.ends_with(name))
        {
            return path;
        }
    }
    panic!(
        "no _assets copy ending with {name} in {}",
        directory.display()
    );
}

// ── the fixture app compiles into OUT_DIR ──────────────────────────────

#[test]
fn tsx_app_in_writes_the_generated_project_into_out_dir() {
    let root = fixture_root();
    let out = temp_dir("project");
    let report: BuildReport = zant_build::tsx_app_in(&root, &out).expect("compile the fixture");

    // The entry: crate-root module declarations, no inner attributes (it is
    // `include!`d from src/main.rs), parsing as Rust.
    let entry = read(&out.join("main.rs"));
    assert!(
        entry.contains("#[path = \"App.rs\"]\npub mod app;"),
        "{entry}"
    );
    assert!(
        entry.contains("#[path = \"Greeting.rs\"]\npub mod greeting;"),
        "{entry}"
    );
    assert!(!entry.contains("#!["), "{entry}");
    parse(&entry, "OUT_DIR/main.rs");

    // One module per .tsx file, each a valid Rust file with the header the
    // compiler adds (inner attributes are fine as a `#[path]` module).
    let app = read(&out.join("App.rs"));
    assert!(app.contains("pub struct App"), "{app}");
    assert!(app.contains("impl ui::App for App"), "{app}");
    let greeting = read(&out.join("Greeting.rs"));
    assert!(greeting.contains("pub struct Greeting"), "{greeting}");
    assert!(greeting.contains("pub struct GreetingProps"), "{greeting}");
    for output in &report.outputs {
        let source = read(output);
        parse(&source, &output.display().to_string());
    }

    // The imported asset is copied next to the modules and referenced
    // relative to them.
    assert!(app.contains("include_bytes!(\"_assets/"), "{app}");
    let copy = asset_copy(&out, "-statusbar.png");
    assert_eq!(
        std::fs::read(copy).expect("copy"),
        std::fs::read(root.join("assets/icons/statusbar.png")).expect("source")
    );
    assert!(
        report
            .assets
            .iter()
            .any(|asset| asset.ends_with("statusbar.png"))
    );

    // The dep file lists every source and the asset (`zant-tsx --dep` format).
    let dep = read(&report.dep_file);
    assert!(dep.ends_with('\n'), "{dep}");
    for expected in [
        "src/App.tsx",
        "src/components/Greeting.tsx",
        "statusbar.png",
    ] {
        assert!(dep.contains(expected), "{dep}");
    }

    // icons.rs: `icon_<name>`, `-` → `_`, relative include_bytes paths.
    let icons = read(&report.icons_module);
    parse(&icons, "OUT_DIR/icons.rs");
    assert!(
        icons.contains(
            "pub const icon_tray_dark: &'static [u8] = include_bytes!(\"_icons/tray-dark.png\");"
        ),
        "{icons}"
    );
    assert!(
        icons.contains("(\"tray-dark\", icon_tray_dark),"),
        "{icons}"
    );
    assert!(out.join("_icons/tray-dark.png").is_file());

    // app.config.json: parsed, kept in the rerun set, exposed to the caller.
    let config = report.config.as_ref().expect("app.config.json");
    assert_eq!(config.title, "Zant Build Fixture");
    assert_eq!(config.bundle_id, "dev.zant.build-fixture");
    assert!(config.single_instance);
    assert_eq!(config.window.width, 360.0);
    assert_eq!(config.window.min_height, 160.0);
    assert!(!config.window.resizable);
    assert!(config.window.always_on_top);
    assert!(config.window.movable_by_background);
    assert!(!config.window.center);
    assert_eq!(config.window.background, 0xff10_1010);

    // Every input is in the rerun set, absolute so cargo can watch it.
    for expected in [
        "src/App.tsx",
        "src/components/Greeting.tsx",
        "src/main.rs",
        "src/app_config.rs",
    ] {
        assert!(
            report
                .sources
                .iter()
                .any(|source| source.ends_with(expected)),
            "{expected} in {:?}",
            report.sources
        );
    }

    let _ = std::fs::remove_dir_all(&out);
}

// ── the failure modes are actionable ───────────────────────────────────

#[test]
fn a_missing_root_component_names_src_app_tsx() {
    let root = temp_dir("missing-root");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(root.join("src/format.rs"), "").expect("format.rs");
    let out = temp_dir("missing-root-out");
    let error = zant_build::tsx_app_in(&root, &out).expect_err("no App.tsx");
    let message = error.to_string();
    assert!(matches!(error, Error::NoRootComponent { .. }), "{message}");
    assert!(message.contains("src/App.tsx"), "{message}");
    assert!(message.contains("export default function App"), "{message}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_project_without_a_src_directory_is_reported() {
    let root = temp_dir("no-src");
    let out = temp_dir("no-src-out");
    let message = zant_build::tsx_app_in(&root, &out)
        .expect_err("no src")
        .to_string();
    assert!(message.contains("has no `src/` directory"), "{message}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_root_must_have_a_default_export() {
    let root = temp_dir("no-default");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(
        root.join("src/App.tsx"),
        "export function App() { return <View/>; }",
    )
    .expect("App.tsx");
    let out = temp_dir("no-default-out");
    let message = zant_build::tsx_app_in(&root, &out)
        .expect_err("no default export")
        .to_string();
    assert!(
        message.contains("has no default exported component"),
        "{message}"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn codegen_errors_are_rendered_with_the_source_location() {
    let root = temp_dir("codegen-error");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(
        root.join("src/App.tsx"),
        "export default function App() {\n  return <NotATag/>;\n}\n",
    )
    .expect("App.tsx");
    let out = temp_dir("codegen-error-out");
    let error = zant_build::tsx_app_in(&root, &out).expect_err("unknown tag");
    let message = error.to_string();
    assert!(matches!(error, Error::Compile(_)), "{message}");
    assert!(message.contains("src/App.tsx:2:"), "{message}");
    assert!(message.contains("error: unknown tag"), "{message}");
    // the rendered diagnostic has the caret line
    assert!(message.contains("return <NotATag/>;"), "{message}");
    assert!(message.contains('^'), "{message}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_broken_app_config_json_is_an_error() {
    let root = temp_dir("bad-config");
    copy_tree(&fixture_root(), &root);
    std::fs::write(root.join("app.config.json"), "{ not json").expect("config");
    let out = temp_dir("bad-config-out");
    let error = zant_build::tsx_app_in(&root, &out).expect_err("bad config");
    let message = error.to_string();
    assert!(message.contains("app.config.json"), "{message}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

// ── rebuilding ─────────────────────────────────────────────────────────

#[test]
fn a_second_run_overwrites_the_previous_output() {
    let root = fixture_root();
    let out = temp_dir("rebuild");
    zant_build::tsx_app_in(&root, &out).expect("first build");
    let first = read(&out.join("main.rs"));
    let report = zant_build::tsx_app_in(&root, &out).expect("second build");
    assert_eq!(read(&out.join("main.rs")), first);
    assert!(report.outputs.iter().all(|output| output.is_file()));
    assert_eq!(
        std::fs::read_dir(out.join("_assets"))
            .expect("_assets")
            .count(),
        1,
        "the asset copy is not duplicated"
    );
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_windows_icon_of_the_fixture_is_a_valid_ico() {
    let ico = zant_build::windows_icon(&fixture_root())
        .expect("ico")
        .expect("the fixture has app-windows.png");
    assert_eq!(&ico[0..4], &[0, 0, 1, 0]);
    assert_eq!(u16::from_le_bytes([ico[4], ico[5]]), 8);
}

// ── the generated output is path-independent ──────────────────────────

#[test]
fn the_generated_output_does_not_depend_on_the_checkout_location() {
    let root = fixture_root();
    let first = temp_dir("deterministic-a");
    let second = temp_dir("deterministic-b");
    let a = zant_build::tsx_app_in(&root, &first).expect("first");
    let b = zant_build::tsx_app_in(&root, &second).expect("second");
    assert_eq!(read(&a.dep_file), read(&b.dep_file));
    // The dep file names the sources app-relative, like `zant-tsx --dep`.
    assert!(read(&a.dep_file).contains("src/App.tsx"));
    // The asset copies (and therefore the generated `include_bytes!` names)
    // only hash the path inside the project, not the temp directory.
    let names = |out: &Path| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(out.join("_assets"))
            .expect("_assets")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    };
    assert_eq!(names(&first), names(&second));
    assert!(names(&first).iter().all(|name| !name.contains("tmp")));
    let _ = std::fs::remove_dir_all(&first);
    let _ = std::fs::remove_dir_all(&second);
}

// ── the Cargo directives the build script prints ───────────────────────

#[test]
fn the_directives_cover_the_rerun_set() {
    let root = fixture_root();
    let out = temp_dir("directives");
    let report = zant_build::tsx_app_in(&root, &out).expect("compile");
    let directives = zant_build::cargo_directives(&report, &root);
    let has = |expected: &str| directives.iter().any(|line| line.contains(expected));

    // The build script itself, then the project directories (new files).
    assert!(has("cargo:rerun-if-changed=build.rs"), "{directives:?}");
    for directory in ["src", "native", "assets"] {
        assert!(
            has(&format!(
                "cargo:rerun-if-changed={}",
                root.join(directory).display()
            )),
            "{directory}: {directives:?}"
        );
    }
    // Every compiler input, the config and every icon.
    assert!(has("cargo:rerun-if-changed=src/App.tsx"), "{directives:?}");
    assert!(
        has("cargo:rerun-if-changed=src/components/Greeting.tsx"),
        "{directives:?}"
    );
    assert!(has("cargo:rerun-if-changed=src/main.rs"), "{directives:?}");
    assert!(
        has(&format!(
            "cargo:rerun-if-changed={}",
            root.join("app.config.json").display()
        )),
        "{directives:?}"
    );
    assert!(has("tray-dark.png"), "{directives:?}");
    // No fallback warnings for a complete fixture app.
    assert!(
        !directives
            .iter()
            .any(|line| line.starts_with("cargo:warning=")),
        "{directives:?}"
    );
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_project_without_the_generated_config_warns_about_zant_sync() {
    let root = temp_dir("directives-config");
    copy_tree(&fixture_root(), &root);
    std::fs::remove_file(root.join("src/app_config.rs")).expect("remove app_config.rs");
    let out = temp_dir("directives-config-out");
    let report = zant_build::tsx_app_in(&root, &out).expect("compile");
    let directives = zant_build::cargo_directives(&report, &root);
    assert!(
        directives
            .iter()
            .any(|line| line.contains("src/app_config.rs is missing") && line.contains("zant sync")),
        "{directives:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

// ── native module specs ────────────────────────────────────────────────

/// The fixture app plus one `native/*.ts` spec, in a temp directory.
fn app_with_native_spec(prefix: &str) -> PathBuf {
    let root = temp_dir(prefix);
    copy_tree(&fixture_root(), &root);
    std::fs::write(
        root.join("native/clip.ts"),
        "export interface Clip { row: number; preview: string }\n\
         export declare function list(query: string): Clip[];\n",
    )
    .expect("native/clip.ts");
    root
}

#[test]
fn native_specs_become_the_apps_generated_spec_module() {
    let root = app_with_native_spec("native-spec");
    let out = temp_dir("native-spec-out");
    let report = zant_build::tsx_app_in(&root, &out).expect("compile the app");

    // One combined `native/spec.rs` is written into the app (next to the
    // handwritten `native/<name>.rs` implementations), not into `$OUT_DIR`:
    // the app declares `pub mod spec;` in `native/lib.rs`.
    let spec_path = report.spec_module.clone().expect("the spec module path");
    assert_eq!(root.join("native/spec.rs"), spec_path);
    let spec = read(&spec_path);
    assert!(spec.contains("pub mod clip"), "{spec}");
    assert!(spec.contains("pub struct Clip"), "{spec}");
    assert!(spec.contains("pub trait ClipSpec"), "{spec}");
    assert!(
        spec.contains("fn list(query: zant::Str) -> tsx::List<Clip>;"),
        "{spec}"
    );
    assert!(spec.contains("use zant::tsx;"), "{spec}");
    parse(&spec, "native/spec.rs");

    // The module list is the .tsx modules only: `native/spec.rs` is declared
    // by `native/lib.rs`, and no per-spec generated file remains in OUT_DIR.
    let entry = read(&out.join("main.rs"));
    assert!(!entry.contains("spec"), "{entry}");
    assert!(
        !report.outputs.iter().any(|output| output
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("_spec_"))),
        "{:?}",
        report.outputs
    );

    // The spec is a compiler input (rerun and diagnostics) and the generated
    // file is in the rerun set so deleting it rebuilds the app.
    assert!(
        report
            .sources
            .iter()
            .any(|source| source == Path::new("native/clip.ts")),
        "{:?}",
        report.sources
    );
    let directives = zant_build::cargo_directives(&report, &root);
    assert!(
        directives
            .iter()
            .any(|line| line.contains(&spec_path.display().to_string())),
        "{directives:?}"
    );

    // The write is idempotent in content: a stale file is regenerated, and a
    // second build reproduces the same bytes.
    std::fs::write(&spec_path, "stale").expect("stale spec");
    zant_build::tsx_app_in(&root, &out).expect("second build");
    assert_eq!(read(&spec_path), spec);

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn an_app_without_native_specs_gets_no_spec_module() {
    let root = temp_dir("no-native-spec");
    copy_tree(&fixture_root(), &root);
    let out = temp_dir("no-native-spec-out");
    let report = zant_build::tsx_app_in(&root, &out).expect("compile the app");
    assert!(report.spec_module.is_none(), "{:?}", report.spec_module);
    assert!(!root.join("native/spec.rs").exists());
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}
