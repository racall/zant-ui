//! Unit tests.

use super::{check_and_format, parse_and_format};

#[test]
fn unparsable_generated_code_is_reported_not_panicked() {
    assert_eq!(
        check_and_format("fn f() {}", "app.tsx").unwrap(),
        "fn f() {}\n"
    );
    let message = check_and_format("fn f( {", "app.tsx").unwrap_err();
    assert!(message.contains("internal compiler error"), "{message}");
    assert!(message.contains("zant-tsx bug"), "{message}");
    assert!(message.contains("ZANT_TSX_DUMP"), "{message}");
    // a snippet that does not parse is kept as written
    assert_eq!(parse_and_format("impl ("), "impl (");
}
