//! Block-level Markdown parser (CommonMark + GFM): headings, paragraphs,
//! fenced/indented code, block quotes and GFM alerts, nested lists with task
//! items, tables, math, HTML blocks, footnote and link definitions, front
//! matter.
//!
//! Container blocks are parsed recursively on their stripped lines. The
//! top-level pass can also report the source line range of every block,
//! which `blocks.rs` (Streamdown's parse-blocks) uses to split documents.

mod containers;
mod definitions;
mod fences;
mod html;
mod lines;
mod table;

use std::collections::HashMap;

use crate::core::hash::{HashValue, Wyhash};
use crate::markdown::ast::{self, Block, Span};
use crate::markdown::parser::inline;
use crate::markdown::scan;

pub use definitions::Definition;
use definitions::{FootnoteDef, footnote_def_start};
use fences::{bracket_math_start, math_start};
use html::is_block_tag;
use lines::line_offset;
pub use lines::{line_starts, split_lines};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Options {
    /// `$x$` inline math with pandoc's rules (see inline.rs), on by default.
    pub single_dollar_math: bool,
    /// Recognise YAML front matter at the very start of the document.
    pub front_matter: bool,
    /// Also recognise LaTeX's `\(…\)` (inline) and `\[…\]` (display) math
    /// delimiters, as many language models write them. Off by default
    /// (remark-math does not; `\[` is otherwise an escaped bracket).
    pub latex_delimiters: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            single_dollar_math: true,
            front_matter: true,
            latex_delimiters: false,
        }
    }
}

/// Structural hash used by `cache.rs` for the parse options.
impl HashValue for Options {
    fn hash_value(&self, h: &mut Wyhash) {
        self.single_dollar_math.hash_value(h);
        self.front_matter.hash_value(h);
        self.latex_delimiters.hash_value(h);
    }
}

pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

pub struct Parser {
    pub options: Options,
    pub refs: inline::RefMap,
    pub footnote_order: Vec<String>,
    footnote_defs: Vec<FootnoteDef>,
    pub slugs: HashMap<String, usize>,
    /// When set, inline content is not parsed (block splitting only).
    pub structure_only: bool,
}

impl Parser {
    pub fn new(options: Options) -> Parser {
        Parser {
            options,
            refs: inline::RefMap::new(),
            footnote_order: Vec::new(),
            footnote_defs: Vec::new(),
            slugs: HashMap::new(),
            structure_only: false,
        }
    }

    fn inlines(&mut self, text: &str) -> Vec<Span> {
        if self.structure_only {
            return Vec::new();
        }
        let mut ctx = inline::Context {
            refs: &self.refs,
            footnote_order: &mut self.footnote_order,
            single_dollar_math: self.options.single_dollar_math,
            latex_delimiters: self.options.latex_delimiters,
        };
        inline::parse(&mut ctx, text)
    }

    // ── document ────────────────────────────────────────────────────────

    pub fn parse_document(
        &mut self,
        source: &str,
        mut ranges: Option<&mut Vec<LineRange>>,
    ) -> ast::Parts {
        let lines = split_lines(source);
        let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        self.collect_definitions(&line_refs);
        let mut front: &str = "";
        let mut first: usize = 0;
        if self.options.front_matter
            && line_refs.len() > 1
            && line_refs[0].trim_end_matches([' ', '\t']) == "---"
        {
            let mut j: usize = 1;
            while j < line_refs.len() {
                let t = line_refs[j].trim_end_matches([' ', '\t']);
                if t == "---" || t == "..." {
                    break;
                }
                j += 1;
            }
            if j < line_refs.len() && j > 1 {
                let start = line_offset(source, 1);
                let end = line_offset(source, j);
                front = source[start..end].trim_end_matches('\n');
                first = j + 1;
                if let Some(r) = ranges.as_deref_mut() {
                    r.push(LineRange {
                        start: 0,
                        end: first,
                    });
                }
            }
        }
        let blocks = self.parse_lines(&line_refs[first..], ranges, first);
        ast::Parts {
            blocks,
            footnotes: self.collect_footnotes(),
            front_matter: front.to_string(),
        }
    }

    fn slug_for(&mut self, spans: &[Span]) -> String {
        if self.structure_only {
            return String::new();
        }
        let mut out: Vec<u8> = Vec::new();
        for span in spans {
            for c in span.text.bytes() {
                if c.is_ascii_alphanumeric() {
                    out.push(c.to_ascii_lowercase());
                } else if c == b' ' || c == b'-' {
                    out.push(b'-');
                } else if c >= 0x80 {
                    out.push(c);
                }
            }
        }
        let base = String::from_utf8(out).expect("slugs stay valid UTF-8");
        match self.slugs.get_mut(&base) {
            None => {
                self.slugs.insert(base.clone(), 0);
                base
            }
            Some(value) => {
                *value += 1;
                format!("{base}-{value}")
            }
        }
    }

    // ── blocks ──────────────────────────────────────────────────────────

