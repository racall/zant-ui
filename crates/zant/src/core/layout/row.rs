//! Single-line row layout.
#![forbid(unsafe_code)]

use crate::core::layout::Nodes;
use crate::core::layout::container::layout_node;
use crate::core::layout::container::translate_subtree_y_in;
use crate::core::layout::intrinsic::{
    clamp_dim, effective_align, gap_main, intrinsic_width, resolve_width,
};
use crate::core::node::NodeId;
use crate::flex;
use crate::theme::tokens::Theme;

/// Lays out a non-wrapping horizontal flex line.
#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_row(
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
    let mut items = super::ScratchItems::take();
    items.reserve(n);
    for &child_id in ids {
        let child_style = nodes[child_id].style;
        let mut base = child_style
            .basis
            .or_else(|| resolve_width(&child_style, inner_width))
            .unwrap_or_else(|| intrinsic_width(nodes, child_id, inner_width, theme));
        base = clamp_dim(&child_style, base, true);
        items.push(flex::Item {
            base,
            grow: child_style.grow,
            shrink: child_style.shrink,
            min: child_style.min_width.unwrap_or(0.0),
            max: child_style.max_width.unwrap_or(f32::INFINITY),
            margin_start: child_style.margin.left,
            margin_end: child_style.margin.right,
        });
    }

    let mut widths = super::ScratchFloats::take();
    widths.resize(n, 0.0);
    let mut free = flex::resolve(&items, inner_width, main_gap, &mut widths);
    if free < 0.0 {
        free = 0.0;
    }
    let distribution = flex::justify_distribution(style.justify, free, n);
    let mut cursor = inner_x + distribution.start_offset;
    let mut tallest: f32 = 0.0;
    for (i, &child_id) in ids.iter().enumerate() {
        let child_style = nodes[child_id].style;
        let child_width = widths[i];
        let left = cursor + child_style.margin.left;
        let alignment = effective_align(&style, &child_style);
        let cross_assigned: Option<f32> = if let Some(inner_height) = inner_height {
            if let Some(explicit) = child_style.height {
                Some(explicit)
            } else if alignment == flex::Align::Stretch {
                Some((inner_height - child_style.margin.vertical()).max(0.0))
            } else {
                None
            }
        } else {
            child_style.height
        };
        let size = layout_node(
            nodes,
            child_id,
            left,
            inner_y + child_style.margin.top,
            Some(child_width),
            cross_assigned,
            child_width,
            inner_height.unwrap_or(1.0e6),
            theme,
            false,
        );
        tallest = tallest.max(size.height + child_style.margin.vertical());
        cursor += child_width + child_style.margin.right + main_gap + distribution.between;
    }

    if inner_height.is_none() {
        nodes[id].bounds.height = clamp_dim(&style, style.padding.vertical() + tallest, false);
    }
    let final_inner_height = (nodes[id].bounds.height - style.padding.vertical()).max(0.0);
    if inner_height.is_none() {
        // CSS: stretch items fill the line's cross size even when the row's
        // height comes from its content.
        for &child_id in ids {
            let child_style = nodes[child_id].style;
            if effective_align(&style, &child_style) != flex::Align::Stretch
                || child_style.height.is_some()
            {
                continue;
            }
            let target = (final_inner_height - child_style.margin.vertical()).max(0.0);
            let bounds = nodes[child_id].bounds;
            if (bounds.height - target).abs() < 0.25 {
                continue;
            }
            let _ = layout_node(
                nodes,
                child_id,
                bounds.x,
                inner_y + child_style.margin.top,
                Some(bounds.width),
                Some(target),
                bounds.width,
                final_inner_height,
                theme,
                false,
            );
        }
    }
    for &child_id in ids {
        let child_style = nodes[child_id].style;
        let alignment = effective_align(&style, &child_style);
        if alignment == flex::Align::Stretch && child_style.height.is_none() {
            continue;
        }
        let bounds = nodes[child_id].bounds;
        let remaining =
            (final_inner_height - bounds.height - child_style.margin.vertical()).max(0.0);
        let offset = match alignment {
            flex::Align::Start | flex::Align::Stretch => 0.0,
            flex::Align::Center => remaining * 0.5,
            flex::Align::End => remaining,
        };
        let target_y = inner_y + child_style.margin.top + offset;
        translate_subtree_y_in(nodes, child_id, target_y - bounds.y);
    }
}
