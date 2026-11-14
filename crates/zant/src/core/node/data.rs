//! Data types of retained nodes.
//!
//! Port of the *data* declarations of `src/core/node.zig` (WP-1.2; the tree,
//! layout and paint behaviour of a retained node is WP-1.3 in
//! `core/node/{mod,text,rich_text}.rs`). `ui::Element` stores these types, so
//! they are needed before the retained tree exists.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::vector::DrawingRef;
use crate::layout::flex;
use crate::motion::motion;
use crate::renderer::text;

/// Index into `Tree::nodes` (`core/tree`).
pub type NodeId = u32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NodeKind {
    #[default]
    Root,
    Container,
    Text,
    Button,
    Input,
    Toggle,
    Image,
    Select,
    RichText,
    Vector,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ToggleKind {
    #[default]
    Checkbox,
    SwitchControl,
    Radio,
}

/// antd button variants (see `docs/components/button.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ButtonType {
    Primary,
    #[default]
    Default,
    Text,
    Link,
    /// Paints nothing: an invisible click target (`ui.dismissLayer`).
    Plain,
    /// Windows caption button (minimize / maximize): square, no radius,
    /// subtle hover fill (`ui.windowControls`).
    Caption,
    /// Windows close caption button: red hover fill, white glyph.
    CaptionClose,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ButtonSize {
    Small,
    #[default]
    Middle,
    Large,
}

/// `enum(c_int)` in Zig: the value is handed to the native image bridge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ImageFit {
    #[default]
    Cover = 0,
    Contain = 1,
    Fill = 2,
}

/// The Zant icon set (`core::icons::Icon`, 24×24 line icons). Zig had a single
/// enum used by both `ui.Element` and the drawing functions, so Rust re-exports
/// the definition from `core/icons.rs` here instead of duplicating it.
pub use crate::core::icons::Icon;

/// Zig: `pub const ButtonIcon = icons.zig.Icon;`
pub type ButtonIcon = Icon;

/// Per-node dirty flags (Zig `packed struct`); WP-1.3 uses them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirtyFlags {
    pub layout: bool,
    pub paint: bool,
}

impl DirtyFlags {
    pub fn any(self) -> bool {
        self.layout || self.paint
    }
}

/// The box model and flex settings of a declared node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexStyle {
    pub direction: flex::Direction,
    pub justify: flex::Justify,
    pub align_items: flex::Align,
    pub gap: f32,
    pub row_gap: Option<f32>,
    pub column_gap: Option<f32>,
    pub wrap: flex::Wrap,
    pub align_content: flex::AlignContent,
    pub order: i32,
    pub align_self: flex::AlignSelf,
    pub padding: Edges,
    pub margin: Edges,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub width_percent: Option<f32>,
    pub height_percent: Option<f32>,
    pub absolute: bool,
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub grow: f32,
    pub shrink: f32,
    pub basis: Option<f32>,
    /// Vertical overflow scrolling (see `ui::BoxOptions::scroll`).
    pub scroll: bool,
    /// Absolute children only: take x / width (and y / height unless set)
    /// from the parent's n-th in-flow child (segmented thumb, tab ink bar).
    pub anchor: Option<u16>,
}

impl Default for FlexStyle {
    /// Field defaults of the Zig struct (note `shrink = 1`).
    fn default() -> Self {
        FlexStyle {
            direction: flex::Direction::Row,
            justify: flex::Justify::Start,
            align_items: flex::Align::Start,
            gap: 0.0,
            row_gap: None,
            column_gap: None,
            wrap: flex::Wrap::Nowrap,
            align_content: flex::AlignContent::Start,
            order: 0,
            align_self: flex::AlignSelf::Auto,
            padding: Edges::default(),
            margin: Edges::default(),
            width: None,
            height: None,
            width_percent: None,
            height_percent: None,
            absolute: false,
            top: None,
            right: None,
            bottom: None,
            left: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            grow: 0.0,
            shrink: 1.0,
            basis: None,
            scroll: false,
            anchor: None,
        }
    }
}

