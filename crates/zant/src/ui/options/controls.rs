//! Options of controls: buttons, inputs, toggles, radio groups, tabs, selects, icon buttons, tooltips.

use super::*;

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
    pub on_selection: Option<crate::ui::actions::RangeAction>,
    /// Key hook (list continuation, shortcuts); return true to consume.
    pub on_key: Option<crate::ui::actions::InputKeyAction>,
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
