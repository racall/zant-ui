//! Option structs of the `ui::*` builders.
//!
//! Every struct derives `Clone` + `zant_macros::TsxProps` (the TSX compiler
//! reflects on them). Many defaults are non-zero, so most `Default` impls are
//! hand-written.
//!
//! The option structs of every builder are declared here; the builder bodies
//! live in `ui::builders` and `components/`.
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{Icon, ImageFit, RichSpan};
use crate::core::vector::{Drawing, DrawingRef};
use crate::flex;
use crate::image::Image;
use crate::motion::motion;
use crate::platform::platform::Cursor;
use crate::renderer::text;

use super::actions::{Action, BoolAction, IndexAction, KeyAction, PointerAction, TextAction};
use super::element::Node;

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

/// `ui::text` / `ui::paragraph` text style options.
#[derive(Clone, zant_macros::TsxProps)]
pub struct TextOptions {
    pub key: Option<Str>,
    pub size: f32,
    pub weight: text::Weight,
    pub color: Option<u32>,
    pub text_align: text::Align,
    pub wrap: bool,
    pub line_height: f32,
    pub max_lines: u16,
    pub family: text::Family,
}

impl Default for TextOptions {
    fn default() -> Self {
        TextOptions {
            key: None,
            size: 14.0,
            weight: text::Weight::Regular,
            color: None,
            text_align: text::Align::Left,
            wrap: false,
            line_height: 0.0,
            max_lines: 0,
            family: text::Family::Sans,
        }
    }
}

/// `ui::heading` (antd heading sizes).
#[derive(Clone, zant_macros::TsxProps)]
pub struct HeadingOptions {
    pub key: Option<Str>,
    pub level: u8,
    pub color: Option<u32>,
    pub weight: text::Weight,
    pub text_align: text::Align,
}

impl Default for HeadingOptions {
    fn default() -> Self {
        HeadingOptions {
            key: None,
            level: 1,
            color: None,
            weight: text::Weight::Semibold,
            text_align: text::Align::Left,
        }
    }
}

/// `ui::paragraph`: wrapping body text with a 1.5 line height.
#[derive(Clone, zant_macros::TsxProps)]
pub struct ParagraphOptions {
    pub key: Option<Str>,
    pub size: f32,
    pub color: Option<u32>,
    pub text_align: text::Align,
    pub line_height: f32,
}

impl Default for ParagraphOptions {
    fn default() -> Self {
        ParagraphOptions {
            key: None,
            size: 14.0,
            color: None,
            text_align: text::Align::Left,
            line_height: 1.5,
        }
    }
}

/// `ui::button`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct ButtonOptions {
    pub key: Option<Str>,
    pub text: Str,
    pub variant: crate::core::node::ButtonType,
    pub size: crate::core::node::ButtonSize,
    pub disabled: bool,
    pub width: Option<f32>,
    pub font_size: Option<f32>,
    pub text_align: text::Align,
    pub label_padding: f32,
    pub text_color: Option<u32>,
    pub icon: Icon,
    pub icon_size: f32,
    pub on_click: Option<Action>,
    pub index_action: Option<IndexAction>,
    pub index: usize,
    /// false: clicking does not take keyboard focus (toolbar buttons that act
    /// on the focused input).
    pub focusable: bool,
    /// antd `danger`: error colours (primary: red fill; default: red border
    /// and text; text / link: red text).
    pub danger: bool,
    /// antd `block`: as wide as the parent (`width` still wins).
    pub block: bool,
    /// antd `loading`: a spinner before the label, clicks are ignored.
    pub loading: bool,
}

impl Default for ButtonOptions {
    fn default() -> Self {
        ButtonOptions {
            key: None,
            text: Str::default(),
            variant: crate::core::node::ButtonType::Default,
            size: crate::core::node::ButtonSize::Middle,
            disabled: false,
            width: None,
            font_size: None,
            // The button's own default; the generic text style default is
            // left-aligned.
            text_align: text::Align::Center,
            label_padding: 0.0,
            text_color: None,
            icon: Icon::None,
            icon_size: 18.0,
            on_click: None,
            index_action: None,
            index: 0,
            focusable: true,
            danger: false,
            block: false,
            loading: false,
        }
    }
}

/// `ui::link`: a `link`-variant button.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct LinkOptions {
    pub key: Option<Str>,
    pub text: Str,
    pub on_click: Option<Action>,
}

