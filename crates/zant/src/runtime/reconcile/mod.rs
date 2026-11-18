//! Reconciling the declarative tree into the retained tree.
//!
//! Matching rules: keyed specs only match a node with the
//! same key and kind; unkeyed specs match the next unused unkeyed node in
//! order (nodes playing an exit transition are never reused); unmatched live
//! nodes are freed, or play their presence exit transition first and are freed
//! by the frame tick when it ends.
//!
//! New nodes are created here as well (`create_node`), including the
//! `ui::memo` expansion and the local-state recording that keeps the state of
//! skipped memo subtrees alive.
#![forbid(unsafe_code)]

use std::collections::HashMap;

use crate::Str;
use crate::core::node::{NodeId, NodeKind};
use crate::motion::motion;
use crate::ui::element::Node;
use crate::ui::memo::resolve_memo;

use super::Runtime;
use super::patch::apply_selection;

mod create;
mod lifecycle;
mod matching;

/// Finds the old child a keyed spec matches without scanning every old child
/// per spec (that was quadratic in long keyed lists).
///
/// Keyed children usually keep their order, so the old child right after the
/// previous keyed match is tried first; the first miss builds a key → index
/// map of the old children (the "map of remaining children" of React / Vue /
/// Inferno). Duplicate keys or a kind mismatch fall back to the full scan.
#[derive(Default)]
struct KeyedLookup {
    next: usize,
    by_key: Option<HashMap<Str, usize>>,
}

/// The retained node must have the same kind as the spec.
fn kind_compatible(node_kind: NodeKind, spec_kind: NodeKind) -> bool {
    node_kind == spec_kind
}

impl Runtime {}

#[cfg(test)]
mod tests;
