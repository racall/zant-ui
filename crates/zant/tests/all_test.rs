//! Port of the two inline tests of `tests/all.zig` (the Zig test harness
//! smoke test and the `zant.unicode` helper assertions).

use zant::core::unicode;

#[test]
fn test_harness_starts() {
    // Zig: `try std.testing.expect(true)`; a trivially true check here too.
    assert_eq!(1 + 1, 2);
}

#[test]
fn unicode_helpers() {
    assert_eq!(3, unicode::next_grapheme("中A", 0));
    assert_eq!(1, unicode::byte_to_utf16("中A", 3));
}
