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

/// `ButtonData`: label, antd variant, interaction state and its transitions.
pub struct ButtonData {
    pub label: Str,
    pub button_type: ButtonType,
    pub size: ButtonSize,
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    /// Focused from the keyboard: paint the focus ring (antd / CSS
    /// `:focus-visible`; a mouse click focuses without a ring).
    pub focus_visible: bool,
    // transitions (targets follow the booleans above) and their sampled
    // values for paint
    pub hover: Animated,
    pub press: Animated,
    /// Click wave (antd `Wave`), 0..1 over 600ms; 1 = idle.
    pub wave: Animated,
    pub hover_t: f32,
    pub press_t: f32,
    pub wave_t: f32,
    pub action: Option<Action>,
    pub index_action: Option<IndexAction>,
    pub index: usize,
    pub layout: Option<TextLayout>,
    pub layout_dirty: bool,
    pub layout_color: u32,
    pub font_size: f32,
    pub text_align: rtext::Align,
    pub label_padding: f32,
    pub text_color: Option<u32>,
    pub icon: crate::core::icons::Icon,
    pub icon_size: f32,
}

impl Default for ButtonData {
    fn default() -> Self {
        ButtonData {
            label: Str::default(),
            button_type: ButtonType::Default,
            size: ButtonSize::Middle,
            hovered: false,
            pressed: false,
            focused: false,
            focus_visible: false,
            hover: Animated::default(),
            press: Animated::default(),
            wave: Animated::at(1.0),
            hover_t: 0.0,
            press_t: 0.0,
            wave_t: 1.0,
            action: None,
            index_action: None,
            index: 0,
            layout: None,
            layout_dirty: true,
            layout_color: 0,
            font_size: 14.0,
            text_align: rtext::Align::Center,
            label_padding: 0.0,
            text_color: None,
            icon: crate::core::icons::Icon::None,
            icon_size: 18.0,
        }
    }
}

impl ButtonData {
    pub fn invalidate_layout(&mut self) {
        self.layout_dirty = true;
        self.layout = None;
    }

    pub fn ensure_layout(&mut self, color: u32) -> Option<&TextLayout> {
        if !self.layout_dirty && self.layout.is_some() && self.layout_color == color {
            return self.layout.as_ref();
        }
        let style = rtext::Style {
            font_size: self.font_size,
            weight: rtext::Weight::Medium,
            color,
            wrap: false,
            text_align: rtext::Align::Left,
            ..rtext::Style::default()
        };
        let layout = TextLayout::init(&self.label, style, 4096.0).ok()?;
        self.layout = Some(layout);
        self.layout_dirty = false;
        self.layout_color = color;
        self.layout.as_ref()
    }
}

/// `InputData`: the editing state of a single- or multi-line input.
///
/// The text buffers are `String`s: all edits happen on UTF-8 boundaries.
pub struct InputData {
    pub text: String,
    pub placeholder: Str,
    pub read_only: bool,
    pub password: bool,
    pub multiline: bool,
    pub focus_ring: bool,
    pub available_width: f32,
    pub focused: bool,
    pub hovered: bool,
    pub hover: Animated,
    pub focus_anim: Animated,
    pub hover_t: f32,
    pub focus_t: f32,
    pub cursor: usize,
    pub anchor: usize,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub line_height: f32,
    pub composition: String,
    pub composition_cursor: usize,
    pub has_composition: bool,
    pub blink_on: bool,
    pub next_blink_ms: i64,
    pub text_action: Option<TextAction>,
    pub submit_action: Option<Action>,
    pub undo_action: Option<Action>,
    pub redo_action: Option<Action>,
    pub selection_action: Option<RangeAction>,
    pub key_action: Option<InputKeyAction>,
    pub applied_selection: Option<[usize; 2]>,
    pub reported_selection: [usize; 2],
    pub layout: Option<TextLayout>,
    pub layout_width: f32,
    pub layout_dirty: bool,
    pub display_text: String,
    pub placeholder_layout: Option<TextLayout>,
    pub font_size: f32,
    pub text_color: u32,
    pub layout_color: u32,
    /// Centred text column width for wide multiline inputs (0 = full width).
    pub content_max_width: f32,
    /// Column kept while moving the caret up/down through lines.
    pub preferred_x: Option<f32>,
    /// Scroll the caret into view on the next paint (set by edits, caret
    /// moves and controlled selection; scrolling itself never clears it).
    pub reveal_caret: bool,
}

