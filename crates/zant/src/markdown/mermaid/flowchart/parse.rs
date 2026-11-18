//! Flowchart node and edge parsing, label cleanup, and cycle detection.

use super::super::canvas::{strip_quotes, trim_leading, trim_spaces};
use super::{EdgeStyle, FEdge, FNode, Flow, Shape};
use crate::Str;

struct Link<'a> {
    arrow: bool,
    style: EdgeStyle,
    label: Str,
    rest: &'a str,
}

impl Flow {
    fn node_index(&mut self, id: &str) -> usize {
        for (i, node) in self.nodes.iter().enumerate() {
            if node.id.as_str() == id {
                return i;
            }
        }
        self.nodes.push(FNode::new(id));
        self.nodes.len() - 1
    }

    /// Parses `id`, `id[label]`, `id(label)`, ... Returns node index and rest.
    fn parse_node<'a>(&mut self, text: &'a str) -> Option<(usize, &'a str)> {
        let t = trim_leading(text);
        let bytes = t.as_bytes();
        let mut i: usize = 0;
        while i < t.len() {
            let c = bytes[i];
            let hyphen = c == b'-'
                && i + 1 < t.len()
                && bytes[i + 1].is_ascii_alphanumeric()
                && !(i + 1 < t.len() && bytes[i + 1] == b'-');
            if c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80 || hyphen {
                i += 1;
            } else {
                break;
            }
        }
        if i == 0 {
            return None;
        }
        let id = &t[..i];
        let index = self.node_index(id);
        let mut rest = &t[i..];
        const OPENS: [(&str, &str, Shape); 12] = [
            ("(((", ")))", Shape::Circle),
            ("((", "))", Shape::Circle),
            ("([", "])", Shape::Stadium),
            ("[[", "]]", Shape::Subroutine),
            ("[(", ")]", Shape::Cylinder),
            ("{{", "}}", Shape::Hexagon),
            ("[/", "/]", Shape::Rect),
            ("[\\", "\\]", Shape::Rect),
            ("(", ")", Shape::Round),
            ("[", "]", Shape::Rect),
            ("{", "}", Shape::Diamond),
            (">", "]", Shape::Asymmetric),
        ];
        for (open, close, shape) in OPENS {
            if rest.starts_with(open) {
                let Some(relative) = rest[open.len()..].find(close) else {
                    continue;
                };
                let close_at = open.len() + relative;
                let node = &mut self.nodes[index];
                node.label = clean_label(&rest[open.len()..close_at]);
                node.shape = shape;
                rest = &rest[close_at + close.len()..];
                break;
            }
        }
        // :::class suffix
        if rest.starts_with(":::") {
            let mut j: usize = 3;
            while j < rest.len()
                && (rest.as_bytes()[j].is_ascii_alphanumeric()
                    || rest.as_bytes()[j] == b'_'
                    || rest.as_bytes()[j] == b'-')
            {
                j += 1;
            }
            rest = &rest[j..];
        }
        Some((index, rest))
    }

    fn parse_link<'a>(&self, text: &'a str) -> Option<Link<'a>> {
        let t = trim_leading(text);
        if t.len() < 2 {
            return None;
        }
        let bytes = t.as_bytes();
        let mut label = Str::default();
        // inline text form: -- text --> / == text ==> / -. text .->
        if t.starts_with("-- ") || t.starts_with("== ") || t.starts_with("-. ") {
            const CLOSERS: [&str; 6] = ["-->", "---", "==>", "===", ".->", ".-"];
            let mut best: Option<usize> = None;
            let mut best_len: usize = 0;
            for closer in CLOSERS {
                if let Some(relative) = t[3..].find(closer) {
                    let at = 3 + relative;
                    let replace = match best {
                        None => true,
                        Some(best_at) => at < best_at,
                    };
                    if replace {
                        best = Some(at);
                        best_len = closer.len();
                    }
                }
            }
            if let Some(at) = best {
                label = clean_label(trim_spaces(&t[3..at]));
                let style = if bytes[0] == b'=' {
                    EdgeStyle::Thick
                } else if bytes[1] == b'.' {
                    EdgeStyle::Dotted
                } else {
                    EdgeStyle::Solid
                };
                let arrow = bytes[at + best_len - 1] == b'>';
                return Some(Link {
                    arrow,
                    style,
                    label,
                    rest: &t[at + best_len..],
                });
            }
        }
        let mut i: usize = 0;
        while i < t.len() && b"-=.<>ox".contains(&bytes[i]) {
            // stop "o"/"x" heads that start an identifier
            if (bytes[i] == b'o' || bytes[i] == b'x')
                && i + 1 < t.len()
                && bytes[i + 1].is_ascii_alphanumeric()
            {
                break;
            }
            i += 1;
        }
        let op = &t[..i];
        if op.len() < 2 || (!op.contains('-') && !op.contains('=')) {
            return None;
        }
        let mut rest = &t[i..];
        if !rest.is_empty() && rest.as_bytes()[0] == b'|' {
            let relative = rest[1..].find('|')?;
            let close = 1 + relative;
            label = clean_label(&rest[1..close]);
            rest = &rest[close + 1..];
        }
        Some(Link {
            arrow: op.bytes().any(|b| b == b'>' || b == b'o' || b == b'x'),
            style: if op.contains('=') {
                EdgeStyle::Thick
            } else if op.contains('.') {
                EdgeStyle::Dotted
            } else {
                EdgeStyle::Solid
            },
            label,
            rest,
        })
    }

    /// `A & B --> C --> D`
    pub(in crate::markdown::mermaid) fn parse_statement(&mut self, statement: &str) {
        let mut rest = statement;
        let mut current: Vec<usize> = Vec::new();
        loop {
            let Some((index, tail)) = self.parse_node(rest) else {
                return;
            };
            current.push(index);
            rest = trim_leading(tail);
            if !rest.is_empty() && rest.as_bytes()[0] == b'&' {
                rest = &rest[1..];
                continue;
            }
            break;
        }
        loop {
            let Some(link) = self.parse_link(rest) else {
                return;
            };
            rest = link.rest;
            let mut targets: Vec<usize> = Vec::new();
            loop {
                let Some((index, tail)) = self.parse_node(rest) else {
                    return;
                };
                targets.push(index);
                rest = trim_leading(tail);
                if !rest.is_empty() && rest.as_bytes()[0] == b'&' {
                    rest = &rest[1..];
                    continue;
                }
                break;
            }
            for from in &current {
                for to in &targets {
                    self.edges.push(FEdge {
                        from: *from,
                        to: *to,
                        label: link.label.clone(),
                        arrow: link.arrow,
                        style: link.style,
                    });
                }
            }
            current = targets;
        }
    }
}