    /// Parses a sequence of (container-stripped) lines. When `ranges` is set,
    /// records the line range of each emitted block (offset by `base`).
    pub fn parse_lines(
        &mut self,
        lines: &[&str],
        mut ranges: Option<&mut Vec<LineRange>>,
        base: usize,
    ) -> Vec<Block> {
        let mut blocks: Vec<Block> = Vec::new();
        let mut i: usize = 0;
        while i < lines.len() {
            let line = lines[i];
            if scan::is_blank(line) {
                i += 1;
                continue;
            }
            let start = i;
            let before = blocks.len();
            i = self.parse_block(lines, i, &mut blocks);
            if let Some(r) = ranges.as_deref_mut()
                && (blocks.len() > before || i > start)
            {
                r.push(LineRange {
                    start: base + start,
                    end: base + i,
                });
            }
        }
        blocks
    }

    fn parse_block(&mut self, lines: &[&str], i: usize, out: &mut Vec<Block>) -> usize {
        let line = lines[i];
        let indent = scan::indent_of(line);

        if indent >= 4 {
            return self.indented_code(lines, i, out);
        }
        if let Some(fence) = scan::fence_open(line) {
            return self.fenced_code(lines, i, fence, out);
        }
        if let Some(fence) = math_start(line) {
            return self.math_block(lines, i, fence, out);
        }
        if self.options.latex_delimiters && bracket_math_start(line) {
            return self.bracket_math_block(lines, i, out);
        }
        if let Some(h) = scan::atx_heading(line) {
            let spans = self.inlines(h.text);
            let id = self.slug_for(&spans);
            out.push(Block::Heading(ast::Heading {
                level: h.level,
                spans,
                id,
            }));
            return i + 1;
        }
        if scan::is_thematic_break(line) {
            out.push(Block::Rule);
            return i + 1;
        }
        let t = &line[indent..];
        if !t.is_empty() && t.as_bytes()[0] == b'>' {
            return self.blockquote(lines, i, out);
        }
        if scan::list_marker(line).is_some() {
            return self.list(lines, i, out);
        }
        if let Some(fd) = footnote_def_start(line) {
            return self.footnote_def(lines, i, fd, out);
        }
        if Parser::parse_definition(line).is_some() {
            return i + 1;
        }
        if scan::is_html_block_start(line) {
            return self.html_block(lines, i, out);
        }
        let next = if i + 1 < lines.len() && line.as_bytes().contains(&b'|') {
            self.table(lines, i, out)
        } else {
            None
        };
        if let Some(next) = next {
            return next;
        }
        self.paragraph(lines, i, out)
    }

    fn interrupts_paragraph(&self, line: &str) -> bool {
        if scan::indent_of(line) > 3 {
            return false;
        }
        if self.options.latex_delimiters && bracket_math_start(line) {
            return true;
        }
        if scan::atx_heading(line).is_some()
            || scan::fence_open(line).is_some()
            || scan::is_thematic_break(line)
        {
            return true;
        }
        let t = line.trim_start_matches(' ');
        if !t.is_empty() && t.as_bytes()[0] == b'>' {
            return true;
        }
        if math_start(line).is_some() {
            return true;
        }
        if let Some(m) = scan::list_marker(line) {
            // empty items and ordered lists not starting at 1 cannot interrupt
            if m.rest.is_empty() {
                return false;
            }
            if m.ordered && m.number != 1 {
                return false;
            }
            return true;
        }
        if scan::is_html_block_start(line) {
            // only block-level tags interrupt a paragraph
            return is_block_tag(t);
        }
        false
    }

    fn paragraph(&mut self, lines: &[&str], start: usize, out: &mut Vec<Block>) -> usize {
        let mut body = String::new();
        let mut i = start;
        while i < lines.len() {
            let line = lines[i];
            if scan::is_blank(line) {
                break;
            }
            if i > start {
                if let Some(level) = scan::setext_level(line) {
                    let spans = self.inlines(&body);
                    let id = self.slug_for(&spans);
                    out.push(Block::Heading(ast::Heading { level, spans, id }));
                    return i + 1;
                }
                if self.interrupts_paragraph(line) {
                    break;
                }
                // a table can start right after paragraph text
                if line.as_bytes().contains(&b'|')
                    && i + 1 < lines.len()
                    && let Some(aligns) = scan::table_delimiter(lines[i + 1])
                    && scan::split_table_row(line).len() == aligns.len()
                {
                    break;
                }
                body.push('\n');
            }
            body.push_str(line.trim_start_matches(' '));
            i += 1;
        }
        let text = body.trim_end_matches([' ', '\t']);
        let spans = self.inlines(text);
        // a paragraph that is a single image becomes an image block
        if spans.len() == 1 && spans[0].style.image {
            out.push(Block::Image(ast::Image {
                alt: spans[0].text.clone(),
                url: spans[0].url.clone(),
                title: String::new(),
            }));
        } else {
            out.push(Block::Paragraph(spans));
        }
        i
    }
}
