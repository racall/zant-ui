//! Projects: components, types and imports across files.

use super::*;

#[test]
fn a_project_resolves_components_and_types_across_files() {
    let files = [
        SourceFile::tsx("src/Row.tsx", PROJECT_TESTDATA),
        SourceFile::tsx(
            "src/App.tsx",
            "import { useState } from \"zant\";\n\
             import { Row, type Todo } from \"./Row\";\n\
             export default function App() {\n\
             const [todo, setTodo] = useState<Todo>({ id: 1, title: \"a\" });\n\
             const [picked, setPicked] = useState<u32>(0);\n\
             return <Row todo={todo} onPick={(id) => setPicked(id)} />;\n\
             }\n",
        ),
    ];
    let result = generated_in_project(&files, 1).expect("project compiles");
    // Row.rs declares its types and the component, App.rs imports them by path
    let row = generated_in_project(&files, 0).expect("row compiles");
    contains(&row, "pub struct Todo");
    contains(&row, "pub fn build");
    // the imported type resolves to the declaring module (`imports.named`)
    contains(&result, "crate::row::Todo");
}

#[test]
fn imports_name_exported_things_of_project_files() {
    let files = [
        SourceFile::tsx(
            "src/Row.tsx",
            "function Hidden(p: { a: string }) { return <View/>; }",
        ),
        SourceFile::tsx(
            "src/App.tsx",
            "import { Hidden } from \"./Row\"; export default function App() { return <View/>; }",
        ),
    ];
    let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
    assert_eq!(1, error.file);
    assert_eq!("`Hidden` is not exported by `./Row`", error.diag.msg);
}

#[test]
fn an_import_of_a_missing_project_file_is_a_diagnostic() {
    let files = [SourceFile::tsx(
        "src/App.tsx",
        "import { Row } from \"./Missing\"; export default function App() { return <View/>; }",
    )];
    let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
    assert!(
        error
            .diag
            .msg
            .contains("cannot find `./Missing` among the project's .tsx files"),
        "{}",
        error.diag.msg
    );
}

// ── windows ──────────────────────────────────────────────────────────