fn clean_label(raw: &str) -> Str {
    let mut t = strip_quotes(raw);
    let bytes = t.as_bytes();
    // markdown strings "`**x**`" and <br> line breaks: keep text simple
    if t.len() >= 2 && bytes[0] == b'`' && bytes[t.len() - 1] == b'`' {
        t = &t[1..t.len() - 1];
    }
    if !t.contains("<br") && !t.contains('*') {
        return Str::copy(t);
    }
    let bytes = t.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(t.len());
    let mut i: usize = 0;
    while i < t.len() {
        if bytes[i] == b'<'
            && let Some(relative) = t[i..].find('>')
        {
            out.push(b' ');
            i = i + relative + 1;
            continue;
        }
        if bytes[i] != b'*' {
            out.push(bytes[i]);
        }
        i += 1;
    }
    Str::from(String::from_utf8(out).expect("clean_label preserves valid UTF-8"))
}

pub(super) fn dfs_visit(flow: &Flow, state: &mut [u8], back: &mut [bool], v: usize) {
    state[v] = 1;
    for (k, edge) in flow.edges.iter().enumerate() {
        if edge.from != v {
            continue;
        }
        if state[edge.to] == 1 {
            back[k] = true;
        } else if state[edge.to] == 0 {
            dfs_visit(flow, state, back, edge.to);
        }
    }
    state[v] = 2;
}
