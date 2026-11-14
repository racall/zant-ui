//! Port of `src/core/layout.zig` — WP-1.3: CSS-style flex layout of the
//! retained tree (intrinsic sizes, containers, rows, columns, wrap).
//!
//! Split by layout stage: [`intrinsic`] (min/max-content measures),
//! [`container`] (the recursive entry point and absolute positioning),
//! [`row`], [`column`] and [`wrap`].
//!
//! **Decomposition choice (plan.md §4).** The Zig layout functions take
//! `[]*Node` plus a `*Node`; Rust cannot hand out `&mut RetainedNode` across a
//! recursive call into the same `Vec`. The internal functions therefore take
//! `&mut Nodes<'_>` — a thin wrapper around `&mut [RetainedNode]` that indexes
//! by [`NodeId`] — so the translation keeps Zig's shape (`nodes[id].bounds`),
//! and each slot is re-borrowed after every recursive call. The public entry
//! points still take `&mut [RetainedNode]` + id as the contract asks.
#![forbid(unsafe_code)]

use std::ops::{Index, IndexMut};

use crate::core::node::{NodeId, RetainedNode};

pub mod column;
pub mod container;
pub mod intrinsic;
pub mod row;
pub mod wrap;

pub use container::{layout_tree, translate_subtree_x, translate_subtree_y};

/// A mutable view of the node arena indexed by [`NodeId`] (see module docs).
pub struct Nodes<'a> {
    pub slice: &'a mut [RetainedNode],
}

impl<'a> Nodes<'a> {
    pub fn new(slice: &'a mut [RetainedNode]) -> Nodes<'a> {
        Nodes { slice }
    }

    pub fn len(&self) -> usize {
        self.slice.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, RetainedNode> {
        self.slice.iter_mut()
    }
}

impl Index<NodeId> for Nodes<'_> {
    type Output = RetainedNode;

    fn index(&self, id: NodeId) -> &RetainedNode {
        &self.slice[id as usize]
    }
}

impl IndexMut<NodeId> for Nodes<'_> {
    fn index_mut(&mut self, id: NodeId) -> &mut RetainedNode {
        &mut self.slice[id as usize]
    }
}
