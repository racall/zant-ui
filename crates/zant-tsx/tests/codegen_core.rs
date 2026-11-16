//! Codegen core tests: expression / statement / type generation
//! against hand-reviewed Rust, and `syn::parse_file` validation of everything
//! that is emitted.
//!
//! The cases only need plain functions (functions, loops, emission and
//! builtins); components and JSX have their own suites.

use zant_tsx::ast;
use zant_tsx::codegen::{self, Generator, functions, model};
use zant_tsx::parser::Parser;

struct Compiled {
    declarations: String,
    functions: String,
}

impl Compiled {
    /// The whole generated module wrapped in the preamble the application layer adds.
    fn module(&self) -> String {
        format!(
            "#![allow(clippy::redundant_clone)]\n#![allow(clippy::too_many_arguments)]\n\
             use zant::tsx;\nuse zant::ui;\n\n{}{}",
            self.declarations, self.functions
        )
    }

    fn assert_parses(&self) {
        let source = self.module();
        if let Err(error) = syn::parse_file(&source) {
            panic!("generated code does not parse: {error}\n---\n{source}");
        }
    }
}

fn parse_module(src: &str) -> ast::Module {
    let mut parser = Parser::init(src);
    match parser.parse_module() {
        Ok(module) => module,
        Err(_) => match parser.diag {
            Some(diag) => panic!("parse error at {}: {}", diag.loc, diag.msg),
            None => panic!("parse error without a diagnostic"),
        },
    }
}

/// Generate Rust for a module that only holds plain functions and types.
fn compile(src: &str) -> Result<Compiled, String> {
    let module = parse_module(src);
    let mut diag = None;
    let result = {
        let mut g = Generator::new(&mut diag);
        g.type_decls = module.types.clone();
        g.react_imports = module.imports.clone();
        g.resolving = vec![false; module.types.len()];
        (|| -> codegen::Result<Compiled> {
            for decl in &module.types {
                let _ = model::named(&mut g, &decl.name.name)?;
            }
            functions::collect(&mut g, &module)?;
            let declarations = model::declarations(&g);
            let functions = functions::emit_all(&mut g)?;
            Ok(Compiled {
                declarations,
                functions,
            })
        })()
    };
    result.map_err(|_| match diag {
        Some(diag) => format!("{} @{}", diag.msg, diag.loc),
        None => "codegen failed without a diagnostic".to_owned(),
    })
}

fn generated(src: &str) -> Compiled {
    match compile(src) {
        Ok(compiled) => {
            compiled.assert_parses();
            compiled
        }
        Err(message) => panic!("unexpected diagnostic: {message}"),
    }
}

fn diagnostic(src: &str) -> String {
    match compile(src) {
        Ok(compiled) => panic!("expected a diagnostic, generated:\n{}", compiled.functions),
        Err(message) => message,
    }
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "missing `{needle}` in:\n---\n{haystack}\n---"
    );
}

fn assert_functions(src: &str, expected: &str) {
    let compiled = generated(src);
    assert_eq!(expected, compiled.functions, "for source: {src}");
}

// ── arithmetic, operators, precedence ────────────────────────────────

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
    let mut _items: Vec<f64> = vec![3.0f64];
    _items.extend(xs.clone().iter().cloned());
    tsx::List::from_vec(_items)
};
}

"#,
    );
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

#[test]
fn classic_for_runs_the_update_at_the_top_of_the_loop() {
    assert_functions(
        r#"function sum(n: number): number { let total = 0; for (let i = 0; i < n; i++) { total += i; } return total; }"#,
        r#"pub fn sum(n: f64) -> f64 {
let mut total: f64 = 0.0f64;
{
let mut i: f64 = 0.0f64;
let mut _first = true;
loop {
if _first {
_first = false;
} else {
i += 1.0;
}
if !(i < n) {
break;
}
{
total = total + i;
}
}
}
return total;
}

"#,
    );
}

#[test]
fn for_of_iterates_cloned_list_items() {
    assert_functions(
        r#"function each(xs: number[]): number { let total = 0; for (const x of xs) { total += x; } return total; }"#,
        r#"pub fn each(xs: tsx::List<f64>) -> f64 {
let mut total: f64 = 0.0f64;
{
let _items: tsx::List<f64> = xs.clone();
for x in _items.iter().cloned() {
{
total = total + x;
}
}
}
return total;
}

"#,
    );
}

