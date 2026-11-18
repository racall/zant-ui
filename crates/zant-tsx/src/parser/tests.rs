//! Unit tests.

use super::*;
use crate::ast::{self, Body, Child, Expr, ExprData, Function, Stmt, StmtData};

fn block(f: &Function) -> &[Box<Stmt>] {
    match &f.body {
        Body::Block(stmts) => stmts,
        Body::Expr(_) => panic!("expected a block body"),
    }
}

fn ret_expr(s: &Stmt) -> &Expr {
    match &s.data {
        StmtData::Ret(Some(e)) => e,
        _ => panic!("expected a return with a value"),
    }
}

fn text(value: &[u8]) -> &str {
    std::str::from_utf8(value).expect("text is UTF-8")
}

#[test]
fn anonymous_default_components_use_app_without_a_file_name() {
    let source = r#"import { View } from "zant";
export default function () { return <View />; }"#;
    let m = parse_ok(source);
    assert_eq!(m.functions.len(), 1);
    let f = &m.functions[0];
    assert_eq!("App", text(&f.func.name));
    assert!(f.default && f.exported);
    assert!(matches!(
        &ret_expr(&block(&f.func)[0]).data,
        ExprData::Jsx(_)
    ));
    crate::compile::compile(source, "Other.tsx").expect("anonymous default compiles as App");
    expect_syntax_error(
        "function () {}",
        "expected `a function name`, found `(`",
        "(",
    );
    expect_syntax_error(
        "export function () {}",
        "expected `a function name`, found `(`",
        "(",
    );
}

#[test]
fn enums_lower_to_a_type_and_inlined_members() {
    let m =
        parse_ok("export enum Kind { A = 'a', B = 'b' }\nconst enum Level { Low, High = 10, Max }");
    assert_eq!(m.types.len(), 2);
    assert!(
        m.consts.is_empty(),
        "members are inlined, not a runtime object"
    );
    let values: Vec<_> = m.enums[1]
        .members
        .iter()
        .map(|member| member.value.clone())
        .collect();
    assert_eq!(
        values,
        [
            ast::EnumValue::Number("0.0".into()),
            ast::EnumValue::Number("10.0".into()),
            ast::EnumValue::Number("11.0".into()),
        ]
    );
    for (source, message, at) in [
        (
            "enum Mixed { A = 'a', B = 1 }",
            "enums mixing string and number members are not supported; use one kind",
            "Mixed",
        ),
        (
            "enum After { A = 'a', B }",
            "a member after a string member needs a value (`Member = \"…\"`)",
            "B",
        ),
        (
            "enum Computed { A = 1 + 2 }",
            "enum members are string or number literals; computed members are not supported",
            "1",
        ),
    ] {
        expect_syntax_error(source, message, at);
    }
}

