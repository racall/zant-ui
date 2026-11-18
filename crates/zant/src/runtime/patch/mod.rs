//! Patching a retained node with the element of the same position
//! (`apply_common`, `patch_node`) and the small equality / selection helpers
//! it needs.
//!
//! Patching only writes what changed: layout is marked dirty when a box /
//! visual / content affecting layout changed, paint only for colours and
//! component state. Actions are cloned out of the element into the node — the
//! runtime clones them out of the node again before calling.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::node::{InputData, NodeData, NodeId, RichSpan, SelectData};
use crate::motion::motion::Animated;
use crate::renderer::text;
use crate::ui::element::{Element, Node};

use super::Runtime;

mod common;
mod node;
#[cfg(test)]
mod tests;

impl Runtime {}

/// Applies a controlled input selection when it changed.
/// Returns true when the input must repaint.
pub(crate) fn apply_selection(data: &mut InputData, selection: Option<[usize; 2]>) -> bool {
    let Some(selection) = selection else {
        return false;
    };
    if let Some(previous) = data.applied_selection
        && previous[0] == selection[0]
        && previous[1] == selection[1]
    {
        return false;
    }
    data.applied_selection = Some(selection);
    let len = data.text.len();
    data.anchor = clamp_boundary(&data.text, selection[0].min(len));
    data.cursor = clamp_boundary(&data.text, selection[1].min(len));
    data.reported_selection = [data.anchor, data.cursor];
    data.blink_on = true;
    data.preferred_x = None;
    data.reveal_caret = true;
    true
}

/// The largest char boundary `<= index`.
pub(crate) fn clamp_boundary(bytes: &str, index: usize) -> usize {
    let mut index = index.min(bytes.len());
    while index > 0 && !bytes.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// `renderer::text::Style` has no `PartialEq` (it is a plain option struct);
/// compare the fields the patch pass cares about.
fn text_style_equal(a: &text::Style, b: &text::Style) -> bool {
    a.font_size == b.font_size
        && a.weight == b.weight
        && a.text_align == b.text_align
        && a.wrap == b.wrap
        && a.line_height == b.line_height
        && a.color == b.color
        && a.max_lines == b.max_lines
        && a.family == b.family
}

/// Field-by-field, embed drawings by content hash.
fn rich_spans_equal(a: &[RichSpan], b: &[RichSpan]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (x, y) in a.iter().zip(b) {
        if x.font_size != y.font_size
            || x.weight != y.weight
            || x.color != y.color
            || x.italic != y.italic
            || x.family != y.family
            || x.background != y.background
            || x.underline != y.underline
            || x.strike != y.strike
            || x.text != y.text
            || x.link != y.link
        {
            return false;
        }
        match (x.copy.as_deref(), y.copy.as_deref()) {
            (Some(left), Some(right)) => {
                if left != right {
                    return false;
                }
            }
            (None, None) => {}
            _ => return false,
        }
        match (&x.embed, &y.embed) {
            (Some(left), Some(right)) => {
                if left.content_hash() != right.content_hash() {
                    return false;
                }
            }
            (None, None) => {}
            _ => return false,
        }
    }
    true
}

/// Whether the select already holds exactly `options`.
fn select_options_equal(data: &SelectData, options: &[Str]) -> bool {
    if data.options.len() != options.len() {
        return false;
    }
    data.options
        .iter()
        .zip(options)
        .all(|(old, new)| old.label.as_str() == new.as_str())
}