impl Default for InputData {
    fn default() -> Self {
        InputData {
            text: String::new(),
            placeholder: Str::default(),
            read_only: false,
            password: false,
            multiline: false,
            focus_ring: true,
            available_width: 1.0e6,
            focused: false,
            hovered: false,
            hover: Animated::default(),
            focus_anim: Animated::default(),
            hover_t: 0.0,
            focus_t: 0.0,
            cursor: 0,
            anchor: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            line_height: 0.0,
            composition: String::new(),
            composition_cursor: 0,
            has_composition: false,
            blink_on: true,
            next_blink_ms: 0,
            text_action: None,
            submit_action: None,
            undo_action: None,
            redo_action: None,
            selection_action: None,
            key_action: None,
            applied_selection: None,
            reported_selection: [0, 0],
            layout: None,
            layout_width: -1.0,
            layout_dirty: true,
            display_text: String::new(),
            placeholder_layout: None,
            font_size: 14.0,
            text_color: 0xe000_0000,
            layout_color: 0,
            content_max_width: 0.0,
            preferred_x: None,
            reveal_caret: false,
        }
    }
}

impl InputData {
    pub fn invalidate_layout(&mut self) {
        self.layout_dirty = true;
        self.layout = None;
        self.layout_width = -1.0;
    }

    /// Rebuilds `display_text`: the text as painted
    /// (masked when `password`, composition text at the caret).
    pub fn rebuild_display(&mut self) {
        self.display_text.clear();
        let cursor = self.cursor.min(self.text.len());
        if self.password {
            // One mask byte per source byte keeps caret/selection offsets aligned.
            for _ in 0..cursor {
                self.display_text.push('*');
            }
            if self.has_composition {
                for _ in 0..self.composition.len() {
                    self.display_text.push('*');
                }
            }
            for _ in cursor..self.text.len() {
                self.display_text.push('*');
            }
        } else {
            let cursor = floor_char_boundary(&self.text, cursor);
            self.display_text.push_str(&self.text[..cursor]);
            if self.has_composition {
                self.display_text.push_str(&self.composition);
            }
            self.display_text.push_str(&self.text[cursor..]);
        }
        self.invalidate_layout();
    }
}

/// Largest char boundary `<= at` (the text is UTF-8, so never split a code
/// point when rendering).
fn floor_char_boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// `ToggleData`: checkbox / switch / radio state.
pub struct ToggleData {
    pub label: Str,
    pub kind: ToggleKind,
    pub checked: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    /// Focused from the keyboard (see `ButtonData::focus_visible`).
    pub focus_visible: bool,
    pub hover: Animated,
    pub press: Animated,
    /// Fill / border / track / knob position.
    pub check: Animated,
    /// Checkmark scale (checkbox) / dot scale (radio).
    pub mark: Animated,
    pub hover_t: f32,
    pub press_t: f32,
    pub check_t: f32,
    pub mark_t: f32,
    pub font_size: f32,
    pub action: Option<BoolAction>,
    pub index_action: Option<IndexAction>,
    pub index: usize,
    pub layout: Option<TextLayout>,
    pub layout_dirty: bool,
    pub layout_color: u32,
}

impl Default for ToggleData {
    fn default() -> Self {
        ToggleData {
            label: Str::default(),
            kind: ToggleKind::Checkbox,
            checked: false,
            hovered: false,
            pressed: false,
            focused: false,
            focus_visible: false,
            hover: Animated::default(),
            press: Animated::default(),
            check: Animated::default(),
            mark: Animated::default(),
            hover_t: 0.0,
            press_t: 0.0,
            check_t: 0.0,
            mark_t: 0.0,
            font_size: 14.0,
            action: None,
            index_action: None,
            index: 0,
            layout: None,
            layout_dirty: true,
            layout_color: 0,
        }
    }
}

impl ToggleData {
    pub fn invalidate_layout(&mut self) {
        self.layout_dirty = true;
        self.layout = None;
    }

