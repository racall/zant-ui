//! Golden snapshots, compiled project roots and reported diagnostics.

use super::*;

#[test]
fn untitled_windows_keep_the_create_window_name_as_the_frame_key() {
    for options in ["", ", {}"] {
        let source = generated(&format!(
            r#"import {{ createWindow, View }} from "zant";
function Panel() {{ return <View />; }}
export const panel = createWindow(Panel{options});
export default function App() {{ return <View />; }}"#
        ));
        contains(&source, "name: Some(\"panel\".into())");
    }
}

// ── golden snapshots ─────────────────────────────────────────────────

pub(super) fn snapshot(name: &str, source: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.rs"));
    if std::env::var("ZANT_TSX_WRITE_SNAPSHOTS").is_ok() {
        std::fs::create_dir_all(path.parent().expect("directory")).expect("snapshot directory");
        std::fs::write(&path, source).expect("write snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing snapshot {} ({error}); regenerate with ZANT_TSX_WRITE_SNAPSHOTS=1",
            path.display()
        )
    });
    if expected != source {
        let first = expected
            .lines()
            .zip(source.lines())
            .position(|(old, new)| old != new)
            .map(|line| line + 1)
            .unwrap_or(0);
        panic!("snapshot `{name}` differs at line {first}");
    }
}

pub(super) const COUNTER: &str = r#"
import { useState, View, Row, Text, Button, Input } from "zant";
import type { u32 } from "zant";

export default function Counter() {
  const [count, setCount] = useState<u32>(0);
  const [name, setName] = useState("App");
  const doubled = count * 2;
  return (
    <View padding={24} gap={14}>
      <Text key="title">{`Hello, ${name}!`}</Text>
      <Text key="count">Count: {count} (x2 = {doubled})</Text>
      <Input key="name" value={name} onChange={setName} placeholder="Your name" width={240} />
      <Row gap={8}>
        <Button key="inc" variant="primary" onClick={() => setCount(count + 1)}>+</Button>
        <Button key="reset" disabled={count === 0} onClick={() => setCount(0)}>Reset</Button>
      </Row>
    </View>
  );
}
"#;

#[test]
fn the_application_layer_matches_its_golden_snapshots() {
    snapshot("counter", &generated(COUNTER));
    snapshot("windows", &generated(WINDOWS));
    snapshot("app_hooks", &generated(APP_HOOKS));
    snapshot("platform", &generated(PLATFORM));
    snapshot("desktop", &generated(DESKTOP));
}

// ── the project-root build integration ───────────────────────────────

