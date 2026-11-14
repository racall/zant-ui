//! The retained-tree layout pass.
//!
//! `layout_tree` is the entry point the `Tree` calls with a viewport; it lays
//! the root out at the viewport size and clears the dirty flags. The recursive
//! helpers take `&mut [RetainedNode]` plus a `NodeId`: nodes are addressed by
//! index, so no `&mut RetainedNode` is ever held across a recursive call —
//! every write re-borrows the slot.
#![forbid(unsafe_code)]

use crate::core::geometry::Size;
use crate::core::layout::Nodes;
use crate::core::layout::column::layout_column;
use crate::core::layout::intrinsic::{
    clamp_dim, input_measure_height, intrinsic_height, intrinsic_width, resolve_width,
};
use crate::core::layout::row::layout_row;
use crate::core::layout::wrap::layout_row_wrap;
use crate::core::node::{NodeData, NodeId, NodeKind, RetainedNode};
use crate::layout::flex;
use crate::theme::tokens::Theme;

/// Lays out the whole tree and resets the dirty flags.
pub fn layout_tree(nodes: &mut [RetainedNode], root_id: NodeId, viewport: Size, theme: Theme) {
    let mut wrapped = Nodes::new(nodes);
    layout_tree_in(&mut wrapped, root_id, viewport, theme);
}

fn layout_tree_in(nodes: &mut Nodes<'_>, root_id: NodeId, viewport: Size, theme: Theme) {
    let _ = layout_node(
        nodes,
        root_id,
        0.0,
        0.0,
        Some(viewport.width),
        Some(viewport.height),
        viewport.width,
        viewport.height,
        theme,
        true,
    );
    for node in nodes.iter_mut() {
        node.dirty.layout = false;
        node.dirty.paint = true;
    }
}

/// Lays out one node at (x, y) and returns the size it ended up with.
#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_node(
    nodes: &mut Nodes<'_>,
    id: NodeId,
    x: f32,
    y: f32,
    assigned_width: Option<f32>,
    assigned_height: Option<f32>,
    available_width: f32,
    available_height: f32,
    theme: Theme,
    is_root: bool,
) -> Size {
    let _ = is_root;
    let style = nodes[id].style;
    let mut width = match assigned_width {
        Some(width) => width,
        None => resolve_width(&style, available_width)
            .unwrap_or_else(|| intrinsic_width(nodes, id, available_width, theme)),
    };
    width = clamp_dim(&style, width.max(0.0), true);
    {
        let node = &mut nodes[id];
        node.bounds.x = x;
        node.bounds.y = y;
        node.bounds.width = width;
    }

    match nodes[id].data.kind() {
        NodeKind::Text => {
            let font_size = match &nodes[id].data {
                NodeData::Text(data) => data.style.font_size,
                _ => 14.0,
            };
            let mut height = assigned_height
                .or_else(|| {
                    style
                        .height_percent
                        .map(|percent| available_height.max(0.0) * percent / 100.0)
                })
                .or(style.height);
            let max_width = match &nodes[id].data {
                NodeData::Text(data) => data.layout_width(width),
                _ => width,
            };
            let measured = match &mut nodes[id].data {
                NodeData::Text(data) => data
                    .ensure_layout(max_width)
                    .map(|layout| layout.metrics().height),
                _ => None,
            };
            if height.is_none() {
                height = Some(measured.unwrap_or(font_size * 1.2));
            }
            nodes[id].bounds.height = clamp_dim(&style, height.unwrap_or(0.0).max(0.0), false);
        }
        NodeKind::Button => {
            let height = assigned_height
                .or_else(|| {
                    style
                        .height_percent
                        .map(|percent| available_height.max(0.0) * percent / 100.0)
                })
                .unwrap_or_else(|| match &nodes[id].data {
                    NodeData::Button(data) => {
                        crate::components::button::measure_height(&style, data)
                    }
                    _ => 0.0,
                });
            nodes[id].bounds.height = clamp_dim(&style, height, false);
        }
        NodeKind::Input => {
            let height = assigned_height
                .or_else(|| {
                    style
                        .height_percent
                        .map(|percent| available_height.max(0.0) * percent / 100.0)
                })
                .unwrap_or_else(|| input_measure_height(&style));
            nodes[id].bounds.height = clamp_dim(&style, height, false);
        }
        NodeKind::Toggle => {
            let height = assigned_height
                .or_else(|| {
                    style
                        .height_percent
                        .map(|percent| available_height.max(0.0) * percent / 100.0)
                })
                .unwrap_or_else(|| crate::components::toggle::measure_height(&style));
            nodes[id].bounds.height = clamp_dim(&style, height, false);
        }
        NodeKind::Image => {
            let height = assigned_height
                .or_else(|| {
                    style
                        .height_percent
                        .map(|percent| available_height.max(0.0) * percent / 100.0)
                })
                .or(style.height)
                .unwrap_or_else(|| match &nodes[id].data {
                    NodeData::Image(data) => {
                        crate::components::image::measure_height(&style, data, width)
                    }
                    _ => 0.0,
                });
            nodes[id].bounds.height = clamp_dim(&style, height, false);
        }
        NodeKind::Select => {
            let height = assigned_height
                .or_else(|| {
                    style
                        .height_percent
                        .map(|percent| available_height.max(0.0) * percent / 100.0)
                })
                .unwrap_or_else(|| crate::components::select::measure_height(&style));
            nodes[id].bounds.height = clamp_dim(&style, height, false);
        }
        NodeKind::Vector => {
            let height =
                assigned_height
                    .or(style.height)
                    .unwrap_or_else(|| match &nodes[id].data {
                        NodeData::Vector(data) => data.drawing.height(),
                        _ => 0.0,
                    });
            nodes[id].bounds.height = clamp_dim(&style, height, false);
        }
        NodeKind::RichText => {
            let layout_h = match &mut nodes[id].data {
                NodeData::RichText(data) => data.ensure_layout(width).map(|layout| layout.height),
                _ => None,
            };
            let height = assigned_height.unwrap_or_else(|| layout_h.unwrap_or(0.0));
            nodes[id].bounds.height = clamp_dim(&style, height, false);
        }
        NodeKind::Container | NodeKind::Root => {
            let definite_height = assigned_height
                .or_else(|| {
                    style
                        .height_percent
                        .map(|percent| available_height.max(0.0) * percent / 100.0)
                })
                .or(style.height);
            layout_container(nodes, id, width, definite_height, theme);
        }
    }
    nodes[id].bounds.size()
}

