//! Column layout.
#![forbid(unsafe_code)]

use crate::core::layout::Nodes;
use crate::core::layout::container::layout_node;
use crate::core::layout::container::translate_subtree_x_in;
use crate::core::layout::intrinsic::{
    clamp_dim, effective_align, gap_main, intrinsic_height, intrinsic_width, resolve_width,
};
use crate::core::node::NodeId;
use crate::flex;
use crate::theme::tokens::Theme;

/// Lays out a vertical flex line (also used for scroll
/// containers with no definite height, which then measure their content).
#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_column(
    nodes: &mut Nodes<'_>,
    id: NodeId,
    ids: &[NodeId],
    inner_x: f32,
    inner_y: f32,
    inner_width: f32,
    inner_height: Option<f32>,
    theme: Theme,
) {
    let n = ids.len();
    let style = nodes[id].style;
    if n == 0 {
        if inner_height.is_none() {
            nodes[id].bounds.height = style.padding.vertical();
        }
        return;
    }
    let main_gap = gap_main(&style);
    let mut widths = super::ScratchFloats::take();
    widths.resize(n, 0.0);
    let mut heights = super::ScratchFloats::take();
    heights.resize(n, 0.0);
    let mut items = super::ScratchItems::take();
    items.reserve(n);

    for (i, &child_id) in ids.iter().enumerate() {
        let child_style = nodes[child_id].style;
        let mut child_width = resolve_width(&child_style, inner_width).unwrap_or_else(|| {
            if effective_align(&style, &child_style) == flex::Align::Stretch {
                (inner_width - child_style.margin.horizontal()).max(0.0)
            } else {
                intrinsic_width(nodes, child_id, inner_width, theme)
            }
        });
        child_width = clamp_dim(&child_style, child_width, true);
        widths[i] = child_width;
        // flex bases only matter with a definite height (grow / shrink);
        // otherwise heights come from laying the children out below, and
        // measuring here would lay out the whole subtree a second time at
        // estimated widths (re-shaping every paragraph of a long chat)
        let base_height: f32 = if inner_height.is_none() {
            0.0
        } else {
            child_style
                .basis
                .or_else(|| {
                    child_style
                        .height_percent
                        .map(|percent| inner_height.unwrap_or(0.0).max(0.0) * percent / 100.0)
                })
                .or(child_style.height)
                .unwrap_or_else(|| intrinsic_height(nodes, child_id, child_width, theme))
        };
        items.push(flex::Item {
            base: base_height,
            grow: child_style.grow,
            shrink: child_style.shrink,
            min: child_style.min_height.unwrap_or(0.0),
            max: child_style.max_height.unwrap_or(f32::INFINITY),
            margin_start: child_style.margin.top,
            margin_end: child_style.margin.bottom,
        });
    }

    let mut free: f32 = 0.0;
    if let Some(h) = inner_height {
        free = flex::resolve(&items, h, main_gap, &mut heights);
        if free < 0.0 {
            free = 0.0;
        }
    } else {
        for (i, item) in items.iter().enumerate() {
            let child_style = nodes[ids[i]].style;
            heights[i] = clamp_dim(&child_style, item.base, false);
        }
    }
    let distribution = if inner_height.is_some() {
        flex::justify_distribution(style.justify, free, n)
    } else {
        flex::Distribution::default()
    };
    let mut cursor = inner_y + distribution.start_offset;
    for (i, &child_id) in ids.iter().enumerate() {
        let child_style = nodes[child_id].style;
        let child_height: Option<f32> = if inner_height.is_some() {
            Some(heights[i])
        } else {
            child_style.height
        };
        let size = layout_node(
            nodes,
            child_id,
            inner_x + child_style.margin.left,
            cursor + child_style.margin.top,
            Some(widths[i]),
            child_height,
            widths[i],
            inner_height.unwrap_or(1.0e6),
            theme,
            false,
        );
        cursor += size.height + child_style.margin.bottom + main_gap + distribution.between;
    }

    if inner_height.is_none() {
        let mut content_height: f32 = 0.0;
        for (i, &child_id) in ids.iter().enumerate() {
            let child_style = nodes[child_id].style;
            content_height += nodes[child_id].bounds.height + child_style.margin.vertical();
            if i > 0 {
                content_height += main_gap;
            }
        }
        nodes[id].bounds.height =
            clamp_dim(&style, style.padding.vertical() + content_height, false);
    }

    let final_inner_width = (nodes[id].bounds.width - style.padding.horizontal()).max(0.0);
    for &child_id in ids {
        let child_style = nodes[child_id].style;
        let alignment = effective_align(&style, &child_style);
        let bounds = nodes[child_id].bounds;
        let remaining =
            (final_inner_width - bounds.width - child_style.margin.horizontal()).max(0.0);
        let offset = match alignment {
            flex::Align::Start | flex::Align::Stretch => 0.0,
            flex::Align::Center => remaining * 0.5,
            flex::Align::End => remaining,
        };
        let target_x = inner_x + child_style.margin.left + offset;
        translate_subtree_x_in(nodes, child_id, target_x - bounds.x);
    }
}
