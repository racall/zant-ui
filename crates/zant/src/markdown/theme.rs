//! Visual tokens for rendered Markdown (Streamdown's typography classes and
//! shadcn-style CSS variables), with light and dark presets. Code colours
//! follow Streamdown's default Shiki themes: github-light / github-dark.

use crate::core::hash::Wyhash;
use crate::renderer::text;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeColors {
    pub plain: u32,
    pub keyword: u32,
    pub string: u32,
    pub comment: u32,
    pub number: u32,
    pub function: u32,
    pub type_: u32,
    pub constant: u32,
    pub property: u32,
    pub tag: u32,
    pub attribute: u32,
    pub punctuation: u32,
    pub meta: u32,
    pub inserted: u32,
    pub inserted_bg: u32,
    pub deleted: u32,
    pub deleted_bg: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub background: u32,
    pub foreground: u32,
    pub muted: u32,
    pub muted_foreground: u32,
    pub border: u32,
    pub heading: u32,
    pub link: u32,
    pub link_blocked: u32,
    pub inline_code_bg: u32,
    pub inline_code_fg: u32,
    pub code_bg: u32,
    pub code_header_bg: u32,
    pub code_border: u32,
    pub line_number: u32,
    pub quote_bar: u32,
    pub quote_text: u32,
    pub table_header_bg: u32,
    pub table_border: u32,
    pub highlight_bg: u32,
    pub marker: u32,
    pub caret: u32,
    pub button_fg: u32,
    pub overlay: u32,
    pub surface: u32,
    pub note: u32,
    pub tip: u32,
    pub important: u32,
    pub warning: u32,
    pub caution: u32,
    pub math: u32,
    pub math_error: u32,
    pub code: CodeColors,

    pub body_family: text::Family,
    pub heading_family: text::Family,
    pub heading_weight: text::Weight,
    /// Heading sizes relative to the body size (h1..h6).
    pub heading_scale: [f32; 6],
    /// Gap between top-level blocks, relative to the body size.
    pub block_gap: f32,
    pub line_height: f32,
}

impl Default for Theme {
    /// Callers normally pass [`light`] or [`dark`]; `Default` has to pick
    /// one, and the light preset is the reference rendering.
    fn default() -> Theme {
        light()
    }
}

pub const GITHUB_LIGHT: CodeColors = CodeColors {
    plain: 0xff24292e,
    keyword: 0xffd73a49,
    string: 0xff032f62,
    comment: 0xff6a737d,
    number: 0xff005cc5,
    function: 0xff6f42c1,
    type_: 0xffe36209,
    constant: 0xff005cc5,
    property: 0xff005cc5,
    tag: 0xff22863a,
    attribute: 0xff6f42c1,
    punctuation: 0xff24292e,
    meta: 0xff6a737d,
    inserted: 0xff22863a,
    inserted_bg: 0xfff0fff4,
    deleted: 0xffb31d28,
    deleted_bg: 0xffffeef0,
};

pub const GITHUB_DARK: CodeColors = CodeColors {
    plain: 0xffc9d1d9,
    keyword: 0xffff7b72,
    string: 0xffa5d6ff,
    comment: 0xff8b949e,
    number: 0xff79c0ff,
    function: 0xffd2a8ff,
    type_: 0xffffa657,
    constant: 0xff79c0ff,
    property: 0xff79c0ff,
    tag: 0xff7ee787,
    attribute: 0xffd2a8ff,
    punctuation: 0xffc9d1d9,
    meta: 0xff8b949e,
    inserted: 0xffaff5b4,
    inserted_bg: 0xff033a16,
    deleted: 0xffffdcd7,
    deleted_bg: 0xff67060c,
};

pub fn light() -> Theme {
    Theme {
        dark: false,
        background: 0xffffffff,
        foreground: 0xff0a0a0a,
        muted: 0xfff5f5f5,
        muted_foreground: 0xff737373,
        border: 0xffe5e5e5,
        heading: 0xff0a0a0a,
        link: 0xff2563eb,
        link_blocked: 0xff737373,
        inline_code_bg: 0xfff5f5f5,
        inline_code_fg: 0xff0a0a0a,
        code_bg: 0xffffffff,
        code_header_bg: 0xfffafafa,
        code_border: 0xffe5e5e5,
        line_number: 0xffa3a3a3,
        quote_bar: 0xffe5e5e5,
        quote_text: 0xff525252,
        table_header_bg: 0xfffafafa,
        table_border: 0xffe5e5e5,
        highlight_bg: 0xfffef08a,
        marker: 0xff737373,
        caret: 0xff0a0a0a,
        button_fg: 0xff737373,
        overlay: 0x66000000,
        surface: 0xffffffff,
        note: 0xff0969da,
        tip: 0xff1a7f37,
        important: 0xff8250df,
        warning: 0xff9a6700,
        caution: 0xffcf222e,
        math: 0xff0a0a0a,
        math_error: 0xff737373,
        code: GITHUB_LIGHT,
        body_family: text::Family::Sans,
        heading_family: text::Family::Sans,
        heading_weight: text::Weight::Semibold,
        heading_scale: [1.875, 1.5, 1.25, 1.125, 1.0, 0.875],
        block_gap: 1.0,
        line_height: 1.75,
    }
}

