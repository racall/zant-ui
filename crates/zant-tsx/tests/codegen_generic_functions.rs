//! Generic functions (`codegen::generic_functions`): monomorphized per type
//! argument list, with tsc-style inference from the arguments.

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
fn each_type_argument_list_gets_one_instance() {
    let rust = compile_module(
        r#"interface Box<T> { value: T }
interface Todo { id: number; title: string }
function first<T>(xs: T[]): T | null { return xs.length > 0 ? xs[0] : null; }
function unwrap<T>(box: Box<T>): T { return box.value; }
function pair<A, B = A>(a: A, b: B): string { return String(a) + String(b); }
function byId<T extends { id: number }>(items: T[], id: number): T | null { return items.find((item) => item.id === id) ?? null; }
function count<T>(...xs: T[]): number { return xs.length; }
function uses(todos: Todo[], b: Box<string>): string {
  const a = first([1, 2]) ?? 0;
  const c = first<number>([]) ?? 0;
  const d = first(todos);
  return String(a + c + count(1, 2) + count("x")) + unwrap(b) + pair(1, "x") + pair("a", "b") + (byId(todos, 1)?.title ?? "") + (d?.title ?? "");
}"#,
    )
    .unwrap();
    for needle in [
        "pub fn first__f64(xs: tsx::List<f64>) -> Option<f64>",
        "pub fn first__Todo(xs: tsx::List<Todo>) -> Option<Todo>",
        "pub fn unwrap__Str(box_: Box<zant::Str>) -> zant::Str",
        "pub fn pair__f64__Str(a: f64, b: zant::Str)",
        "pub fn pair__Str__Str(a: zant::Str, b: zant::Str)",
        "pub fn byId__Todo(",
        "pub fn count__f64(xs: tsx::List<f64>)",
        "pub fn count__Str(xs: tsx::List<zant::Str>)",
    ] {
        assert_contains(&rust, needle);
    }
    // `first([1, 2])` and `first<number>([])` share one instance
    assert_eq!(rust.matches("pub fn first__f64(").count(), 1);
    assert!(
        !rust.contains("pub fn first("),
        "the template itself is not emitted"
    );
}

#[test]
fn recursion_reuses_its_instance() {
    let rust = compile_module(
        "function depth<T>(x: T, n: number): number { return n <= 0 ? 0 : 1 + depth(x, n - 1); }\nfunction use1(): number { return depth(true, 3); }",
    )
    .unwrap();
    assert_contains(&rust, "1.0f64 + depth__bool(x");
}

#[test]
fn inference_failures_and_constraints_are_reported() {
    for (src, expected) in [
        (
            "function make<T>(): T[] { return []; }\nfunction f(): number { return make().length; }",
            "cannot infer `T` of `make` from these arguments; pass it explicitly: `make<…>(…)`",
        ),
        (
            "function first<T>(xs: T[]): T | null { return null; }\nfunction f(): number { return first([]) === null ? 1 : 0; }",
            "cannot infer `T` of `first`",
        ),
        (
            "interface Note { title: string }\nfunction byId<T extends { id: number }>(items: T[]): number { return items.length; }\nfunction f(notes: Note[]): number { return byId(notes); }",
            "`Note` does not satisfy `T extends { id: number }`",
        ),
        (
            "function same<T>(a: T, b: T): T { return a; }\nfunction f(): number { return same(1, \"x\"); }",
            "cannot convert",
        ),
        (
            "function id<T>(x: T): T { return x; }\nfunction f(): number { return id<number, string>(1); }",
            "`id` takes 1 type arguments",
        ),
    ] {
        let message = compile_module(src).unwrap_err();
        assert!(message.contains(expected), "{src}: {message}");
    }
}

#[test]
fn optional_fallbacks_keep_the_optional() {
    let rust = compile_module(
        "interface Todo { id: number }\nfunction find(items: Todo[], other: Todo | null): Todo | null { return items.find((t) => t.id === 1) ?? other; }\nfunction plain(items: Todo[]): Todo | null { return items.find((t) => t.id === 1) ?? null; }",
    )
    .unwrap();
    assert_contains(&rust, ".or_else(|| other");
}