#[test]
fn compile_project_root_writes_modules_entry_assets_and_the_dep_file() {
    let directory = std::env::temp_dir().join(format!(
        "zant-tsx-wp56-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let src = directory.join("src");
    let assets = src.join("assets");
    std::fs::create_dir_all(&assets).expect("source directory");
    std::fs::write(
        src.join("App.tsx"),
        "import { useState, View, Text, Button } from \"zant\";\n\
         import logo from \"./assets/logo.png\";\n\
         export default function App() {\n\
         const [count, setCount] = useState(0);\n\
         return <View><Text>{`${count}`}</Text><Button onClick={() => setCount(count + 1)}>+</Button></View>;\n\
         }\n",
    )
    .expect("App.tsx");
    std::fs::write(
        src.join("Row.tsx"),
        "export function Row() { return <View/>; }\n",
    )
    .expect("Row.tsx");
    std::fs::write(assets.join("logo.png"), [137u8, 80, 78, 71]).expect("logo.png");
    std::fs::write(
        directory.join("app.config.json"),
        "{\n  \"title\": \"Clips\",\n  \"bundle_id\": \"io.x.clips\",\n  \"window\": {\n    \"style\": \"hidden-titlebar\",\n    \"width\": 680,\n    \"height\": 700,\n    \"background\": \"#112233\"\n  }\n}\n",
    )
    .expect("app.config.json");
    let out = directory.join("generated");
    let dep = directory.join("generated/app.d");
    let options = compile::ProjectOptions {
        root: directory.to_string_lossy().into_owned(),
        out: out.to_string_lossy().into_owned(),
        src: src.to_string_lossy().into_owned(),
        native: directory.join("native").to_string_lossy().into_owned(),
        main: Some("App".to_owned()),
        app_config: None,
        dep: Some(dep.to_string_lossy().into_owned()),
    };
    let build = compile::compile_project_root_with(&options, Some(install_stubs))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(2, build.files.len());
    assert!(build.files.iter().any(|file| file.name == "App.rs"));
    assert!(build.files.iter().any(|file| file.name == "Row.rs"));
    assert_eq!(1, build.assets.len());
    let entry = build.entry.as_deref().expect("entry");
    contains(entry, "#[path = \"App.rs\"] pub mod app;");
    contains(entry, "#[path = \"Row.rs\"] pub mod row;");
    contains(entry, "zant::tsx::app::run::<app::App>(");
    contains(entry, "title: \"Clips\".into()");
    contains(entry, "style: zant::desktop::WindowStyle::HiddenTitlebar");
    contains(entry, "background: 0xff112233");
    build
        .write(&options.out, options.dep.as_deref())
        .expect("writes the build");
    let main = std::fs::read_to_string(out.join("main.rs")).expect("main.rs");
    assert!(main.contains("pub mod app;"), "{main}");
    let app = std::fs::read_to_string(out.join("App.rs")).expect("App.rs");
    contains(&app, "include_bytes!(");
    assert!(
        out.join("_assets")
            .read_dir()
            .expect("assets directory")
            .next()
            .is_some(),
        "the asset was copied"
    );
    let dep = std::fs::read_to_string(out.join("app.d")).expect("dep file");
    assert!(dep.contains("logo.png"), "{dep}");
    std::fs::remove_dir_all(&directory).expect("cleanup");
}

/// `source` with every whitespace character removed (the generated code is
/// line-broken by `prettyplease`, so call-site assertions compare tokens).
pub(super) fn compact(source: &str) -> String {
    source.chars().filter(|ch| !ch.is_whitespace()).collect()
}

#[test]
fn a_types_only_spec_imports_tsx_for_hot_state_types() {
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "export default function App() { return <View/>; }",
        ),
        SourceFile::native_spec(
            "native/system.ts",
            "export type Os = \"macos\" | \"windows\";\n\
             export interface SystemInfo { os: Os; cpus: u32 }\n\
             export declare function info(): SystemInfo;",
        ),
    ];
    let build = compile::compile_project_with(&files, Some(install_stubs)).expect("spec compiles");
    let root = build.spec_root.as_deref().expect("the spec root");
    assert!(root.contains("pub trait SystemSpec"), "{root}");
    // Even scalar-only specs use TSX for their generated Hot implementations.
    assert!(root.contains("use zant::tsx;"), "{root}");
    assert!(root.contains("impl tsx::hot::Hot for SystemInfo"), "{root}");
}

#[test]
fn a_component_call_passes_the_props_struct_not_the_component() {
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "import { useState, View } from \"zant\";\n\
             import { Dashboard } from \"./components/Dashboard\";\n\
             import { Badge } from \"./Badge\";\n\
             export default function App() {\n\
             const [count, setCount] = useState(0);\n\
             return <View>\n\
             <Dashboard count={count} onIncrement={() => setCount(count + 1)} onReset={() => setCount(0)} />\n\
             <Badge />\n\
             </View>;\n\
             }\n",
        ),
        SourceFile::tsx(
            "src/components/Dashboard.tsx",
            "import { View } from \"zant\";\n\
             export interface DashboardProps { count: number; onIncrement: () => void; onReset: () => void }\n\
             export function Dashboard({ count, onIncrement, onReset }: DashboardProps) { return <View/>; }\n",
        ),
        SourceFile::tsx(
            "src/Badge.tsx",
            "import { View } from \"zant\";\n\
             export function Badge() { return <View/>; }\n",
        ),
    ];
    // Real JSX emitters (not the snapshot stubs): the call site is what this
    // test is about.
    let build = compile::compile_project_with(&files, None).expect("compiles");
    let app = &build.outputs[0];
    let compact_app = compact(app);
    // The props argument is the target module's props struct, not `Dashboard`.
    assert!(
        compact_app
            .contains("crate::dashboard::Dashboard::build(crate::dashboard::DashboardProps{"),
        "{app}"
    );
    // A component without props gets the unit `Props` default.
    assert!(
        compact_app.contains("crate::badge::Badge::build(Default::default())"),
        "{app}"
    );
    // A root callback for a child's `ui::Action` prop is wrapped for the root
    // state (`ui::action(|s: &mut App| …)`), like a ui event handler.
    assert!(compact_app.contains("onIncrement:ui::action("), "{app}");
    // The props struct's no-op default matches the action type.
    let dashboard = &build.outputs[1];
    assert!(
        dashboard.contains("onIncrement: ui::Action::new(|_cx| ())"),
        "{dashboard}"
    );
    assert!(
        dashboard.contains("onReset: ui::Action::new(|_cx| ())"),
        "{dashboard}"
    );
}

