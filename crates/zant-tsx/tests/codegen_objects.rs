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
