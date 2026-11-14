//! Port of `src/core/layout.zig` — WP-1.3: wrapping row layout (`wrap`).
#![forbid(unsafe_code)]

use crate::core::layout::Nodes;
use crate::core::layout::container::layout_node;
use crate::core::layout::intrinsic::{
    clamp_dim, effective_align, gap_cross, gap_main, intrinsic_height, intrinsic_width,
    resolve_width,
};
use crate::core::node::NodeId;
use crate::layout::flex;
use crate::theme::tokens::Theme;

struct WrapLine {
    start: usize,
    end: usize,
}

/// Port of `layoutRowWrap`: rows wrap into lines, each justified on its own.
#[allow(clippy::too_many_arguments)] // mirrors the Zig signature
pub(crate) fn layout_row_wrap(
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
    let mut items: Vec<flex::Item> = Vec::with_capacity(n);
    let mut widths: Vec<f32> = vec![0.0; n];
    let mut heights: Vec<f32> = vec![0.0; n];

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

    let mut lines: Vec<WrapLine> = Vec::new();
    let mut start = 0usize;
    let mut used: f32 = 0.0;
    for (i, &child_id) in ids.iter().enumerate() {
        let child_style = nodes[child_id].style;
        let outer = items[i].base + child_style.margin.horizontal();
        if i > start && used + main_gap + outer > inner_width + 0.01 {
            lines.push(WrapLine { start, end: i });
            start = i;
            used = outer;
        } else {
            if i > start {
                used += main_gap;
            }
            used += outer;
        }
    }
    lines.push(WrapLine { start, end: n });

    for line in &lines {
        let mut line_height: f32 = 0.0;
        for &child_id in &ids[line.start..line.end] {
            let child_style = nodes[child_id].style;
            let intrinsic = child_style.height.unwrap_or_else(|| {
                intrinsic_height(nodes, child_id, items[line.start].base, theme)
            });
            line_height = line_height.max(intrinsic + child_style.margin.vertical());
        }
        if line_height <= 0.0 {
            line_height = 1.0;
        }
        heights[line.start] = line_height;
    }

    let mut total: f32 = 0.0;
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            total += gap_cross(&style);
        }
        total += heights[line.start];
    }
    let final_height = inner_height.unwrap_or(total);
    if inner_height.is_none() {
        nodes[id].bounds.height = clamp_dim(&style, style.padding.vertical() + total, false);
    }

    let free_cross = (final_height - total).max(0.0);
    let mut offset: f32 = 0.0;
    let mut between: f32 = 0.0;
    let line_count = lines.len();
    match style.align_content {
        flex::AlignContent::Start | flex::AlignContent::Stretch => {}
        flex::AlignContent::End => offset = free_cross,
        flex::AlignContent::Center => offset = free_cross * 0.5,
        flex::AlignContent::SpaceBetween => {
            if line_count > 1 {
                between = free_cross / (line_count - 1) as f32;
            }
        }
        flex::AlignContent::SpaceAround => {
            if line_count > 0 {
                offset = free_cross / (line_count * 2) as f32;
                between = free_cross / line_count as f32;
            }
        }
    }

    let mut line_y = inner_y + offset;
    for (index, line) in lines.iter().enumerate() {
        let line_height = heights[line.start];
        let count = line.end - line.start;
        let mut free = flex::resolve(
            &items[line.start..line.end],
            inner_width,
            main_gap,
            &mut widths[line.start..line.end],
        );
        if free < 0.0 {
            free = 0.0;
        }
        let distribution = flex::justify_distribution(style.justify, free, count);
        let mut cursor = inner_x + distribution.start_offset;
        for offset_in_line in 0..count {
            let i = line.start + offset_in_line;
            let child_id = ids[i];
            let child_style = nodes[child_id].style;
            let alignment = effective_align(&style, &child_style);
            let intrinsic_h = child_style
                .height
                .unwrap_or_else(|| intrinsic_height(nodes, child_id, widths[i], theme));
            let assigned_h: Option<f32> = if alignment == flex::Align::Stretch {
                Some((line_height - child_style.margin.vertical()).max(0.0))
            } else {
                child_style.height
            };
            let remaining = (line_height - child_style.margin.vertical() - intrinsic_h).max(0.0);
            let cross = match alignment {
                flex::Align::Start | flex::Align::Stretch => 0.0,
                flex::Align::Center => remaining * 0.5,
                flex::Align::End => remaining,
            };
            let _ = layout_node(
                nodes,
                child_id,
                cursor + child_style.margin.left,
                line_y + child_style.margin.top + cross,
                Some(widths[i]),
                assigned_h,
                widths[i],
                line_height,
                theme,
                false,
            );
            cursor += widths[i] + child_style.margin.right + main_gap + distribution.between;
        }
        line_y += line_height;
        if index + 1 < line_count {
            line_y += gap_cross(&style) + between;
        }
    }
}
