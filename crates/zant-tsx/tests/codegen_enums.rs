//! TypeScript `enum` (`enum_parser`, `codegen::enums`): a type plus members
//! inlined where they are read, as tsc does for `const enum`.

use zant_tsx::compile::{self, SourceFile};

fn squash(s: &str) -> String {
    s.split_whitespace().collect()
}

#[test]
fn string_and_numeric_enums_inline_their_members() {
    let source = r#"import { View, Text, useState } from "zant";
enum Mode { Light = "light", Dark = "dark" }
enum Level { Low, Mid = 5, High }
function describe(m: Mode): string {
  switch (m) {
    case Mode.Light: return "L";
    case Mode.Dark: return "D";
  }
}
export default function App() {
  const [mode, setMode] = useState<Mode>(Mode.Light);
  return <Text>{`${mode} ${Mode.Dark} ${Level.High}` + describe(mode)}</Text>;
}
"#;
    let rust = squash(&compile::compile(source, "App.tsx").unwrap());
    for needle in [
        "pub enum Mode",
        "Mode::Dark",
        "(6.0f64)",
        // an exhaustive switch needs no default
        "unreachable!(\"every member is handled\")",
    ] {
        assert!(
            rust.contains(&squash(needle)),
            "missing `{needle}` in {rust}"
        );
    }
}

#[test]
fn enums_are_imported_from_other_files() {
    let files = [
        SourceFile::tsx(
            "src/kinds.ts",
            "export enum Kind { Note = \"note\", Task = \"task\" }\n",
        ),
        SourceFile::tsx(
            "src/App.tsx",
            "import { View, Text } from \"zant\";\nimport { Kind } from \"./kinds\";\nexport default function App() { const k: Kind = Kind.Task; return <Text>{k}</Text>; }\n",
        ),
    ];
    let project = compile::compile_project(&files).unwrap();
    assert!(project.outputs.iter().any(|out| out.contains("Kind::Task")));
}

#[test]
fn unknown_members_list_the_members() {
    let source = "import { View } from \"zant\";\nenum Mode { Light = \"light\" }\nexport default function App() { const m = Mode.Dim; return <View/>; }\n";
    let diag = compile::compile(source, "App.tsx").unwrap_err();
    assert_eq!(diag.msg, "`Mode` has no member `Dim`; members: Light");
}
