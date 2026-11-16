//! Untyped object literals and structural assignability (`aggregate::object`,
//! `types::coerce`), following TypeScript's structural typing.

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
fn untyped_literals_share_one_struct_per_shape() {
    let rust = compile_module(
        "const origin = { x: 0, y: 0 };\nfunction f(): number { const p = { x: 3, y: 4 }; const q = { ...p, y: 5 }; return p.x + q.y + origin.x; }",
    )
    .unwrap();
    assert_eq!(rust.matches("pub x: f64").count(), 1, "{rust}");
}

#[test]
fn objects_convert_structurally_to_the_types_they_satisfy() {
    let rust = compile_module(
        "interface Point { x: number; y: number }\ninterface Named { name: string; note?: string }\ninterface Labeled extends Point { label: string }\nfunction len(p: Point): number { return p.x + p.y; }\nfunction title(n: Named): string { return n.name; }\nfunction f(l: Labeled): number { const user = { name: \"Ada\", age: 36 }; return len({ x: 1, y: 2 }) + len(l) + title(user).length; }",
    )
    .unwrap();
    let flat = squash(&rust);
    assert!(
        flat.contains(&squash(
            "let _from: Labeled = l.clone(); Point { x: _from.x, y: _from.y }"
        )),
        "{rust}"
    );
    // extra fields are left out; a missing optional field is None
    assert!(
        flat.contains(&squash("Named { name: _from.name, note: None,")),
        "{rust}"
    );
}

#[test]
fn shapes_that_do_not_fit_are_reported() {
    for (src, expected) in [
        (
            "interface Point { x: number; y: number }\nfunction len(p: Point): number { return p.x; }\nfunction f(): number { const half = { x: 1 }; return len(half); }",
            "is missing field `y`",
        ),
        (
            "interface Point { x: number }\nfunction len(p: Point): number { return p.x; }\nfunction f(): number { const s = { x: \"1\" }; return len(s); }",
            "cannot convert",
        ),
        (
            "function f(): number { const o = { a: null }; return 1; }",
            "`a` is null",
        ),
    ] {
        let message = compile_module(src).unwrap_err();
        assert!(message.contains(expected), "{src}: {message}");
    }
}

#[test]
fn tuples_are_rust_tuples() {
    let rust = compile_module(
        "type Pair = [string, number];\nfunction minMax(xs: number[]): [number, number] { return [xs[0], xs[1]]; }\nfunction label(p: Pair): string { return p[0] + String(p[1]) + String(p.length); }\nfunction f(): number { const [lo, hi] = minMax([3, 1]); const [, n] = [\"a\", 2] as Pair; return lo + hi + n; }",
    )
    .unwrap();
    let flat = squash(&rust);
    for needle in [
        "pub fn minMax(xs: tsx::List<f64>) -> (f64, f64)",
        "pub fn label(p: (zant::Str, f64)) -> zant::Str",
        "p.clone().0.clone()",
        "2usize",
    ] {
        assert!(
            flat.contains(&squash(needle)),
            "missing `{needle}` in {rust}"
        );
    }
    let message =
        compile_module("function f(p: [string, number], i: number): string { return p[i]; }")
            .unwrap_err();
    assert!(message.contains("literal position"), "{message}");
}

#[test]
fn type_operators() {
    let rust = compile_module(
        "const labels = { open: \"Open\", done: \"Done\" };\ntype Key = keyof typeof labels;\ninterface Todo { id: number; title: string }\ntype Title = Todo[\"title\"];\ntype Size = 1 | 2 | 3;\nfunction label(k: Key): string { return labels[k]; }\nfunction title(t: Todo): Title { return t.title; }\nfunction twice(s: Size): number { return s * 2; }",
    )
    .unwrap();
    let flat = squash(&rust);
    for needle in [
        "pub enum Key",
        "Key::Open => _object.open.clone()",
        "pub fn title(t: Todo) -> zant::Str",
        "pub fn twice(s: f64) -> f64",
    ] {
        assert!(
            flat.contains(&squash(needle)),
            "missing `{needle}` in {rust}"
        );
    }
    let message = compile_module(
        "interface T { id: number; title: string }\nfunction f(t: T, k: keyof T): string { return t[k]; }",
    )
    .unwrap_err();
    assert!(message.contains("different types"), "{message}");
}