#[test]
fn do_while_and_labeled_continue_keep_the_condition() {
    assert_functions(
        r#"function doer(start: number): number { let n = start; do { n--; if (n === 2) continue; } while (n > 0); return n; }"#,
        r#"pub fn doer(start: f64) -> f64 {
let mut n: f64 = start;
{
let mut _first = true;
'_loop: while _first || (n > 0.0f64) {
_first = false;
{
n -= 1.0;
if n == 2.0f64 {
continue '_loop;
}
}
}
}
return n;
}

"#,
    );
}

#[test]
fn nested_loops_label_only_the_jumps_they_use() {
    let compiled = generated(
        r#"function nested(n: number): number { let total = 0; for (let i = 0; i < n; i++) { for (let j = 0; j < i; j++) { if (j === 1) break; total += j; } if (i === 2) continue; total += i; } return total; }"#,
    );
    assert_contains(&compiled.functions, "'_loop: loop {");
    assert_contains(&compiled.functions, "'_loop_2: loop {");
    assert_contains(&compiled.functions, "break '_loop_2;");
    assert_contains(&compiled.functions, "continue '_loop;");
}

#[test]
fn switch_becomes_a_labeled_if_chain_with_static_case_values() {
    assert_functions(
        r#"function classify(mode: string): number { switch (mode) { case "a": return 1; case "b": return 7; default: return 0; } }"#,
        r#"pub fn classify(mode: zant::Str) -> f64 {
{
let _value: zant::Str = mode.clone();
if (_value == zant::Str::Static("a")) {
return 1.0f64;
} else if (_value == zant::Str::Static("b")) {
return 7.0f64;
} else {
return 0.0f64;
}
}
}

"#,
    );
}

#[test]
fn switch_case_tests_are_arbitrary_expressions() {
    // Case tests may call functions and run in source order, so the if chain
    // is the only sound lowering.
    let compiled = generated(
        r#"function next(n: number): number { return n; }
function select(): number { let total = 0; switch (next(2)) { case next(1): total = 1; break; case next(2): total = 20; break; default: total = 90; break; } return total; }"#,
    );
    assert_contains(&compiled.functions, "if (_value == next(1.0f64)) {");
    assert_contains(&compiled.functions, "} else if (_value == next(2.0f64)) {");
    assert_contains(&compiled.functions, "} else {");
}

#[test]
fn switch_fallthrough_is_a_diagnostic() {
    let message = diagnostic(
        r#"function bad(x: number): number { switch (x) { case 1: console.log(1); case 2: break; } return 0; }"#,
    );
    assert!(
        message.contains("add `break`; falling through to the next case is not supported"),
        "{message}"
    );
}

// ── closures / function types ────────────────────────────────────────

