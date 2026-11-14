//! Port of `src/core/layout.zig` — WP-1.3: intrinsic (min/max-content) sizes.
//!
//! `intrinsic_width` / `intrinsic_height` measure a subtree without laying it
//! out at its final size. Text nodes use their per-width measure caches
//! (`TextData::measure_width`, `RichTextData::natural_width`), which never
//! replace the laid-out paragraph with a narrower one.
//!
//! Component measures (`measureWidth` / `measureHeight` of
//! `src/components/{button,input,toggle,image,select}.zig`) live in their
//! component modules (`components::{button,input,toggle,image,select}`); this
//! file only dispatches to them, exactly like Zig `core/layout.zig:113-166`.
#![forbid(unsafe_code)]

use crate::core::layout::Nodes;
use crate::core::node::{FlexStyle, NodeData, NodeId, NodeKind};
use crate::layout::flex;
use crate::theme::tokens::Theme;

/// Zig `clampDim`: applies a node's min / max size.
pub(crate) fn clamp_dim(style: &FlexStyle, value: f32, width: bool) -> f32 {
    if width {
        let lo = style.min_width.unwrap_or(0.0);
        let hi = style.max_width.unwrap_or(f32::INFINITY);
        value.max(lo).min(hi)
    } else {
        let lo = style.min_height.unwrap_or(0.0);
        let hi = style.max_height.unwrap_or(f32::INFINITY);
        value.max(lo).min(hi)
    }
}

/// Zig `resolveWidth`: percentage width wins over a fixed width.
pub(crate) fn resolve_width(style: &FlexStyle, available: f32) -> Option<f32> {
    if let Some(percent) = style.width_percent {
        return Some(available.max(0.0) * percent / 100.0);
    }
    style.width
}

/// Zig `effectiveAlign`: `align-self` of the child wins over the parent's
/// `align-items`.
pub(crate) fn effective_align(parent: &FlexStyle, child: &FlexStyle) -> flex::Align {
    match child.align_self {
        flex::AlignSelf::Auto => parent.align_items,
        flex::AlignSelf::Start => flex::Align::Start,
        flex::AlignSelf::Center => flex::Align::Center,
        flex::AlignSelf::End => flex::Align::End,
        flex::AlignSelf::Stretch => flex::Align::Stretch,
    }
}

/// Zig `gapMain`: the gap along the container's main axis.
pub(crate) fn gap_main(style: &FlexStyle) -> f32 {
    if style.direction == flex::Direction::Row {
        style.column_gap.unwrap_or(style.gap)
    } else {
        style.row_gap.unwrap_or(style.gap)
    }
}

/// Zig `gapCross`: the gap along the container's cross axis.
pub(crate) fn gap_cross(style: &FlexStyle) -> f32 {
    if style.direction == flex::Direction::Row {
        style.row_gap.unwrap_or(style.gap)
    } else {
        style.column_gap.unwrap_or(style.gap)
    }
}

/// Zig `finite`: `fallback` when `v` is not a positive finite number.
fn finite(v: f32, fallback: f32) -> f32 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        fallback
    }
}

// ── component measures (see the module docs) ───────────────────────────────

/// `input.zig:measureWidth` (`components::input` owns the port).
pub(crate) fn input_measure_width(style: &FlexStyle) -> f32 {
    crate::components::input::measure_width(style)
}

/// `input.zig:measureHeight` (`components::input` owns the port).
pub(crate) fn input_measure_height(style: &FlexStyle) -> f32 {
    crate::components::input::measure_height(style)
}

// ── intrinsic sizes ────────────────────────────────────────────────────────

/// Zig `intrinsicWidth`: the width a node wants without being told one.
pub(crate) fn intrinsic_width(
    nodes: &mut Nodes<'_>,
    id: NodeId,
    max_avail: f32,
    theme: Theme,
) -> f32 {
    let style = nodes[id].style;
    if let Some(percent) = style.width_percent {
        return clamp_dim(&style, max_avail.max(0.0) * percent / 100.0, true);
    }
    if let Some(width) = style.width {
        return clamp_dim(&style, width, true);
    }
    let avail = finite(max_avail, 1.0e6);
    let value = match nodes[id].data.kind() {
        NodeKind::Root | NodeKind::Container => {
            let mut total: f32 = 0.0;
            let mut widest: f32 = 0.0;
            let direction = style.direction;
            let mut index = 0usize;
            let children: Vec<NodeId> = nodes[id].children.clone();
            for child_id in children {
                // absolutely positioned children do not size their parent
                let child_style = nodes[child_id].style;
                let in_flow = nodes[child_id].in_flow();
                if !in_flow || child_style.absolute {
                    continue;
                }
                let child_width = intrinsic_width(nodes, child_id, avail, theme)
                    + child_style.margin.left
                    + child_style.margin.right;
                if direction == flex::Direction::Row {
                    total += child_width;
                    if index > 0 {
                        total += style.gap;
                    }
                } else {
                    widest = widest.max(child_width);
                }
                index += 1;
            }
            let content = if direction == flex::Direction::Row {
                total
            } else {
                widest
            };
            content + style.padding.horizontal()
        }
        NodeKind::Text => {
            let (wrap, font_size) = match &nodes[id].data {
                NodeData::Text(data) => (data.style.wrap, data.style.font_size),
                _ => (false, 14.0),
            };
            let measure_at = if wrap { avail } else { 1.0e6 };
            let _ = font_size;
            match &mut nodes[id].data {
                NodeData::Text(data) => data.measure_width(measure_at),
                _ => 0.0,
            }
        }
        NodeKind::Button => {
            let enabled = nodes[id].enabled;
            match &mut nodes[id].data {
                NodeData::Button(data) => {
                    crate::components::button::measure_width(data, &style, enabled, theme, avail)
                }
                _ => 0.0,
            }
        }
        NodeKind::Input => input_measure_width(&style),
        NodeKind::Toggle => {
            let enabled = nodes[id].enabled;
            match &mut nodes[id].data {
                NodeData::Toggle(data) => {
                    crate::components::toggle::measure_width(data, &style, enabled, theme, avail)
                }
                _ => 0.0,
            }
        }
        NodeKind::Image => match &nodes[id].data {
            NodeData::Image(data) => crate::components::image::measure_width(&style, data),
            _ => 0.0,
        },
        NodeKind::Select => crate::components::select::measure_width(&style),
        NodeKind::RichText => match &mut nodes[id].data {
            NodeData::RichText(data) => data.natural_width(),
            _ => 0.0,
        },
        NodeKind::Vector => match &nodes[id].data {
            NodeData::Vector(data) => data.drawing.width,
            _ => 0.0,
        },
    };
    clamp_dim(&style, value, true)
}

