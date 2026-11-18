//! Unit tests.

use super::*;
use crate::codegen::Generator;

fn parse(src: &str) -> ast::Module {
    let mut parser = crate::parser::Parser::init(src);
    parser.parse_module().expect("test source parses")
}

fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    f(&mut g);
}

#[test]
fn component_names_start_with_an_uppercase_letter() {
    let module = parse(
        r#"function bytes(n: number): number { return n; }
function Row(props: number): void {}"#,
    );
    assert!(is_function(&module.functions[0].func));
    assert!(!is_function(&module.functions[1].func));
}

#[test]
fn required_stops_at_the_first_optional_or_defaulted_parameter() {
    let module = parse(
        r#"function label(n: number, unit = "item", extra?: boolean): string { return unit; }"#,
    );
    let params = &module.functions[0].func.params;
    assert_eq!(1, required(params));
    assert!(params[1].default.is_some());
    assert!(params[2].optional);
}

#[test]
fn a_missing_optional_argument_is_none_and_a_default_is_emitted() {
    let module = parse(
        r#"function label(n: number, unit = "item", extra?: boolean): string { return unit; }"#,
    );
    let params = &module.functions[0].func.params;
    with_generator(|g| {
        assert_eq!("None", missing(g, &params[2], Type::Boolean).unwrap());
        assert_eq!(
            "zant::Str::from(\"item\")",
            missing(g, &params[1], Type::String).unwrap()
        );
    });
}

#[test]
fn duplicate_and_untyped_functions_are_diagnostics() {
    let module = parse(
        r#"function twice(n): number { return n; }
function twice(n: number): number { return n; }"#,
    );
    with_generator(|g| {
        assert!(collect(g, &module).is_err());
        let diag = g.diag.as_ref().expect("a diagnostic");
        assert!(
            diag.msg.contains("parameter `n` of `twice` needs a type"),
            "{}",
            diag.msg
        );
    });
}
