//! Shared diagnostic vocabulary for string lowering and rejected regex
//! arguments.
//!
//! `expr_parser` reports `"a".replace(/re/, ..)` as an unsupported regex
//! overload naming the methods it does accept; the code generator reuses the
//! same list for its "unknown string method" diagnostic.

/// String methods supported by the TSX subset.
pub const SUPPORTED: &str = "includes, startsWith, endsWith, indexOf, lastIndexOf, trim, trimStart, trimEnd, toLowerCase, toUpperCase, slice, substring, padStart, padEnd, repeat, replace, replaceAll, split, charAt, at, charCodeAt, codePointAt";

#[cfg(test)]
mod tests {
    use super::*;

    /// The diagnostic text is user visible: the list is pinned method by
    /// method.
    #[test]
    fn supported_lists_every_string_method_in_order() {
        assert_eq!(
            vec![
                "includes",
                "startsWith",
                "endsWith",
                "indexOf",
                "lastIndexOf",
                "trim",
                "trimStart",
                "trimEnd",
                "toLowerCase",
                "toUpperCase",
                "slice",
                "substring",
                "padStart",
                "padEnd",
                "repeat",
                "replace",
                "replaceAll",
                "split",
                "charAt",
                "at",
                "charCodeAt",
                "codePointAt",
            ],
            SUPPORTED.split(", ").collect::<Vec<_>>()
        );
    }
}