pub fn dark() -> Theme {
    Theme {
        dark: true,
        background: 0xff0a0a0a,
        foreground: 0xfffafafa,
        muted: 0xff262626,
        muted_foreground: 0xffa3a3a3,
        border: 0xff2e2e2e,
        heading: 0xfffafafa,
        link: 0xff60a5fa,
        link_blocked: 0xffa3a3a3,
        inline_code_bg: 0xff262626,
        inline_code_fg: 0xfffafafa,
        code_bg: 0xff0d1117,
        code_header_bg: 0xff161b22,
        code_border: 0xff30363d,
        line_number: 0xff6e7681,
        quote_bar: 0xff404040,
        quote_text: 0xffa3a3a3,
        table_header_bg: 0xff171717,
        table_border: 0xff2e2e2e,
        highlight_bg: 0xff854d0e,
        marker: 0xffa3a3a3,
        caret: 0xfffafafa,
        button_fg: 0xffa3a3a3,
        overlay: 0x99000000,
        surface: 0xff171717,
        note: 0xff4493f8,
        tip: 0xff3fb950,
        important: 0xffab7df8,
        warning: 0xffd29922,
        caution: 0xfff85149,
        math: 0xfffafafa,
        math_error: 0xffa3a3a3,
        code: GITHUB_DARK,
        body_family: text::Family::Sans,
        heading_family: text::Family::Sans,
        heading_weight: text::Weight::Semibold,
        heading_scale: [1.875, 1.5, 1.25, 1.125, 1.0, 0.875],
        block_gap: 1.0,
        line_height: 1.75,
    }
}

impl Theme {
    /// Structural hash of every field in declaration order (part of
    /// `Options::hash`); `bool`s are hashed as a byte.
    pub(crate) fn hash_into(&self, h: &mut Wyhash) {
        h.update(&[self.dark as u8]);
        for value in [
            self.background,
            self.foreground,
            self.muted,
            self.muted_foreground,
            self.border,
            self.heading,
            self.link,
            self.link_blocked,
            self.inline_code_bg,
            self.inline_code_fg,
            self.code_bg,
            self.code_header_bg,
            self.code_border,
            self.line_number,
            self.quote_bar,
            self.quote_text,
            self.table_header_bg,
            self.table_border,
            self.highlight_bg,
            self.marker,
            self.caret,
            self.button_fg,
            self.overlay,
            self.surface,
            self.note,
            self.tip,
            self.important,
            self.warning,
            self.caution,
            self.math,
            self.math_error,
        ] {
            h.update(&value.to_le_bytes());
        }
        let code = &self.code;
        for value in [
            code.plain,
            code.keyword,
            code.string,
            code.comment,
            code.number,
            code.function,
            code.type_,
            code.constant,
            code.property,
            code.tag,
            code.attribute,
            code.punctuation,
            code.meta,
            code.inserted,
            code.inserted_bg,
            code.deleted,
            code.deleted_bg,
        ] {
            h.update(&value.to_le_bytes());
        }
        h.update(&(self.body_family as i32).to_le_bytes());
        h.update(&(self.heading_family as i32).to_le_bytes());
        h.update(&(self.heading_weight as i32).to_le_bytes());
        for scale in self.heading_scale {
            h.update(&scale.to_bits().to_le_bytes());
        }
        h.update(&self.block_gap.to_bits().to_le_bytes());
        h.update(&self.line_height.to_bits().to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_preset_constants() {
        let light = light();
        assert!(!light.dark);
        assert_eq!(light.background, 0xffffffff);
        assert_eq!(light.foreground, 0xff0a0a0a);
        assert_eq!(light.code, GITHUB_LIGHT);
        assert_eq!(light.heading_scale, [1.875, 1.5, 1.25, 1.125, 1.0, 0.875]);
        assert_eq!(light.block_gap, 1.0);
        assert_eq!(light.line_height, 1.75);

        let dark = dark();
        assert!(dark.dark);
        assert_eq!(dark.background, 0xff0a0a0a);
        assert_eq!(dark.foreground, 0xfffafafa);
        assert_eq!(dark.code_bg, 0xff0d1117);
        assert_eq!(dark.code, GITHUB_DARK);
        // `Default` renders like the light preset.
        assert_eq!(Theme::default(), light);
    }
}
