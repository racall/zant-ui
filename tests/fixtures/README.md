`remend_cases.json` holds every `expect(remend(input)).toBe(expected)` case
(string literals only, default options) from Streamdown's
`packages/remend/__tests__` (Apache-2.0, see THIRD_PARTY_NOTICES.md).
`crates/zant/tests/remend_test.rs` runs them against the Rust port.
