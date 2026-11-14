//! Port of `tests/flex_test.zig` (24 cases) — deferred to WP-2.10.
//!
//! Every case in the Zig file is a retained-tree integration test
//! (`tree.addContainer` / `tree.layout` / `tree.addButton` / `tree.addRichText`
//! / `tree.hitTest` …), which needs `core::tree`, `core::node`, `core::layout`
//! and the text renderer (WP-1.1c / 1.2 / 1.3). WP-1.1a only owns
//! `src/layout/flex.zig`, whose five unit tests live in
//! `crates/zant/src/layout/flex.rs`. The 24 tree-level cases are tracked for
//! WP-2.10 in `docs/rust-port/status.md`.