/// `ui::input`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct InputOptions {
    pub key: Option<Str>,
    pub value: Option<Str>,
    pub placeholder: Str,
    pub disabled: bool,
    pub read_only: bool,
    pub password: bool,
    pub multiline: bool,
    pub focus_ring: bool,
    /// Takes the keyboard focus when it appears (the field of a dialog that
    /// just opened, a search box): `autoFocus` in TSX.
    pub auto_focus: bool,
    pub width: Option<f32>,
    pub height: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub on_change: Option<TextAction>,
    pub on_submit: Option<Action>,
    pub on_undo: Option<Action>,
    pub on_redo: Option<Action>,
    /// Controlled selection `(anchor, cursor)` in UTF-8 bytes. Applied whenever
    /// the value differs from the previously applied one.
    pub selection: Option<[usize; 2]>,
    pub on_selection: Option<super::actions::RangeAction>,
    /// Key hook (list continuation, shortcuts); return true to consume.
    pub on_key: Option<super::actions::InputKeyAction>,
    /// Multiline: keep the text in a centred column of at most this width
    /// while the input (and its scroll bar) spans the full width.
    pub content_max_width: Option<f32>,
}

impl Default for InputOptions {
    fn default() -> Self {
        InputOptions {
            key: None,
            value: None,
            placeholder: Str::default(),
            disabled: false,
            read_only: false,
            password: false,
            multiline: false,
            focus_ring: true,
            auto_focus: false,
            width: None,
            height: 32.0,
            font_size: 14.0,
            line_height: 0.0,
            on_change: None,
            on_submit: None,
            on_undo: None,
            on_redo: None,
            selection: None,
            on_selection: None,
            on_key: None,
            content_max_width: None,
        }
    }
}

/// `ui::checkbox` / `ui::switch`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct ToggleOptions {
    pub key: Option<Str>,
    pub label: Str,
    pub checked: bool,
    pub disabled: bool,
    pub font_size: f32,
    pub on_change: Option<BoolAction>,
}

impl Default for ToggleOptions {
    fn default() -> Self {
        ToggleOptions {
            key: None,
            label: Str::default(),
            checked: false,
            disabled: false,
            font_size: 14.0,
            on_change: None,
        }
    }
}

/// One radio of `ui::radio_group`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct RadioOption {
    #[tsx(required)]
    pub label: Str,
    pub disabled: bool,
}

/// `ui::radio_group`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct RadioGroupOptions {
    pub key: Option<Str>,
    pub options: Vec<RadioOption>,
    pub selected: usize,
    pub direction: flex::Direction,
    pub gap: f32,
    pub on_change: Option<IndexAction>,
}

impl Default for RadioGroupOptions {
    fn default() -> Self {
        RadioGroupOptions {
            key: None,
            options: Vec::new(),
            selected: 0,
            direction: flex::Direction::Column,
            gap: 8.0,
            on_change: None,
        }
    }
}

/// One tab of `ui::tabs`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct TabItem {
    #[tsx(required)]
    pub label: Str,
    #[tsx(required)]
    pub content: Node,
}

/// `ui::tabs`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct TabsOptions {
    pub key: Option<Str>,
    pub items: Vec<TabItem>,
    pub active: usize,
    pub on_change: Option<IndexAction>,
}

/// `ui::tooltip`: wraps a child node.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct TooltipOptions {
    #[tsx(required)]
    pub text: Str,
    #[tsx(required)]
    pub child: Node,
}

/// `ui::image`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct ImageOptions {
    pub key: Option<Str>,
    /// Image file path (PNG, JPEG, GIF, BMP, ICO).
    pub src: Str,
    /// In-memory image (`zant::image::Image`); used instead of `src`.
    pub image: Option<Rc<Image>>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub fit: ImageFit,
    pub radius: f32,
    pub tooltip: Option<Str>,
}

impl Default for ImageOptions {
    fn default() -> Self {
        ImageOptions {
            key: None,
            src: Str::default(),
            image: None,
            width: None,
            height: None,
            fit: ImageFit::Cover,
            radius: 8.0,
            tooltip: None,
        }
    }
}

/// `ui::select`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct SelectOptions {
    pub key: Option<Str>,
    pub options: Vec<Str>,
    pub selected: usize,
    pub width: Option<f32>,
    pub height: f32,
    pub disabled: bool,
    pub tooltip: Option<Str>,
    pub on_change: Option<IndexAction>,
}

impl Default for SelectOptions {
    fn default() -> Self {
        SelectOptions {
            key: None,
            options: Vec::new(),
            selected: 0,
            width: None,
            height: 32.0,
            disabled: false,
            tooltip: None,
            on_change: None,
        }
    }
}