/// Declarative motion of a node (see `docs/motion.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionStyle {
    /// Enter / exit transition.
    pub presence: motion::Presence,
    /// Transform origin for scale presets.
    pub origin: motion::Origin,
    /// Animate position / size changes (FLIP); leaf containers only.
    pub layout_transition: Option<motion::Transition>,
}

impl Default for MotionStyle {
    /// Zig: `.{ .presence = .none, .origin = .center, .layout_transition = null }`.
    fn default() -> Self {
        MotionStyle {
            presence: motion::Presence::None,
            origin: motion::Origin::Center,
            layout_transition: None,
        }
    }
}

/// Painted look of a node.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Visual {
    pub background: Option<u32>,
    pub border_color: Option<u32>,
    pub border_width: f32,
    pub radius: f32,
    pub shadow_color: Option<u32>,
    pub shadow_blur: f32,
    pub shadow_offset_y: f32,
    pub clip: bool,
    /// Swallows clicks on the node's empty areas so they do not reach what
    /// is painted behind it (modals, popover panels).
    pub block_pointer: bool,
    /// Text inside can be selected with the pointer and copied
    /// (`BoxOptions::selectable`).
    pub selectable: bool,
    /// Text inside is left out of selections (CSS `user-select: none`):
    /// gutters, labels, toolbars inside selectable content.
    pub unselectable: bool,
    /// Tab / Shift+Tab stay inside the topmost focus scope (modals,
    /// popover panels).
    pub focus_scope: bool,
    /// Shown only while the nearest ancestor with a hover look is hovered,
    /// fading with it (row actions: CSS `.row:hover .actions`).
    pub show_on_hover: bool,
}

/// Look of a container while the pointer is over it (`BoxOptions::hover_*`),
/// applied at paint time with antd's hover transition — no rebuild.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HoverVisual {
    pub background: Option<u32>,
    pub border_color: Option<u32>,
}

/// Pointer handling of a container (`ui::pointer_area`).
#[derive(Clone)]
pub struct PointerArea {
    pub action: crate::ui::actions::PointerAction,
    /// Wheel / trackpad scroll goes to the handler instead of scroll containers.
    pub capture_scroll: bool,
    /// false: presses are not taken (no capture, no `down` / `drag`), so they
    /// reach what is inside — text selection in a scrolling list.
    pub clicks: bool,
}

impl Default for PointerArea {
    /// Zig: `.{ .action = .., .capture_scroll = false, .clicks = true }`.
    fn default() -> Self {
        PointerArea {
            action: crate::ui::actions::PointerAction::new(|_, _| false),
            capture_scroll: false,
            clicks: true,
        }
    }
}

/// One run of styled text inside `ui::rich_text` / `node::RichTextData`.
#[derive(Clone)]
pub struct RichSpan {
    pub text: Str,
    pub font_size: f32,
    pub weight: text::Weight,
    pub color: u32,
    pub italic: bool,
    pub family: text::Family,
    pub background: u32,
    pub underline: bool,
    pub strike: bool,
    /// Inline drawing (e.g. inline math) laid out as a placeholder box whose
    /// baseline sits on the text baseline. `text` is ignored.
    pub embed: Option<DrawingRef>,
    /// Optional link target. Clicking the span calls the rich text `on_link`.
    pub link: Str,
    /// Text copied for this span when selected text is copied (`None`: the
    /// span's `text`; "" leaves it out). Embeds use it for their source,
    /// e.g. "$x^2$" for inline math.
    pub copy: Option<Str>,
}

impl Default for RichSpan {
    /// Field defaults of the Zig struct (`text` is required there; an empty
    /// span is harmless and keeps `..Default::default()` usable).
    fn default() -> Self {
        RichSpan {
            text: Str::default(),
            font_size: 15.0,
            weight: text::Weight::Regular,
            color: 0xff29_2d2a,
            italic: false,
            family: text::Family::Sans,
            background: 0,
            underline: false,
            strike: false,
            embed: None,
            link: Str::default(),
            copy: None,
        }
    }
}
