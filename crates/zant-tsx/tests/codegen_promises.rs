//! `fetch` and promise reactions (`codegen::promises`). The generated code of
//! the common forms is the `fetch` golden (checked with `cargo check`); the
//! run-time behavior is tested in `zant::tsx::{promise, fetch}`.

use zant_tsx::compile;

fn compile_app(body: &str) -> Result<String, String> {
    let source = format!(
        "import {{ useState, useEffect, View, Text, Button }} from \"zant\";\n\
         interface Item {{ id: number; name: string }}\n{body}\n"
    );
    compile::compile(&source, "probe.tsx").map_err(|diag| diag.msg)
}

#[test]
fn json_meets_its_type_at_the_first_typed_reaction() {
    let rust = compile_app(
        r#"export default function App() {
  const [items, setItems] = useState<Item[]>([]);
  useEffect(() => {
    fetch("https://example.com/items").then((r) => r.json()).then(setItems);
  }, []);
  return <View>{items.map((item) => <Text key={item.id}>{item.name}</Text>)}</View>;
}"#,
    )
    .unwrap();
    assert!(rust.contains("json_value()"), "{rust}");
    assert!(rust.contains("zant::tsx::json::decode::<"), "{rust}");
    // a root reaction reaches the component through the runtime context
    assert!(rust.contains("_cx.state::<App>()"), "{rust}");
}

#[test]
fn child_reactions_survive_unmounting() {
    let rust = compile_app(
        r#"function Row({ id }: { id: number }) {
  const [name, setName] = useState("");
  useEffect(() => {
    fetch("https://example.com/" + id).then((r) => r.text()).then(setName);
  }, [id]);
  return <Text>{name}</Text>;
}
export default function App() { return <View><Row id={1} /></View>; }"#,
    )
    .unwrap();
    assert!(rust.contains("_state.upgrade()"), "{rust}");
    assert!(rust.contains("Row::default()"), "{rust}");
}

#[test]
fn mistakes_are_reported_with_the_fix() {
    let untyped = compile_app(
        r#"export default function App() {
  const [n, setN] = useState(0);
  function load() { fetch("u").then((r) => r.json()).then((data) => setN(1)); }
  return <Button onClick={load}>Go</Button>;
}"#,
    )
    .unwrap_err();
    assert!(untyped.contains("give the JSON a type"), "{untyped}");

    let render = compile_app(
        r#"export default function App() {
  fetch("u");
  return <View />;
}"#,
    )
    .unwrap_err();
    assert!(render.contains("not while rendering"), "{render}");

    let option = compile_app(
        r#"export default function App() {
  function go() { fetch("u", { mode: "cors" }); }
  return <Button onClick={go}>Go</Button>;
}"#,
    )
    .unwrap_err();
    assert!(option.contains("fetch option `mode`"), "{option}");

    let signal = compile_app(
        r#"export default function App() {
  function go() { fetch("u", { signal: 3 }); }
  return <Button onClick={go}>Go</Button>;
}"#,
    )
    .unwrap_err();
    assert!(signal.contains("AbortSignal"), "{signal}");
}
