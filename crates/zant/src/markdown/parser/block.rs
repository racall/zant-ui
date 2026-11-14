//! Block-level Markdown parser (CommonMark + GFM): headings, paragraphs,
//! fenced/indented code, block quotes and GFM alerts, nested lists with task
//! items, tables, math, HTML blocks, footnote and link definitions, front
//! matter.
//!
//! Container blocks are parsed recursively on their stripped lines. The
//! top-level pass can also report the source line range of every block,
//! which `blocks.rs` (Streamdown's parse-blocks) uses to split documents.

use std::collections::HashMap;

use crate::core::hash::{HashValue, Wyhash};
use crate::markdown::ast::{self, Block, Span};
use crate::markdown::parser::inline;
use crate::markdown::scan;

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

#[derive(Clone)]
struct FootnoteDef {
    label: String,
    blocks: Vec<Block>,
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

    // ── link reference definitions ──────────────────────────────────────

    pub fn parse_definition(line: &str) -> Option<Definition<'_>> {
        if scan::indent_of(line) > 3 {
            return None;
        }
        let t = scan::trim_space_tab(line);
        if t.len() < 4 || t.as_bytes()[0] != b'[' || (t.len() > 1 && t.as_bytes()[1] == b'^') {
            return None;
        }
        let close = t.find("]:")?;
        if close <= 1 {
            return None;
        }
        let label = &t[1..close];
        if label.as_bytes().contains(&b']') {
            return None;
        }
        let mut rest = scan::trim_space_tab(&t[close + 2..]);
        if rest.is_empty() {
            return None;
        }
        let url: &str;
        if rest.as_bytes()[0] == b'<' {
            let end = rest.find('>')?;
            url = &rest[1..end];
            rest = scan::trim_space_tab(&rest[end + 1..]);
        } else {
            let end = rest
                .bytes()
                .position(|c| c == b' ' || c == b'\t')
                .unwrap_or(rest.len());
            url = &rest[..end];
            rest = scan::trim_space_tab(&rest[end..]);
        }
        let mut title: &str = "";
        if rest.len() >= 2
            && (rest.as_bytes()[0] == b'"'
                || rest.as_bytes()[0] == b'\''
                || rest.as_bytes()[0] == b'(')
        {
            let closing: u8 = if rest.as_bytes()[0] == b'(' {
                b')'
            } else {
                rest.as_bytes()[0]
            };
            if rest.as_bytes()[rest.len() - 1] != closing {
                return None;
            }
            title = &rest[1..rest.len() - 1];
        } else if !rest.is_empty() {
            return None;
        }
        Some(Definition { label, url, title })
    }

    fn collect_definitions(&mut self, lines: &[&str]) {
        let mut fence: Option<scan::Fence> = None;
        let mut prev_blank = true;
        for raw in lines {
            let line = strip_quote_prefix(raw);
            if let Some(f) = fence.as_ref() {
                if scan::is_fence_close(line, f) {
                    fence = None;
                }
                continue;
            }
            if let Some(f) = scan::fence_open(line) {
                fence = Some(f);
                continue;
            }
            if (prev_blank || Parser::parse_definition(line).is_some())
                && let Some(def) = Parser::parse_definition(line)
            {
                let key = inline::normalize_ref_label(def.label);
                self.refs.entry(key).or_insert(inline::LinkDef {
                    url: def.url.to_string(),
                    title: def.title.to_string(),
                });
                continue;
            }
            prev_blank = scan::is_blank(line);
        }
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

    fn collect_footnotes(&self) -> Vec<ast::Footnote> {
        if self.footnote_defs.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        // referenced notes in reference order, then the unreferenced ones
        for (idx, label) in self.footnote_order.iter().enumerate() {
            for def in &self.footnote_defs {
                if &def.label == label {
                    out.push(ast::Footnote {
                        label: label.clone(),
                        number: idx + 1,
                        blocks: def.blocks.clone(),
                    });
                    break;
                }
            }
        }
        out
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

    fn indented_code(&mut self, lines: &[&str], start: usize, out: &mut Vec<Block>) -> usize {
        let mut body = String::new();
        let mut i = start;
        let mut last_content = start;
        while i < lines.len() {
            let line = lines[i];
            if scan::is_blank(line) {
                i += 1;
                continue;
            }
            if scan::indent_of(line) < 4 {
                break;
            }
            last_content = i;
            i += 1;
        }
        for (k, line) in lines[start..last_content + 1].iter().enumerate() {
            if k > 0 {
                body.push('\n');
            }
            body.push_str(scan::strip_indent(line, 4));
        }
        out.push(Block::Code(ast::Code {
            text: body,
            fenced: false,
            ..Default::default()
        }));
        last_content + 1
    }

    fn fenced_code(
        &mut self,
        lines: &[&str],
        start: usize,
        fence: scan::Fence,
        out: &mut Vec<Block>,
    ) -> usize {
        let mut body = String::new();
        let mut i = start + 1;
        let mut closed = false;
        while i < lines.len() {
            if scan::is_fence_close(lines[i], &fence) {
                closed = true;
                i += 1;
                break;
            }
            if i > start + 1 {
                body.push('\n');
            }
            body.push_str(scan::strip_indent(lines[i], fence.indent));
            i += 1;
        }
        let info = fence.info;
        let lang_end = info
            .bytes()
            .position(|c| c == b' ' || c == b'\t' || c == b'{')
            .unwrap_or(info.len());
        out.push(Block::Code(ast::Code {
            lang: info[..lang_end].to_string(),
            meta: scan::trim_space_tab(&info[lang_end..]).to_string(),
            text: body,
            closed,
            fenced: true,
        }));
        i
    }

    fn math_block(
        &mut self,
        lines: &[&str],
        start: usize,
        fence: MathFence,
        out: &mut Vec<Block>,
    ) -> usize {
        let mut source = String::new();
        let mut i = start + 1;
        let mut closed = false;
        while i < lines.len() {
            if is_math_close(lines[i], &fence) {
                closed = true;
                i += 1;
                break;
            }
            // still streaming: the last line is the start of the closing fence
            if i + 1 == lines.len() && is_partial_math_close(lines[i], &fence) {
                break;
            }
            if i > start + 1 {
                source.push('\n');
            }
            source.push_str(scan::strip_indent(lines[i], fence.indent));
            i += 1;
        }
        out.push(Block::Math(ast::Math { source, closed }));
        i
    }

    /// `\[ … \]` display math: on one line or up to the line ending in `\]`.
    fn bracket_math_block(&mut self, lines: &[&str], start: usize, out: &mut Vec<Block>) -> usize {
        let mut source = String::new();
        let first = lines[start].trim_start_matches(' ');
        let mut rest = &first[2..];
        let mut i = start;
        let mut closed = false;
        loop {
            let t = rest.trim_end_matches([' ', '\t']);
            if let Some(body) = t.strip_suffix("\\]") {
                source.push_str(body);
                closed = true;
                i += 1;
                break;
            }
            source.push_str(rest);
            i += 1;
            if i >= lines.len() {
                break;
            }
            source.push('\n');
            rest = lines[i];
        }
        let mut trimmed = source.trim_matches([' ', '\n']);
        // still streaming: a lone trailing backslash is the start of `\]`
        if !closed && trailing_backslashes(trimmed) % 2 == 1 {
            trimmed = trimmed[..trimmed.len() - 1].trim_end_matches([' ', '\n']);
        }
        out.push(Block::Math(ast::Math {
            source: trimmed.to_string(),
            closed,
        }));
        i
    }

    fn blockquote(&mut self, lines: &[&str], start: usize, out: &mut Vec<Block>) -> usize {
        let mut inner: Vec<&str> = Vec::new();
        let mut i = start;
        let mut lazy_ok = false;
        while i < lines.len() {
            let line = lines[i];
            if is_quote_line(line) {
                let stripped = strip_quote(line);
                inner.push(stripped);
                lazy_ok = !scan::is_blank(stripped) && scan::fence_open(stripped).is_none();
                i += 1;
                continue;
            }
            // lazy continuation of a paragraph inside the quote
            if lazy_ok && !scan::is_blank(line) && !self.interrupts_paragraph(line) {
                inner.push(line);
                i += 1;
                continue;
            }
            break;
        }
        // GFM alerts: > [!NOTE]
        if !inner.is_empty() {
            let first = scan::trim_space_tab(inner[0]);
            if let Some(kind) = callout_kind(first) {
                let blocks = self.parse_lines(&inner[1..], None, 0);
                out.push(Block::Callout(ast::Callout { kind, blocks }));
                return i;
            }
        }
        let blocks = self.parse_lines(&inner, None, 0);
        out.push(Block::Quote(blocks));
        i
    }

    fn list(&mut self, lines: &[&str], start: usize, out: &mut Vec<Block>) -> usize {
        let first = scan::list_marker(lines[start]).expect("list marker");
        let mut items: Vec<ast::ListItem> = Vec::new();
        let mut tight = true;
        let mut i = start;
        while i < lines.len() {
            let Some(marker) = scan::list_marker(lines[i]) else {
                break;
            };
            if !same_list_type(&first, &marker) {
                break;
            }
            if scan::is_thematic_break(lines[i]) {
                break;
            }
            let marker_indent = scan::indent_of(lines[i]);
            let content_indent = marker.content_indent;
            let mut item_lines: Vec<&str> = Vec::new();
            let mut rest = marker.rest;
            let mut checked: Option<bool> = None;
            if rest.len() >= 3
                && rest.as_bytes()[0] == b'['
                && rest.as_bytes()[2] == b']'
                && (rest.len() == 3 || rest.as_bytes()[3] == b' ')
            {
                match rest.as_bytes()[1] {
                    b' ' => checked = Some(false),
                    b'x' | b'X' => checked = Some(true),
                    _ => {}
                }
                if checked.is_some() {
                    rest = if rest.len() > 4 { &rest[4..] } else { "" };
                }
            }
            item_lines.push(rest);
            let mut j = i + 1;
            let mut saw_blank = false;
            let mut in_fence: Option<scan::Fence> = scan::fence_open(rest);
            while j < lines.len() {
                let line = lines[j];
                if scan::is_blank(line) {
                    item_lines.push("");
                    saw_blank = true;
                    j += 1;
                    continue;
                }
                let ind = scan::indent_of(line);
                let nested_marker = ind >= marker_indent + 2
                    && scan::list_marker(scan::strip_indent(line, ind)).is_some();
                if ind >= content_indent
                    || nested_marker
                    || (in_fence.is_some() && ind > marker_indent)
                {
                    let stripped = scan::strip_indent(line, ind.min(content_indent));
                    if let Some(f) = in_fence.as_ref() {
                        if scan::is_fence_close(stripped, f) {
                            in_fence = None;
                        }
                    } else if let Some(f) = scan::fence_open(stripped) {
                        in_fence = Some(f);
                    }
                    item_lines.push(stripped);
                    j += 1;
                    continue;
                }
                // lazy paragraph continuation
                if !saw_blank
                    && in_fence.is_none()
                    && !self.interrupts_paragraph(line)
                    && scan::list_marker(line).is_none()
                {
                    let prev = *item_lines.last().expect("item lines are never empty");
                    if !scan::is_blank(prev) {
                        item_lines.push(line.trim_start_matches(' '));
                        j += 1;
                        continue;
                    }
                }
                break;
            }
            // trailing blank lines belong between items, not to this item
            let mut content_end = item_lines.len();
            while content_end > 1 && scan::is_blank(item_lines[content_end - 1]) {
                content_end -= 1;
            }
            let trailing_blank = content_end < item_lines.len();
            for l in &item_lines[..content_end] {
                if scan::is_blank(l) {
                    tight = false;
                    break;
                }
            }
            let blocks = self.parse_lines(&item_lines[..content_end], None, 0);
            items.push(ast::ListItem { checked, blocks });
            i = j;
            if i < lines.len()
                && trailing_blank
                && let Some(next) = scan::list_marker(lines[i])
                && same_list_type(&first, &next)
            {
                tight = false;
            }
            // return the trailing blank lines to the caller when the list ends
            let ends_list = match if i < lines.len() {
                scan::list_marker(lines[i])
            } else {
                None
            } {
                None => true,
                Some(next) => !same_list_type(&first, &next),
            };
            if trailing_blank && ends_list {
                i -= item_lines.len() - content_end;
                break;
            }
        }
        out.push(Block::List(ast::List {
            ordered: first.ordered,
            start: first.number,
            tight,
            items,
        }));
        i
    }

    fn footnote_def(
        &mut self,
        lines: &[&str],
        start: usize,
        fd: FootnoteStart,
        out: &mut Vec<Block>,
    ) -> usize {
        let _ = out;
        let mut body: Vec<&str> = vec![fd.rest];
        let mut i = start + 1;
        let mut saw_blank = false;
        while i < lines.len() {
            let line = lines[i];
            if scan::is_blank(line) {
                body.push("");
                saw_blank = true;
                i += 1;
                continue;
            }
            if scan::indent_of(line) >= 2 {
                body.push(scan::strip_indent(line, 4));
                saw_blank = false;
                i += 1;
                continue;
            }
            if !saw_blank && !self.interrupts_paragraph(line) && footnote_def_start(line).is_none()
            {
                body.push(line);
                i += 1;
                continue;
            }
            break;
        }
        let mut end = body.len();
        while end > 1 && scan::is_blank(body[end - 1]) {
            end -= 1;
        }
        let consumed_blank = body.len() - end;
        let blocks = self.parse_lines(&body[..end], None, 0);
        self.footnote_defs.push(FootnoteDef {
            label: fd.label.to_string(),
            blocks,
        });
        i - consumed_blank
    }

    fn html_block(&mut self, lines: &[&str], start: usize, out: &mut Vec<Block>) -> usize {
        let mut raw = String::new();
        let first = lines[start].trim_start_matches(' ');
        let is_comment = first.starts_with("<!--");
        let mut i = start;
        while i < lines.len() {
            let line = lines[i];
            if !is_comment && i > start && scan::is_blank(line) {
                break;
            }
            if !raw.is_empty() {
                raw.push('\n');
            }
            raw.push_str(line);
            if is_comment && line.contains("-->") {
                i += 1;
                break;
            }
            i += 1;
        }
        out.push(Block::Html(raw));
        i
    }

    fn table(&mut self, lines: &[&str], start: usize, out: &mut Vec<Block>) -> Option<usize> {
        let aligns = scan::table_delimiter(lines[start + 1])?;
        let header_cells = scan::split_table_row(lines[start]);
        if header_cells.len() != aligns.len() {
            return None;
        }
        let cols = aligns.len();
        let mut headers: Vec<Vec<Span>> = Vec::with_capacity(cols);
        let mut raw_headers: Vec<String> = Vec::with_capacity(cols);
        for cell in &header_cells {
            let spans = self.inlines(cell);
            raw_headers.push(if self.structure_only {
                cell.to_string()
            } else {
                ast::plain_text(&spans)
            });
            headers.push(spans);
        }
        let mut rows: Vec<Vec<Vec<Span>>> = Vec::new();
        let mut raw_rows: Vec<Vec<String>> = Vec::new();
        let mut i = start + 2;
        while i < lines.len() {
            let line = lines[i];
            if scan::is_blank(line) || self.interrupts_paragraph(line) {
                break;
            }
            let cells = scan::split_table_row(line);
            let mut row: Vec<Vec<Span>> = Vec::with_capacity(cols);
            let mut raw_row: Vec<String> = Vec::with_capacity(cols);
            for c in 0..cols {
                let cell = if c < cells.len() {
                    cells[c].as_ref()
                } else {
                    ""
                };
                let spans = self.inlines(cell);
                raw_row.push(if self.structure_only {
                    cell.to_string()
                } else {
                    ast::plain_text(&spans)
                });
                row.push(spans);
            }
            rows.push(row);
            raw_rows.push(raw_row);
            i += 1;
        }
        out.push(Block::Table(ast::Table {
            headers,
            rows,
            aligns,
            raw_headers,
            raw_rows,
        }));
        Some(i)
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

pub struct Definition<'a> {
    pub label: &'a str,
    pub url: &'a str,
    pub title: &'a str,
}

struct FootnoteStart<'a> {
    label: &'a str,
    rest: &'a str,
}

#[derive(Clone, Copy)]
struct MathFence {
    len: usize,
    indent: usize,
}

/// micromark-extension-math (mathFlow) opening fence: up to 3 spaces,
/// two or more `$`, then meta that must not contain `$`.
fn math_start(line: &str) -> Option<MathFence> {
    let indent = scan::indent_of(line);
    if indent > 3 {
        return None;
    }
    let t = &line[indent..];
    let mut n: usize = 0;
    while n < t.len() && t.as_bytes()[n] == b'$' {
        n += 1;
    }
    if n < 2 {
        return None;
    }
    if t.as_bytes()[n..].contains(&b'$') {
        return None;
    }
    Some(MathFence { len: n, indent })
}

/// `\[` opening a display formula (up to 3 spaces of indent).
fn bracket_math_start(line: &str) -> bool {
    let indent = scan::indent_of(line);
    if indent > 3 {
        return false;
    }
    line[indent..].starts_with("\\[")
}

fn trailing_backslashes(text: &str) -> usize {
    let mut n: usize = 0;
    while n < text.len() && text.as_bytes()[text.len() - 1 - n] == b'\\' {
        n += 1;
    }
    n
}

fn is_math_close(line: &str, fence: &MathFence) -> bool {
    let indent = scan::indent_of(line);
    if indent > 3 {
        return false;
    }
    let t = line[indent..].trim_end_matches([' ', '\t']);
    if t.len() < fence.len {
        return false;
    }
    t.bytes().all(|c| c == b'$')
}

fn is_partial_math_close(line: &str, fence: &MathFence) -> bool {
    let indent = scan::indent_of(line);
    if indent > 3 {
        return false;
    }
    let t = line[indent..].trim_end_matches([' ', '\t']);
    if t.is_empty() || t.len() >= fence.len {
        return false;
    }
    t.bytes().all(|c| c == b'$')
}

fn is_quote_line(line: &str) -> bool {
    if scan::indent_of(line) > 3 {
        return false;
    }
    let t = line.trim_start_matches(' ');
    !t.is_empty() && t.as_bytes()[0] == b'>'
}

fn strip_quote(line: &str) -> &str {
    let t = line.trim_start_matches(' ');
    if t.len() > 1 && t.as_bytes()[1] == b' ' {
        &t[2..]
    } else {
        &t[1..]
    }
}

fn callout_kind(line: &str) -> Option<ast::CalloutKind> {
    if line.len() < 5 || !line.starts_with("[!") || !line.ends_with(']') {
        return None;
    }
    let name = &line[2..line.len() - 1];
    for (label, kind) in [
        ("NOTE", ast::CalloutKind::Note),
        ("TIP", ast::CalloutKind::Tip),
        ("IMPORTANT", ast::CalloutKind::Important),
        ("WARNING", ast::CalloutKind::Warning),
        ("CAUTION", ast::CalloutKind::Caution),
    ] {
        if name.eq_ignore_ascii_case(label) {
            return Some(kind);
        }
    }
    None
}

fn same_list_type(a: &scan::ListMarker<'_>, b: &scan::ListMarker<'_>) -> bool {
    if a.ordered != b.ordered {
        return false;
    }
    if a.ordered {
        a.delimiter == b.delimiter
    } else {
        a.bullet == b.bullet
    }
}

fn footnote_def_start(line: &str) -> Option<FootnoteStart<'_>> {
    if scan::indent_of(line) > 3 {
        return None;
    }
    let t = line.trim_start_matches(' ');
    if !t.starts_with("[^") {
        return None;
    }
    let close = t.find("]:")?;
    let label = &t[2..close];
    if label.is_empty() || label.bytes().any(|c| c == b' ' || c == b'\t' || c == b']') {
        return None;
    }
    Some(FootnoteStart {
        label,
        rest: t[close + 2..].trim_start_matches(' '),
    })
}

