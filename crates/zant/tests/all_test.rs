//! Test harness smoke test and the `zant::core::unicode` helper assertions.

use zant::core::unicode;

#[test]
fn test_harness_starts() {
    // a trivially true check
    assert_eq!(1 + 1, 2);
}

#[test]
fn unicode_helpers() {
    assert_eq!(3, unicode::next_grapheme("中A", 0));
    assert_eq!(1, unicode::byte_to_utf16("中A", 3));
}
