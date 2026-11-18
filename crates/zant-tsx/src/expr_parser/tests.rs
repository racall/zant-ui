//! Unit tests.

use super::*;
use crate::ast::{Body, Child, Expr, Function, Stmt, StmtData, TypeDetail};
use crate::parser::{expect_syntax_error, parse_ok};

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

#[test]
fn regex_literals_become_regexp_constructions_and_division_stays() {
    let module = parse_ok(
        r#"function f(s: string, a: number, b: number): number {
  const re = /a[/\]]b\/c/gi;
  const total = a / b / 2;
  return re.test(s) ? total : 0;
}"#,
    );
    let ss = block(&module.functions[0].func);
    let StmtData::Decl(decl) = &ss[0].data else {
        panic!("a declaration")
    };
    let ExprData::Call(call) = &decl.init.as_ref().unwrap().data else {
        panic!("a call")
    };
    assert!(call.is_new);
    assert!(matches!(&call.callee.data, ExprData::Ident(n) if n == b"RegExp"));
    assert!(matches!(&call.args[0].data, ExprData::String(v) if v == br"a[/\]]b\/c"));
    assert!(matches!(&call.args[1].data, ExprData::String(v) if v == b"gi"));
    let StmtData::Decl(decl) = &ss[1].data else {
        panic!("a declaration")
    };
    assert!(matches!(&decl.init.as_ref().unwrap().data, ExprData::Binary(b) if b.op == b"/"));
    expect_syntax_error(
        "function f(): number { const re = /abc; return 1; }",
        "unterminated regular expression literal",
        "/abc",
    );
}

fn expect_codegen_error(source: &str, message: &str, at: &str) {
    parse_ok(source);
    let diag = crate::compile::compile(source, "test.tsx").expect_err("codegen error");
    assert_eq!(diag.msg, message);
    assert_eq!(
        diag.loc as usize,
        source.find(at).expect("diagnostic location")
    );
}

fn text(value: &[u8]) -> &str {
    std::str::from_utf8(value).expect("text is UTF-8")
}

#[test]
fn const_assertions_preserve_literal_expressions() {
    for literal in [
        "42",
        "-42",
        "true",
        "false",
        r#""ready""#,
        "`ready`",
        "[]",
        r#"[1, "two", { done: true }]"#,
        r#"({ kind: "ready", values: [1, 2] })"#,
        r#"{ kind: "ready" as const, values: [1, 2] as const }"#,
    ] {
        let plain = parse_ok(&format!("const value = {literal};"));
        let asserted = parse_ok(&format!("const value = {literal} as const;"));
        assert_eq!(plain.consts[0].value, asserted.consts[0].value, "{literal}");
    }
    let m = parse_ok("const value = ([1, 2] as const)[0];");
    assert!(matches!(&m.consts[0].value.data, ExprData::Index(_)));
}

#[test]
fn const_assertions_do_not_relax_other_assertion_targets() {
    expect_codegen_error(
        "const value = 1 as WeakRef<number>;\nexport default function App() { return null; }",
        "generic type `WeakRef<…>` is not supported yet; declare it in this file (`interface X<T>`) or use a concrete type",
        "WeakRef",
    );
    // `typeof` names a top-level constant of the file
    expect_codegen_error(
        "const value = 1 as typeof other;\nexport default function App() { return null; }",
        "`typeof other`: only top-level constants of this file can be named here",
        "other",
    );
}

#[test]
fn comparison_is_not_a_type_argument_list() {
    let m = parse_ok("function f() { return a < b; }");
    let e = ret_expr(&block(&m.functions[0].func)[0]);
    match &e.data {
        ExprData::Binary(b) => assert_eq!("<", text(&b.op)),
        _ => panic!("expected a binary expression"),
    }
}

#[test]
fn typed_arrows_report_errors_inside_their_parameters() {
    expect_codegen_error(
        "function f() { return (v: WeakRef<number>) => v; }",
        "generic type `WeakRef<…>` is not supported yet; declare it in this file (`interface X<T>`) or use a concrete type",
        "WeakRef",
    );
    expect_syntax_error(
        "function f() { return (): number => ; }",
        "unexpected `;`",
        ";",
    );
}

