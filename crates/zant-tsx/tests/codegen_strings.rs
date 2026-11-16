//! String-method codegen:
//!
//! - the diagnostics that stop unsupported calls before invalid Rust is
//!   emitted (unknown names, arity, argument types, optional receivers,
//!   top-level constants); and
//! - the source-evaluation-order contract of [`tsx::strings`] helper
//!   temporaries (receiver and arguments run exactly once, left to right).
//!
//! A regex argument (`"a".replace(/a/, "b")`) is not listed here: the TSX
//! parser does not accept regex literals, so that case is a parse diagnostic.

use zant_tsx::compile;

const APP: &str = "import { useState, View, Text } from \"zant\";\n";

fn source(body: &str) -> String {
    format!("{APP}export default function App() {{\n{body}\n  return <View/>;\n}}\n")
}

#[test]
fn string_method_diagnostics_stop_unsupported_calls() {
    let top_level = format!(
        "{APP}const s = \"a\".trim();\nexport default function App() {{\n  return <View><Text>{{s}}</Text></View>;\n}}\n"
    );
    let cases: [(String, &str); 11] = [
        (
            source("  const x = \"a\".normalize();"),
            "unknown string method `normalize`; supported:",
        ),
        (
            source("  const x = \"a\".localeCompare(\"b\");"),
            "unknown string method",
        ),
        (
            source("  const x = \"a\".trim(1);"),
            "expects 0..0 arguments",
        ),
        (
            source("  const x = \"a\".replace(\"b\");"),
            "expects 2..2 arguments",
        ),
        (
            source("  const x = \"a\".includes(1);"),
            "cannot convert f64 to zant::Str",
        ),
        (
            source("  const x = \"a\".slice(\"b\");"),
            "cannot convert zant::Str to f64",
        ),
        (
            source("  const x = \"a\".padStart(2, 1);"),
            "cannot convert f64 to zant::Str",
        ),
        (
            source("  const x = \"a\".split(true);"),
            "cannot convert bool to zant::Str",
        ),
        (
            // `s?.trim()` is fine (an optional result); a plain call is not
            source("  const [s, setS] = useState<string | null>(null);\n  const x = s.trim();"),
            "optional value: call the method with `?.` (`value?.trim()`), or use ?? / !",
        ),
        (
            source("  const x: string = \"a\".at(9);"),
            "optional value needs a fallback with ?? or a non-null assertion !",
        ),
        (
            top_level,
            "string methods cannot run in a top-level constant",
        ),
    ];
    for (tsx, expected) in cases {
        let message = compile::compile(&tsx, "strings.tsx")
            .expect_err("the source must be rejected")
            .msg;
        assert!(
            message.contains(expected),
            "missing `{expected}` in `{message}` for:\n{tsx}"
        );
    }
}

#[test]
fn string_helper_temporaries_preserve_source_evaluation_order() {
    // one call each, receiver then arguments; the generated expression is the
    // `tsx::strings::replace(&_text, &_arg, &_arg_2)` helper call
    let tsx = concat!(
        "import { View, Text } from \"zant\";\n",
        "function source(): string { return \"abc\"; }\n",
        "function pattern(): string { return \"a\"; }\n",
        "function replacement(): string { return \"b\"; }\n",
        "export default function App() {\n",
        "  const result = source().replace(pattern(), replacement());\n",
        "  return <View><Text key=\"t\">{result}</Text></View>;\n",
        "}\n"
    );
    let generated = compile::compile(tsx, "order.tsx").expect("source order compiles");
    let compact: String = generated.split_whitespace().collect();

    let call = |name: &str| {
        compact.find(&format!("={name}();")).unwrap_or_else(|| {
            panic!("no `= {name}();` call in\n{compact}");
        })
    };
    assert!(call("source") < call("pattern"), "{compact}");
    assert!(call("pattern") < call("replacement"), "{compact}");
    assert_eq!(
        1,
        compact.matches("=source();").count(),
        "the receiver runs once\n{compact}"
    );
    assert_eq!(1, compact.matches("=pattern();").count(), "{compact}");
    assert_eq!(1, compact.matches("=replacement();").count(), "{compact}");
    assert!(
        compact.contains("tsx::strings::replace(&_text,&_arg,&_arg_2)"),
        "{compact}"
    );
}
