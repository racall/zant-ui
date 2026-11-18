//! Native Mermaid renderer for the diagram types AI answers and notes use
//! most: `flowchart`/`graph`, `sequenceDiagram` and `pie`. Produces a
//! [`vector::Drawing`](crate::core::vector::Drawing); unsupported diagrams return `None` and are shown as code.

use crate::core::hash::Wyhash;
use crate::core::vector::Drawing;

mod canvas;
mod flowchart;
mod pie;
mod sequence;

#[cfg(test)]
mod tests;

use self::canvas::trim_line;
use self::flowchart::flowchart;
use self::pie::pie;
use self::sequence::sequence;

/// Theme colours supplied by the markdown renderer (see
/// `markdown::render::diagram`: text / node fill / node border / edge / label
/// background / muted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colors {
    pub text: u32,
    pub node_fill: u32,
    pub node_border: u32,
    pub edge: u32,
    pub label_bg: u32,
    pub muted: u32,
}

/// `colors` is mandatory, so there is no `Default` impl: start from
/// [`Options::new`].
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub font_size: f32,
    pub colors: Colors,
    pub max_width: f32,
}

impl Options {
    /// The default options with `colors`: 14 px text, diagrams wider than
    /// 640 px shrink to fit.
    pub fn new(colors: Colors) -> Options {
        Options {
            font_size: 14.0,
            colors,
            max_width: 640.0,
        }
    }
}

const PALETTE: [u32; 10] = [
    0xff4e79a7, 0xfff28e2b, 0xffe15759, 0xff76b7b2, 0xff59a14f, 0xffedc948, 0xffb07aa1, 0xffff9da7,
    0xff9c755f, 0xffbab0ac,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Flowchart,
    Sequence,
    Pie,
}

pub fn detect(source: &str) -> Option<Kind> {
    let mut in_front_matter = false;
    for raw in source.split('\n') {
        let line = trim_line(raw);
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if line == "---" {
            in_front_matter = !in_front_matter;
            continue;
        }
        if in_front_matter {
            continue;
        }
        let word_end = line.find([' ', '\t']).unwrap_or(line.len());
        let word = &line[..word_end];
        if word == "graph" || word == "flowchart" {
            return Some(Kind::Flowchart);
        }
        if word == "sequenceDiagram" {
            return Some(Kind::Sequence);
        }
        if word == "pie" {
            return Some(Kind::Pie);
        }
        return None;
    }
    None
}

pub fn render(source: &str, options: Options) -> Option<Drawing> {
    let kind = detect(source)?;
    let mut drawing = match kind {
        Kind::Flowchart => flowchart(source, &options),
        Kind::Sequence => sequence(source, &options),
        Kind::Pie => pie(source, &options),
    }?;
    // wide diagrams shrink to the column (like display math)
    if options.max_width > 0.0 && drawing.width > options.max_width {
        drawing = drawing.scaled(options.max_width / drawing.width);
    }
    let mut hasher = Wyhash::init(0x6d6d64);
    hasher.update(source.as_bytes());
    // The options are hashed as their little-endian bytes: font_size, then
    // the six colours in declaration order, then max_width.
    hasher.update(&options.font_size.to_le_bytes());
    for color in colors_in_memory_order(options.colors) {
        hasher.update(&color.to_le_bytes());
    }
    hasher.update(&options.max_width.to_le_bytes());
    drawing.hash = hasher.finish();
    Some(drawing)
}

/// The colours in `Colors` declaration order (the order `mem::asBytes` saw).
fn colors_in_memory_order(colors: Colors) -> [u32; 6] {
    [
        colors.text,
        colors.node_fill,
        colors.node_border,
        colors.edge,
        colors.label_bg,
        colors.muted,
    ]
}
