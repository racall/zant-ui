//! Generic type declarations (`codegen::generic_types`): one Rust generic
//! struct per object declaration, substituted aliases, defaults and
//! constraints.

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

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        squash(haystack).contains(&squash(needle)),
        "missing `{needle}` in:\n{haystack}"
    );
}

#[test]
fn object_declarations_become_one_generic_struct() {
    let rust = compile_module(
        r#"interface Box<T> { value: T; label?: string }
type Pair<A, B = A> = { first: A; second: B };
function unbox(b: Box<number>): number { return b.value; }
function name(b: Box<string>): string { return b.value + (b.label ?? ""); }
function make(): Pair<number> { return { first: 1, second: 2 }; }"#,
    )
    .unwrap();
    assert_contains(
        &rust,
        "pub struct Box_<T> { pub value: T, pub label: Option<zant::Str>, }",
    );
    assert_contains(
        &rust,
        "impl<T: tsx::hot::Hot + Clone + Default> tsx::hot::Hot for Box_<T>",
    );
    assert_contains(&rust, "pub fn unbox(b: Box_<f64>) -> f64");
    assert_contains(&rust, "pub fn name(b: Box_<zant::Str>) -> zant::Str");
    // the default `B = A`, and the turbofish in a struct literal
    assert_contains(&rust, "pub fn make() -> Pair<f64, f64>");
    assert_contains(&rust, "Pair::<f64, f64> {");
    assert_eq!(rust.matches("pub struct Box_<T>").count(), 1);
}

#[test]
fn other_alias_bodies_are_substituted() {
    let rust = compile_module(
        r#"type Maybe<T> = T | null;
type Items<T> = T[];
function first(xs: Items<string>): Maybe<string> { return xs.length > 0 ? xs[0] : null; }"#,
    )
    .unwrap();
    assert_contains(
        &rust,
        "pub fn first(xs: tsx::List<zant::Str>) -> Option<zant::Str>",
    );
}

#[test]
fn arguments_are_checked_like_typescript() {
    for (src, expected) in [
        (
            "interface Box<T> { value: T }\nfunction f(b: Box): number { return 1; }",
            "`Box` needs a type argument for `T`",
        ),
        (
            "interface Box<T> { value: T }\nfunction f(b: Box<number, string>): number { return 1; }",
            "`Box` takes 1 type arguments",
        ),
        (
            "interface Page<T extends { id: number }> { items: T[] }\ninterface Note { title: string }\nfunction f(p: Page<Note>): number { return 1; }",
            "`Note` does not satisfy `T extends { id: number }`",
        ),
    ] {
        let message = compile_module(src).unwrap_err();
        assert!(message.contains(expected), "{src}: {message}");
    }
    // a type with the constraint's fields (and more) satisfies it
    compile_module(
        "interface Page<T extends { id: number }> { items: T[] }\ninterface Todo { id: number; title: string }\nfunction f(p: Page<Todo>): number { return p.items.length; }",
    )
    .unwrap();
}
