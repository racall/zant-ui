//! Options of layout and interaction containers: boxes, pointer areas, clickables, spaces, dismiss layers, memos.

use super::*;

/// Layout and look of a container (`ui::column` / `ui::row` / `ui::view`).
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct BoxOptions {
    pub key: Option<Str>,
    pub gap: f32,
    pub row_gap: Option<f32>,
    pub column_gap: Option<f32>,
    pub wrap: flex::Wrap,
    pub align_content: flex::AlignContent,
    pub align_self: flex::AlignSelf,
    pub order: i32,
    pub padding: Edges,
    pub margin: Edges,
    pub justify: flex::Justify,
    pub justify_content: Option<flex::Justify>,
    pub align_items: flex::Align,
    pub flex_direction: Option<flex::Direction>,
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
    pub background: Option<u32>,
    pub border_color: Option<u32>,
    pub border_width: f32,
    pub radius: f32,
    pub clip: bool,
    /// Vertical scroll container: children keep their natural height, overflow
    /// is clipped and the wheel/trackpad scrolls the content. Needs a definite
    /// height (`height`, `height_percent`, `grow` inside a sized parent, ...).
    pub scroll: bool,
    /// Swallow clicks on empty areas (modal backdrops, popover panels).
    pub block_pointer: bool,
    /// Text inside (text, paragraphs, rich text) can be selected by dragging
    /// across nodes and copied with ⌘C / Ctrl+C.
    pub selectable: bool,
    /// Leave this subtree out of text selections (gutters, labels).
    pub unselectable: bool,
    /// Background / border while the pointer is over the container (with
    /// antd's 200 ms transition), painted without rebuilding.
    pub hover_background: Option<u32>,
    pub hover_border_color: Option<u32>,
    /// Keyboard focus moves (Tab / Shift+Tab) stay inside this container while
    /// it is the topmost scope shown (dialogs, popover panels).
    pub focus_scope: bool,
    /// Painted only while the nearest ancestor with `hover_background` /
    /// `hover_border_color` is hovered, fading in with it.
    pub show_on_hover: bool,
    /// Called after layout whenever this container's size changes (first
    /// layout included), then the UI is rebuilt in the same frame.
    pub on_resize: Option<Action>,
    /// Enter / exit transition (antd presets, `docs/motion.md`); give the
    /// container a stable `key`.
    pub motion: motion::Presence,
    /// Transform origin of the zoom / slide presets (default centre).
    pub motion_origin: motion::Origin,
    /// Animate position / size changes with antd's 200 ms ease (FLIP).
    pub layout_transition: bool,
    pub children: Vec<Node>,
}

/// Layout and look of a pointer area (`ui::pointer_area`).
#[derive(Clone, zant_macros::TsxProps)]
pub struct PointerAreaOptions {
    pub r#box: BoxOptions,
    /// Receives down / drag / up, hover / leave, and scroll when
    /// `capture_scroll`. Return true when state changed and the UI must be
    /// rebuilt.
    #[tsx(required)]
    pub on_pointer: PointerAction,
    /// Wheel / trackpad scrolling goes to `on_pointer` (`.scroll`) instead of
    /// enclosing scroll containers (canvases, document viewers).
    pub capture_scroll: bool,
    /// false: the area takes only hover and scroll; presses behave as if it
    /// were a plain container (text inside stays selectable).
    pub clicks: bool,
    /// Cursor over the area (and while dragging it).
    pub cursor: Option<Cursor>,
    /// Take keyboard focus on click (Tab reaches it too); keys go to `on_key`.
    pub focusable: bool,
    pub on_key: Option<KeyAction>,
}

impl Default for PointerAreaOptions {
    fn default() -> Self {
        PointerAreaOptions {
            r#box: BoxOptions::default(),
            on_pointer: PointerAction::new(|_, _| false),
            capture_scroll: false,
            clicks: true,
            cursor: None,
            focusable: false,
            on_key: None,
        }
    }
}

/// Layout, look and callbacks of `ui::clickable`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct ClickableOptions {
    /// Layout and look of the area (a column container); `hover_background`
    /// gives the hover feedback.
    pub r#box: BoxOptions,
    pub on_click: Option<Action>,
    /// Called with `index` instead of `on_click` (rows of a list).
    pub index_action: Option<IndexAction>,
    pub index: usize,
    pub cursor: Cursor,
}

impl Default for ClickableOptions {
    fn default() -> Self {
        ClickableOptions {
            r#box: BoxOptions::default(),
            on_click: None,
            index_action: None,
            index: 0,
            cursor: Cursor::PointingHand,
        }
    }
}

/// `ui::space`: a flex row / column with a default gap of 8.
#[derive(Clone, zant_macros::TsxProps)]
pub struct SpaceOptions {
    pub key: Option<Str>,
    pub direction: flex::Direction,
    pub gap: f32,
    pub align_items: flex::Align,
    pub justify: flex::Justify,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub children: Vec<Node>,
}

impl Default for SpaceOptions {
    fn default() -> Self {
        SpaceOptions {
            key: None,
            direction: flex::Direction::Row,
            gap: 8.0,
            align_items: flex::Align::Center,
            justify: flex::Justify::Start,
            width: None,
            height: None,
            children: Vec::new(),
        }
    }
}

/// `ui::dismiss_layer`: a full-size invisible click catcher.
#[derive(Clone, zant_macros::TsxProps)]
pub struct DismissLayerOptions {
    pub key: Option<Str>,
    pub on_dismiss: Option<Action>,
    /// Optional dimming colour (transparent by default).
    pub background: Option<u32>,
    /// Esc dismisses too, when this is the topmost such layer shown
    /// (a modal's mask, a popover).
    pub escape: bool,
}

impl Default for DismissLayerOptions {
    fn default() -> Self {
        DismissLayerOptions {
            key: Some(Str::Static("dismiss-layer")),
            on_dismiss: None,
            background: None,
            escape: false,
        }
    }
}

/// `ui::memo`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct MemoOptions {
    pub key: Option<Str>,
    /// Must change whenever anything the builder reads changes.
    pub hash: u64,
}
