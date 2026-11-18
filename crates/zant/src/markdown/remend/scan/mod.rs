//! remend `scan.ts`: single-pass classification of a text into code and
//! prose regions, plus lazy masks for math, link URLs and HTML tags.
//!
//! Positions are UTF-8 byte offsets. Every delimiter remend looks at is ASCII,
//! so byte positions behave exactly like the UTF-16 positions of the original.

mod containers;
mod fences;
mod masks;
mod spans;

use self::containers::*;
use self::fences::*;
use self::masks::*;
use self::spans::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Region {
    #[default]
    Prose,
    /// The ``` or ~~~ run that opens or closes a fence
    FenceMarker,
    /// The info string on a fence opener line
    FenceInfo,
    FenceBody,
    /// A complete inline code span, including its backtick markers
    CodeSpan,
    /// An inline code span whose closing run has not arrived yet
    CodeSpanOpen,
}

#[derive(Clone, Copy, Debug)]
pub struct OpenFence {
    pub char: u8,
    pub length: usize,
    pub list_indent: usize,
    pub quote_depth: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct OpenSpan {
    pub run_length: usize,
    pub start: usize,
}

#[derive(Debug)]
pub struct Scan {
    pub text: String,
    pub regions: Vec<Region>,
    pub open_fence: Option<OpenFence>,
    pub open_span: Option<OpenSpan>,
    pub math_mask: Option<Vec<bool>>,
    pub link_url_mask: Option<Vec<bool>>,
    pub html_tag_mask: Option<Vec<bool>>,
}

impl Scan {
    pub fn new(text: &str) -> Scan {
        let mut regions = vec![Region::Prose; text.len()];
        let open_fence = paint_fences(text, &mut regions);
        let open_span = paint_spans(text, &mut regions);
        Scan {
            text: text.to_owned(),
            regions,
            open_fence,
            open_span,
            math_mask: None,
            link_url_mask: None,
            html_tag_mask: None,
        }
    }

    /// A code construct here means a fence or inline span.
    pub fn is_code_at(&self, position: isize) -> bool {
        if position >= self.regions.len() as isize {
            return self.open_fence.is_some() || self.open_span.is_some();
        }
        if position < 0 {
            return false;
        }
        self.regions[position as usize] != Region::Prose
    }

    /// Whether the position is inside a fenced code block (marker, info, or body)
    pub fn is_fence_at(&self, position: isize) -> bool {
        if position >= self.regions.len() as isize {
            return self.open_fence.is_some();
        }
        if position < 0 {
            return false;
        }
        matches!(
            self.regions[position as usize],
            Region::FenceMarker | Region::FenceInfo | Region::FenceBody
        )
    }

    pub fn is_complete_span_at(&self, position: isize) -> bool {
        if position < 0 || position >= self.regions.len() as isize {
            return false;
        }
        self.regions[position as usize] == Region::CodeSpan
    }

    pub fn is_prose(&self, position: usize) -> bool {
        position < self.regions.len() && self.regions[position] == Region::Prose
    }

    pub fn in_math_at(&mut self, position: usize) -> bool {
        if position >= self.text.len() {
            return false;
        }
        if self.math_mask.is_none() {
            let mask = if has_math_delimiters(&self.text) {
                build_math_mask(self)
            } else {
                Vec::new()
            };
            self.math_mask = Some(mask);
        }
        let mask = self.math_mask.as_ref().expect("math mask was built above");
        position < mask.len() && mask[position]
    }

    pub fn in_link_url_at(&mut self, position: usize) -> bool {
        if position >= self.text.len() {
            return false;
        }
        if self.link_url_mask.is_none() {
            let mask = if self.text.contains("](") {
                build_link_url_mask(self)
            } else {
                Vec::new()
            };
            self.link_url_mask = Some(mask);
        }
        let mask = self
            .link_url_mask
            .as_ref()
            .expect("link url mask was built above");
        position < mask.len() && mask[position]
    }

    pub fn in_html_tag_at(&mut self, position: usize) -> bool {
        if position >= self.text.len() {
            return false;
        }
        if self.html_tag_mask.is_none() {
            let mask = if self.text.contains('<') {
                build_html_tag_mask(self)
            } else {
                Vec::new()
            };
            self.html_tag_mask = Some(mask);
        }
        let mask = self
            .html_tag_mask
            .as_ref()
            .expect("html tag mask was built above");
        position < mask.len() && mask[position]
    }

    /// Counts non-overlapping double-character pairs (**, ~~, $$) in prose.
    pub fn count_double_pairs(&self, ch: u8) -> usize {
        let text = self.text.as_bytes();
        let mut count = 0usize;
        let mut i = 0usize;
        while i + 1 < text.len() {
            if text[i] == ch && text[i + 1] == ch {
                if self.regions[i] == Region::Prose {
                    count += 1;
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        count
    }
}

fn is_digit(text: &str, i: usize) -> bool {
    let b = text.as_bytes();
    i < b.len() && b[i] >= b'0' && b[i] <= b'9'
}

fn at(text: &str, i: usize) -> u8 {
    let b = text.as_bytes();
    if i < b.len() { b[i] } else { 0 }
}

fn fill(regions: &mut [Region], value: Region, start: usize, end: usize) {
    let s = start.min(regions.len());
    let e = end.min(regions.len());
    if s < e {
        regions[s..e].fill(value);
    }
}

fn has_math_delimiters(text: &str) -> bool {
    text.contains('$') || text.contains("\\(") || text.contains("\\[")
}

#[test]
fn fences_and_spans_are_painted() {
    let text = "a `b` c\n```js\nx\n```\n``open";
    let scan = Scan::new(text);
    assert_eq!(scan.regions[0], Region::Prose);
    assert_eq!(scan.regions[2], Region::CodeSpan);
    assert!(scan.is_fence_at(12));
    assert!(scan.open_span.is_some());
    assert!(scan.open_fence.is_none());
}
