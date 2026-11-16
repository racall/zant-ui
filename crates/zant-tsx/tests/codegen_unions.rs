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
