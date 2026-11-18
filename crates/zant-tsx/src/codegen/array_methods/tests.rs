//! Unit tests.

use super::*;

// The method table is pinned.
#[test]
fn the_method_table_is_complete() {
    for name in [
        "map",
        "filter",
        "find",
        "findIndex",
        "some",
        "every",
        "reduce",
        "indexOf",
        "includes",
        "slice",
        "join",
        "concat",
    ] {
        assert!(lookup(name.as_bytes()).is_some(), "{name}");
    }
    assert!(lookup(b"forEach").is_none());
}
