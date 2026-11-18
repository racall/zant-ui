//! Retained nodes: data of every node kind plus the per-node state the tree,
//! layout and painting share.
//!
//! The node arena itself lives in `core::tree`; this module only describes a
//! node. A `RetainedNode` is stored by value in `Tree::nodes`: a
//! `Vec<RetainedNode>` with `NodeId = u32` indices gives stable addressing
//! without pointers.
//!
//! Data types that the retained node owns (`ContainerData`, `ButtonData`,
//! `InputData`, `ToggleData`, `ImageData`, `SelectData`, `VectorData`) live
//! here; the painting and measuring
//! behaviour of the components stays in `components/*`. `TextData` and
//! `RichTextData` (with the layout caches described in AGENTS.md) are in the
//! sibling modules.
//!
//! `ButtonData` / `InputData` store the typed `ui::actions` callbacks.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Rect;
use crate::core::vector::DrawingRef;
use crate::motion::motion::{self, Animated};
use crate::platform::platform::Cursor;
use crate::renderer::text::{self as rtext, TextLayout};
use crate::ui::actions::{
    Action, BoolAction, IndexAction, InputKeyAction, KeyAction, RangeAction, TextAction,
};

pub mod data;
pub mod rich_text;
pub mod text;

pub use data::{
    ButtonIcon, ButtonSize, ButtonType, DirtyFlags, FlexStyle, HoverVisual, Icon, ImageFit,
    MotionStyle, NodeId, NodeKind, PointerArea, RichSpan, ToggleKind, Visual,
};
pub use rich_text::RichTextData;
pub use text::TextData;

mod controls;
mod media;
mod resize;
mod retained;

pub use self::controls::*;
pub use self::media::*;
pub use self::resize::ResizeStep;
pub use self::retained::*;

/// Per-node animation state, allocated lazily (`Tree::ensure_anim`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeAnim {
    pub presence: Animated,
    pub fx: motion::Fx,
    pub flip: Animated,
    pub flip_active: bool,
    /// Bounds were laid out at least once (no FLIP on first layout).
    pub flip_ready: bool,
    /// Parent-relative bounds before the layout pass (x, y, w, h).
    pub flip_prev: [f32; 4],
    pub flip_from: [f32; 4],
    /// Current offset of the painted box from the laid-out bounds.
    pub flip_delta: [f32; 4],
}

impl Default for NodeAnim {
    fn default() -> Self {
        NodeAnim {
            presence: Animated::at(1.0),
            fx: motion::Fx::default(),
            flip: Animated::at(0.0),
            flip_active: false,
            flip_ready: false,
            flip_prev: [0.0; 4],
            flip_from: [0.0; 4],
            flip_delta: [0.0; 4],
        }
    }
}

/// `ContainerData`: pointer handling, hover look and scrolling of a container.
pub struct ContainerData {
    /// Pointer area handler (`ui::pointer_area`); makes the container
    /// interactive.
    pub pointer: Option<PointerArea>,
    /// Hover look (`BoxOptions::hover_background` / `hover_border_color`).
    pub hover_visual: Option<HoverVisual>,
    pub hovered: bool,
    pub hover: Animated,
    pub hover_t: f32,
    /// Key handler while the container is focused (focusable pointer areas).
    pub key_action: Option<KeyAction>,
    /// Called after layout when the container's size changed (`on_resize`).
    pub resize_action: Option<Action>,
    /// Granularity of the `resize_action` reports (`None`: every change).
    pub resize_step: Option<ResizeStep>,
    pub reported_width: f32,
    pub reported_height: f32,
    /// Cursor shown over the container (pointer areas).
    pub cursor: Option<Cursor>,
    /// Current vertical scroll offset for `scroll` containers.
    pub scroll_offset: f32,
    /// Natural height of the children, measured during layout.
    pub content_height: f32,
    /// Animated `scroll_into_view` (cancelled by the user scrolling).
    pub scroll_anim: Animated,
    pub scroll_animating: bool,
}

impl Default for ContainerData {
    fn default() -> Self {
        ContainerData {
            pointer: None,
            hover_visual: None,
            hovered: false,
            hover: Animated::default(),
            hover_t: 0.0,
            key_action: None,
            resize_action: None,
            resize_step: None,
            reported_width: -1.0,
            reported_height: -1.0,
            cursor: None,
            scroll_offset: 0.0,
            content_height: 0.0,
            scroll_anim: Animated::default(),
            scroll_animating: false,
        }
    }
}

impl ContainerData {
    pub fn max_scroll(&self, viewport_height: f32) -> f32 {
        (self.content_height - viewport_height).max(0.0)
    }
}