/// Lays out the children of a container.
fn layout_container(
    nodes: &mut Nodes<'_>,
    id: NodeId,
    width: f32,
    definite_height: Option<f32>,
    theme: Theme,
) {
    let mut visible = super::ScratchIds::take();
    let child_count = nodes[id].children.len();
    for child_index in 0..child_count {
        let child = nodes[id].children[child_index];
        if nodes[child].in_flow() {
            visible.push(child);
        }
    }
    order_ids(nodes, &mut visible);

    let style = nodes[id].style;
    let mut flow = super::ScratchIds::take();
    let mut absolute = super::ScratchIds::take();
    for &child in visible.iter() {
        if nodes[child].style.absolute {
            absolute.push(child);
        } else {
            flow.push(child);
        }
    }

    let padding = style.padding;
    let bounds = nodes[id].bounds;
    let inner_x = bounds.x + padding.left;
    let inner_y = bounds.y + padding.top;
    let inner_width = (width - padding.horizontal()).max(0.0);
    let inner_height = match definite_height {
        Some(height) => (height - padding.vertical()).max(0.0),
        None => 0.0,
    };
    let n = flow.len();
    if let Some(height) = definite_height {
        nodes[id].bounds.height = clamp_dim(&style, height, false);
    }

    if style.direction == flex::Direction::Row {
        if style.wrap != flex::Wrap::Nowrap {
            layout_row_wrap(
                nodes,
                id,
                &flow,
                inner_x,
                inner_y,
                inner_width,
                definite_height.map(|_| inner_height),
                theme,
            );
        } else {
            layout_row(
                nodes,
                id,
                &flow,
                inner_x,
                inner_y,
                inner_width,
                definite_height.map(|_| inner_height),
                theme,
            );
        }
    } else if style.scroll && definite_height.is_some() {
        // Scroll containers lay children out at their natural height, then
        // shift them by the retained scroll offset.
        layout_column(nodes, id, &flow, inner_x, inner_y, inner_width, None, theme);
        let content = (nodes[id].bounds.height - padding.vertical()).max(0.0);
        nodes[id].bounds.height = clamp_dim(&style, definite_height.unwrap_or(0.0), false);
        let viewport = nodes[id].bounds.height;
        if let Some(data) = nodes[id].container_mut() {
            data.content_height = content + padding.vertical();
            let max = data.max_scroll(viewport);
            data.scroll_offset = data.scroll_offset.max(0.0).min(max);
        }
        let offset = nodes[id]
            .container()
            .map(|data| data.scroll_offset)
            .unwrap_or(0.0);
        for &child in flow.iter() {
            translate_subtree_y_in(nodes, child, -offset);
        }
    } else {
        layout_column(
            nodes,
            id,
            &flow,
            inner_x,
            inner_y,
            inner_width,
            definite_height.map(|_| inner_height),
            theme,
        );
    }

    // CSS containing block: absolute children are positioned and sized
    // against the parent's padding box (padding included), so a full-size hit
    // target / hover background also covers the parent's padding.
    let box_x = nodes[id].bounds.x;
    let box_y = nodes[id].bounds.y;
    let box_w = nodes[id].bounds.width;
    let box_h = nodes[id].bounds.height;
    for &child_id in absolute.iter() {
        let child_style = nodes[child_id].style;
        // CSS: `left` + `right` without a width stretch between the insets
        let inset_w: Option<f32> = if child_style.left.is_some()
            && child_style.right.is_some()
            && child_style.width.is_none()
            && child_style.width_percent.is_none()
        {
            Some(
                (box_w - child_style.left.unwrap_or(0.0) - child_style.right.unwrap_or(0.0))
                    .max(0.0),
            )
        } else {
            None
        };
        let w = inset_w
            .or_else(|| resolve_width(&child_style, box_w))
            .unwrap_or_else(|| intrinsic_width(nodes, child_id, box_w, theme));
        // content-sized and placed from the top: lay it out at its natural
        // height directly (measuring it first would lay out the subtree twice)
        if child_style.height.is_none()
            && child_style.height_percent.is_none()
            && child_style.anchor.is_none()
            && (child_style.top.is_some() || child_style.bottom.is_none())
        {
            let mut cx = box_x;
            if let Some(left) = child_style.left {
                cx = box_x + left;
            } else if let Some(right) = child_style.right {
                cx = box_x + box_w - w - right;
            }
            let cy = box_y + child_style.top.unwrap_or(0.0);
            let _ = layout_node(
                nodes,
                child_id,
                cx,
                cy,
                Some(w),
                None,
                w,
                box_h,
                theme,
                false,
            );
            continue;
        }
        // a percentage wins over a builder's fixed height (as in flow layout),
        // so `ui::styled(button, .{ .height_percent = 100 })` fills the parent
        let h = if let Some(percent) = child_style.height_percent {
            box_h * percent / 100.0
        } else if let Some(explicit) = child_style.height {
            explicit
        } else {
            intrinsic_height(nodes, child_id, w, theme)
        };
        let mut x = box_x;
        let mut y = box_y;
        if let Some(left) = child_style.left {
            x = box_x + left;
        } else if let Some(right) = child_style.right {
            x = box_x + box_w - w - right;
        }
        if let Some(top) = child_style.top {
            y = box_y + top;
        } else if let Some(bottom) = child_style.bottom {
            y = box_y + box_h - h - bottom;
        }
        if let Some(anchor) = child_style.anchor
            && (anchor as usize) < flow.len()
        {
            // follow the n-th in-flow sibling (FLIP animates the moves)
            let target = nodes[flow[anchor as usize]].bounds;
            let own_vertical = child_style.top.is_some()
                || child_style.bottom.is_some()
                || child_style.height.is_some()
                || child_style.height_percent.is_some();
            let _ = layout_node(
                nodes,
                child_id,
                target.x,
                if own_vertical { y } else { target.y },
                Some(target.width),
                Some(if own_vertical { h } else { target.height }),
                target.width,
                box_h,
                theme,
                false,
            );
            continue;
        }
        let _ = layout_node(
            nodes,
            child_id,
            x,
            y,
            Some(w),
            Some(h),
            w,
            box_h,
            theme,
            false,
        );
    }

    if let Some(height) = definite_height {
        nodes[id].bounds.height = clamp_dim(&style, height, false);
    } else if n == 0 {
        nodes[id].bounds.height = clamp_dim(&style, padding.vertical(), false);
    }
}

