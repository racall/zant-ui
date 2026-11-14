//! Port of `src/core/tree.zig` — WP-1.3: the retained node tree.
//!
//! A `Tree` owns a node arena (`nodes: Vec<RetainedNode>`), the recycling list
//! of freed slots (`free_ids`), the key index, the set of nodes with a running
//! transition (`animating`) and the focus / theme state. `NodeId` is an index
//! into `nodes`; freed slots stay as tombstones (`alive == false`) and their
//! ids are reused, exactly like the Zig arena.
//!
//! **Hard rule (plan.md §4):** no `&mut RetainedNode` is held across a tree
//! mutation. Methods re-borrow `self.nodes[id]` after every call; recursive
//! helpers copy the child-id list first. The implementation is split by
//! concern: allocation / builders / dirty state in [`alloc`], painting in
//! [`paint`], hit testing in [`hit`], scrolling in [`scroll`], focus in
//! [`focus`] and text / input / toggle state changes in [`input_ops`].
#![forbid(unsafe_code)]

use std::collections::HashMap;

use crate::core::geometry::{Rect, Size};
use crate::core::layout;
use crate::layout::flex;
use crate::motion::motion;
use crate::theme::tokens::Theme;

pub mod alloc;
pub mod focus;
pub mod hit;
pub mod input_ops;
pub mod paint;
pub mod scroll;

pub use crate::core::node::{
    ButtonData, ButtonIcon, ButtonSize, ButtonType, ContainerData, DirtyFlags, FlexStyle,
    HoverVisual, Icon, ImageData, ImageFit, InputData, MotionStyle, NodeAnim, NodeData, NodeId,
    NodeKind, PointerArea, RetainedNode, RichSpan, RichTextData, SelectData, SelectOption,
    TextData, ToggleData, ToggleKind, VectorData, Visual,
};
pub use crate::core::tree::input_ops::Move;

/// Zig `Tree`.
pub struct Tree {
    /// Node slots. Freed slots become tombstones (`alive == false`) and their
    /// ids are reused through `free_ids`.
    pub nodes: Vec<RetainedNode>,
    pub free_ids: Vec<NodeId>,
    /// Nodes with a running transition (see [`Tree::poke`]); the runtime
    /// advances only these each frame instead of scanning the whole tree.
    pub animating: Vec<NodeId>,
    /// Frame timestamp (ms) sampled by the runtime.
    pub now_ms: i64,
    /// Layout transitions may start (off for the frame of a window resize).
    pub flip_enabled: bool,
    pub root: NodeId,
    pub focused: Option<NodeId>,
    /// The last input was the keyboard, so focus changes show a focus ring on
    /// buttons and toggles (the browser's `:focus-visible` heuristic that antd
    /// relies on). The runtime sets it on key presses and clears it on pointer
    /// presses.
    pub keyboard_modality: bool,
    pub theme: Theme,
    /// Node by key (the most recently keyed node for a key); lookups fall
    /// back to a scan when the indexed node is not usable.
    pub key_index: HashMap<String, NodeId>,
    /// Fill behind selected text (`RetainedNode::text_sel`).
    pub selection_color: u32,
}

impl Tree {
    pub fn init(theme: Theme) -> Tree {
        let mut tree = Tree {
            nodes: Vec::new(),
            free_ids: Vec::new(),
            animating: Vec::new(),
            now_ms: 0,
            flip_enabled: true,
            root: 0,
            focused: None,
            keyboard_modality: false,
            theme,
            key_index: HashMap::new(),
            selection_color: 0x4016_77ff,
        };
        let root = tree.create_node(NodeData::Root);
        tree.root = root;
        tree.get_mut(root).style.direction = flex::Direction::Column;
        tree
    }

    /// Zig `Tree.deinit`: Rust frees the nodes on `Drop`; kept so ported call
    /// sites compile unchanged.
    pub fn deinit(&mut self) {}

    pub fn get(&self, id: NodeId) -> &RetainedNode {
        &self.nodes[id as usize]
    }

