//! Flowchart layout and vector drawing.

use super::super::Options;
use super::super::canvas::{Canvas, anchor_top, trim_line};
use super::parse::dfs_visit;
use super::{Direction, EdgeStyle, Flow, Shape};
use crate::core::vector::{self, Drawing};
use crate::renderer::text::Family;

pub(in crate::markdown::mermaid) fn flowchart(source: &str, options: &Options) -> Option<Drawing> {
    let mut flow = Flow::default();
    let mut direction = Direction::Td;
    let mut header_seen = false;
    for raw in source.split(['\n', ';']) {
        let line = trim_line(raw);
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if !header_seen {
            header_seen = true;
            let mut words = line.split(' ').filter(|word| !word.is_empty());
            let _ = words.next();
            if let Some(dir) = words.next() {
                if dir.eq_ignore_ascii_case("LR") {
                    direction = Direction::Lr;
                } else if dir.eq_ignore_ascii_case("RL") {
                    direction = Direction::Rl;
                } else if dir.eq_ignore_ascii_case("BT") {
                    direction = Direction::Bt;
                }
            }
            continue;
        }
        const SKIP: [&str; 8] = [
            "classDef",
            "class ",
            "style ",
            "linkStyle",
            "click ",
            "subgraph",
            "end",
            "direction",
        ];
        if SKIP.iter().any(|prefix| line.starts_with(prefix)) {
            continue;
        }
        flow.parse_statement(line);
    }
    if flow.nodes.is_empty() {
        return None;
    }
    let fs = options.font_size;

    // measure nodes
    for node in &mut flow.nodes {
        let m = vector::measure(&node.label, fs, Family::Sans, 400, false);
        node.w = (m.advance + 28.0).max(56.0);
        node.h = fs * 2.6;
        match node.shape {
            Shape::Diamond => {
                node.w = (node.w * 1.35).max(node.h * 1.6);
                node.h *= 1.45;
            }
            Shape::Circle => {
                node.w = node.w.max(node.h);
                node.h = node.w;
            }
            Shape::Hexagon => node.w += 16.0,
            _ => {}
        }
    }

    // layering: longest path, ignoring back edges found by DFS
    let mut back = vec![false; flow.edges.len()];
    {
        let mut state = vec![0u8; flow.nodes.len()]; // 0 new, 1 active, 2 done
        for v in 0..flow.nodes.len() {
            if state[v] == 0 {
                dfs_visit(&flow, &mut state, &mut back, v);
            }
        }
    }
    let mut changed = true;
    let mut guard: usize = 0;
    while changed && guard < flow.nodes.len() + 2 {
        guard += 1;
        changed = false;
        for (k, edge) in flow.edges.iter().enumerate() {
            if back[k] || edge.from == edge.to {
                continue;
            }
            let from_layer = flow.nodes[edge.from].layer;
            if flow.nodes[edge.to].layer < from_layer + 1 {
                flow.nodes[edge.to].layer = from_layer + 1;
                changed = true;
            }
        }
    }
    let mut layer_count: usize = 0;
    for node in &flow.nodes {
        layer_count = layer_count.max(node.layer + 1);
    }

    // ordering: appearance order, then two barycenter sweeps
    for (i, node) in flow.nodes.iter_mut().enumerate() {
        node.order = i as f32;
    }
    for _ in 0..2 {
        for v in 0..flow.nodes.len() {
            let layer = flow.nodes[v].layer;
            let mut sum: f32 = 0.0;
            let mut count: f32 = 0.0;
            for edge in &flow.edges {
                if edge.to == v && flow.nodes[edge.from].layer < layer {
                    sum += flow.nodes[edge.from].order;
                    count += 1.0;
                }
            }
            if count > 0.0 {
                flow.nodes[v].order = sum / count + v as f32 * 0.001;
            }
        }
    }

    let horizontal = direction == Direction::Lr || direction == Direction::Rl;
    let rank_gap: f32 = if horizontal { 56.0 } else { 48.0 };
    let node_gap: f32 = 28.0;
    // per layer: sorted members, extent along the cross axis
    let mut layer_main = vec![0.0f32; layer_count]; // thickness of layer along main axis
    for node in &flow.nodes {
        let thickness = if horizontal { node.w } else { node.h };
        layer_main[node.layer] = layer_main[node.layer].max(thickness);
    }
    let mut cross_max: f32 = 0.0;
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); layer_count];
    for (i, node) in flow.nodes.iter().enumerate() {
        members[node.layer].push(i);
    }
    for layer in &mut members {
        layer.sort_by(|a, b| flow.nodes[*a].order.total_cmp(&flow.nodes[*b].order));
        let mut extent: f32 = 0.0;
        for (k, idx) in layer.iter().enumerate() {
            if k > 0 {
                extent += node_gap;
            }
            extent += if horizontal {
                flow.nodes[*idx].h
            } else {
                flow.nodes[*idx].w
            };
        }
        cross_max = cross_max.max(extent);
    }
    let mut main_pos: f32 = 8.0;
    for (layer, layer_members) in members.iter().enumerate() {
        let mut extent: f32 = 0.0;
        for (k, idx) in layer_members.iter().enumerate() {
            if k > 0 {
                extent += node_gap;
            }
            extent += if horizontal {
                flow.nodes[*idx].h
            } else {
                flow.nodes[*idx].w
            };
        }
        let mut cross = 8.0 + (cross_max - extent) / 2.0;
        for idx in layer_members {
            let node = &mut flow.nodes[*idx];
            if horizontal {
                node.x = main_pos + (layer_main[layer] - node.w) / 2.0;
                node.y = cross;
                cross += node.h + node_gap;
            } else {
                node.y = main_pos + (layer_main[layer] - node.h) / 2.0;
                node.x = cross;
                cross += node.w + node_gap;
            }
        }
        main_pos += layer_main[layer] + rank_gap;
    }
    let mut width: f32 = 0.0;
    let mut height: f32 = 0.0;
    for node in &flow.nodes {
        width = width.max(node.x + node.w + 8.0);
        height = height.max(node.y + node.h + 8.0);
    }
    // flip for BT / RL
    if direction == Direction::Bt || direction == Direction::Rl {
        for node in &mut flow.nodes {
            if direction == Direction::Bt {
                node.y = height - node.y - node.h;
            } else {
                node.x = width - node.x - node.w;
            }
        }
    }

    let mut canvas = Canvas::new();
    let colors = options.colors;
    let mut back_lane: f32 = 0.0;
    // edges first (under nodes)
    for (k, edge) in flow.edges.iter().enumerate() {
        let a = &flow.nodes[edge.from];
        let b = &flow.nodes[edge.to];
        let lw: f32 = if edge.style == EdgeStyle::Thick {
            2.6
        } else {
            1.3
        };
        let dashed = edge.style == EdgeStyle::Dotted;
        let (label_x, label_y);
        if back[k] || edge.from == edge.to {
            // route around the side of the diagram
            back_lane += 14.0;
            if horizontal {
                let y = height + back_lane - 6.0;
                let ax = a.x + a.w / 2.0;
                let bx = b.x + b.w / 2.0;
                canvas.line(ax, a.y + a.h, ax, y, colors.edge, lw, dashed);
                canvas.line(ax, y, bx, y, colors.edge, lw, dashed);
                canvas.line(bx, y, bx, b.y + b.h, colors.edge, lw, dashed);
                if edge.arrow {
                    canvas.arrow_head(bx, b.y + b.h, 0.0, -1.0, colors.edge);
                }
                label_x = (ax + bx) / 2.0;
                label_y = y;
            } else {
                let x = width + back_lane - 6.0;
                let ay = a.y + a.h / 2.0;
                let by = b.y + b.h / 2.0;
                canvas.line(a.x + a.w, ay, x, ay, colors.edge, lw, dashed);
                canvas.line(x, ay, x, by, colors.edge, lw, dashed);
                canvas.line(x, by, b.x + b.w, by, colors.edge, lw, dashed);
                if edge.arrow {
                    canvas.arrow_head(b.x + b.w, by, -1.0, 0.0, colors.edge);
                }
                label_x = x;
                label_y = (ay + by) / 2.0;
            }
        } else if horizontal {
            let forward = b.x >= a.x;
            let x0 = if forward { a.x + a.w } else { a.x };
            let x1 = if forward { b.x } else { b.x + b.w };
            let y0 = a.y + a.h / 2.0;
            let y1 = b.y + b.h / 2.0;
            let mid = (x0 + x1) / 2.0;
            if (y0 - y1).abs() < 1.0 {
                canvas.line(x0, y0, x1, y1, colors.edge, lw, dashed);
            } else {
                canvas.poly(
                    &[[x0, y0], [mid, y0], [mid, y1], [x1, y1]],
                    colors.edge,
                    lw,
                    false,
                );
            }
            if edge.arrow {
                canvas.arrow_head(x1, y1, if forward { 1.0 } else { -1.0 }, 0.0, colors.edge);
            }
            label_x = mid;
            label_y = (y0 + y1) / 2.0;
        } else {
            let down = b.y >= a.y;
            let x0 = a.x + a.w / 2.0;
            let x1 = b.x + b.w / 2.0;
            let y0 = if down { a.y + a.h } else { a.y };
            let y1 = if down { b.y } else { b.y + b.h };
            let mid = y1
                - (if down {
                    rank_gap / 2.0
                } else {
                    -rank_gap / 2.0
                });
            if (x0 - x1).abs() < 1.0 {
                canvas.line(x0, y0, x1, y1, colors.edge, lw, dashed);
            } else {
                canvas.poly(
                    &[[x0, y0], [x0, mid], [x1, mid], [x1, y1]],
                    colors.edge,
                    lw,
                    false,
                );
            }
            if edge.arrow {
                canvas.arrow_head(x1, y1, 0.0, if down { 1.0 } else { -1.0 }, colors.edge);
            }
            label_x = (x0 + x1) / 2.0;
            label_y = if (x0 - x1).abs() < 1.0 {
                (y0 + y1) / 2.0
            } else {
                mid
            };
        }
        if !edge.label.is_empty() {
            let m = vector::measure(&edge.label, fs * 0.86, Family::Sans, 400, false);
            canvas.rect(
                label_x - m.advance / 2.0 - 4.0,
                label_y - fs * 0.62,
                m.advance + 8.0,
                fs * 1.24,
                colors.label_bg,
                3.0,
            );
            canvas.centered_text(label_x, label_y, &edge.label, fs * 0.86, colors.muted);
        }
    }
    // nodes
    for node in &flow.nodes {
        match node.shape {
            Shape::Diamond => {
                let points = [
                    [node.x + node.w / 2.0, node.y],
                    [node.x + node.w, node.y + node.h / 2.0],
                    [node.x + node.w / 2.0, node.y + node.h],
                    [node.x, node.y + node.h / 2.0],
                ];
                canvas.poly(&points, colors.node_fill, 0.0, true);
                canvas.poly(&points, colors.node_border, 1.2, true);
            }
            Shape::Hexagon => {
                let d: f32 = 12.0;
                let points = [
                    [node.x + d, node.y],
                    [node.x + node.w - d, node.y],
                    [node.x + node.w, node.y + node.h / 2.0],
                    [node.x + node.w - d, node.y + node.h],
                    [node.x + d, node.y + node.h],
                    [node.x, node.y + node.h / 2.0],
                ];
                canvas.poly(&points, colors.node_fill, 0.0, true);
                canvas.poly(&points, colors.node_border, 1.2, true);
            }
            Shape::Asymmetric => {
                let d: f32 = 12.0;
                let points = [
                    [node.x, node.y],
                    [node.x + node.w, node.y],
                    [node.x + node.w, node.y + node.h],
                    [node.x, node.y + node.h],
                    [node.x + d, node.y + node.h / 2.0],
                ];
                canvas.poly(&points, colors.node_fill, 0.0, true);
                canvas.poly(&points, colors.node_border, 1.2, true);
            }
            _ => {
                let r: f32 = match node.shape {
                    Shape::Round => 8.0,
                    Shape::Stadium => node.h / 2.0,
                    Shape::Circle => node.w / 2.0,
                    Shape::Cylinder => 10.0,
                    _ => 3.0,
                };
                canvas.rect(node.x, node.y, node.w, node.h, colors.node_fill, r);
                canvas.rounded_border(
                    node.x,
                    node.y,
                    node.w,
                    node.h,
                    r.min(node.w.min(node.h) / 2.0),
                    colors.node_border,
                );
                if node.shape == Shape::Subroutine {
                    canvas.line(
                        node.x + 7.0,
                        node.y,
                        node.x + 7.0,
                        node.y + node.h,
                        colors.node_border,
                        1.2,
                        false,
                    );
                    canvas.line(
                        node.x + node.w - 7.0,
                        node.y,
                        node.x + node.w - 7.0,
                        node.y + node.h,
                        colors.node_border,
                        1.2,
                        false,
                    );
                }
            }
        }
        canvas.centered_text(
            node.x + node.w / 2.0,
            node.y + node.h / 2.0,
            &node.label,
            fs,
            colors.text,
        );
    }
    if horizontal {
        height += back_lane;
    } else {
        width += back_lane;
    }
    Some(anchor_top(canvas.finish(width, height)))
}
