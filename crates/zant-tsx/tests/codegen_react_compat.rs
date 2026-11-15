//! React compatibility codegen: `import NS from "react"`,
//! `import * as NS from "react"` and named React imports must lower exactly
//! like their `"zant"` counterparts (same generated Rust), fragments/root
//! shorthands emit one native root, and the import diagnostics stay the same.
//!
//! The assertions use the generated Rust shapes (`ui::local_state::<`,
//! `tsx::List<ui::Node>`, `zant::Str`).

use zant_tsx::compile;

fn compile_ok(source: &str, name: &str) -> String {
    compile::compile(source, name).unwrap_or_else(|error| panic!("{}: {}", name, error.msg))
}

fn diagnostic(source: &str) -> String {
    compile::compile(source, "react.tsx")
        .expect_err("the source must be rejected")
        .msg
}

#[test]
fn react_namespaces_lower_like_plain_hooks_and_fragments() {
    let body = "\
function Counter(): NS.ReactNode {\n\
  const [n, setN] = NS.useState(0);\n\
  const r = NS.useRef(0);\n\
  const doubled = NS.useMemo(() => n * 2, [n]);\n\
  NS.useEffect(() => { r.current = n; }, [n]);\n\
  return <NS.Fragment><Button onClick={() => setN(n + 1)}>{doubled}</Button></NS.Fragment>;\n\
}\n\
export default function App() { return <Counter/>; }\n";
    for prefix in [
        "import NS from \"react\";\n",
        "import * as NS from \"react\";\n",
    ] {
        let source = compile_ok(&format!("{prefix}{body}"), "react.tsx");
        // the stateful child becomes `ui::local_state`, like the "zant" hooks
        assert!(
            source.contains("ui::local_state::local_state_hot::<"),
            "{source}"
        );
        // the React namespace and Fragment leave no trace in the output
        assert!(!source.contains("NS"), "{source}");
        assert!(!source.contains("Fragment"), "{source}");
    }
}

#[test]
fn named_react_imports_share_the_zant_output() {
    let names = "{ useState, useEffect, useMemo, useRef, type ReactNode, ReactElement, Text, u32 }";
    let body = "export default function App(): ReactNode { const [n, setN] = useState<u32>(0); \
                return <Text>{n}</Text>; }";
    let react = compile_ok(
        &format!("import {names} from \"react\"; {body}"),
        "same.tsx",
    );
    let zant = compile_ok(&format!("import {names} from \"zant\"; {body}"), "same.tsx");
    assert_eq!(zant, react, "React imports lower to the same Rust");
}

#[test]
fn root_shorthands_and_react_fragments_emit_the_same_native_root() {
    let plain = compile_ok(
        "export default function App() { return <><Text>a</Text><Text>b</Text></>; }",
        "root.tsx",
    );
    let react = compile_ok(
        "import R from \"react\"; export default function App() { \
         return <R.Fragment><Text>a</Text><Text>b</Text></R.Fragment>; }",
        "root.tsx",
    );
    assert_eq!(plain, react, "the fragment shorthands emit the same root");
}

#[test]
fn children_typed_one_element_or_several_take_one_child_or_many() {
    let source = compile_ok(
        "interface GroupProps { title: string; children: JSX.Element | JSX.Element[] }\n\
         function Group(props: GroupProps) { return <Column>{props.children}</Column>; }\n\
         export default function App() {\n\
           return <View><Group title=\"a\"><Text>one</Text></Group>\
         <Group title=\"b\"><Text>1</Text><Text>2</Text></Group></View>;\n\
         }",
        "group.tsx",
    );
    assert!(
        source.contains("pub children: tsx::List<ui::Node>,"),
        "{source}"
    );
}

#[test]
fn react_import_diagnostics() {
    let cases = [
        (
            "export default function App() { const [n, setN] = React.useState(0); \
             return <Text>{n}</Text>; }",
            "destructuring is only supported for useState",
        ),
        (
            "export default function App() { return <React.Fragment/>; }",
            "unknown tag <React.Fragment>",
        ),
    ];
    for (source, expected) in cases {
        let message = diagnostic(source);
        assert!(
            message.contains(expected),
            "missing `{expected}` in `{message}` for:\n{source}"
        );
    }
}