// ── `zant` features ───────────────────────────────────────────────────

/// With the app's `zant` features known (`zant_build` passes them on), a tag
/// or API of a feature that is off is a TSX error at its position.
#[test]
fn tags_of_a_feature_that_is_off_are_reported() {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            codegen::app_state::set_framework_features(None);
        }
    }
    let _reset = Reset;
    codegen::app_state::set_framework_features(Some(vec!["math".to_owned()]));
    let cases = [
        (
            "import { Markdown } from \"zant\";\nexport default function App() {\n  return ~<Markdown source=\"# a\" />;\n}\n",
            "<Markdown> needs the `markdown` feature of zant",
        ),
        (
            "import { PdfViewer } from \"zant\";\nexport default function App() {\n  return ~<PdfViewer src=\"a.pdf\" />;\n}\n",
            "<PdfViewer> needs the `pdf` feature of zant",
        ),
        (
            "import { useEffect, View } from \"zant\";\nexport default function App() {\n  useEffect(() => { ~fetch(\"https://example.com\"); }, []);\n  return <View />;\n}\n",
            "fetch needs the `fetch` feature of zant",
        ),
    ];
    // the real JSX emitter (`expect_diagnostic` stubs it out)
    for (marked, message) in cases {
        let at = marked.find('~').expect("`~` marker");
        let src = format!("{}{}", &marked[..at], &marked[at + 1..]);
        let diag = compile::compile(&src, "app.tsx").expect_err(message);
        assert!(diag.msg.contains(message), "{}\nsource: {src}", diag.msg);
        let expected = zant_tsx::error::position(&src, at as u32);
        let actual = zant_tsx::error::position(&src, diag.loc);
        assert_eq!(
            (expected.line, expected.column),
            (actual.line, actual.column),
            "{src}"
        );
    }
    // everything allowed when the features are unknown
    codegen::app_state::set_framework_features(None);
    let src = "import { Markdown } from \"zant\";\nexport default function App() {\n  return <Markdown source=\"# a\" />;\n}\n";
    assert!(compile::compile(src, "app.tsx").is_ok());
}

// ── several errors at once ───────────────────────────────────────────

/// Independent errors in two components of one file and in another file
/// are reported together, in source order, each in its own file.
#[test]
fn independent_errors_are_reported_together() {
    let first = "import { View, Text } from \"zant\";\n\
                 function A() { return <Text>{missingA}</Text>; }\n\
                 function B() { return <Text>{missingB}</Text>; }\n\
                 export default function App() { return <View><A/><B/></View>; }\n";
    let second = "import { Text } from \"zant\";\n\
                  export function C() { return <Text>{missingC}</Text>; }\n";
    let files = [
        SourceFile::tsx("App.tsx", first),
        SourceFile::tsx("C.tsx", second),
    ];
    let error = match compile::compile_project(&files) {
        Ok(_) => panic!("expected errors"),
        Err(error) => error,
    };
    let all: Vec<(usize, String)> = error
        .all()
        .map(|(file, diag)| (file, diag.msg.clone()))
        .collect();
    assert_eq!(all.len(), 3, "{all:?}");
    assert!(all[0].0 == 0 && all[0].1.contains("missingA"), "{all:?}");
    assert!(all[1].0 == 0 && all[1].1.contains("missingB"), "{all:?}");
    assert!(all[2].0 == 1 && all[2].1.contains("missingC"), "{all:?}");
    // the rendered report names each file and line
    let report = error.render(&files);
    assert!(report.contains("App.tsx:2:"), "{report}");
    assert!(report.contains("App.tsx:3:"), "{report}");
    assert!(report.contains("C.tsx:2:"), "{report}");
}