#[test]
fn parses_a_component() {
    let m = parse_ok(
        r#"import { useState, View, Text } from "zant";
import type { u32 } from "zant";
export default function App() {
  const [n, setN] = useState<u32>(0);
  function inc() { setN(n + 1) }
  return (
    <View gap={8} style={{ padding: 4 }}>
      <Text size={20}>Count: {n} {`x${n * 2}y`}</Text>
      {/* comment */}
      <Button onClick={() => setN(0)} disabled>Reset &amp; go</Button>
    </View>
  );
}"#,
    );
    assert_eq!(m.imports.len(), 2);
    assert!(m.imports[1].type_only);
    assert_eq!(m.functions.len(), 1);
    let f = &m.functions[0];
    assert!(f.default);
    assert_eq!("App", text(&f.func.name));
    let body = block(&f.func);
    assert_eq!(body.len(), 3);
    let init = match &body[0].data {
        StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
        _ => panic!("expected a declaration"),
    };
    let call = match &init.data {
        ExprData::Call(c) => c,
        _ => panic!("expected a call"),
    };
    assert_eq!(call.type_args.len(), 1);
    let root = match &ret_expr(&body[2]).data {
        ExprData::Jsx(j) => j,
        _ => panic!("expected JSX"),
    };
    assert_eq!("View", text(&root.name));
    assert_eq!(root.attrs.len(), 2);
    assert_eq!(root.children.len(), 3);
    let text_el = match &root.children[0] {
        Child::Element(el) => el,
        _ => panic!("expected an element"),
    };
    assert_eq!(text_el.children.len(), 4);
    match &text_el.children[0] {
        Child::Text(t) => assert_eq!("Count: ", text(&t.value)),
        _ => panic!("expected text"),
    }
    match &text_el.children[2] {
        Child::Text(t) => assert_eq!(" ", text(&t.value)),
        _ => panic!("expected text"),
    }
    match &text_el.children[3] {
        Child::Expr(Some(e)) => match &e.data {
            ExprData::Template(t) => assert_eq!(t.quasis.len(), 2),
            _ => panic!("expected a template"),
        },
        _ => panic!("expected an expression child"),
    }
    assert!(matches!(&root.children[1], Child::Expr(None)));
    let button = match &root.children[2] {
        Child::Element(el) => el,
        _ => panic!("expected an element"),
    };
    assert!(button.attrs[1].value.is_none());
    match &button.children[0] {
        Child::Text(t) => assert_eq!("Reset & go", text(&t.value)),
        _ => panic!("expected text"),
    }
}

#[test]
fn syntax_errors_carry_a_location() {
    let mut p = Parser::init("function App() { return <View></Text>; }");
    assert!(p.parse_module().is_err());
    let d = p.diag.clone().expect("diagnostic");
    assert_eq!("expected `</View>` to close <View>", d.msg);
    assert_eq!(32, d.loc);
}

#[test]
fn arrow_function_components() {
    let m = parse_ok(
        r#"import { View } from "zant";
const Box = () => <View />;
export const Panel = (): JSX.Element => {
  return <View />;
};
export default Box;"#,
    );
    assert_eq!(m.functions.len(), 2);
    assert_eq!("Box", text(&m.functions[0].func.name));
    assert!(m.functions[0].default);
    let body = block(&m.functions[0].func);
    assert_eq!(body.len(), 1);
    assert!(matches!(&ret_expr(&body[0]).data, ExprData::Jsx(_)));
    assert_eq!("Panel", text(&m.functions[1].func.name));
    assert!(m.functions[1].exported && !m.functions[1].default);
}

#[test]
fn statements_end_at_semicolon_brace_end_or_line_break_tscs_asi() {
    // no semicolons at all is fine
    let m = parse_ok("function f() {\n  const a = 1\n  let b = a\n  b = 2\n  return b\n}");
    assert_eq!(block(&m.functions[0].func).len(), 4);
    expect_syntax_error(
        "function f() { g() h() }",
        "expected `;` or a line break before `h`",
        "h()",
    );
    expect_syntax_error(
        "function f() {\n  return\n    <View />\n}",
        "a line break after `return` ends the statement; start the value on the same line (`return (`)",
        "<View",
    );
}

#[test]
fn postfix_increment_needs_the_operand_on_the_same_line() {
    let m = parse_ok("function f() {\n  a\n  ++b\n}");
    let body = block(&m.functions[0].func);
    assert_eq!(body.len(), 2);
    assert!(matches!(
        &body[0].data,
        StmtData::Expr(e) if matches!(&e.data, ExprData::Ident(_))
    ));
    assert!(matches!(
        &body[1].data,
        StmtData::Expr(e) if matches!(&e.data, ExprData::Update(_))
    ));
}

#[test]
fn destructured_props_and_optional_parameters() {
    let m = parse_ok(
        r#"function Row({ todo, onToggle }: RowProps) { return <View />; }
function g(a: number, b?: string) {}"#,
    );
    let p = &m.functions[0].func.params[0];
    assert_eq!("props", text(&p.name.name));
    assert_eq!(p.destructure.len(), 2);
    assert_eq!("onToggle", text(&p.destructure[1].name));
    assert_eq!("RowProps", text(&p.ty.as_ref().expect("type").name));
    assert!(m.functions[1].func.params[1].optional);
}

