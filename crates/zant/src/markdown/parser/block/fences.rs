//! Indented and fenced code blocks and display math delimiters.

use crate::markdown::ast::{self, Block};
use crate::markdown::scan;

use super::Parser;

impl Parser {
    pub(super) fn indented_code(
        &mut self,
        lines: &[&str],
        start: usize,
        out: &mut Vec<Block>,
    ) -> usize {
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

    pub(super) fn fenced_code(
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

    pub(super) fn math_block(
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
    pub(super) fn bracket_math_block(
        &mut self,
        lines: &[&str],
        start: usize,
        out: &mut Vec<Block>,
    ) -> usize {
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
}

#[derive(Clone, Copy)]
pub(super) struct MathFence {
    len: usize,
    indent: usize,
}

/// micromark-extension-math (mathFlow) opening fence: up to 3 spaces,
/// two or more `$`, then meta that must not contain `$`.
pub(super) fn math_start(line: &str) -> Option<MathFence> {
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
pub(super) fn bracket_math_start(line: &str) -> bool {
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