const BLOCK_TAGS: [&str; 25] = [
    "div",
    "p",
    "table",
    "pre",
    "details",
    "summary",
    "section",
    "article",
    "aside",
    "blockquote",
    "figure",
    "ul",
    "ol",
    "li",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "center",
    "nav",
    "header",
    "footer",
];

fn is_block_tag(t: &str) -> bool {
    let mut name_start: usize = 1;
    if t.len() > 1 && t.as_bytes()[1] == b'/' {
        name_start = 2;
    }
    let mut end = name_start;
    while end < t.len() {
        let c = t.as_bytes()[end];
        if c.is_ascii_alphanumeric() || c == b'!' || c == b'-' {
            end += 1;
        } else {
            break;
        }
    }
    let name = &t[name_start.min(t.len())..end.min(t.len())];
    BLOCK_TAGS.iter().any(|tag| tag.eq_ignore_ascii_case(name))
}

fn strip_quote_prefix(line: &str) -> &str {
    let mut t = line;
    loop {
        let s = t.trim_start_matches(' ');
        if !s.is_empty() && s.as_bytes()[0] == b'>' {
            t = if s.len() > 1 && s.as_bytes()[1] == b' ' {
                &s[2..]
            } else {
                &s[1..]
            };
        } else {
            return t;
        }
    }
}

pub fn split_lines(source: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in source.split('\n') {
        let line = raw.trim_end_matches('\r');
        lines.push(scan::expand_leading_tabs(line).into_owned());
    }
    // a trailing newline does not start another line
    if !lines.is_empty() && !source.is_empty() && source.ends_with('\n') {
        let _ = lines.pop();
    }
    lines
}

/// Byte offset of line `index` in `source` (lines come from `split_lines`).
fn line_offset(source: &str, index: usize) -> usize {
    let mut line: usize = 0;
    let mut i: usize = 0;
    while line < index && i < source.len() {
        if source.as_bytes()[i] == b'\n' {
            line += 1;
        }
        i += 1;
    }
    i
}

/// Byte offsets of the start of every line, plus `source.len()` at the end.
pub fn line_starts(source: &str) -> Vec<usize> {
    let mut starts: Vec<usize> = vec![0];
    for (i, c) in source.bytes().enumerate() {
        if c == b'\n' && i + 1 < source.len() {
            starts.push(i + 1);
        }
    }
    starts.push(source.len());
    starts
}
