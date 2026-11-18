//! Inline Markdown parser: emphasis, code spans, math, links, images,
//! autolinks, footnote references, inline HTML and character references.
//!
//! Emphasis follows CommonMark's flanking rules, relaxed the way Streamdown's
//! CJK plugin relaxes them: a closing run only needs a non-space character
//! before it, so `**中文。**后文` stays bold.

mod autolinks;
mod bytes;
mod code_math;
mod emphasis;
mod html;
mod links;
mod parsing;
#[cfg(test)]
mod tests;

use std::borrow::Cow;

use super::super::ast::{Span, Style};
use super::super::entities;
use parsing::parse_into;

pub use links::{LinkDef, RefMap, lookup_ref, normalize_ref_label};

pub struct Context<'a> {
    pub refs: &'a RefMap,
    /// Footnote labels in order of first reference.
    pub footnote_order: &'a mut Vec<String>,
    /// Recognise `$x$` inline math. On by default with pandoc's rules so
    /// prices stay text: the opener must be followed by a non-space, the
    /// closer preceded by a non-space and not followed by a digit
    /// (`$20 and $10` is text, `$x$` is math).
    pub single_dollar_math: bool,
    /// `\(…\)` and `\[…\]` math (see `block::Options.latex_delimiters`).
    pub latex_delimiters: bool,
}

/// The parser's span builder. It owns the final strings and tracks the source
/// offset of the last pushed run, so runs that are adjacent in the source are
/// merged.
struct Builder<'a, 'r> {
    ctx: &'a mut Context<'r>,
    source: &'a str,
    out: Vec<Span>,
    /// Source byte range of the last span when its text is a sub-slice of
    /// `source` (used to detect a run that continues right after it).
    last_source: Option<(usize, usize)>,
}

impl<'a, 'r> Builder<'a, 'r> {
    fn source_range(&self, text: &str) -> Option<(usize, usize)> {
        let base = self.source.as_ptr() as usize;
        let start = text.as_ptr() as usize;
        if start >= base && start + text.len() <= base + self.source.len() {
            Some((start - base, start - base + text.len()))
        } else {
            None
        }
    }

    fn push(&mut self, text: &str, style: Style, url: &str) {
        if text.is_empty() {
            return;
        }
        let range = self.source_range(text);
        let adjacent = match (self.last_source, range) {
            (Some((last_start, last_end)), Some((start, _end))) if last_end == start => {
                Some((last_start, last_end))
            }
            _ => None,
        };
        if let Some(last) = self.out.last_mut()
            // merge adjacent runs with identical styling
            && last.style == style
            && last.url == url
            && !style.footnote_ref
            && let Some((last_start, last_end)) = adjacent
        {
            last.text.push_str(text);
            self.last_source = Some((last_start, last_end + text.len()));
            return;
        }
        self.out.push(Span {
            text: text.to_string(),
            style,
            url: url.to_string(),
        });
        self.last_source = range;
    }

    fn push_text(&mut self, text: &str, style: Style, url: &str) {
        if text.is_empty() {
            return;
        }
        if style.code || style.math {
            return self.push(text, style, url);
        }
        let decoded = entities::decode(text);
        if matches!(&decoded, Cow::Borrowed(_)) {
            return self.push(text, style, url);
        }
        self.out.push(Span {
            text: decoded.into_owned(),
            style,
            url: url.to_string(),
        });
        self.last_source = None;
    }
}

pub fn parse(ctx: &mut Context<'_>, text: &str) -> Vec<Span> {
    let mut b = Builder {
        ctx,
        source: text,
        out: Vec::new(),
        last_source: None,
    };
    parse_into(&mut b, text, Style::default(), "");
    b.out
}

/// Parses with a base style (used for table cells / headings with extra style).
pub fn parse_styled(ctx: &mut Context<'_>, text: &str, style: Style) -> Vec<Span> {
    let mut b = Builder {
        ctx,
        source: text,
        out: Vec::new(),
        last_source: None,
    };
    parse_into(&mut b, text, style, "");
    b.out
}
