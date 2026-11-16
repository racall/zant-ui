//! `RegExp` (`codegen::regexps`) and flow narrowing of `let` locals. The
//! run-time behavior is compared with Node by `scripts/tsx/node-parity.sh`
//! (`tests/node_parity/regexp.ts`).

use zant_tsx::compile;

fn compile_module(src: &str) -> Result<String, String> {
    let source = format!(
        "import {{ View }} from \"zant\";\n{src}\nexport default function App() {{ return <View/>; }}\n"
    );
    compile::compile(&source, "probe.tsx").map_err(|diag| diag.msg)
}

#[test]
fn literals_are_checked_and_compiled_once_per_site() {
    let rust = compile_module(
        r#"function digits(s: string): boolean { return /^\d+$/.test(s); }
function pattern(text: string): RegExp { return new RegExp(text, "g"); }"#,
    )
    .unwrap();
    assert!(
        rust.contains("thread_local!") && rust.contains("RegExp::fresh"),
        "{rust}"
    );
    // a computed pattern can throw a SyntaxError
    assert!(
        rust.contains(
            "pub fn pattern(text: zant::Str) -> Result<zant::tsx::RegExp, zant::tsx::Thrown>"
        ),
        "{rust}"
    );
    let message =
        compile_module("function f(s: string): boolean { return /(a/.test(s); }").unwrap_err();
    assert!(
        message.contains("Invalid regular expression: /(a/"),
        "{message}"
    );
    let message =
        compile_module("function f(s: string): number { return [...s.matchAll(/a/)].length; }")
            .unwrap_err();
    assert!(
        message.contains("matchAll needs a global pattern"),
        "{message}"
    );
    let message =
        compile_module("function f(s: string, t: string): number { return s.search(t); }")
            .unwrap_err();
    assert!(message.contains("takes a pattern"), "{message}");
}

#[test]
fn a_let_is_narrowed_until_it_is_assigned() {
    // the regex loop: narrowed in the body until the reassignment
    compile_module(
        r#"function count(text: string): number {
  const re = /a/g;
  let n = 0;
  let m = re.exec(text);
  while (m !== null) {
    n += m.index;
    m = re.exec(text);
  }
  return n;
}"#,
    )
    .unwrap();
    // read after the assignment: widened again (TypeScript reports it too)
    let message = compile_module(
        r#"function f(s: string): number {
  let m = s.match(/a/);
  if (m) {
    m = s.match(/b/);
    return m.index;
  }
  return 0;
}"#,
    )
    .unwrap_err();
    assert!(message.contains("optional"), "{message}");
    // a loop that assigns it: the next iteration may read the new value
    let message = compile_module(
        r#"function f(s: string): number {
  let m = s.match(/a/);
  if (!m) { return 0; }
  let total = 0;
  for (let i = 0; i < 3; i++) {
    total += m.index;
    m = s.match(/b/);
  }
  return total;
}"#,
    )
    .unwrap_err();
    assert!(message.contains("optional"), "{message}");
}
