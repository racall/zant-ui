//! Unit tests.

use super::capture_count;

#[test]
fn capture_groups_are_counted_like_the_engine() {
    assert_eq!(0, capture_count(r"a+b"));
    assert_eq!(2, capture_count(r"(\d+)-(\d+)"));
    assert_eq!(1, capture_count(r"(?:x)(?<name>y)(?=z)(?!w)(?<=v)(?<!u)"));
    assert_eq!(1, capture_count(r"\((a)\)[(]"));
}
