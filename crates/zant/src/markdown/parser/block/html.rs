//! HTML blocks and block-level tag recognition.

use crate::markdown::ast::Block;
use crate::markdown::scan;

use super::Parser;

impl Parser {
    pub(super) fn html_block(
        &mut self,
        lines: &[&str],
        start: usize,
        out: &mut Vec<Block>,
    ) -> usize {
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

pub(super) fn is_block_tag(t: &str) -> bool {
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
