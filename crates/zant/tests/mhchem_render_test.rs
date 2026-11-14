//! Port of `tests/mhchem_render_test.zig` — every mhchem fixture output parses
//! and lays out without unknown commands or parse errors (the TeX layout
//! engine covers what mhchem emits).

use serde::Deserialize;

use zant::core::vector::Command;
use zant::math::layout::Options as MathOptions;
use zant::math::{self, mhchem};

#[derive(Deserialize)]
struct Case {
    input: String,
    machine: String,
    /// `null` == mhchem.js throws
    expected: Option<String>,
}

fn has_error(commands: &[Command], error_color: u32) -> bool {
    for command in commands {
        if let Command::Text(text) = command
            && text.color == error_color
        {
            return true;
        }
    }
    false
}

#[test]
fn mhchem_all_fixture_cases_lay_out_cleanly_via_ce_and_pu() {
    let cases: Vec<Case> =
        serde_json::from_str(mhchem::FIXTURE_JSON).expect("crates/zant/src/math/mhchem_cases.json");
    let error_color: u32 = 0xffff00ff;
    let mut failures: usize = 0;
    for case in &cases {
        let Some(exp) = case.expected.as_deref() else {
            continue;
        };
        // inputs KaTeX cannot render either: unknown macros, alignment
        // characters outside an environment, upstream's `\color{r}` bug
        let katex_errors = [
            "\\foo",
            "\\cf{",
            "\\x{",
            "\\alphaH",
            "\\color{r}",
            "\\color{\\red}",
            "&",
            "\\hline",
            "{_}",
            "{^}",
        ];
        if katex_errors.iter().any(|k| exp.contains(k)) {
            continue;
        }
        let src = format!("\\{}{{{}}}", case.machine, case.input);
        let drawing = math::layout::layout(
            &src,
            MathOptions {
                error_color,
                color: 0xff000000,
                ..Default::default()
            },
        );
        if has_error(&drawing.commands, error_color) {
            failures += 1;
            if failures <= 25 {
                println!("unrendered: {src}  ->  {exp}");
            }
        }
    }
    assert_eq!(failures, 0);
}
