//! Options of text builders: text, headings, paragraphs, links, rich text.

use super::*;

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

/// `ui::link`: a `link`-variant button.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct LinkOptions {
    pub key: Option<Str>,
    pub text: Str,
    pub on_click: Option<Action>,
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
