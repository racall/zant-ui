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

use std::cell::RefCell;
use std::ops::{Deref, DerefMut, Index, IndexMut};

use crate::core::node::{NodeId, RetainedNode};

pub mod column;
pub mod container;
pub mod intrinsic;
pub mod row;
pub mod wrap;

pub use container::{layout_tree, translate_subtree_x, translate_subtree_y};

/// Reusable scratch buffers: layout runs every frame and Zig used arena/stack
/// storage for these lists, so the Rust port caches them per thread instead of
/// allocating a fresh `Vec` for every container / row / column (WP-2.11
/// follow-up: the `stream` bench scenario had its margin eaten by allocator
/// traffic).
const SCRATCH_POOL_MAX: usize = 64;

thread_local! {
    static IDS_POOL: RefCell<Vec<Vec<NodeId>>> = const { RefCell::new(Vec::new()) };
    static ITEMS_POOL: RefCell<Vec<Vec<crate::layout::flex::Item>>> = const { RefCell::new(Vec::new()) };
    static FLOATS_POOL: RefCell<Vec<Vec<f32>>> = const { RefCell::new(Vec::new()) };
}

macro_rules! scratch_pool {
    ($vis:vis $name:ident, $ty:ty, $pool:ident) => {
        /// A buffer taken from the layout scratch pool; returns itself on drop.
        $vis struct $name(Vec<$ty>);

        impl $name {
            pub(crate) fn take() -> $name {
                let mut v = $pool.with(|pool| pool.borrow_mut().pop()).unwrap_or_default();
                v.clear();
                $name(v)
            }
        }

        impl Deref for $name {
            type Target = Vec<$ty>;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                let value = std::mem::take(&mut self.0);
                $pool.with(|pool| {
                    let mut pool = pool.borrow_mut();
                    if pool.len() < SCRATCH_POOL_MAX {
                        pool.push(value);
                    }
                });
            }
        }
    };
}

scratch_pool!(pub(crate) ScratchIds, NodeId, IDS_POOL);
scratch_pool!(pub(crate) ScratchItems, crate::layout::flex::Item, ITEMS_POOL);
scratch_pool!(pub(crate) ScratchFloats, f32, FLOATS_POOL);

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