    pub fn get_mut(&mut self, id: NodeId) -> &mut RetainedNode {
        &mut self.nodes[id as usize]
    }

    /// Lays the tree out at `viewport`, with FLIP layout transitions.
    pub fn layout(&mut self, viewport: Size) {
        // FLIP: remember where layout-transition nodes were painted, lay out,
        // then animate from the old box to the new one
        for id in 0..self.nodes.len() {
            if !self.nodes[id].alive || self.nodes[id].motion.layout_transition.is_none() {
                continue;
            }
            if self.nodes[id].anim.is_none() {
                continue;
            }
            let id = id as NodeId;
            let rel = self.relative_bounds(id);
            let anim = self.nodes[id as usize]
                .anim
                .as_mut()
                .expect("checked above");
            for ((prev, &rel_value), &delta) in anim
                .flip_prev
                .iter_mut()
                .zip(rel.iter())
                .zip(anim.flip_delta.iter())
            {
                *prev = rel_value + delta;
            }
        }
        layout::layout_tree(&mut self.nodes, self.root, viewport, self.theme);
        let now = self.now_ms;
        for id in 0..self.nodes.len() {
            if !self.nodes[id].alive {
                continue;
            }
            let Some(tr) = self.nodes[id].motion.layout_transition else {
                continue;
            };
            let id = id as NodeId;
            let rel = self.relative_bounds(id);
            let flip_enabled = self.flip_enabled;
            let animations_on = motion::enabled() && flip_enabled;
            let moved = {
                let Some(anim) = self.ensure_anim(id) else {
                    continue;
                };
                let mut moved = false;
                if anim.flip_ready && animations_on {
                    let mut from = [0.0f32; 4];
                    for k in 0..4 {
                        from[k] = anim.flip_prev[k] - rel[k];
                        if from[k].abs() > 0.5 {
                            moved = true;
                        }
                    }
                    if moved {
                        anim.flip_from = from;
                        anim.flip_delta = from;
                        anim.flip = motion::Animated::start(1.0, 0.0, now, tr);
                        anim.flip_active = true;
                    }
                } else if !flip_enabled {
                    anim.flip = motion::Animated::at(0.0);
                    anim.flip_active = false;
                    anim.flip_delta = [0.0; 4];
                }
                anim.flip_ready = true;
                moved
            };
            if moved {
                self.poke(id);
            }
        }
    }

    /// Bounds relative to the parent (x, y, w, h): scrolling or moving an
    /// ancestor does not count as a layout change.
    fn relative_bounds(&self, id: NodeId) -> [f32; 4] {
        let node = &self.nodes[id as usize];
        let mut x = node.bounds.x;
        let mut y = node.bounds.y;
        if let Some(parent) = node.parent {
            let parent = &self.nodes[parent as usize];
            x -= parent.bounds.x;
            y -= parent.bounds.y;
        }
        [x, y, node.bounds.width, node.bounds.height]
    }

    /// Animation state of `id`, allocated on first use.
    pub fn ensure_anim(&mut self, id: NodeId) -> Option<&mut NodeAnim> {
        let index = id as usize;
        if self.nodes[index].anim.is_none() {
            self.nodes[index].anim = Some(NodeAnim::default());
        }
        self.nodes[index].anim.as_mut()
    }

    /// The node Zig's tree exposes as `Node` for the current theme.
    pub fn visual_of(&self, id: NodeId) -> Visual {
        self.nodes[id as usize].visual
    }

    /// The bounds of `id` as a `Rect`.
    pub fn bounds(&self, id: NodeId) -> Rect {
        self.nodes[id as usize].bounds
    }

    /// The kind of `id`.
    pub fn kind(&self, id: NodeId) -> NodeKind {
        self.nodes[id as usize].data.kind()
    }

    /// A flat list of the live node ids (debug / tests).
    pub fn live_ids(&self) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|node| node.alive)
            .map(|node| node.id)
            .collect()
    }
}

impl Default for Tree {
    fn default() -> Self {
        Tree::init(Theme::default())
    }
}
