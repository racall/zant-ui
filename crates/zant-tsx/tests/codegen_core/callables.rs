//! Function types, arrows, builtins and loop / function diagnostics.

use super::*;

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
fn local_arrows_are_function_values() {
    // a local function value (`closures`)
    let compiled = generated(r#"function good(): number { const f = () => 1; return f(); }"#);
    assert_contains(&compiled.functions, "std::rc::Rc::new(move ||");
    // without a return value it is not a function value
    let message = diagnostic(r#"function bad(): number { const f = (x) => { }; return 1; }"#);
    assert!(
        message.contains("needs typed parameters and a return value"),
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
    // numbers / strings are tested by JavaScript truthiness; a union is not
    let message = diagnostic(r#"function loops(v: string | number): void { while (v) {} }"#);
    assert!(
        message.contains("compare this value explicitly"),
        "{message}"
    );
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
