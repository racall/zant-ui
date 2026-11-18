//! TeX box layout, ported from KaTeX's HTML builder (`buildHTML.js`,
//! `functions/{supsub,genfrac,sqrt,op,delimsizing,accent,…}.js`,
//! `environments/array.js`) and its font metrics (`fontMetrics.js`).
//!
//! Every box is measured in pixels with its origin on the baseline at the
//! left edge; y grows downwards. Glyphs are drawn with STIX Two Math.
//!
//! The public entry point ([`Options`] + [`layout`]) lives at the end of this
//! file; the engine's own options are [`EngineOptions`]. The `ui` wrappers
//! (`math::block`, `math::render`) are in `math/block.rs`.

use crate::core::hash::{HashValue, Wyhash};
pub use crate::core::vector::Drawing;
use crate::core::vector::{self, Command, Text as VectorText};
use crate::math::parser;
use crate::renderer::text::Family;

mod accents;
mod arrays;
mod arrows;
mod boxes;
mod decorations;
mod delimiters;
mod fractions;
mod primitives;
mod scripts;
mod style;

pub use boxes::Box;
pub use style::{Size, Style};

/// The layout engine's options.
#[derive(Clone, Copy, Debug)]
pub struct EngineOptions {
    /// Pixel size of 1em in text style.
    pub size: f32,
    pub color: u32,
    pub error_color: u32,
}

pub struct Engine {
    pub options: EngineOptions,
    pub color: u32,
    pub tag: Option<Box>,
    /// Delimiter height for `\middle` inside the current `\left…\right`.
    pub middle_total: Option<f32>,
}

impl Engine {
    pub fn init(options: EngineOptions) -> Engine {
        Engine {
            options,
            color: options.color,
            tag: None,
            middle_total: None,
        }
    }
}

// ── public entry point ───────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Font size of the surrounding text (px).
    pub size: f32,
    /// Math is drawn `scale` times larger than the surrounding text
    /// (KaTeX: 1.21 with Computer Modern; STIX Two has a larger x-height).
    pub scale: f32,
    pub color: u32,
    /// Streamdown's default `errorColor` is the muted foreground.
    pub error_color: u32,
    /// Display (block) style; false = inline/text style.
    pub display: bool,
    /// Shrinks the formula to fit this width (display math).
    pub max_width: Option<f32>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            size: 16.0,
            scale: 1.1,
            color: 0xff292d2a,
            error_color: 0xff8b918c,
            display: true,
            max_width: None,
        }
    }
}

impl HashValue for Options {
    fn hash_value(&self, h: &mut Wyhash) {
        // the fields in declaration order
        self.size.hash_value(h);
        self.scale.hash_value(h);
        self.color.hash_value(h);
        self.error_color.hash_value(h);
        self.display.hash_value(h);
        self.max_width.hash_value(h);
    }
}

/// Lays out `source` into a drawing.
pub fn layout(source: &str, options: Options) -> Drawing {
    let mut drawing = layout_once(source, &options, options.size * options.scale);
    if let Some(max) = options.max_width
        && drawing.width > max
        && drawing.width > 0.0
    {
        let factor = (max / drawing.width).max(0.5);
        drawing = layout_once(source, &options, options.size * options.scale * factor);
    }
    let mut h = Wyhash::init(0x6d617468);
    h.update(source.as_bytes());
    options.hash_value(&mut h);
    drawing.hash = h.finish();
    drawing
}

fn layout_once(source: &str, options: &Options, size: f32) -> Drawing {
    let nodes = match parser::parse(source) {
        Ok(nodes) => nodes,
        Err(_) => return error_drawing(source, options, size),
    };
    let mut engine = Engine::init(EngineOptions {
        size,
        color: options.color,
        error_color: options.error_color,
    });
    let style = if options.display {
        Style::DISPLAY
    } else {
        Style::TEXT
    };
    let mut box_ = engine.list(&nodes, style);
    if let Some(tag) = engine.tag.take() {
        // \tag{n}: placed after a quad
        let mut cmds: Vec<Command> = box_.cmds.clone();
        let x = box_.width + 2.0 * size;
        for cmd in &tag.cmds {
            cmds.push(vector::translate(cmd, x, 0.0));
        }
        box_.cmds = cmds;
        box_.width = x + tag.width;
        box_.height = box_.height.max(tag.height);
        box_.depth = box_.depth.max(tag.depth);
    }
    Drawing {
        width: box_.width,
        ascent: box_.height,
        descent: box_.depth,
        commands: box_.cmds,
        hash: 0,
    }
}

fn error_drawing(source: &str, options: &Options, size: f32) -> Drawing {
    let text = collapse_whitespace(source);
    let m = vector::measure(&text, size * 0.9, Family::Math, 400, false);
    let cmds = vec![Command::Text(VectorText {
        x: 0.0,
        y: 0.0,
        text: crate::Str::copy(&text),
        size: size * 0.9,
        family: Family::Math,
        weight: 400,
        italic: false,
        color: options.error_color,
        scale_y: 1.0,
    })];
    Drawing {
        width: m.advance,
        ascent: m.ascent.max(m.font_ascent),
        descent: m.descent.max(m.font_descent),
        commands: cmds,
        hash: 0,
    }
}

/// The source shown for a formula that does not parse (often one still
/// streaming): whitespace runs, line breaks included, become one space, as the
/// browser shows KaTeX's error span. A newline would draw as a missing glyph.
fn collapse_whitespace(source: &str) -> String {
    let trimmed = source.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    let mut out: Vec<u8> = Vec::with_capacity(trimmed.len());
    let mut space = false;
    for &c in trimmed.as_bytes() {
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            space = true;
            continue;
        }
        if space {
            out.push(b' ');
            space = false;
        }
        out.push(c);
    }
    // Only ASCII spaces were inserted at character boundaries, so this is
    // still valid UTF-8.
    String::from_utf8(out).expect("collapse_whitespace keeps UTF-8 valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_source_collapses_line_breaks() {
        assert_eq!(
            collapse_whitespace("\n\\begin{aligned}\n  f(x) &= a \\\\\r\n&= b\n"),
            "\\begin{aligned} f(x) &= a \\\\ &= b"
        );
    }
}