#[test]
fn phase_2_expressions_literals_with_spread_indexing_optional_chain_non_null_as() {
    let m = parse_ok(
        r#"const empty: Todo[] = [];
function f() {
  const [todos, setTodos] = useState<Todo[]>([])
  const next = [...todos, { id: 1, ...base, done: false }]
  const first = todos[0]?.title ?? "none"
  const found = todos.find((t: Todo): boolean => t.done)!
  const n = count as u32
  return todos.map((t, i) => <Text key={t.id}>{i}</Text>)
}"#,
    );
    assert_eq!(m.consts.len(), 1);
    match &m.consts[0].value.data {
        ExprData::Array(items) => assert_eq!(items.len(), 0),
        _ => panic!("expected an array literal"),
    }
    let body = block(&m.functions[0].func);
    let init = match &body[0].data {
        StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
        _ => panic!("expected a declaration"),
    };
    let call = match &init.data {
        ExprData::Call(c) => c,
        _ => panic!("expected a call"),
    };
    match call.type_args[0].detail.as_deref() {
        Some(TypeDetail::Array(inner)) => assert_eq!("Todo", text(&inner.name)),
        _ => panic!("expected an array type"),
    }
    let next = match &body[1].data {
        StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
        _ => panic!("expected a declaration"),
    };
    let items = match &next.data {
        ExprData::Array(items) => items,
        _ => panic!("expected an array literal"),
    };
    assert!(matches!(&items[0].data, ExprData::Spread(_)));
    let obj = match &items[1].data {
        ExprData::Object(props) => props,
        _ => panic!("expected an object literal"),
    };
    assert!(obj[1].spread && !obj[0].spread);
    let coalesce = match &body[2].data {
        StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
        _ => panic!("expected a declaration"),
    };
    let binary = match &coalesce.data {
        ExprData::Binary(b) => b,
        _ => panic!("expected a binary expression"),
    };
    assert_eq!("??", text(&binary.op));
    let member = match &binary.lhs.data {
        ExprData::Member(m) => m,
        _ => panic!("expected a member expression"),
    };
    assert!(member.optional);
    assert!(matches!(&member.object.data, ExprData::Index(_)));
    assert!(matches!(
        &body[3].data,
        StmtData::Decl(d) if matches!(&d.init.as_ref().unwrap().data, ExprData::NonNull(_))
    ));
    let as_expr = match &body[4].data {
        StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
        _ => panic!("expected a declaration"),
    };
    match &as_expr.data {
        ExprData::As(a) => assert_eq!("u32", text(&a.ty.name)),
        _ => panic!("expected an `as` expression"),
    }
    let map_call = match &ret_expr(&body[5]).data {
        ExprData::Call(c) => c,
        _ => panic!("expected a call"),
    };
    let arrow = match &map_call.args[0].data {
        ExprData::Arrow(f) => f,
        _ => panic!("expected an arrow function"),
    };
    assert_eq!(arrow.params.len(), 2);
}

#[test]
fn a_parenthesized_expression_is_not_taken_for_an_arrow() {
    let m = parse_ok("function f() { return x ? (a) : b; }");
    let e = ret_expr(&block(&m.functions[0].func)[0]);
    let c = match &e.data {
        ExprData::Cond(c) => c,
        _ => panic!("expected a conditional"),
    };
    assert!(matches!(&c.then.data, ExprData::Ident(_)));
}

#[test]
fn greater_than_joins_into_greater_or_equal_only_as_an_operator() {
    let m = parse_ok("function f() { return a >= 10 && b > 2; }");
    let e = ret_expr(&block(&m.functions[0].func)[0]);
    let binary = match &e.data {
        ExprData::Binary(b) => b,
        _ => panic!("expected a binary expression"),
    };
    assert_eq!("&&", text(&binary.op));
    let lhs = match &binary.lhs.data {
        ExprData::Binary(b) => b,
        _ => panic!("expected a binary expression"),
    };
    assert_eq!(">=", text(&lhs.op));
    let rhs = match &binary.rhs.data {
        ExprData::Binary(b) => b,
        _ => panic!("expected a binary expression"),
    };
    assert_eq!(">", text(&rhs.op));
    // `>` closing a tag followed by `=` text stays apart
    let t = parse_ok("function f() { return <Text>=</Text>; }");
    let jsx = match &ret_expr(&block(&t.functions[0].func)[0]).data {
        ExprData::Jsx(j) => j,
        _ => panic!("expected JSX"),
    };
    match &jsx.children[0] {
        Child::Text(child) => assert_eq!("=", text(&child.value)),
        _ => panic!("expected text"),
    }
}
