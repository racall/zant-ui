//! Generated code parses; satisfies, extends, intersections and rest parameters.

use super::*;

#[test]
fn every_generated_snippet_passes_syn_parse() {
    let sources = [
        r#"function calc(a: number, b: number): number { return (a + b) * 2 - a / b; }"#,
        r#"function greet(name: string): string { return "hi " + name + "!"; }"#,
        r#"function tag(label: string, n: number): string { return `${label}:${n}`; }"#,
        r#"interface Point { x: number; y: number } function total(p: Point): number { return p.x + p.y; }"#,
        r#"type Mode = "a" | "in-progress"; function label(m: Mode): string { return m === "a" ? "A" : "B"; }"#,
        r#"function pick(xs: number[]): number[] { return [...xs, 3]; }"#,
        r#"function maybe(name: string | null): string { if (name != null) { return name; } return "none"; }"#,
        r#"function sum(n: number): number { let total = 0; for (let i = 0; i < n; i++) { total += i; } return total; }"#,
        r#"function each(xs: number[]): number { let total = 0; for (const x of xs) { total += x; } return total; }"#,
        r#"function classify(mode: string): number { switch (mode) { case "a": return 1; default: return 0; } }"#,
        r#"function shape(x: number): number { return Math.floor(x) + Math.max(x, 1); }"#,
        r#"function log(x: number): void { console.log("x", x); }"#,
        r#"interface Cb { on_done: (value: number) => void } function make(cb: Cb): number { return 1; }"#,
        r#"interface Pair { a: number; b?: string } function spread(p: Pair): Pair { return { ...p, a: 2 }; }"#,
    ];
    for source in sources {
        let compiled = generated(source);
        compiled.assert_parses();
    }
}

#[test]
fn generated_modules_round_trip_through_syn_and_prettyplease() {
    let compiled =
        generated(r#"function calc(a: number, b: number): number { return (a + b) * 2 - a / b; }"#);
    let source = compiled.module();
    let formatted = codegen::parse_and_format(&source);
    // prettyplease indents the body and keeps the generated header attributes.
    assert!(
        formatted.contains("pub fn calc(a: f64, b: f64) -> f64 {"),
        "{formatted}"
    );
    assert!(
        formatted.contains("return (a + b) * 2.0f64 - a / b;"),
        "{formatted}"
    );
    assert!(
        formatted.contains("#![allow(clippy::redundant_clone)]"),
        "{formatted}"
    );
    syn::parse_file(&formatted).expect("formatted output parses");
}

// ── `satisfies` ──────────────────────────────────────────────────────

#[test]
fn satisfies_types_the_value_without_a_cast() {
    let compiled = generated(
        r#"interface P { a: number; name: string }
function make(): P { const p = { a: 1, name: "x" } satisfies P; return p; }
function count(): u32 { return 3 satisfies u32; }"#,
    );
    assert_contains(
        &compiled.functions,
        "let p: P = P {\n    a: 1.0f64,\n    name: zant::Str::from(\"x\"),\n};",
    );
    assert_contains(&compiled.functions, "return 3u32;");
}

#[test]
fn satisfies_rejects_values_the_type_does_not_accept() {
    let message = diagnostic(
        r#"interface P { a: number }
function make(): P { return { a: "x" } satisfies P; }"#,
    );
    assert!(message.contains("cannot convert"), "{message}");
}

// ── `interface extends` / `&` ────────────────────────────────────────

pub(super) fn struct_fields(compiled: &Compiled, name: &str) -> String {
    let start = compiled
        .declarations
        .find(&format!("pub struct {name} {{"))
        .unwrap_or_else(|| panic!("no struct {name} in:\n{}", compiled.declarations));
    let rest = &compiled.declarations[start..];
    rest[..rest.find('}').unwrap() + 1].to_owned()
}

#[test]
fn interface_extends_copies_every_base_field_first() {
    let compiled = generated(
        r#"interface Id { id: number }
interface Named { name?: string }
interface Todo extends Id, Named { done: boolean }
function make(): Todo { return { id: 1, done: false }; }"#,
    );
    assert_eq!(
        "pub struct Todo {\n    pub id: f64,\n    pub name: Option<zant::Str>,\n    pub done: bool,\n}",
        struct_fields(&compiled, "Todo")
    );
}

#[test]
fn intersections_merge_fields_and_a_required_side_wins() {
    let compiled = generated(
        r#"type Base = { id: number; label?: string };
type Row = Base & { label: string } & { selected: boolean };
function label(r: Row | null): string { return r === null ? "" : r.label; }"#,
    );
    assert_eq!(
        "pub struct Row {\n    pub id: f64,\n    pub label: zant::Str,\n    pub selected: bool,\n}",
        struct_fields(&compiled, "Row")
    );
}

#[test]
fn intersections_reject_conflicting_fields_and_non_objects() {
    let message = diagnostic(
        r#"interface A { id: number }
type B = A & { id: string };
function f(b: B): number { return 1; }"#,
    );
    assert!(
        message.contains("field `id` has different types"),
        "{message}"
    );
    let message = diagnostic(
        r#"type B = string & { id: string };
function f(b: B): number { return 1; }"#,
    );
    assert!(message.contains("combine object types"), "{message}");
}

// ── rest parameters ──────────────────────────────────────────────────

#[test]
fn rest_parameters_collect_the_remaining_arguments_into_a_list() {
    let compiled = generated(
        r#"function sum(base: number, ...xs: number[]): number { let t = base; for (const x of xs) t += x; return t; }
function calls(more: number[]): number { return sum(0) + sum(1, 2, 3) + sum(1, ...more) + sum(0, 1, ...more, 4); }"#,
    );
    let f = &compiled.functions;
    assert_contains(f, "pub fn sum(base: f64, xs: tsx::List<f64>) -> f64 {");
    assert_contains(f, "sum(0.0f64, tsx::List::from_vec(Vec::<f64>::new()))");
    assert_contains(f, "sum(1.0f64, tsx::List::from_vec(vec![2.0f64, 3.0f64]))");
    // a lone spread passes the list through instead of copying it
    assert_contains(f, "sum(1.0f64, more.clone())");
    assert_contains(f, "_items.extend(more.clone().iter().cloned());");
}

#[test]
fn rest_parameters_and_spread_arguments_report_misuse() {
    for (src, expected) in [
        (
            "function f(...xs: number[], y: number): number { return y; }",
            "must be the last",
        ),
        (
            "function g(): number { const h = (...xs: number[]) => 1; return 0; }",
            "supported in `function` declarations",
        ),
    ] {
        let mut parser = Parser::init(src);
        assert!(parser.parse_module().is_err(), "{src}");
        let message = parser.diag.expect("diagnostic").msg;
        assert!(message.contains(expected), "{src}: {message}");
    }
    for (src, expected) in [
        (
            "function f(...xs: number): number { return 1; }",
            "needs an array type",
        ),
        (
            "function f(a: number): number { return a; }\nfunction g(xs: number[]): number { return f(...xs); }",
            "can only fill a rest parameter",
        ),
    ] {
        let message = match compile(src) {
            Ok(compiled) => panic!("expected an error for {src}:\n{}", compiled.functions),
            Err(message) => message,
        };
        assert!(message.contains(expected), "{src}: {message}");
    }
}