    pub fn ensure_layout(&mut self, color: u32) -> Option<&TextLayout> {
        if !self.layout_dirty && self.layout.is_some() && self.layout_color == color {
            return self.layout.as_ref();
        }
        self.layout_color = color;
        let style = rtext::Style {
            font_size: self.font_size,
            weight: rtext::Weight::Regular,
            color,
            wrap: false,
            ..rtext::Style::default()
        };
        let layout = TextLayout::init(&self.label, style, 4096.0).ok()?;
        self.layout = Some(layout);
        self.layout_dirty = false;
        self.layout.as_ref()
    }
}

/// `ImageData`: an image source and how it fills its box.
pub struct ImageData {
    pub path: Str,
    /// In-memory source; the tree shares the loaded `Image` through `Rc`.
    pub handle: Option<std::rc::Rc<crate::image::Image>>,
    pub source_id: u64,
    pub intrinsic_width: i32,
    pub intrinsic_height: i32,
    pub fit: ImageFit,
    pub radius: f32,
}

impl Default for ImageData {
    fn default() -> Self {
        ImageData {
            path: Str::default(),
            handle: None,
            source_id: 0,
            intrinsic_width: 0,
            intrinsic_height: 0,
            fit: ImageFit::Cover,
            radius: 0.0,
        }
    }
}

/// One select option label.
pub struct SelectOption {
    pub label: Str,
}

/// `SelectData`: a dropdown select control.
pub struct SelectData {
    pub options: Vec<SelectOption>,
    pub selected: usize,
    pub open: bool,
    pub hovered: bool,
    pub focused: bool,
    pub hover: Animated,
    pub focus_anim: Animated,
    pub open_anim: Animated,
    pub hover_t: f32,
    pub focus_t: f32,
    pub open_t: f32,
    pub hovered_option: Option<usize>,
    pub font_size: f32,
    pub action: Option<IndexAction>,
    pub layout: Option<TextLayout>,
    pub option_layouts: Vec<TextLayout>,
    pub layout_dirty: bool,
    pub layout_color: u32,
    pub option_color: u32,
}

impl Default for SelectData {
    fn default() -> Self {
        SelectData {
            options: Vec::new(),
            selected: 0,
            open: false,
            hovered: false,
            focused: false,
            hover: Animated::default(),
            focus_anim: Animated::default(),
            open_anim: Animated::default(),
            hover_t: 0.0,
            focus_t: 0.0,
            open_t: 0.0,
            hovered_option: None,
            font_size: 14.0,
            action: None,
            layout: None,
            option_layouts: Vec::new(),
            layout_dirty: true,
            layout_color: 0,
            option_color: 0,
        }
    }
}

impl SelectData {
    pub fn clear_options(&mut self) {
        self.options.clear();
        self.option_layouts.clear();
        self.layout = None;
        self.layout_dirty = true;
    }
}

/// `VectorData`: a retained vector drawing (math blocks, diagrams).
pub struct VectorData {
    pub drawing: DrawingRef,
    pub hash: u64,
    /// Text copied for the drawing in a text selection ("" = not selectable),
    /// e.g. a formula's LaTeX.
    pub copy: Str,
}

/// What a retained node paints and lays out, by [`NodeKind`].
///
/// The variants deliberately differ a lot in size; boxing one would add an
/// allocation per node of that kind.
#[allow(clippy::large_enum_variant)]
pub enum NodeData {
    Root,
    Container(ContainerData),
    Text(TextData),
    Button(ButtonData),
    Input(InputData),
    Toggle(ToggleData),
    Image(ImageData),
    Select(SelectData),
    RichText(RichTextData),
    Vector(VectorData),
}

impl NodeData {
    pub fn kind(&self) -> NodeKind {
        match self {
            NodeData::Root => NodeKind::Root,
            NodeData::Container(_) => NodeKind::Container,
            NodeData::Text(_) => NodeKind::Text,
            NodeData::Button(_) => NodeKind::Button,
            NodeData::Input(_) => NodeKind::Input,
            NodeData::Toggle(_) => NodeKind::Toggle,
            NodeData::Image(_) => NodeKind::Image,
            NodeData::Select(_) => NodeKind::Select,
            NodeData::RichText(_) => NodeKind::RichText,
            NodeData::Vector(_) => NodeKind::Vector,
        }
    }
}

