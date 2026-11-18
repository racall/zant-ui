//! The recursive inline parsing loop.

use crate::markdown::{ast::Style, scan};

use super::Builder;
use super::autolinks::{bare_autolink_end, email_autolink};
use super::bytes::{at, index_of_pos, index_of_scalar_pos, is_ascii_punct, run_length};
use super::code_math::{
    code_span_end, collapse_newlines, math_text_end, single_dollar_end, trim_space_newline,
};
use super::emphasis::{find_closer, left_flanking, right_flanking, underscore_can_open};
use super::html::{INLINE_TAGS, find_closing_tag, parse_tag};
use super::links::parse_link_like;

pub(super) fn parse_into(b: &mut Builder<'_, '_>, text: &str, base: Style, url: &str) {
    let bytes = text.as_bytes();
    let mut i: usize = 0;
    let mut plain: usize = 0;
    while i < text.len() {
        let c = bytes[i];
        match c {
            b'\\' => {
                if b.ctx.latex_delimiters
                    && i + 1 < text.len()
                    && (bytes[i + 1] == b'(' || bytes[i + 1] == b'[')
                {
                    let close: &str = if bytes[i + 1] == b'(' { "\\)" } else { "\\]" };
                    if let Some(end) = index_of_pos(text, i + 2, close) {
                        let inner = trim_space_newline(&text[i + 2..end]);
                        if !inner.is_empty() {
                            b.push_text(&text[plain..i], base, url);
                            let mut style = base;
                            style.math = true;
                            let collapsed = collapse_newlines(inner);
                            b.push(&collapsed, style, url);
                            i = end + 2;
                            plain = i;
                            continue;
                        }
                    }
                }
                if i + 1 < text.len() && is_ascii_punct(bytes[i + 1]) {
                    b.push_text(&text[plain..i], base, url);
                    b.push(&text[i + 1..i + 2], base, url);
                    i += 2;
                    plain = i;
                    continue;
                }
                if i + 1 < text.len() && bytes[i + 1] == b'\n' {
                    // hard break
                    b.push_text(&text[plain..i], base, url);
                    b.push(&text[i + 1..i + 2], base, url);
                    i += 2;
                    plain = i;
                    continue;
                }
            }
            b'`' => {
                let n = run_length(text, i, b'`');
                if let Some(end) = code_span_end(text, i, n) {
                    b.push_text(&text[plain..i], base, url);
                    let mut body = &text[i + n..end];
                    if body.len() >= 2
                        && body.as_bytes()[0] == b' '
                        && body.as_bytes()[body.len() - 1] == b' '
                        && !body.trim_matches(' ').is_empty()
                    {
                        body = &body[1..body.len() - 1];
                    }
                    let mut style = base;
                    style.code = true;
                    let collapsed = collapse_newlines(body);
                    b.push(&collapsed, style, url);
                    i = end + n;
                    plain = i;
                    continue;
                }
                i += n;
                continue;
            }
            b'$' => {
                // micromark-extension-math (mathText): a run of N dollars
                // closes on a run of exactly N; single `$` only when enabled.
                let n = run_length(text, i, b'$');
                if n == 1
                    && b.ctx.single_dollar_math
                    && let Some(end) = single_dollar_end(text, i)
                {
                    b.push_text(&text[plain..i], base, url);
                    let mut style = base;
                    style.math = true;
                    let collapsed = collapse_newlines(&text[i + 1..end]);
                    b.push(&collapsed, style, url);
                    i = end + 1;
                    plain = i;
                    continue;
                }
                if n >= 2
                    && let Some(end) = math_text_end(text, i, n)
                {
                    b.push_text(&text[plain..i], base, url);
                    let mut body = &text[i + n..end];
                    if body.len() >= 2
                        && (body.as_bytes()[0] == b' ' || body.as_bytes()[0] == b'\n')
                        && (body.as_bytes()[body.len() - 1] == b' '
                            || body.as_bytes()[body.len() - 1] == b'\n')
                        && !trim_space_newline(body).is_empty()
                    {
                        body = &body[1..body.len() - 1];
                    }
                    let mut style = base;
                    style.math = true;
                    let collapsed = collapse_newlines(body);
                    b.push(&collapsed, style, url);
                    i = end + n;
                    plain = i;
                    continue;
                }
                i += n;
                continue;
            }
            b'*' | b'_' => {
                let n = run_length(text, i, c);
                let can_open = if c == b'_' {
                    underscore_can_open(text, i, n)
                } else {
                    left_flanking(text, i, n)
                };
                if can_open {
                    let use_ = n.min(3);
                    if let Some(close) = find_closer(text, i + use_, c, use_) {
                        b.push_text(&text[plain..i], base, url);
                        // emit any extra opening markers literally (****x)
                        if n > use_ {
                            b.push(&text[i..i + n - use_], base, url);
                        }
                        let mut style = base;
                        if use_ >= 2 {
                            style.bold = true;
                        }
                        if use_ != 2 {
                            style.italic = true;
                        }
                        parse_into(b, &text[i + n..close], style, url);
                        i = close + use_;
                        plain = i;
                        continue;
                    }
                    // fall back to shorter runs: **a* → *a* with literal '*'
                    if use_ > 1
                        && let Some(close) = find_closer(text, i + n, c, 1)
                    {
                        let close_len = run_length(text, close, c);
                        let k = close_len.min(use_);
                        b.push_text(&text[plain..i], base, url);
                        b.push(&text[i..i + n - k], base, url);
                        let mut style = base;
                        if k >= 2 {
                            style.bold = true;
                        }
                        if k != 2 {
                            style.italic = true;
                        }
                        parse_into(b, &text[i + n..close], style, url);
                        i = close + k;
                        plain = i;
                        continue;
                    }
                }
                i += n;
                continue;
            }
            b'~' => {
                let n = run_length(text, i, b'~');
                if (n == 1 || n == 2) && left_flanking(text, i, n) {
                    let mut matched = false;
                    let mut search = i + n;
                    while let Some(close) = index_of_scalar_pos(text, search, b'~') {
                        let m = run_length(text, close, b'~');
                        if m == n && right_flanking(text, close) && bytes[close - 1] != b'\\' {
                            b.push_text(&text[plain..i], base, url);
                            let mut style = base;
                            style.strike = true;
                            parse_into(b, &text[i + n..close], style, url);
                            i = close + n;
                            plain = i;
                            matched = true;
                            break;
                        }
                        search = close + m;
                    }
                    if matched {
                        continue;
                    }
                }
                i += n;
                continue;
            }
            b'=' => {
                if at(text, i + 1) == b'='
                    && left_flanking(text, i, 2)
                    && let Some(close) = index_of_pos(text, i + 2, "==")
                    && close > i + 2
                    && right_flanking(text, close)
                {
                    b.push_text(&text[plain..i], base, url);
                    let mut style = base;
                    style.highlight = true;
                    parse_into(b, &text[i + 2..close], style, url);
                    i = close + 2;
                    plain = i;
                    continue;
                }
            }
            b'!' | b'[' => {
                let is_image = c == b'!';
                let open = if is_image { i + 1 } else { i };
                if at(text, open) == b'['
                    && let Some(next) =
                        parse_link_like(b, text, i, open, is_image, base, url, plain)
                {
                    i = next;
                    plain = i;
                    continue;
                }
            }
            b'<' => {
                if let Some(close) = index_of_scalar_pos(text, i + 1, b'>') {
                    let inner = &text[i + 1..close];
                    let is_uri = (inner.contains("://") || inner.starts_with("mailto:"))
                        && !inner
                            .as_bytes()
                            .iter()
                            .any(|b| *b == b' ' || *b == b'<' || *b == b'\n');
                    let is_email = !is_uri
                        && inner.as_bytes().contains(&b'@')
                        && !inner
                            .as_bytes()
                            .iter()
                            .any(|b| *b == b' ' || *b == b'<' || *b == b'\n' || *b == b'/');
                    if is_uri || is_email {
                        b.push_text(&text[plain..i], base, url);
                        let mut style = base;
                        style.link = true;
                        let target = if is_email {
                            format!("mailto:{inner}")
                        } else {
                            inner.to_string()
                        };
                        b.push(inner, style, &target);
                        i = close + 1;
                        plain = i;
                        continue;
                    }
                }
                if let Some(tag) = parse_tag(text, i) {
                    if tag.name.eq_ignore_ascii_case("br") {
                        b.push_text(&text[plain..i], base, url);
                        b.push("\n", base, url);
                        i = tag.end;
                        plain = i;
                        continue;
                    }
                    if !tag.closing && !tag.self_closing {
                        let mut matched = false;
                        for known in INLINE_TAGS.iter() {
                            if !known.name.eq_ignore_ascii_case(tag.name) {
                                continue;
                            }
                            if let Some(close) = find_closing_tag(text, tag.end, tag.name) {
                                b.push_text(&text[plain..i], base, url);
                                let mut style = base;
                                (known.apply)(&mut style);
                                let a_link =
                                    tag.name.eq_ignore_ascii_case("a") && !tag.href.is_empty();
                                if a_link {
                                    style.link = true;
                                }
                                let inner_url = if a_link { tag.href } else { url };
                                if style.code {
                                    b.push(&text[tag.end..close.0], style, inner_url);
                                } else {
                                    parse_into(b, &text[tag.end..close.0], style, inner_url);
                                }
                                i = close.1;
                                plain = i;
                                matched = true;
                                break;
                            }
                        }
                        if matched {
                            continue;
                        }
                    }
                    // unknown or unbalanced tag: drop the tag, keep content
                    b.push_text(&text[plain..i], base, url);
                    i = tag.end;
                    plain = i;
                    continue;
                }
                if text[i..].starts_with("<!--")
                    && let Some(end) = index_of_pos(text, i + 4, "-->")
                {
                    b.push_text(&text[plain..i], base, url);
                    i = end + 3;
                    plain = i;
                    continue;
                }
            }
            b'h' | b'w' => {
                if !base.link
                    && let Some(end) = bare_autolink_end(text, i)
                {
                    b.push_text(&text[plain..i], base, url);
                    let mut style = base;
                    style.link = true;
                    let target = if bytes[i] == b'w' {
                        format!("http://{}", &text[i..end])
                    } else {
                        text[i..end].to_string()
                    };
                    b.push(&text[i..end], style, &target);
                    i = end;
                    plain = i;
                    continue;
                }
            }
            b'@' => {
                if !base.link
                    && let Some((start, end)) = email_autolink(text, i)
                    && start >= plain
                {
                    b.push_text(&text[plain..start], base, url);
                    let mut style = base;
                    style.link = true;
                    let addr = &text[start..end];
                    let target = format!("mailto:{addr}");
                    b.push(addr, style, &target);
                    i = end;
                    plain = i;
                    continue;
                }
            }
            b'\n' => {
                // soft break: drop trailing spaces; between CJK characters no space
                let mut line_end = i;
                while line_end > plain && bytes[line_end - 1] == b' ' {
                    line_end -= 1;
                }
                let hard = i - line_end >= 2;
                b.push_text(&text[plain..line_end], base, url);
                let prev = scan::cp_before(text, line_end);
                let mut next_start = i + 1;
                while next_start < text.len() && bytes[next_start] == b' ' {
                    next_start += 1;
                }
                let next = scan::cp_at(text, next_start);
                if hard {
                    b.push("\n", base, url);
                } else if !(scan::is_cjk(prev) && scan::is_cjk(next)) {
                    b.push(" ", base, url);
                }
                i = next_start;
                plain = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    b.push_text(&text[plain..], base, url);
}