#[test]
fn destructured_props_keep_property_names_local_names_and_defaults() {
    let m = parse_ok(
        r#"function Label({ id, title: label = "Untitled", onSave: save }: {
                id: number; title?: string; onSave: () => void;
            }) { return <Text>{label}</Text>; }"#,
    );
    let p = &m.functions[0].func.params[0];
    assert_eq!(3, p.destructure.len());
    assert_eq!("id", text(&p.destructure[0].name));
    assert_eq!("id", text(&p.destructure[0].local.name));
    assert_eq!("title", text(&p.destructure[1].name));
    assert_eq!("label", text(&p.destructure[1].local.name));
    assert_eq!("onSave", text(&p.destructure[2].name));
    assert_eq!("save", text(&p.destructure[2].local.name));
    assert!(p.destructure_defaults[0].is_none());
    assert!(matches!(
        &p.destructure_defaults[1].as_ref().expect("default").data,
        ExprData::String(value) if value == b"Untitled"
    ));
    assert!(p.destructure_defaults[2].is_none());
    expect_syntax_error(
        "function f({ title: }: P) {}",
        "expected `a variable name`, found `}`",
        "}",
    );
}

#[test]
fn destructured_defaults_accept_assignment_level_arrows() {
    let m = parse_ok(
        "function Probe({ onClose = () => {}, onPick: pick = (id: number) => save(id), onPair = (a, b) => { save(a); } }: Props) { return <View />; }",
    );
    let p = &m.functions[0].func.params[0];
    for (index, count) in [0, 1, 2].into_iter().enumerate() {
        let ExprData::Arrow(f) = &p.destructure_defaults[index].as_ref().unwrap().data else {
            panic!("expected an arrow default");
        };
        assert_eq!(count, f.params.len());
    }
    assert_eq!(b"pick", p.destructure[1].local.name.as_slice());
}

#[test]
fn typed_local_object_destructuring_keeps_keys_and_bindings() {
    let m = parse_ok(
        r#"function f() {
                const item: Item = { title: "Hello", id: 1 };
                const { title: label, id } = item;
                let { title: other }: Item = item;
            }"#,
    );
    let body = block(&m.functions[0].func);
    let StmtData::Decl(d) = &body[1].data else {
        panic!("declaration");
    };
    let Pattern::Object(bindings) = &d.pattern else {
        panic!("object destructuring");
    };
    assert_eq!("title", text(&bindings[0].key.name));
    assert_eq!("label", text(&bindings[0].local.name));
    assert_eq!(bindings[1].key, bindings[1].local);
    let StmtData::Decl(d) = &body[2].data else {
        panic!("typed declaration");
    };
    assert!(!d.is_const);
    assert_eq!("Item", text(&d.ty.as_ref().expect("type").name));
}

// Every TSX file of the repository (the golden corpus, the fixtures and
// the examples) must parse.
#[test]
fn parser_accepts_the_repository_tsx_corpus() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    let testdata = root.join("crates/zant-tsx/testdata/tsx");
    if !testdata.is_dir() {
        return;
    }
    for entry in std::fs::read_dir(&testdata).expect("testdata directory") {
        let path = entry.expect("testdata entry").path();
        if path.extension().is_some_and(|e| e == "tsx") {
            paths.push(path);
        }
    }
    assert_eq!(37, paths.len());
    let examples = root.join("crates/zant-examples/tsx/corpus");
    if examples.is_dir() {
        collect_tsx(&examples, &mut paths);
        assert!(paths.len() > 34);
    }
    for path in paths {
        let src = std::fs::read_to_string(&path).expect("read TSX file");
        let mut p = Parser::init(&src);
        if p.parse_module().is_err() {
            panic!(
                "{}: {}",
                path.display(),
                p.diag.as_ref().map(|d| d.msg.as_str()).unwrap_or("")
            );
        }
    }
}

fn collect_tsx(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_tsx(&path, out);
        } else if path.extension().is_some_and(|e| e == "tsx") {
            out.push(path);
        }
    }
}
