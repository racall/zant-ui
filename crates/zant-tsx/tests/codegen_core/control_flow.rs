//! Loops, labels and switch statements.

use super::*;

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
