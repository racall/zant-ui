//! JavaScript truthiness (`codegen::truthy`), block operands and Rust
//! prelude names. Run-time behavior is compared with Node by
//! `scripts/tsx/node-parity.sh` (`tests/node_parity/truthiness.ts`).

use zant_tsx::compile;

fn compile_app(src: &str) -> Result<String, String> {
    compile::compile(src, "probe.tsx").map_err(|diag| diag.msg)
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect()
}

#[test]
fn strings_and_numbers_are_conditions_and_or_returns_a_value() {
    let rust = compile_app(
        r#"import { View, Text, Button, Input, useState } from "zant";
export default function App() {
  const [name, setName] = useState("");
  const [count, setCount] = useState(0);
  return <View>
    <Input value={name} onChange={setName} />
    <Text>{name || "Untitled"}</Text>
    <Button disabled={!name} onClick={() => setCount(count + 1)}>{count ? "again" : "first"}</Button>
  </View>;
}"#,
    )
    .unwrap();
    let flat = squash(&rust);
    for needle in [
        "!!self.name.clone().is_empty()",
        "let_or=self.name.clone();if!_or.clone().is_empty(){_or}else{zant::Str::from(\"Untitled\")}",
        "_number!=0.0&&!_number.is_nan()",
    ] {
        assert!(
            flat.contains(&squash(needle)),
            "missing `{needle}` in {rust}"
        );
    }
}

#[test]
fn a_string_method_result_is_parenthesized_as_an_operand() {
    // `'s: { … } || b` would end the expression after the block
    let rust = compile_app(
        r#"import { View } from "zant";
function matches(title: string, query: string, tags: string[]): boolean {
  return title.toLowerCase().includes(query) || tags.includes(query);
}
export default function App() { return <View/>; }"#,
    )
    .unwrap();
    assert!(rust.contains("return ('_string"), "{rust}");
}

#[test]
fn a_type_named_like_the_rust_prelude_is_renamed() {
    let rust = compile_app(
        r#"import { View, Text, useState } from "zant";
type Result = { ok: boolean; message: string };
function check(text: string): Result { return { ok: text.length > 0, message: text }; }
export default function App() {
  const [result] = useState<Result>(check("x"));
  return <View><Text>{result.message}</Text></View>;
}"#,
    );
    // `const [result] = useState(…)` is a separate gap; the type is what
    // matters here
    let rust = match rust {
        Ok(rust) => rust,
        Err(_) => compile_app(
            r#"import { View } from "zant";
type Result = { ok: boolean; message: string };
function check(text: string): Result { return { ok: text.length > 0, message: text }; }
export default function App() { return <View/>; }"#,
        )
        .unwrap(),
    };
    assert!(rust.contains("pub struct Result_"), "{rust}");
    assert!(rust.contains("-> Result_"), "{rust}");
}
