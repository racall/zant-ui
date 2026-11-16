//! `JSON.parse` / `JSON.stringify` (`codegen::json`): typed decoding, Node
//! output (compared with Node by `scripts/tsx/node-parity.sh`,
//! `tests/node_parity/json.ts`), and the diagnostics for what JSON cannot
//! hold.

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
fn types_get_json_impls_and_calls_decode_into_the_context_type() {
    let rust = compile_module(
        r#"interface Item { id: number; note?: string; owner: string | null }
type Mode = "light" | "dark";
function save(items: Item[], mode: Mode): string { return JSON.stringify({ items, mode }, null, 2); }
function load(text: string): Item[] { return JSON.parse(text) as Item[]; }
function annotated(text: string): Mode { const mode: Mode = JSON.parse(text); return mode; }"#,
    )
    .unwrap();
    let flat = squash(&rust);
    for needle in [
        "impl tsx::json::Json for Item",
        // a TS optional field is left out when absent; `| null` writes null
        ".field(\"id\", &self.id).optional(\"note\", &self.note).field(\"owner\", &self.owner)",
        "note: tsx::json::read_optional(value, \"note\", path)?,",
        "impl tsx::json::Json for Mode",
        "tsx::json::stringify_value(&",
        "tsx::json::parse_as::<tsx::List<Item>>(",
        "tsx::json::parse_as::<Mode>(",
        // a parse can throw: the functions propagate it
        "pub fn load(text: zant::Str) -> Result<tsx::List<Item>, zant::tsx::Thrown>",
    ] {
        assert!(
            flat.contains(&squash(needle)),
            "missing `{needle}` in {rust}"
        );
    }
}

#[test]
fn what_json_cannot_hold_is_a_diagnostic() {
    for (source, expected) in [
        (
            "function f(m: Map<string, number>): string { return JSON.stringify(m); }",
            "Object.fromEntries(map)",
        ),
        (
            "function f(s: Set<string>): string { return JSON.stringify(s); }",
            "[...set]",
        ),
        (
            "interface Op { run: (x: number) => number } function f(o: Op): string { return JSON.stringify(o); }",
            "functions are left out of JSON",
        ),
        (
            "function f(t: string): number { const v = JSON.parse(t); return 1; }",
            "JSON.parse needs the type to read",
        ),
        (
            "function f(x: number): string { return JSON.stringify(x, (k: string, v: number): number => v); }",
            "a replacer is not supported",
        ),
    ] {
        let message = compile_module(source).unwrap_err();
        assert!(message.contains(expected), "{source}: {message}");
    }
}

#[test]
fn global_number_constants() {
    let rust = compile_module(
        "function f(): number { return NaN + Infinity + Number.POSITIVE_INFINITY + Number.MIN_SAFE_INTEGER; }",
    )
    .unwrap();
    for needle in ["f64::NAN", "f64::INFINITY", "-9_007_199_254_740_991f64"] {
        assert!(rust.contains(needle), "missing `{needle}` in {rust}");
    }
}
