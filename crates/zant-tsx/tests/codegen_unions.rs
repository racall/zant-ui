//! Unions as payload enums, narrowed by typeof / discriminant checks
//! (`codegen::unions`).

use zant_tsx::compile;

fn compile_module(src: &str) -> Result<String, String> {
    let source = format!(
        "import {{ View }} from \"zant\";\n{src}\nexport default function App() {{ return <View/>; }}\n"
    );
    compile::compile(&source, "probe.tsx").map_err(|diag| diag.msg)
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect()
}

#[test]
fn discriminated_unions_narrow_by_their_tag() {
    let rust = compile_module(
        r#"type Shape = { kind: "circle"; r: number } | { kind: "square"; w: number };
function area(s: Shape): number {
  switch (s.kind) {
    case "circle": return 3 * s.r * s.r;
    case "square": return s.w * s.w;
  }
}
function label(s: Shape): string { return s.kind === "circle" ? String(s.r) : s.kind; }
function make(): Shape { return { kind: "square", w: 2 }; }"#,
    )
    .unwrap();
    let flat = squash(&rust);
    for needle in [
        "pub enum Shape { Circle(",
        "Shape::Circle(_member) => _member, _ => unreachable!(\"narrowed by the condition\")",
        "unreachable!(\"every member is handled\")",
        "Shape::Square(",
    ] {
        assert!(
            flat.contains(&squash(needle)),
            "missing `{needle}` in {rust}"
        );
    }
}

#[test]
fn primitive_unions_narrow_by_typeof() {
    let rust = compile_module(
        r#"type Id = string | number;
function describe(id: Id): string {
  if (typeof id === "string") { return id.toUpperCase(); }
  return String(id + 1);
}
function show(id: Id): string { return `${id}` + typeof id; }
function ids(): Id[] { return ["a", 7]; }"#,
    )
    .unwrap();
    let flat = squash(&rust);
    for needle in [
        "pub enum Id { Str(zant::Str), Num(f64), }",
        "Id::Num(7",
        "zant::Str::Static(\"number\")",
    ] {
        assert!(
            flat.contains(&squash(needle)),
            "missing `{needle}` in {rust}"
        );
    }
}

#[test]
fn union_mistakes_are_reported() {
    for (src, expected) in [
        (
            "type Shape = { kind: \"c\"; r: number } | { kind: \"s\"; w: number };\nfunction f(s: Shape): number { return s.r; }",
            "narrow it first",
        ),
        (
            "type Shape = { kind: \"c\"; r: number } | { kind: \"s\"; w: number };\nfunction f(): Shape { return { r: 1 }; }",
            "give the object its `kind`",
        ),
    ] {
        let message = compile_module(src).unwrap_err();
        assert!(message.contains(expected), "{src}: {message}");
    }
}

/// A component's own `function`, array callbacks, `renderItem` and promise
/// reactions: the return-path check runs after the `switch` is generated, so
/// covering every member is enough there too (not only in top-level functions).
#[test]
fn an_exhaustive_switch_needs_no_default_inside_components_and_callbacks() {
    let component = |body: &str, view: &str| {
        format!(
            "import {{ useState, View, Text, Button, List }} from \"zant\";\n\
             type Kind = \"a\" | \"b\";\n\
             type Shape = {{ kind: \"circle\"; r: number }} | {{ kind: \"rect\"; w: number }};\n\
             export default function App() {{\n\
             const [k, setK] = useState<Kind>(\"a\");\n\
             const [ks, setKs] = useState<Kind[]>([\"a\", \"b\"]);\n\
             const [s, setS] = useState(\"\");\n\
             {body}\n\
             return {view};\n}}\n"
        )
    };
    let cases = [
        // a `function` declared in the component
        component(
            "function label(x: Kind): string { switch (x) { case \"a\": return \"A\"; case \"b\": return \"B\"; } }",
            "<Text>{label(k)}</Text>",
        ),
        component(
            "function area(x: Shape): number { switch (x.kind) { case \"circle\": return x.r; case \"rect\": return x.w; } }",
            "<Text>{`${area({ kind: \"rect\", w: 2 })}`}</Text>",
        ),
        // array callbacks, with a value and with elements
        component(
            "const ls = ks.map((x) => { switch (x) { case \"a\": return \"A\"; case \"b\": return \"B\"; } });",
            "<Text>{ls.join(\",\")}</Text>",
        ),
        component(
            "",
            "<View>{ks.map((x) => { switch (x) { case \"a\": return <Text key=\"a\">A</Text>; case \"b\": return <Text key=\"b\">B</Text>; } })}</View>",
        ),
        // `renderItem`
        component(
            "",
            "<List data={ks} itemHeight={20} renderItem={(x: Kind) => { switch (x) { case \"a\": return <Text>A</Text>; case \"b\": return <Text>B</Text>; } }} />",
        ),
        // a promise reaction that returns a value
        component(
            "const go = () => { Promise.resolve(k).then((x) => { switch (x) { case \"a\": return \"A\"; case \"b\": return \"B\"; } }).then((v) => { setS(v); }); };",
            "<Button onClick={go}>{s}</Button>",
        ),
    ];
    for source in &cases {
        if let Err(message) = compile::compile(source, "probe.tsx") {
            panic!("{}\nsource:\n{source}", message.msg);
        }
    }
    // leaving a member out still needs a default
    let missing = [
        (
            component(
                "function label(x: Kind): string { switch (x) { case \"a\": return \"A\"; } }",
                "<Text>{label(k)}</Text>",
            ),
            "function must return a value on every path",
        ),
        (
            component(
                "const ls = ks.map((x) => { switch (x) { case \"a\": return \"A\"; } });",
                "<Text>{ls.join(\",\")}</Text>",
            ),
            "array callback must return on every path",
        ),
        (
            component(
                "",
                "<List data={ks} itemHeight={20} renderItem={(x: Kind) => { switch (x) { case \"a\": return <Text>A</Text>; } }} />",
            ),
            "renderItem must return an element on every path",
        ),
        (
            component(
                "const go = () => { Promise.resolve(k).then((x) => { switch (x) { case \"a\": return \"A\"; } }).then((v) => { setS(v); }); };",
                "<Button onClick={go}>{s}</Button>",
            ),
            "this callback must return a value on every path",
        ),
    ];
    for (source, expected) in &missing {
        match compile::compile(source, "probe.tsx") {
            Ok(_) => panic!("expected `{expected}`\nsource:\n{source}"),
            Err(diag) => assert!(
                diag.msg.contains(expected),
                "{}\nsource:\n{source}",
                diag.msg
            ),
        }
    }
}