/// `ui::icon`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct IconOptions {
    pub key: Option<Str>,
    #[tsx(required)]
    pub name: Icon,
    pub size: f32,
    pub color: Option<u32>,
    /// Stroke width in px (default scales with size: 1.5px at 18px).
    pub stroke: Option<f32>,
}

impl Default for IconOptions {
    fn default() -> Self {
        IconOptions {
            key: None,
            name: Icon::None,
            size: 18.0,
            color: None,
            stroke: None,
        }
    }
}

/// `ui::icon_button`: a square icon-only button.
#[derive(Clone, zant_macros::TsxProps)]
pub struct IconButtonOptions {
    pub key: Option<Str>,
    #[tsx(required)]
    pub icon: Icon,
    /// Icon size; the button is `box` × `box`.
    pub size: f32,
    pub r#box: f32,
    pub color: Option<u32>,
    /// Set: no hover / press fill, the icon fades to this colour instead
    /// (clear buttons inside inputs).
    pub hover_color: Option<u32>,
    /// Draws `active_background` behind the icon (toggled state).
    pub active: bool,
    pub active_background: Option<u32>,
    pub disabled: bool,
    /// Toolbar buttons should not steal focus from the input they act on.
    pub focusable: bool,
    pub tooltip: Option<Str>,
    pub on_click: Option<Action>,
    pub index_action: Option<IndexAction>,
    pub index: usize,
}

impl Default for IconButtonOptions {
    fn default() -> Self {
        IconButtonOptions {
            key: None,
            icon: Icon::None,
            size: 18.0,
            r#box: 32.0,
            color: None,
            hover_color: None,
            active: false,
            active_background: None,
            disabled: false,
            focusable: false,
            tooltip: None,
            on_click: None,
            index_action: None,
            index: 0,
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

/// `ui::vector`: a retained vector drawing.
#[derive(Clone, zant_macros::TsxProps)]
pub struct VectorOptions {
    pub key: Option<Str>,
    #[tsx(required)]
    pub drawing: DrawingRef,
    /// Fixed box width (default: the drawing's width). The drawing is
    /// left-aligned; wrap in a row to center it.
    pub width: Option<f32>,
    /// Text copied when the drawing is part of a text selection (e.g. a
    /// formula's LaTeX); empty keeps it out of selections.
    pub copy: Str,
}

impl Default for VectorOptions {
    fn default() -> Self {
        VectorOptions {
            key: None,
            drawing: Rc::new(Drawing::default()),
            width: None,
            copy: Str::default(),
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

/// `ui::rich_text`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct RichTextOptions {
    pub key: Option<Str>,
    pub spans: Vec<RichSpan>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Line height multiplier; 0 uses the font's natural line height.
    pub line_height: f32,
    /// Called with `RichSpan.link` when a linked span is clicked.
    pub on_link: Option<TextAction>,
}

impl Default for RichTextOptions {
    fn default() -> Self {
        RichTextOptions {
            key: None,
            spans: Vec::new(),
            width: None,
            height: None,
            line_height: 2.0,
            on_link: None,
        }
    }
}

/// `ui::pdf_preview`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct PdfOptions {
    pub key: Option<Str>,
    #[tsx(required)]
    pub src: Str,
    pub page: usize,
    pub width: f32,
    pub height: f32,
    pub show_toolbar: bool,
    pub background: Option<u32>,
    pub on_page_change: Option<IndexAction>,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions {
            key: None,
            src: Str::default(),
            page: 0,
            width: 320.0,
            height: 420.0,
            show_toolbar: true,
            background: None,
            on_page_change: None,
        }
    }
}

/// `ui::divider`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct DividerOptions {
    pub key: Option<Str>,
    pub margin: Edges,
    pub color: Option<u32>,
}

/// `ui::tag`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct TagOptions {
    pub key: Option<Str>,
    pub text: Str,
    pub color: Option<u32>,
    pub background: Option<u32>,
}

/// `ui::card`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct CardOptions {
    pub key: Option<Str>,
    pub title: Option<Str>,
    pub extra: Option<Str>,
    pub children: Vec<Node>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub gap: f32,
    pub padding: Edges,
    pub bordered: bool,
}

impl Default for CardOptions {
    fn default() -> Self {
        CardOptions {
            key: None,
            title: None,
            extra: None,
            children: Vec::new(),
            width: None,
            height: None,
            gap: 12.0,
            padding: Edges::all(20.0),
            bordered: true,
        }
    }
}

