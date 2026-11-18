//! Retained data of controls: buttons, inputs, toggles and selects.

use super::*;

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
    /// The colour hover / press fade the label or icon to (borderless icon
    /// buttons); `None` keeps the colour and uses the variant's fill.
    pub hover_text_color: Option<u32>,
    pub icon: crate::core::icons::Icon,
    pub icon_size: f32,
    /// antd `danger`: the error colours instead of the primary ones.
    pub danger: bool,
    /// antd `loading`: a spinner replaces the icon and clicks are ignored.
    pub loading: bool,
    /// Spinner rotation, 0..1 per turn (sampled by `button::animate`).
    pub spin_t: f32,
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
            hover_text_color: None,
            icon: crate::core::icons::Icon::None,
            icon_size: 18.0,
            danger: false,
            loading: false,
            spin_t: 0.0,
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
pub(super) fn floor_char_boundary(text: &str, at: usize) -> usize {
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
