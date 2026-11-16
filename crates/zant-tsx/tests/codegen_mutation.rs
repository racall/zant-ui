//! In-place changes of arrays and objects (`codegen::place`,
//! `codegen::array_mutations`): accepted on fresh locals, reported where
//! TypeScript's shared references would behave differently.

use zant_tsx::compile;

const APP: &str = "\nexport default function App() { return <View/>; }\n";

fn compile_module(src: &str) -> Result<String, String> {
    let source = format!("import {{ View }} from \"zant\";\n{src}{APP}");
    compile::compile(&source, "probe.tsx").map_err(|diag| diag.msg)
}

fn generated(src: &str) -> String {
    match compile_module(src) {
        Ok(rust) => {
            if let Err(error) = syn::parse_file(&rust) {
                panic!("generated code does not parse: {error}\n{rust}");
            }
            rust
        }
        Err(message) => panic!("unexpected diagnostic: {message}"),
    }
}

fn diagnostic(src: &str) -> String {
    match compile_module(src) {
        Ok(rust) => panic!("expected a diagnostic for {src}:\n{rust}"),
        Err(message) => message,
    }
}

/// Whitespace-insensitive: the output is pretty-printed.
fn assert_contains(haystack: &str, needle: &str) {
    let squash = |s: &str| s.split_whitespace().collect::<String>();
    assert!(
        squash(haystack).contains(&squash(needle)),
        "missing `{needle}` in:\n{haystack}"
    );
}

// ── in-place changes (mutable value semantics) ───────────────────────

#[test]
fn fresh_locals_change_in_place() {
    let f = &generated(
        r#"interface Todo { id: number; done: boolean; tags: string[] }
function build(n: number): Todo[] {
  const out: Todo[] = [];
  for (let i = 0; i < n; i++) { out.push({ id: i, done: false, tags: [] }); }
  out[0].done = true;
  out[0].tags.push("a", ...["b"]);
  out[1] = { id: 9, done: true, tags: [] };
  out.sort((a, b) => a.id - b.id);
  const ids = out.map((t) => t.id);
  ids.sort();
  ids.length = 0;
  return out;
}"#,
    );
    assert_contains(f, "let mut out: tsx::List<Todo>");
    assert_contains(
        f,
        "let _target = &mut out;\n_target.extend_from(_items.as_slice());",
    );
    assert_contains(f, "out[tsx::index(0.0f64)].done = _value;");
    assert_contains(f, "let _target = &mut out[tsx::index(0.0f64)].tags;");
    assert_contains(f, "out.assign(_at, _value);");
    assert_contains(f, "_target.sort_js(");
    assert_contains(f, "_target.sort_default(");
    assert_contains(f, "ids.clear();");
}

#[test]
fn changes_that_typescript_would_share_are_reported() {
    for (src, expected) in [
        (
            "function f(xs: number[]): number { xs.push(1); return 1; }",
            "may hold the same value",
        ),
        (
            "function f(): number { const a: number[] = [1]; const b = a; b.push(2); return 1; }",
            "may hold the same value",
        ),
        (
            "function g(xs: number[]): number { return 1; }\nfunction f(): number { const a: number[] = []; g(a); a.push(2); return 1; }",
            "already used as a whole value",
        ),
        (
            "interface T { done: boolean }\nfunction f(ts: T[]): number { const all: T[] = [...ts]; for (const t of all) { t.done = true; } return 1; }",
            "`t` may hold the same value",
        ),
        (
            "function f(): number { const a: number[] = [1]; a.length = 1; return 1; }",
            "only `length = 0`",
        ),
    ] {
        let message = diagnostic(src);
        assert!(message.contains(expected), "{src}: {message}");
    }
}

#[test]
fn projections_are_not_escapes() {
    // reading a field, an item, the length or calling a non-mutating method
    // leaves the local unshared
    generated(
        "function f(): number { const a: number[] = [1]; const n = a.length + a[0] + a.indexOf(1); a.push(n); return a.length; }",
    );
}

#[test]
fn fresh_temporaries_and_copies() {
    // `[...xs].sort()` sorts a copy; spreading numbers copies them, so `xs`
    // may still change afterwards; numbers join through String(…)
    let rust = generated(
        "function f(): string { const xs: number[] = [3, 1, 2]; const sorted = [...xs].sort((a, b) => a - b); xs.push(4); return sorted.join(\",\") + xs.join(\"-\"); }",
    );
    assert_contains(&rust, "let mut _receiver: tsx::List<f64>");
    assert_contains(&rust, "_target.sort_js(");
    assert_contains(&rust, "tsx::builtins::string(");
    // a shared array is not a temporary
    let message = diagnostic(
        "function f(xs: number[]): number { const ys = xs; return ys.reverse().length; }",
    );
    assert!(message.contains("may hold the same value"), "{message}");
    // spreading objects shares them, so later changes are reported
    let message = diagnostic(
        "interface T { done: boolean }\nfunction f(): number { const xs: T[] = []; const copy = [...xs]; xs.push({ done: true }); return copy.length; }",
    );
    assert!(
        message.contains("already used as a whole value"),
        "{message}"
    );
}