/// `ui::alert` status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AlertType {
    #[default]
    Info,
    Success,
    Warning,
    ErrorState,
}

/// `ui::alert`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct AlertOptions {
    pub key: Option<Str>,
    pub r#type: AlertType,
    pub title: Str,
    pub description: Str,
}

/// `ui::avatar`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct AvatarOptions {
    pub key: Option<Str>,
    pub text: Str,
    pub size: f32,
    pub background: Option<u32>,
    pub color: Option<u32>,
}

impl Default for AvatarOptions {
    fn default() -> Self {
        AvatarOptions {
            key: None,
            text: Str::Static("?"),
            size: 32.0,
            background: None,
            color: None,
        }
    }
}

/// `ui::progress`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct ProgressOptions {
    pub key: Option<Str>,
    pub percent: f32,
    pub width: f32,
    pub height: f32,
    pub color: Option<u32>,
}

impl Default for ProgressOptions {
    fn default() -> Self {
        ProgressOptions {
            key: None,
            percent: 0.0,
            width: 200.0,
            height: 8.0,
            color: None,
        }
    }
}

/// Compile-time reflection metadata for the TSX compiler.
///
/// `#[derive(TsxProps)]` emits an `impl TsxProps` for each option struct; the
/// `zant-tsx-tags` generator reads [`TSX_REGISTRY`] and writes
/// `crates/zant-tsx/src/ui_tags/`. A `proc-macro` crate cannot export these
/// types itself, so they live here, next to the structs they describe.
///
/// Field *classification* (integer / enum / color / nested options …) stays
/// in the generator because Rust cannot enumerate an enum's variants at run
/// time.
pub mod tsx_props {
    /// One field of an option struct.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TsxField {
        /// Rust field name with the `r#` prefix stripped (`type` for `r#type`).
        pub name: &'static str,
        /// Rust type as token text (`Option<TextAction>`); the generator
        /// re-parses and classifies it.
        pub ty: &'static str,
        /// The TSX prop must be given (the field has no default value).
        pub required: bool,
        /// `#[tsx(default = "..")]` as written; `None` means `Default::default()`.
        pub default: Option<&'static str>,
    }

    /// One reflected option struct.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TsxStruct {
        /// Rust type name (`ButtonOptions`).
        pub name: &'static str,
        /// The struct's doc comment, lines joined with `\n`.
        pub doc: &'static str,
        /// `module_path!()` of the struct (`zant::ui::options`).
        pub module_path: &'static str,
        /// Fields in declaration order.
        pub fields: &'static [TsxField],
    }

    /// Implemented by `#[derive(TsxProps)]`.
    pub trait TsxProps {
        /// The struct's reflection metadata.
        const TSX: TsxStruct;
    }
}