#[test]
fn function_types_become_rc_dyn_fn() {
    let compiled = generated(
        r#"interface Cb { on_done: (value: number) => void }
function make(cb: Cb): number { return 1; }"#,
    );
    // `Rc<dyn Fn>` is neither `Default` nor `PartialEq`, so the struct derives
    // `Clone` and gets a no-op default closure. A callback that is not a ui
    // action takes the runtime `ActionCx` first (so handlers can call
    // it `ui::effect`-queued): `(value: number) => void` is not an event.
    assert_eq!(
        r#"#[derive(Clone)]
pub struct Cb {
    pub on_done: std::rc::Rc<dyn Fn(&mut ui::ActionCx<'_>, f64)>,
}

impl Default for Cb {
    fn default() -> Self {
        Self {
            on_done: std::rc::Rc::new(|_: &mut ui::ActionCx<'_>, _: f64| ()),
        }
    }
}

"#,
        compiled
            .declarations
            .split("impl tsx::hot::Hot")
            .next()
            .unwrap()
    );
    assert!(compiled.declarations.contains("impl tsx::hot::Hot for Cb"));
    assert_contains(&compiled.functions, "pub fn make(cb: Cb) -> f64 {");
}

#[test]
fn arrow_functions_outside_handler_positions_are_diagnostics() {
    let message = diagnostic(r#"function bad(): number { const f = () => 1; return f(); }"#);
    assert!(
        message.contains("nested functions are not supported yet (closures arrive in phase 2)"),
        "{message}"
    );
}

// ── built-ins ────────────────────────────────────────────────────────

#[test]
fn math_builtins_use_the_runtime_helpers_and_sequence_arguments() {
    assert_functions(
        r#"function shape(x: number): number { return Math.floor(x) + Math.max(x, 1); }"#,
        r#"pub fn shape(x: f64) -> f64 {
return ({
let _arg: f64 = x;
tsx::builtins::unary(tsx::builtins::Unary::Floor, _arg)
}) + ({
let _arg_2: f64 = x;
let _arg_3: f64 = 1.0f64;
tsx::builtins::extrema::<f64>(false, &[_arg_2, _arg_3])
});
}

"#,
    );
}

#[test]
fn console_log_is_a_statement_with_js_number_formatting() {
    assert_functions(
        r#"function log(x: number): void { console.log("x", x); }"#,
        r#"pub fn log(x: f64) {
tsx::builtins::log(&[tsx::Any::from(zant::Str::copy(&format!("x {}", tsx::builtins::string(x))))]);
}

"#,
    );
}

#[test]
fn number_constants_and_conversions_use_the_js_helpers() {
    let compiled = generated(
        r#"function limits(x: number): number { return Number.MAX_SAFE_INTEGER + Number.EPSILON + Math.PI + parseInt("3", 10) + Number(x); }"#,
    );
    assert_contains(&compiled.functions, "9_007_199_254_740_991f64");
    assert_contains(&compiled.functions, "f64::EPSILON");
    assert_contains(&compiled.functions, "std::f64::consts::PI");
    assert_contains(&compiled.functions, "tsx::builtins::parse_int(");
    assert_contains(&compiled.functions, "tsx::builtins::number(");
}

// ── function diagnostics ─────────────────────────────────────────────

#[test]
fn function_diagnostics() {
    let cases = [
        (
            r#"function twice(n): number { return n * 2; }"#,
            "parameter `n` of `twice` needs a type",
        ),
        (
            r#"function row(): JSX.Element { return <View/>; }"#,
            "components start with a capital letter",
        ),
        (
            r#"function f(): number { return 1; }
function f(): number { return 2; }"#,
            "duplicate function `f`",
        ),
        (
            r#"function count(): number { if (true) return 1; }"#,
            "`count` must return a value on every path",
        ),
        (
            r#"function f(): number { return 1; }
function g(): number { return f(1); }"#,
            "`f` takes 0 arguments",
        ),
        (
            r#"function f(): void { const x = useState(0); }"#,
            "hooks must be called at the top level of the component",
        ),
    ];
    for (source, expected) in cases {
        let message = diagnostic(source);
        assert!(
            message.contains(expected),
            "expected `{expected}` in `{message}` for {source}"
        );
    }
}

// ── loop assignment / scoping behaviour ──────────────────────────────

#[test]
fn loop_bindings_use_assigned_tracking_and_do_not_leak() {
    // An assigned binding is `let mut`, an unassigned one plain `let`, and
    // the shadowed loop variable is renamed `i_2`.
    let compiled = generated(
        r#"function bindings(): void {
let total = 0;
for (let x of [1]) { x++; total += x; }
for (let y of [2]) total += y;
for (let i = 0; i < 3; i++) total += i;
const i = 8;
console.log(total, i);
}"#,
    );
    assert_contains(&compiled.functions, "for mut x in");
    assert_contains(&compiled.functions, "for y in");
    assert_contains(&compiled.functions, "let mut i: f64");
    assert_contains(&compiled.functions, "let i_2: f64");
}

#[test]
fn loop_diagnostics_point_at_the_loop() {
    let message = diagnostic(r#"function loops(): void { while (1) {} }"#);
    assert!(message.contains("conditions must be boolean"), "{message}");
    let message = diagnostic(r#"function breaks(): void { break; }"#);
    assert!(
        message.contains("break is only allowed inside a loop or switch"),
        "{message}"
    );
    let message = diagnostic(r#"function each(): void { for (const x of 3) {} }"#);
    assert!(
        message.contains("for...of requires an array or a string"),
        "{message}"
    );
}

// ── every generated snippet must parse ───────────────────────────────

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

fn struct_fields(compiled: &Compiled, name: &str) -> String {
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
