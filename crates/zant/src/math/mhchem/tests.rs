//! Tests: the 823-case upstream fixture and the basics.

use serde::Deserialize;

use super::{FIXTURE_JSON, Machine, to_tex};

#[derive(Deserialize)]
struct Case {
    input: String,
    machine: String,
    /// `null` == mhchem.js throws
    expected: Option<String>,
}

#[test]
fn mhchem_fixture_generated_by_the_real_mhchem_js() {
    let cases: Vec<Case> = serde_json::from_str(FIXTURE_JSON).expect("mhchem_cases.json");
    assert!(cases.len() >= 300);

    let mut failures: usize = 0;
    let mut passed: usize = 0;
    for c in &cases {
        let machine = if c.machine == "pu" {
            Machine::Pu
        } else {
            Machine::Ce
        };
        let actual: Option<String> = to_tex(&c.input, machine).ok();
        let same = match (&actual, &c.expected) {
            (Some(actual), Some(expected)) => actual == expected,
            (None, None) => true,
            _ => false,
        };
        if same {
            passed += 1;
        } else {
            failures += 1;
            println!(
                "mhchem mismatch (\\{}):\n  input:    {}\n  expected: {:?}\n  actual:   {:?}",
                c.machine, c.input, c.expected, actual
            );
        }
    }
    println!("mhchem fixture: {passed}/{} cases pass", cases.len());
    assert_eq!(cases.len(), 823);
    assert_eq!(
        failures,
        0,
        "{failures} of {} mhchem cases failed",
        cases.len()
    );
}

#[test]
fn mhchem_basics() {
    assert_eq!(to_tex("", Machine::Ce).unwrap(), "");
    assert_eq!(
        to_tex("H2O", Machine::Ce).unwrap(),
        "{\\mathrm{H}{\\vphantom{X}}_{\\smash[t]{2}}\\mathrm{O}}"
    );
    assert_eq!(
        to_tex("123 kJ mol-1", Machine::Pu).unwrap(),
        "{123~\\mathrm{kJ}\\mkern3mu \\mathrm{mol^{-1}}}"
    );
    assert!(to_tex("A\\bond{x}B", Machine::Ce).is_err());
}
