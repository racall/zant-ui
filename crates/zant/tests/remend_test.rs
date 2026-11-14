//! Conformance suite for `remend`: every `expect(remend(x)).toBe(y)` case
//! from Streamdown's `packages/remend/__tests__`.

use zant::markdown::remend::{Options, remend};

#[derive(serde::Deserialize)]
struct Case {
    file: String,
    input: String,
    expected: String,
}

#[test]
fn remend_conformance_with_streamdown_test_suite() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../../tests/fixtures/remend_cases.json"))
            .expect("remend fixture parses");
    assert!(cases.len() > 500);
    let mut failures = 0usize;
    for case in &cases {
        let out = remend(&case.input, &Options::default());
        if out != case.expected {
            failures += 1;
            if failures <= 40 {
                println!(
                    "[{}] input={:?}\n  expected={:?}\n  actual  ={:?}",
                    case.file, case.input, case.expected, out
                );
            }
        }
    }
    if failures > 0 {
        println!("remend: {failures}/{} cases differ", cases.len());
    }
    assert_eq!(0usize, failures);
}

#[test]
fn remend_leaves_brackets_inside_math_alone_no_link_sentinel() {
    let cases = [
        // formula still streaming
        ("区间 \\([m,n", "区间 \\([m,n"),
        ("\\[\n[a,b", "\\[\n[a,b"),
        ("$$\n[a,b", "$$\n[a,b\n$$"),
        // closed formula followed by more text
        (
            "\\(f:[0,1]\\to\\mathbb{R}\\) 且 \\([m,n\\) 上",
            "\\(f:[0,1]\\to\\mathbb{R}\\) 且 \\([m,n\\) 上",
        ),
        // a real link after a formula is still healed
        (
            "\\([a,b]\\) 见 [文档",
            "\\([a,b]\\) 见 [文档](streamdown:incomplete-link)",
        ),
    ];
    for (input, expected) in cases {
        let out = remend(input, &Options::default());
        assert_eq!(expected, out, "input={input:?}");
    }
}
