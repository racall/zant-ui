//! Project output: shared stores, native specs, the entry and the repository corpus.

use super::*;

#[test]
fn a_store_exported_by_one_file_is_shared_by_the_files_importing_it() {
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "import { prefs as shared } from \"./store\";\n\
             export default function App() {\n\
             const dark = useStore(shared, (p) => p.dark);\n\
             return <Text>{`${dark}`}</Text>;\n\
             }\n",
        ),
        SourceFile::tsx(
            "src/store.tsx",
            "export interface Prefs { dark: boolean; name: string; }\n\
             export const prefs = createStore<Prefs>({ dark: false, name: \"\" });\n",
        ),
    ];
    let project = compile::compile_project_with(&files, Some(install_stubs)).expect("compiles");
    contains(&project.outputs[1], "pub fn prefs_read()");
    // the importing file reads through the declaring module's accessor
    contains(&project.outputs[0], "crate::store::prefs_read()");
}

#[test]
fn project_rejects_invalid_rust_sources() {
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "export default function App() { return <View/>; }",
        ),
        SourceFile::rust("src/bad.rs", "pub fn broken("),
    ];
    let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
    assert_eq!(1, error.file);
    assert!(!error.diag.msg.is_empty(), "a syntax error message");
}

#[test]
fn native_module_specs_compile_to_the_generated_spec_module() {
    // a spec compiles to `native/spec.rs`
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "export default function App() { return <View/>; }",
        ),
        SourceFile::native_spec(
            "native/clips.ts",
            "export interface Clip { row: number }\nexport declare function list(): Clip[];",
        ),
    ];
    let build = compile::compile_project_with(&files, Some(install_stubs)).expect("spec compiles");
    let spec = build
        .outputs
        .iter()
        .find(|source| source.contains("pub mod clips"))
        .expect("the spec module is generated");
    assert!(spec.contains("pub trait ClipsSpec"), "{spec}");
    assert!(spec.contains("fn list()"), "{spec}");
}

#[test]
fn project_output_names_collide_case_insensitively() {
    let files = [
        SourceFile::tsx(
            "src/Worker.tsx",
            "export default function Worker() { return <View/>; }",
        ),
        SourceFile::rust("src/worker.rs", "pub fn f() {}"),
    ];
    let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
    assert!(
        error.diag.msg.contains("project output collision"),
        "{}",
        error.diag.msg
    );
}

// ── the entry ────────────────────────────────────────────────────────

#[test]
fn the_entry_runs_the_default_export_with_the_project_config() {
    let files = [
        SourceFile::tsx(
            "App.tsx",
            "import { useClipboardChange } from \"zant\";\n\
             export default function Clips() { useClipboardChange(() => {}); return <View/>; }\n",
        ),
        SourceFile::tsx("Row.tsx", "export function Row() { return <View/>; }"),
    ];
    let source = entry::main_source(&files, "App", None).expect("entry");
    contains(&source, "#[path = \"App.rs\"] pub mod app;");
    contains(&source, "#[path = \"Row.rs\"] pub mod row;");
    contains(&source, "zant::tsx::app::run::<app::Clips>(");
    contains(&source, "watch_clipboard: true");
    assert_eq!(
        entry::Error::NoEntry,
        entry::main_source(&files, "Main", None).unwrap_err()
    );
}

#[test]
fn the_entry_carries_app_config_json() {
    let files = [SourceFile::tsx(
        "App.tsx",
        "export default function App() { return <View/>; }",
    )];
    let config = compile::ConfigSpec {
        title: "Clips".to_owned(),
        bundle_id: "io.x.clips".to_owned(),
        single_instance: true,
        window: compile::WindowConfig {
            style: "panel".to_owned(),
            width: 380.0,
            height: 480.0,
            background: 0xff00_0000,
            center: false,
            ..compile::WindowConfig::default()
        },
    };
    let source = entry::main_source(&files, "App", Some(&config)).expect("entry");
    contains(&source, "title: \"Clips\".into()");
    contains(&source, "bundle_id: \"io.x.clips\".into()");
    contains(&source, "single_instance: true");
    contains(&source, "style: zant::desktop::WindowStyle::Panel");
    contains(&source, "width: 380");
    contains(&source, "center: false");
}

// ── the repository corpus ────────────────────────────────────────────

/// Files of the repository corpus that the stub emitter of this suite cannot
/// compile (they need the real JSX / event emitters and are covered by
/// `tests/golden.rs`). Counted, not silently skipped.
pub(super) const KNOWN_MISSING: &[&str] = &[
    // JSX prop lowering / event parameter inference / children
    "arrays",
    "children",
    "components",
    "conditional",
    "counter",
    "defaults",
    "events",
    "hooks",
    "layout",
    "list",
    "list_scroll",
    "local_state",
    "named_handlers",
    "narrowing",
    "nesting",
    "react_compat",
    "spread",
    "store",
    "theme",
    "todos",
    "widgets",
    // TODO: on this path `testdata/strings.tsx` lowers a string method to
    // `label: { … }.clone()` (a labeled block as a method receiver, which is
    // not valid Rust); see `string_methods.rs` / `aggregate.rs`.
    "strings",
    // `setTimeout` / timer lowering lives in the real event emitter
    "timers",
    // `platform.tsx` lowers a JSX `padding={{ left, right }}` object prop in
    // its render body; that is the real JSX emitter's job (the golden
    // `testdata/platform.rs` compiles against `zant`), and this stub path
    // reports `object literal needs a type annotation`.
    "platform",
];

#[test]
fn the_repository_corpus_compiles_without_the_missing_modules() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/tsx");
    let mut failures: Vec<String> = Vec::new();
    let mut compiled = 0usize;
    let mut skipped = 0usize;
    let mut entries: Vec<_> = std::fs::read_dir(&root)
        .expect("testdata directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "tsx"))
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_owned();
        let source = std::fs::read_to_string(&path).expect("read testdata");
        if KNOWN_MISSING.contains(&name.as_str()) {
            skipped += 1;
            continue;
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compile::compile_project_with(
                &[SourceFile::tsx(&format!("src/{name}.tsx"), &source)],
                Some(install_stubs),
            )
        }));
        let result = match result {
            Ok(result) => result,
            Err(_) => {
                failures.push(format!("{name}: the generator panicked (invalid Rust)"));
                continue;
            }
        };
        match result {
            Ok(project) => {
                syn::parse_file(&project.outputs[0]).unwrap_or_else(|error| {
                    panic!("generated module for {name} does not parse: {error}")
                });
                compiled += 1;
            }
            Err(error) => {
                if !KNOWN_MISSING.contains(&name.as_str()) {
                    failures.push(format!(
                        "{name}: {} (at {})",
                        error.diag.msg, error.diag.loc
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "the corpus compiles except the known-missing modules:\n{}",
        failures.join("\n")
    );
    assert!(compiled > 0, "no corpus file compiled");
    assert_eq!(
        KNOWN_MISSING.len(),
        skipped,
        "the known-missing set matches the corpus"
    );
}
