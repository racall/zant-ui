//! `ui::memo` — the memoized-subtree builder.
//!
//! `memo` returns a container placeholder carrying `Element::memo_hash` and
//! `Element::memo_build`; it does **not** run the builder. The reconciler
//! (`runtime/reconcile.rs`) compares the hash with the mounted node's
//! `memo_hash`:
//!
//! - equal → keep the mounted subtree and `local_state::Store::touch` the ids
//!   its memo build recorded last time (so skipped components keep their
//!   state);
//! - different / first mount → run the closure (a `MemoBuild`, which
//!   captures its own context), reconcile its result
//!   while `Store::record`/`end_record` records the local-state ids it asked
//!   for, and store the new hash.
//!
//! This module is the builder side only: the helpers below
//! ([`memo_hash`], [`memo_build`], [`resolve_memo`]) are what the runtime
//! uses, so the runtime does not reimplement the memo expansion rule.
#![forbid(unsafe_code)]

use crate::core::node::NodeKind;

use super::element::{Element, MemoBuild, Node, with_key};
use super::options::MemoOptions;

impl From<u64> for MemoOptions {
    /// `ui::memo(hash, build)` call shape (hash-only memo), a shorthand for
    /// the full `MemoOptions`.
    fn from(hash: u64) -> MemoOptions {
        MemoOptions { key: None, hash }
    }
}

/// React.memo for Zant: `build` runs only when no mounted node with this
/// position / key has the same `hash`; otherwise the existing subtree (layout,
/// text shaping, callbacks) is kept untouched.
///
/// `build` must return a container (`column` / `row`); every callback context
/// inside the subtree must outlive the frame. The closure captures whatever
/// context it needs.
///
/// Accepts the option-struct shape and the hash-only shorthand:
///
/// ```ignore
/// ui::memo(ui::MemoOptions { key: Some("answer".into()), hash: 7 }, || render_answer(&cache));
/// ui::memo(7u64, || render_answer(&cache));
/// ```
pub fn memo(options: impl Into<MemoOptions>, build: impl Fn() -> Node + 'static) -> Node {
    let options = options.into();
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        // translated text inside must rebuild after `setLocale` / a new
        // catalog; revision 0 (no translations) keeps the hash as given
        memo_hash: Some(
            options.hash ^ crate::i18n::context::revision().wrapping_mul(0x9e37_79b9_7f4a_7c15),
        ),
        memo_build: Some(MemoBuild::new(build)),
        ..Element::default()
    })
}

/// The memo hash of `node`, if it is a `ui::memo` placeholder (runtime).
pub fn memo_hash(node: &Node) -> Option<u64> {
    node.element().memo_hash
}

/// The builder of a `ui::memo` placeholder, cloned out of `node` (runtime).
///
/// The `Rc` clone lets the reconciler run the closure after it has stopped
/// borrowing the old node; the closure itself must not hold the build-context
/// borrow (it calls `ui::*` like any render function).
pub fn memo_build(node: &Node) -> Option<MemoBuild> {
    node.element().memo_build.clone()
}

/// Expands a `ui::memo` placeholder into its built subtree: runs the builder
/// and keeps the memo node's key on the built root when the root is not keyed
/// itself.
pub fn resolve_memo(memo_spec: Node) -> Node {
    let Some(build) = memo_build(&memo_spec) else {
        return memo_spec;
    };
    let memo_key = memo_spec.element().key.clone();
    let built = build.build();
    match memo_key {
        Some(key) if built.element().key.is_none() => with_key(built, key),
        _ => built,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::ui::builders::text;
    use crate::ui::options::TextOptions;

    fn text_with_key(key: &'static str) -> Node {
        text(
            key,
            TextOptions {
                key: Some(key.into()),
                ..Default::default()
            },
        )
    }

    fn text_plain(value: &'static str) -> Node {
        text(value, TextOptions::default())
    }

    #[test]
    fn memo_builder_stores_hash_key_and_closure() {
        let node = memo(
            MemoOptions {
                key: Some("block".into()),
                hash: 0xAB,
            },
            || text_with_key("built"),
        );
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("block"));
        assert_eq!(element.memo_hash, Some(0xAB));
        assert!(element.memo_build.is_some());
        assert!(
            element.children.is_empty(),
            "the placeholder has no children; the reconciler expands it"
        );
        assert_eq!(element.layout.shrink, 1.0, "the struct default");
    }

    #[test]
    fn memo_accepts_the_hash_only_shorthand() {
        let node = memo(7u64, || text_with_key("built"));
        assert_eq!(node.element().memo_hash, Some(7));
        assert!(node.element().key.is_none());
        assert!(node.element().memo_build.is_some());
    }

    #[test]
    fn memo_build_runs_the_closure_on_every_call() {
        let calls = Rc::new(Cell::new(0));
        let counted = Rc::clone(&calls);
        let node = memo(1u64, move || {
            counted.set(counted.get() + 1);
            text_with_key("built")
        });
        let build = memo_build(&node).expect("a memo builder");
        assert_eq!(build.build().element().content, "built");
        assert_eq!(build.build().element().content, "built");
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn resolve_memo_builds_and_keeps_the_memo_key() {
        let node = memo(
            MemoOptions {
                key: Some("memo-key".into()),
                hash: 3,
            },
            || text_plain("built"),
        );
        let resolved = resolve_memo(node);
        assert_eq!(resolved.element().kind, NodeKind::Text);
        assert_eq!(resolved.element().content, "built");
        assert_eq!(
            resolved.element().key.as_deref(),
            Some("memo-key"),
            "the memo key moves to the built root"
        );
        // the placeholder itself is unchanged
        assert!(resolved.element().memo_build.is_none());
    }

    #[test]
    fn resolve_memo_keeps_a_key_the_build_already_set() {
        let node = memo(4u64, || text_with_key("own-key"));
        let resolved = resolve_memo(node);
        assert_eq!(resolved.element().key.as_deref(), Some("own-key"));
    }

    #[test]
    fn resolve_memo_returns_plain_nodes_unchanged() {
        let node = text_with_key("plain");
        let resolved = resolve_memo(node.clone());
        assert!(node.ptr_eq(&resolved), "no memo build means no expansion");

        let unkeyed_memo = memo(5u64, || text_plain("built"));
        let resolved = resolve_memo(unkeyed_memo);
        assert!(
            resolved.element().key.is_none(),
            "there is no memo key to move"
        );
        assert_eq!(resolved.element().content, "built");

        // properties of the helpers on a non-memo node
        assert_eq!(memo_hash(&node), None);
        assert!(memo_build(&node).is_none());
    }
}