/// Stable insertion sort of the child ids by `order`.
fn order_ids(nodes: &Nodes<'_>, ids: &mut [NodeId]) {
    // insertion sort, stable, in place (most containers leave `order` at 0)
    if ids.iter().all(|id| nodes[*id].style.order == 0) {
        return;
    }
    let mut i = 1usize;
    while i < ids.len() {
        let value = ids[i];
        let mut j = i;
        while j > 0 && nodes[ids[j - 1]].style.order > nodes[value].style.order {
            ids[j] = ids[j - 1];
            j -= 1;
        }
        ids[j] = value;
        i += 1;
    }
}

/// Translates a subtree horizontally (public slice-shaped wrapper).
pub fn translate_subtree_x(nodes: &mut [RetainedNode], id: NodeId, dx: f32) {
    let mut wrapped = Nodes::new(nodes);
    translate_subtree_x_in(&mut wrapped, id, dx);
}

pub(crate) fn translate_subtree_x_in(nodes: &mut Nodes<'_>, id: NodeId, dx: f32) {
    if dx == 0.0 {
        return;
    }
    nodes[id].bounds.x += dx;
    let count = nodes[id].children.len();
    for index in 0..count {
        let child = nodes[id].children[index];
        translate_subtree_x_in(nodes, child, dx);
    }
}

/// Translates a subtree vertically (public slice-shaped wrapper).
pub fn translate_subtree_y(nodes: &mut [RetainedNode], id: NodeId, dy: f32) {
    let mut wrapped = Nodes::new(nodes);
    translate_subtree_y_in(&mut wrapped, id, dy);
}

pub(crate) fn translate_subtree_y_in(nodes: &mut Nodes<'_>, id: NodeId, dy: f32) {
    if dy == 0.0 {
        return;
    }
    nodes[id].bounds.y += dy;
    let count = nodes[id].children.len();
    for index in 0..count {
        let child = nodes[id].children[index];
        translate_subtree_y_in(nodes, child, dy);
    }
}
