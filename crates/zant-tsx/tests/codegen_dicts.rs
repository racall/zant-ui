//! Records, index signatures, Map and Set (`codegen::dicts`) on the
//! runtime's insertion-ordered `tsx::Dict` / `tsx::Set`.

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

fn assert_has(rust: &str, needles: &[&str]) {
    let flat = squash(rust);
    for needle in needles {
        assert!(
            flat.contains(&squash(needle)),
            "missing `{needle}` in:\n{rust}"
        );
    }
}

#[test]
fn records_and_index_signatures() {
    let rust = compile_module(
        r#"type Status = "open" | "done";
interface Scores { [name: string]: number }
const labels: Record<Status, string> = { open: "Open", done: "Done" };
function tally(words: string[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const w of words) { counts[w] = (counts[w] ?? 0) + 1; }
  delete counts["x"];
  return counts;
}
function report(scores: Scores, s: Status): string {
  let out = labels[s] + labels.open;
  for (const [name, score] of Object.entries(scores)) { out += name + String(score); }
  return out + String(Object.keys(scores).length) + String("ada" in scores);
}"#,
    )
    .unwrap();
    assert_has(
        &rust,
        &[
            "pub fn tally(words: tsx::List<zant::Str>) -> tsx::Dict<zant::Str, f64>",
            "pub fn report(scores: tsx::Dict<zant::Str, f64>",
            ".insert(_key, _value);",
            ".remove(&_key);",
            ".entries()",
            ".contains(&",
            // a union-keyed record read is the value itself
            ".expect(\"the record has every key of its union\")",
        ],
    );
}

#[test]
fn maps_and_sets() {
    let rust = compile_module(
        r#"function demo(): number {
  const m = new Map<string, number>();
  m.set("a", 1);
  m.delete("a");
  let total = 0;
  for (const [k, v] of m) { total += v; }
  const seen = new Set<number>([1, 2, 2]);
  seen.add(3);
  const copy = new Map([["x", 1], ["y", 2]]);
  return total + m.size + seen.size + (copy.get("y") ?? 0) + [...m.keys()].length;
}"#,
    )
    .unwrap();
    assert_has(
        &rust,
        &[
            "tsx::Dict::new()",
            "tsx::Set::from_values(",
            "tsx::Dict::from_entries(",
            "_target.remove(&_key)",
            ".len()",
        ],
    );
}

#[test]
fn dictionary_mistakes_are_reported() {
    for (src, expected) in [
        (
            "type K = \"a\" | \"b\";\nconst r: Record<K, number> = { a: 1 };",
            "missing key `b`",
        ),
        (
            "function f(r: Record<number, string>): number { return 1; }",
            "record keys are strings",
        ),
        (
            "function f(r: Record<string, number>): number { for (const x of r) {} return 1; }",
            "a record is not iterable",
        ),
        (
            "function f(): number { const m = new Map(); return 1; }",
            "give the map its types",
        ),
        (
            "function f(r: Record<string, number>): string { return String(r[\"a\"]); }",
            "give it a fallback",
        ),
    ] {
        let message = compile_module(src).unwrap_err();
        assert!(message.contains(expected), "{src}: {message}");
    }
}