/// Every `#[derive(TsxProps)]` option struct of this module, in declaration
/// order, for `zant-tsx-tags`.
///
/// Keep it in sync when adding an option struct: the generator reports a
/// builder whose options type is missing here. `ui::Styled`'s `Style` lives in
/// `ui/styled.rs` and is not a builder option, so it is not registered.
pub static TSX_REGISTRY: &[&tsx_props::TsxStruct] = &[
    &<BoxOptions as tsx_props::TsxProps>::TSX,
    &<PointerAreaOptions as tsx_props::TsxProps>::TSX,
    &<ClickableOptions as tsx_props::TsxProps>::TSX,
    &<SpaceOptions as tsx_props::TsxProps>::TSX,
    &<TextOptions as tsx_props::TsxProps>::TSX,
    &<HeadingOptions as tsx_props::TsxProps>::TSX,
    &<ParagraphOptions as tsx_props::TsxProps>::TSX,
    &<ButtonOptions as tsx_props::TsxProps>::TSX,
    &<LinkOptions as tsx_props::TsxProps>::TSX,
    &<InputOptions as tsx_props::TsxProps>::TSX,
    &<ToggleOptions as tsx_props::TsxProps>::TSX,
    &<RadioOption as tsx_props::TsxProps>::TSX,
    &<RadioGroupOptions as tsx_props::TsxProps>::TSX,
    &<TabItem as tsx_props::TsxProps>::TSX,
    &<TabsOptions as tsx_props::TsxProps>::TSX,
    &<TooltipOptions as tsx_props::TsxProps>::TSX,
    &<ImageOptions as tsx_props::TsxProps>::TSX,
    &<SelectOptions as tsx_props::TsxProps>::TSX,
    &<IconOptions as tsx_props::TsxProps>::TSX,
    &<IconButtonOptions as tsx_props::TsxProps>::TSX,
    &<DismissLayerOptions as tsx_props::TsxProps>::TSX,
    &<VectorOptions as tsx_props::TsxProps>::TSX,
    &<MemoOptions as tsx_props::TsxProps>::TSX,
    &<RichTextOptions as tsx_props::TsxProps>::TSX,
    &<PdfOptions as tsx_props::TsxProps>::TSX,
    &<DividerOptions as tsx_props::TsxProps>::TSX,
    &<TagOptions as tsx_props::TsxProps>::TSX,
    &<CardOptions as tsx_props::TsxProps>::TSX,
    &<AlertOptions as tsx_props::TsxProps>::TSX,
    &<AvatarOptions as tsx_props::TsxProps>::TSX,
    &<ProgressOptions as tsx_props::TsxProps>::TSX,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_options_have_the_expected_defaults() {
        let options = BoxOptions::default();
        assert_eq!(options.gap, 0.0);
        assert_eq!(options.wrap, flex::Wrap::Nowrap);
        assert_eq!(options.align_content, flex::AlignContent::Start);
        assert_eq!(options.align_self, flex::AlignSelf::Auto);
        assert_eq!(options.justify, flex::Justify::Start);
        assert!(options.justify_content.is_none());
        assert_eq!(options.align_items, flex::Align::Start);
        assert!(options.flex_direction.is_none());
        assert!(!options.absolute);
        assert!(!options.clip);
        assert!(!options.scroll);
        assert!(!options.block_pointer);
        assert_eq!(options.grow, 0.0);
        assert_eq!(options.shrink, 0.0, "a box does not shrink by default");
        assert_eq!(options.border_width, 0.0);
        assert_eq!(options.radius, 0.0);
        assert!(options.children.is_empty());
    }

    #[test]
    fn text_and_button_options_have_the_expected_defaults() {
        let text = TextOptions::default();
        assert_eq!(text.size, 14.0);
        assert_eq!(text.weight, text::Weight::Regular);
        assert!(!text.wrap);
        assert_eq!(text.line_height, 0.0);
        assert_eq!(text.max_lines, 0);
        assert_eq!(text.family, text::Family::Sans);

        let button = ButtonOptions::default();
        assert_eq!(button.variant, crate::core::node::ButtonType::Default);
        assert_eq!(button.size, crate::core::node::ButtonSize::Middle);
        assert_eq!(button.text_align, text::Align::Center);
        assert_eq!(button.icon_size, 18.0);
        assert!(button.focusable);
        assert_eq!(button.text, "");

        let input = InputOptions::default();
        assert_eq!(input.height, 32.0);
        assert_eq!(input.font_size, 14.0);
        assert!(input.focus_ring);
    }

    #[test]
    fn the_other_options_keep_their_non_zero_defaults() {
        assert_eq!(SpaceOptions::default().gap, 8.0);
        assert_eq!(HeadingOptions::default().level, 1);
        assert_eq!(HeadingOptions::default().weight, text::Weight::Semibold);
        assert_eq!(ParagraphOptions::default().line_height, 1.5);
        assert_eq!(ToggleOptions::default().font_size, 14.0);
        assert_eq!(
            RadioGroupOptions::default().direction,
            flex::Direction::Column
        );
        assert_eq!(RadioGroupOptions::default().gap, 8.0);
        assert_eq!(SelectOptions::default().height, 32.0);
        assert_eq!(ImageOptions::default().radius, 8.0);
        assert_eq!(RichTextOptions::default().line_height, 2.0);
        assert_eq!(
            PdfOptions::default().width,
            320.0,
            "PdfOptions width / height / toolbar defaults"
        );
        assert_eq!(PdfOptions::default().height, 420.0);
        assert!(PdfOptions::default().show_toolbar);
        assert_eq!(CardOptions::default().gap, 12.0);
        assert_eq!(CardOptions::default().padding, Edges::all(20.0));
        assert!(CardOptions::default().bordered);
        assert_eq!(AvatarOptions::default().text, "?");
        assert_eq!(AvatarOptions::default().size, 32.0);
        assert_eq!(ProgressOptions::default().width, 200.0);
        assert_eq!(ProgressOptions::default().height, 8.0);
        assert_eq!(AlertOptions::default().r#type, AlertType::Info);
        assert_eq!(
            DismissLayerOptions::default().key.as_deref(),
            Some("dismiss-layer")
        );
        assert_eq!(ClickableOptions::default().cursor, Cursor::PointingHand);
        assert!(PointerAreaOptions::default().clicks);
    }
}
