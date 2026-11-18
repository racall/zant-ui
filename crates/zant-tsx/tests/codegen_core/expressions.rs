//! Operators, strings, arrays, interfaces, objects and optionals.

use super::*;

#[test]
fn arithmetic_preserves_source_grouping_and_typed_literals() {
    assert_functions(
        r#"function calc(a: number, b: number): number { return (a + b) * 2 - a / b; }"#,
        r#"pub fn calc(a: f64, b: f64) -> f64 {
return (a + b) * 2.0f64 - a / b;
}

"#,
    );
}

#[test]
fn comparisons_and_logical_operators_map_to_rust() {
    assert_functions(
        r#"function guarded(x: number): boolean { return x > 0 && x < 10; }"#,
        r#"pub fn guarded(x: f64) -> bool {
return x > 0.0f64 && x < 10.0f64;
}

"#,
    );
    let compiled = generated(r#"function nope(x: boolean): boolean { return !x; }"#);
    assert_contains(&compiled.functions, "return !x;");
}

#[test]
fn integer_division_keeps_rust_truncating_semantics() {
    // Rust's `/` and `%` on integers already truncate toward zero (the
    // JavaScript `Math.trunc(a / b)` / `%` semantics), so the operator stays
    // plain.
    let compiled = generated(r#"function divmod(a: u32, b: u32): u32 { return (a / b) % b; }"#);
    assert_contains(&compiled.functions, "return a / b % b;");
}

// ── strings ──────────────────────────────────────────────────────────

#[test]
fn string_concatenation_and_templates_use_str_formatting() {
    let compiled = generated(
        r#"function greet(name: string): string { return "hi " + name + "!"; }
function tag(label: string, n: number): string { return `${label}:${n}`; }"#,
    );
    assert_eq!(
        r#"pub fn greet(name: zant::Str) -> zant::Str {
return zant::Str::copy(&format!("hi {}!", name.clone()));
}

pub fn tag(label: zant::Str, n: f64) -> zant::Str {
return zant::Str::copy(&format!("{}:{}", label.clone(), tsx::builtins::string(n)));
}

"#,
        compiled.functions
    );
}

#[test]
fn string_equality_names_the_static_str_variant() {
    // `zant::Str` has several `PartialEq` implementations, so a bare
    // `"a".into()` would leave Rust's inference ambiguous.
    let compiled = generated(r#"function same(name: string): boolean { return name === "zant"; }"#);
    assert_contains(
        &compiled.functions,
        "name.clone() == zant::Str::Static(\"zant\")",
    );
}

// ── arrays ───────────────────────────────────────────────────────────

#[test]
fn array_literals_and_spreads_become_lists() {
    assert_functions(
        r#"function pick(xs: number[]): number[] { return [...xs, 3]; }"#,
        r#"pub fn pick(xs: tsx::List<f64>) -> tsx::List<f64> {
return {
    let mut _items: Vec<f64> = Vec::new();
    _items.extend(xs.clone().iter().cloned());
    _items.push(3.0f64);
    tsx::List::from_vec(_items)
};
}

"#,
    );
    // items keep their source order around spreads (`[...xs, x]` appends)
    let compiled = generated(
        r#"function wrap(xs: number[], ys: number[]): number[] { return [1, ...xs, 2, ...ys]; }"#,
    );
    let body = &compiled.functions;
    let order: Vec<usize> = [
        "_items.push(1.0f64);",
        "_items.extend(xs.clone().iter().cloned());",
        "_items.push(2.0f64);",
        "_items.extend(ys.clone().iter().cloned());",
    ]
    .iter()
    .map(|part| {
        body.find(part)
            .unwrap_or_else(|| panic!("{part} in {body}"))
    })
    .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{body}");
    let compiled = generated(
        r#"function empty(): number[] { return []; }
function indexAt(xs: number[], i: number): number { return xs[i] + xs.length; }"#,
    );
    assert_contains(
        &compiled.functions,
        "return tsx::List::from_vec(Vec::<f64>::new());",
    );
    assert_contains(
        &compiled.functions,
        "return xs.clone()[tsx::index(i)] + (xs.clone().len() as f64);",
    );
}

// ── interfaces, object literals, spreads ─────────────────────────────

#[test]
fn interfaces_become_cloneable_structs() {
    assert_functions(
        r#"interface Point { x: number; y: number }
function total(p: Point): number { return p.x + p.y; }"#,
        r#"pub fn total(p: Point) -> f64 {
return p.clone().x + p.clone().y;
}

"#,
    );
    let compiled = generated(
        r#"interface Point { x: number; y: number }
function total(p: Point): number { return p.x + p.y; }"#,
    );
    assert_eq!(
        r#"#[derive(Clone, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

"#,
        compiled
            .declarations
            .split("impl tsx::hot::Hot")
            .next()
            .unwrap()
    );
}

#[test]
fn object_literals_and_spreads_keep_source_order() {
    let compiled = generated(
        r#"interface Pair { a: number; b: string }
function build(): Pair { return { a: 1, b: "x" }; }
function spread(p: Pair): Pair { return { ...p, a: 2 }; }"#,
    );
    assert_contains(
        &compiled.functions,
        r#"return Pair {
    a: 1.0f64,
    b: zant::Str::from("x"),
};"#,
    );
    assert_contains(
        &compiled.functions,
        r#"return {
    let mut _object: Pair = Pair::default();
    _object = p.clone();
    _object.a = 2.0f64;
    _object
};"#,
    );
}

#[test]
fn optional_fields_default_to_none() {
    let compiled = generated(
        r#"interface Opt { a: number; b?: string }
function copy(o: Opt): string { return o.b ?? "none"; }"#,
    );
    assert_contains(&compiled.declarations, "pub b: Option<zant::Str>,");
    assert_contains(
        &compiled.functions,
        r#"return o.clone().b.clone().unwrap_or_else(|| zant::Str::from("none"));"#,
    );
}

// ── narrowing (`??`, `!= null`, `typeof`-style checks, ternaries) ────

#[test]
fn optional_values_narrow_and_unwrap() {
    let compiled = generated(
        r#"function maybe(name: string | null): string { if (name != null) { return name; } return "none"; }
function fallback(name: string | null): string { return name ?? "none"; }
function nul(x: number | null): number { return x === null ? 0 : x; }"#,
    );
    assert_contains(&compiled.functions, "if name.clone().is_some() {");
    assert_contains(&compiled.functions, "return name.clone().unwrap();");
    assert_contains(
        &compiled.functions,
        r#"return name.clone().unwrap_or_else(|| zant::Str::from("none"));"#,
    );
    assert_contains(
        &compiled.functions,
        "return if x.is_none() { 0.0f64 } else { x.unwrap() };",
    );
}

#[test]
fn optional_chains_use_map_and_coalesce() {
    let compiled =
        generated(r#"function opt(xs: number[] | null): number { return xs?.[0] ?? 0; }"#);
    assert_contains(
        &compiled.functions,
        "return xs.clone().map(|_array| _array[tsx::index(0.0f64)]).unwrap_or_else(|| 0.0f64);",
    );
}

// ── loops and switch ─────────────────────────────────────────────────
