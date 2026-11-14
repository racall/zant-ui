//! remend `html-tag-handler.ts`: strips an HTML tag that has not received its
//! closing `>` yet.

use super::context::Context;

fn starts_tag(text: &str, index: usize) -> bool {
    let b = text.as_bytes();
    if index + 1 >= b.len() {
        return false;
    }
    let c = b[index + 1];
    c.is_ascii_alphabetic() || c == b'/'
}

pub fn handle_incomplete_html_tag(ctx: &mut Context, text: &str) -> String {
    // /<[a-zA-Z/][^>]*$/ — leftmost "<x" with no ">" after it
    let mut start: Option<usize> = None;
    let mut i = 0usize;
    while let Some(lt) = text[i..].find('<').map(|x| x + i) {
        i = lt + 1;
        if starts_tag(text, lt) && text[lt..].find('>').is_none() {
            start = Some(lt);
            break;
        }
    }
    let Some(match_index) = start else {
        return text.to_owned();
    };
    let check_math = text.contains('$') || text.contains("\\(") || text.contains("\\[");
    let mut index = match_index;
    while index < text.len() {
        if text.as_bytes()[index] != b'<' || !starts_tag(text, index) {
            index += 1;
            continue;
        }
        if ctx.is_inside_code_block(text, index) {
            index += 1;
            continue;
        }
        if check_math && ctx.scan(text).in_math_at(index) {
            index += 1;
            continue;
        }
        return text[..index]
            .trim_end_matches([' ', '\t', '\r', '\n'])
            .to_owned();
    }
    text.to_owned()
}