/// Zig `intrinsicHeight`: the height a node wants at `width`.
pub(crate) fn intrinsic_height(nodes: &mut Nodes<'_>, id: NodeId, width: f32, theme: Theme) -> f32 {
    let style = nodes[id].style;
    if let Some(height) = style.height {
        return clamp_dim(&style, height, false);
    }
    // A scroll container's natural height is its viewport, not its content.
    if style.scroll {
        return clamp_dim(&style, style.padding.vertical(), false);
    }
    let value = match nodes[id].data.kind() {
        NodeKind::Root | NodeKind::Container => {
            let inner_width = (width - style.padding.horizontal()).max(0.0);
            let mut total: f32 = 0.0;
            let mut tallest: f32 = 0.0;
            let children: Vec<NodeId> = nodes[id].children.clone();
            if style.direction == flex::Direction::Row {
                for child_id in children {
                    let child_style = nodes[child_id].style;
                    let in_flow = nodes[child_id].in_flow();
                    if !in_flow || child_style.absolute {
                        continue;
                    }
                    let child_width = child_style
                        .width
                        .unwrap_or_else(|| intrinsic_width(nodes, child_id, inner_width, theme));
                    let h = intrinsic_height(nodes, child_id, child_width, theme)
                        + child_style.margin.top
                        + child_style.margin.bottom;
                    tallest = tallest.max(h);
                }
                tallest + style.padding.vertical()
            } else {
                let mut index = 0usize;
                for child_id in children {
                    let child_style = nodes[child_id].style;
                    let in_flow = nodes[child_id].in_flow();
                    if !in_flow || child_style.absolute {
                        continue;
                    }
                    let child_width = if style.align_items == flex::Align::Stretch
                        && child_style.width.is_none()
                    {
                        (inner_width - child_style.margin.horizontal()).max(0.0)
                    } else {
                        child_style
                            .width
                            .unwrap_or_else(|| intrinsic_width(nodes, child_id, inner_width, theme))
                    };
                    total += intrinsic_height(nodes, child_id, child_width, theme)
                        + child_style.margin.top
                        + child_style.margin.bottom;
                    if index > 0 {
                        total += style.gap;
                    }
                    index += 1;
                }
                total + style.padding.vertical()
            }
        }
        NodeKind::Text => {
            let (wrap, font_size) = match &nodes[id].data {
                NodeData::Text(data) => (data.style.wrap, data.style.font_size),
                _ => (false, 14.0),
            };
            let width_for_layout = if wrap { width } else { 1.0e6 };
            match &mut nodes[id].data {
                NodeData::Text(data) => data
                    .ensure_layout(width_for_layout)
                    .map_or(font_size * 1.2, |layout| layout.metrics().height),
                _ => font_size * 1.2,
            }
        }
        NodeKind::Button => {
            let style = nodes[id].style;
            match &nodes[id].data {
                NodeData::Button(data) => crate::components::button::measure_height(&style, data),
                _ => 0.0,
            }
        }
        NodeKind::Input => input_measure_height(&style),
        NodeKind::Toggle => crate::components::toggle::measure_height(&style),
        NodeKind::Image => match &nodes[id].data {
            NodeData::Image(data) => crate::components::image::measure_height(&style, data, width),
            _ => 0.0,
        },
        NodeKind::Select => crate::components::select::measure_height(&style),
        NodeKind::RichText => {
            let fallback = match &nodes[id].data {
                NodeData::RichText(data) => data.spans.len() as f32 * 20.0,
                _ => 0.0,
            };
            match &mut nodes[id].data {
                NodeData::RichText(data) => data
                    .ensure_layout(width)
                    .map_or(fallback, |layout| layout.height),
                _ => fallback,
            }
        }
        NodeKind::Vector => match &nodes[id].data {
            NodeData::Vector(data) => data.drawing.height(),
            _ => 0.0,
        },
    };
    clamp_dim(&style, value, false)
}
