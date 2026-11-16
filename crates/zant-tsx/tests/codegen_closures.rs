//! Function values (`codegen::closures`): `Rc<dyn Fn>` closures over copies
//! of what they read, stored, passed, returned and called.

use zant_tsx::compile;

fn compile_module(src: &str) -> Result<String, String> {
    let source = format!(
        "import {{ View, Text, useState }} from \"zant\";\n{src}\nexport default function App() {{ return <View/>; }}\n"
    );
    compile::compile(&source, "probe.tsx").map_err(|diag| diag.msg)
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect()
}

#[test]
fn functions_are_values() {
    let rust = compile_module(
        r#"type Op = (x: number) => number;
interface Named { name: string; run: Op }
function double(x: number): number { return x * 2; }
function compose(f: Op, g: Op): Op { return (x) => g(f(x)); }
function adder(n: number): Op { return (x) => x + n; }
function total(): number {
  const inc = (x: number): number => x + 1;
  const ops: Named[] = [{ name: "dbl", run: double }, { name: "sq", run: (x) => x * x }];
  return ops[1].run(3) + compose(double, inc)(5) + adder(10)(1);
}"#,
    )
    .unwrap();
    let flat = squash(&rust);
    for needle in [
        "pub fn compose(f: std::rc::Rc<dyn Fn(f64) -> f64>,",
        ") -> std::rc::Rc<dyn Fn(f64) -> f64> {",
        "std::rc::Rc::new(move |x: f64| -> f64",
        // a plain function as a value
        "std::rc::Rc::new(|_a0: f64| double(_a0))",
        // a struct with a function field derives Clone only
        "#[derive(Clone)] pub struct Named",
    ] {
        assert!(
            flat.contains(&squash(needle)),
            "missing `{needle}` in {rust}"
        );
    }
}

#[test]
fn a_changing_let_cannot_be_captured() {
    let message = compile_module(
        "function f(): number { let k = 1; const g = (x: number): number => x + k; k = 2; return g(1); }",
    )
    .unwrap_err();
    assert!(
        message.contains("a `let` that may change later"),
        "{message}"
    );
}

#[test]
fn function_props_capture_the_render() {
    let source = r#"import { Text, Column, useState } from "zant";
interface RowProps { value: number; format: (n: number) => string }
function Row({ value, format }: RowProps) { return <Text>{format(value)}</Text>; }
export default function App() {
  const [unit, setUnit] = useState("px");
  return <Column><Row value={1} format={(v) => `${v}${unit}`} /></Column>;
}
"#;
    let rust = compile::compile(source, "App.tsx").unwrap();
    assert!(
        squash(&rust).contains("let_capture0:zant::Str=self.unit.clone();"),
        "{rust}"
    );
}
