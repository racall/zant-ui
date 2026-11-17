//! `async` / `await` (`async_lower`, `codegen::local_functions`). The
//! generated code of the common forms is the `async` golden (checked with
//! `cargo check`); the run-time order was checked in a real app.

use zant_tsx::compile;

fn compile_app(body: &str) -> Result<String, String> {
    let source = format!(
        "import {{ useState, useEffect, View, Text, Button }} from \"zant\";\n\
         interface Item {{ id: number; name: string }}\n{body}\n"
    );
    compile::compile(&source, "probe.tsx").map_err(|diag| diag.msg)
}

fn handler(statements: &str) -> Result<String, String> {
    compile_app(&format!(
        "export default function App() {{\n  const [items, setItems] = useState<Item[]>([]);\n  \
         const [text, setText] = useState(\"\");\n  async function go() {{\n{statements}\n  }}\n  \
         return <Button onClick={{go}}>Go</Button>;\n}}"
    ))
}

#[test]
fn awaits_become_reactions() {
    let rust = handler(
        r#"    const res = await fetch("https://example.com");
    setText(await res.text());
    let n = 0;
    n = await Promise.resolve(2);
    setText(text + n);"#,
    )
    .unwrap();
    assert!(rust.contains(".then_promise("), "{rust}");
    assert!(rust.contains("zant::tsx::Promise::resolve("), "{rust}");
}

#[test]
fn a_local_async_function_in_an_effect_is_a_method() {
    let rust = compile_app(
        r#"export default function App() {
  const [items, setItems] = useState<Item[]>([]);
  useEffect(() => {
    const controller = new AbortController();
    async function load() {
      const res = await fetch("https://example.com", { signal: controller.signal });
      const list: Item[] = await res.json();
      setItems(list);
    }
    load();
    return () => controller.abort();
  }, []);
  return <View>{items.map((item) => <Text key={item.id}>{item.name}</Text>)}</View>;
}"#,
    )
    .unwrap();
    assert!(rust.contains("fn _load_"), "{rust}");
    assert!(rust.contains("zant::tsx::json::decode::<"), "{rust}");
}

#[test]
fn unsupported_places_say_what_to_do() {
    let in_if = handler(
        r#"    if (text) {
      await fetch("u");
    }"#,
    )
    .unwrap_err();
    assert!(
        in_if.contains("not inside `if`, loops or `switch` yet"),
        "{in_if}"
    );

    let conditional =
        handler(r#"    const ok = text === "" || (await fetch("u")).ok;"#).unwrap_err();
    assert!(
        conditional.contains("await into a `const` first"),
        "{conditional}"
    );

    let in_catch = handler(
        r#"    try {
      await fetch("u");
    } catch (e) {
      await fetch("v");
    }"#,
    )
    .unwrap_err();
    assert!(in_catch.contains("`catch` / `finally`"), "{in_catch}");

    let in_finally = handler(
        r#"    try {
      await fetch("u");
    } finally {
      await fetch("v");
    }"#,
    )
    .unwrap_err();
    assert!(in_finally.contains("`catch` / `finally`"), "{in_finally}");

    for (body, expected) in [
        (
            "    for (const u of [\"a\"]) {\n      await fetch(u);\n    }",
            "not inside `if`, loops or `switch` yet",
        ),
        (
            "    switch (text) {\n      case \"a\": await fetch(\"u\"); break;\n    }",
            "not inside `if`, loops or `switch` yet",
        ),
        (
            "    const ok = text !== \"\" && (await fetch(\"u\")).ok;",
            "await into a `const` first",
        ),
        (
            "    const ok = text === \"\" ? false : (await fetch(\"u\")).ok;",
            "await into a `const` first",
        ),
        (
            "    try {\n      await fetch(\"u\");\n      return;\n    } catch (e) {}\n    setText(\"after\");",
            "a `return` inside this `try` would also skip the statements after it",
        ),
        (
            "    const n = await new Promise<number>((resolve) => { resolve(1); });",
            "`new Promise(…)` is not supported",
        ),
    ] {
        let message = handler(body).unwrap_err();
        assert!(message.contains(expected), "{message}\nbody:\n{body}");
    }

    let outside = compile_app(
        r#"export default function App() {
  function go() { await fetch("u"); }
  return <Button onClick={go}>Go</Button>;
}"#,
    )
    .unwrap_err();
    assert!(
        outside.contains("only valid inside an async function"),
        "{outside}"
    );
}
