//! `throw` / `try` / `catch` / `finally` (`codegen::exceptions`): throwing
//! functions return `Result`, handlers report uncaught errors, renders and
//! function values must catch. Run-time behavior is compared with Node by
//! `scripts/tsx/node-parity.sh` (`tests/node_parity/exceptions.ts`).

use zant_tsx::compile;

fn compile_app(src: &str) -> Result<String, String> {
    let source = format!("import {{ View, Text, Button, useState }} from \"zant\";\n{src}\n");
    compile::compile(&source, "probe.tsx").map_err(|diag| diag.msg)
}

fn compile_module(src: &str) -> Result<String, String> {
    compile_app(&format!(
        "{src}\nexport default function App() {{ return <View/>; }}"
    ))
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect()
}

fn assert_has(rust: &str, needles: &[&str]) {
    let flat = squash(rust);
    for needle in needles {
        assert!(
            flat.contains(&squash(needle)),
            "missing `{needle}` in {rust}"
        );
    }
}

#[test]
fn throwing_functions_return_results_and_callers_propagate() {
    let rust = compile_module(
        r#"function check(n: number): number { if (n < 0) { throw new RangeError("negative"); } return n; }
function twice(n: number): number { return check(n) * 2; }
function validate(s: string): void { if (s.length === 0) { throw "empty"; } }
function pure(n: number): number { return n + 1; }"#,
    )
    .unwrap();
    assert_has(
        &rust,
        &[
            "pub fn check(n: f64) -> Result<f64, zant::tsx::Thrown>",
            "return Err(zant::tsx::Thrown::error(zant::tsx::Error::new(\"RangeError\", zant::Str::from(\"negative\")",
            "return Ok(n);",
            "pub fn twice(n: f64) -> Result<f64, zant::tsx::Thrown>",
            "return Ok(check(n)? * 2.0f64);",
            "pub fn validate(s: zant::Str) -> Result<(), zant::tsx::Thrown>",
            "return Err(zant::tsx::Thrown::String(zant::Str::from(\"empty\")));",
            "Ok(())",
            // a function that cannot throw keeps its plain signature
            "pub fn pure(n: f64) -> f64",
        ],
    );
}

#[test]
fn a_catch_stops_propagation_and_narrows_the_error() {
    let rust = compile_module(
        r#"function check(n: number): number { if (n < 0) { throw new Error("negative"); } return n; }
function safe(n: number): string {
  try { return "ok " + check(n); }
  catch (e) { return e instanceof Error ? e.message : String(e); }
}"#,
    )
    .unwrap();
    assert_has(
        &rust,
        &[
            "pub fn safe(n: f64) -> zant::Str",
            "if let Err(_thrown) = '_try:",
            "Err(_thrown) => break '_try Err(_thrown)",
            "let e: zant::tsx::Thrown = _thrown;",
            "e.is_error()",
            "e.as_error()",
            ".to_str()",
        ],
    );
}

#[test]
fn finally_runs_through_a_completion_record() {
    let rust = compile_module(
        r#"function loop(): number {
  let total = 0;
  for (let i = 0; i < 3; i++) {
    try { if (i === 1) { continue; } total += i; } finally { total += 10; }
  }
  return total;
}"#,
    )
    .unwrap();
    assert_has(
        &rust,
        &[
            "zant::tsx::Completion<f64> = '_try:",
            "break '_try zant::tsx::Completion::Jump(0);",
            "zant::tsx::Completion::Jump(0) => { continue '_loop; }",
            // nothing in the protected block throws
            "zant::tsx::Completion::Throw(_thrown) => { unreachable!",
        ],
    );
}

#[test]
fn a_handler_reports_an_uncaught_throw() {
    let rust = compile_app(
        r#"function parse(text: string): number { if (text === "") { throw new Error("empty"); } return 1; }
export default function App() {
  const [n, setN] = useState(0);
  return <Button onClick={() => setN(parse(""))}>{n}</Button>;
}"#,
    )
    .unwrap();
    assert_has(
        &rust,
        &["Err(_thrown) => { zant::tsx::error::uncaught(&(_thrown)); return; }"],
    );
}

#[test]
fn renders_and_function_values_must_catch() {
    let message = compile_app(
        r#"function parse(text: string): number { if (text === "") { throw new Error("empty"); } return 1; }
export default function App() { return <Text>{parse("")}</Text>; }"#,
    )
    .unwrap_err();
    assert!(
        message.contains("a component cannot throw while it renders"),
        "{message}"
    );
    let message = compile_module(
        r#"function parse(text: string): number { if (text === "") { throw new Error("empty"); } return 1; }
function all(items: string[]): number[] { return items.map((item) => parse(item)); }"#,
    )
    .unwrap_err();
    assert!(
        message.contains("a throw cannot leave this function yet"),
        "{message}"
    );
    let message = compile_module(
        r#"type Parse = (text: string) => number;
function parse(text: string): number { if (text === "") { throw new Error("empty"); } return 1; }
function pick(): Parse { return parse; }"#,
    )
    .unwrap_err();
    assert!(message.contains("`parse` can throw"), "{message}");
}

#[test]
fn only_errors_strings_numbers_and_booleans_are_thrown() {
    let message = compile_module(
        r#"interface Point { x: number }
function f(p: Point): number { throw p; }"#,
    )
    .unwrap_err();
    assert!(message.contains("throw an Error"), "{message}");
    let message = compile_module(r#"function f(): number { try { return 1; } }"#).unwrap_err();
    assert!(message.contains("`catch` or `finally`"), "{message}");
}
