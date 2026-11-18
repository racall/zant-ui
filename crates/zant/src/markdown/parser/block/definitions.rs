//! Link reference definitions and footnote parsing and collection.

use crate::markdown::ast::{self, Block};
use crate::markdown::parser::inline;
use crate::markdown::scan;

use super::Parser;
use super::containers::strip_quote_prefix;

pub struct Definition<'a> {
    pub label: &'a str,
    pub url: &'a str,
    pub title: &'a str,
}

pub(super) struct FootnoteStart<'a> {
    label: &'a str,
    rest: &'a str,
}

#[derive(Clone)]
pub(super) struct FootnoteDef {
    label: String,
    blocks: Vec<Block>,
}

impl Parser {
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

    pub(super) fn collect_definitions(&mut self, lines: &[&str]) {
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

    pub(super) fn footnote_def(
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

    pub(super) fn collect_footnotes(&self) -> Vec<ast::Footnote> {
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
}

pub(super) fn footnote_def_start(line: &str) -> Option<FootnoteStart<'_>> {
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
