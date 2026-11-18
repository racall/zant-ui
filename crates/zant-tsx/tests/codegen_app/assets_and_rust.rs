//! Asset imports and hand-written Rust files.

use super::*;

#[test]
fn an_asset_import_embeds_the_bytes() {
    let files = [SourceFile::tsx(
        "src/App.tsx",
        "import logo from \"./assets/logo.png\";\n\
         import { assetImage } from \"zant\";\n\
         export default function App() { return <Image image={assetImage(logo)} />; }\n",
    )];
    let project = compile::compile_project_with(&files, Some(install_stubs));
    // `assetImage` needs the real emitter's `Image` type; only the constant is checked here
    match project {
        Ok(project) => {
            contains(&project.outputs[0], "const logo: &'static [u8]");
            contains(&project.outputs[0], "include_bytes!(\"_assets/");
            assert_eq!(1, project.assets.len());
            assert!(project.assets[0].key.ends_with("-logo.png"));
        }
        Err(error) => {
            assert!(
                error.diag.msg.contains("Image"),
                "unexpected error: {}",
                error.diag.msg
            );
        }
    }
}

#[test]
fn asset_imports_without_a_project_build_are_a_diagnostic() {
    let error = compile::compile(
        "import logo from \"./assets/logo.png\";\nexport default function App() { return <View/>; }",
        "App.tsx",
    )
    .unwrap_err();
    assert!(
        error.msg.contains("asset imports need a project build"),
        "{}",
        error.msg
    );
}

#[test]
fn asset_imports_are_listed_in_the_dep_file() {
    let files = [SourceFile::tsx(
        "src/App.tsx",
        "import logo from \"./assets/logo.png\";\nexport default function App() { return <View/>; }",
    )];
    let project = compile::compile_project_with(&files, Some(install_stubs)).expect("compiles");
    assert_eq!(1, project.assets.len());
    let dep = zant_tsx::assets::dep_file("out", &["src/App.tsx".to_owned()], &project.assets);
    assert!(dep.starts_with("out: \\\n  src/App.tsx \\\n"), "{dep}");
    assert!(dep.contains("logo.png\n"), "{dep}");
}

// ── hand-written Rust files (`rust_imports`) ─────────────────────────

pub(super) const RUST_SOURCE: &str = "\
// Keep this file byte for byte.
pub fn plural(n: usize, word: &str) -> zant::Str { word.into() }
pub fn clamp(n: f32) -> f32 { n }
pub fn record(n: usize) {}
pub fn values(xs: Vec<Option<i32>>) -> Vec<Option<i32>> { xs }
pub fn words(xs: Vec<zant::Str>) -> Vec<zant::Str> { xs }
pub fn maybe(n: Option<usize>) -> Option<usize> { n }
pub fn bad(x: Foo) {}
fn hidden() {}
pub const value: u8 = 1;
";

#[test]
fn rust_functions_are_imported_with_their_signatures() {
    let files = [
        SourceFile::rust("src/format.rs", RUST_SOURCE),
        SourceFile::tsx(
            "src/App.tsx",
            "import { plural as label, clamp } from \"./format.rs\";\n\
             import { record } from \"./format.rs\";\n\
             export default function App() {\n\
             const title = label(3, \"note\");\n\
             function save() { record(1); }\n\
             return <View><Text>{title}</Text><Button width={clamp(24)} onClick={save}>Save</Button></View>;\n\
             }\n",
        ),
    ];
    let project = compile::compile_project_with(&files, Some(install_stubs)).expect("compiles");
    // the Rust file is passed through verbatim
    assert_eq!(RUST_SOURCE, project.outputs[0]);
    contains(&project.outputs[1], "crate::format::plural(");
    contains(&project.outputs[1], "crate::format::record(");
}

#[test]
fn rust_import_diagnostics_point_at_imported_names() {
    let app = " export default function App() { return <View/>; }";
    let cases = [
        (
            "import { plurla } from \"./format.rs\";",
            "`plurla` is not a pub fn of `./format.rs`; did you mean `plural`?",
        ),
        (
            "import { hidden } from \"./format.rs\";",
            "`hidden` is not a pub fn of `./format.rs`",
        ),
        (
            "import { value } from \"./format.rs\";",
            "`value` is not a pub fn of `./format.rs`",
        ),
        (
            "import type { plural } from \"./format.rs\";",
            "is not a pub struct or pub enum",
        ),
    ];
    for (import, message) in cases {
        let files = [
            SourceFile::tsx("src/App.tsx", &format!("{import}{app}")),
            SourceFile::rust("src/format.rs", RUST_SOURCE),
        ];
        let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
        assert_eq!(0, error.file);
        assert!(
            error.diag.msg.contains(message),
            "expected `{message}`, got `{}`",
            error.diag.msg
        );
    }
}

#[test]
fn rust_import_calls_check_argument_counts_and_types() {
    let prefix =
        "import { plural } from \"./format.rs\"; export default function App() { return <Text>{";
    let suffix = "}</Text>; }";
    let cases = [
        ("plural(1)", "`plural` expects 2 arguments"),
        ("plural(true, \"note\")", "cannot convert bool to usize"),
        (
            "plural<usize>(1, \"note\")",
            "do not take TSX type arguments",
        ),
    ];
    for (call, message) in cases {
        let files = [
            SourceFile::rust("src/format.rs", RUST_SOURCE),
            SourceFile::tsx("src/App.tsx", &format!("{prefix}{call}{suffix}")),
        ];
        let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
        assert!(
            error.diag.msg.contains(message),
            "expected `{message}`, got `{}`",
            error.diag.msg
        );
    }
}

#[test]
fn rust_imports_require_project_mode() {
    let error = compile::compile(
        "import { plural } from \"./format.rs\"; export default function App() { return <View/>; }",
        "App.tsx",
    )
    .unwrap_err();
    assert_eq!(
        "Rust imports need a project (zant-tsx --out / zant_build::tsx_app)",
        error.msg
    );
}