/// One slot of the retained tree.
pub struct RetainedNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub style: FlexStyle,
    pub visual: Visual,
    pub key: Option<Str>,
    pub tooltip: Option<Str>,
    /// Hash of the last memoized spec (see `ui::memo`); `None` when not memoized.
    pub memo_hash: Option<u64>,
    /// `ui::local_state` ids the memo build touched (kept alive when skipped).
    pub memo_states: Vec<u64>,
    pub bounds: Rect,
    pub dirty: DirtyFlags,
    /// False for a freed slot (tombstone) waiting in `Tree::free_ids`.
    pub alive: bool,
    /// Listed in `Tree::animating`.
    pub anim_queued: bool,
    pub motion: MotionStyle,
    pub anim: Option<NodeAnim>,
    /// Playing its exit transition: painted, but out of layout, hit testing and
    /// focus; freed when the transition ends.
    pub exiting: bool,
    /// Selected byte range of a text / rich text node (text selection).
    pub text_sel: Option<[u32; 2]>,
    pub visible: bool,
    pub enabled: bool,
    pub focusable: bool,
    pub data: NodeData,
}

impl RetainedNode {
    /// A live node slot with default field values.
    pub fn new(id: NodeId, data: NodeData) -> RetainedNode {
        RetainedNode {
            id,
            parent: None,
            children: Vec::new(),
            style: FlexStyle::default(),
            visual: Visual::default(),
            key: None,
            tooltip: None,
            memo_hash: None,
            memo_states: Vec::new(),
            bounds: Rect::default(),
            dirty: DirtyFlags::default(),
            alive: true,
            anim_queued: false,
            motion: MotionStyle::default(),
            anim: None,
            exiting: false,
            text_sel: None,
            visible: true,
            enabled: true,
            focusable: false,
            data,
        }
    }

    /// A freed slot: the whole node is reset to its defaults with
    /// `alive = false` and a container payload.
    pub fn tombstone(id: NodeId) -> RetainedNode {
        let mut node = RetainedNode::new(id, NodeData::Container(ContainerData::default()));
        node.visible = false;
        node.alive = false;
        node
    }

    pub fn kind(&self) -> NodeKind {
        self.data.kind()
    }

    /// Takes part in layout and input (visible and not exiting).
    pub fn in_flow(&self) -> bool {
        self.visible && !self.exiting
    }

    pub fn is_interactive(&self) -> bool {
        if !self.visible || !self.enabled {
            return false;
        }
        match &self.data {
            NodeData::Button(_)
            | NodeData::Input(_)
            | NodeData::Toggle(_)
            | NodeData::Select(_) => true,
            NodeData::RichText(data) => data.link_action.is_some() && data.has_links(),
            NodeData::Container(data) => data.pointer.is_some(),
            _ => false,
        }
    }

    pub fn mark(&mut self, layout: bool, paint: bool) {
        if layout {
            self.dirty.layout = true;
        }
        if paint || layout {
            self.dirty.paint = true;
        }
    }

    pub fn container(&self) -> Option<&ContainerData> {
        match &self.data {
            NodeData::Container(data) => Some(data),
            _ => None,
        }
    }

    pub fn container_mut(&mut self) -> Option<&mut ContainerData> {
        match &mut self.data {
            NodeData::Container(data) => Some(data),
            _ => None,
        }
    }

    pub fn text_mut(&mut self) -> Option<&mut TextData> {
        match &mut self.data {
            NodeData::Text(data) => Some(data),
            _ => None,
        }
    }

    pub fn rich_text_mut(&mut self) -> Option<&mut RichTextData> {
        match &mut self.data {
            NodeData::RichText(data) => Some(data),
            _ => None,
        }
    }

    pub fn button_mut(&mut self) -> Option<&mut ButtonData> {
        match &mut self.data {
            NodeData::Button(data) => Some(data),
            _ => None,
        }
    }

    pub fn input_mut(&mut self) -> Option<&mut InputData> {
        match &mut self.data {
            NodeData::Input(data) => Some(data),
            _ => None,
        }
    }

    pub fn toggle_mut(&mut self) -> Option<&mut ToggleData> {
        match &mut self.data {
            NodeData::Toggle(data) => Some(data),
            _ => None,
        }
    }
}
