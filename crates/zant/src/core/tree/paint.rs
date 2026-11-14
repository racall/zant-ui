//! Port of `src/core/tree.zig` — WP-1.3: painting the retained tree.
#![forbid(unsafe_code)]

use crate::core::geometry::Rect;
use crate::core::node::{NodeData, NodeId, NodeKind};
use crate::core::tree::Tree;
use crate::core::vector;
use crate::motion::motion;
use crate::renderer::canvas::Canvas;
use crate::renderer::text::TextLayout;
use crate::theme::tokens;
use zant_sys::skia as ffi;

const MAX_SELECTION_RECTS: usize = 64;

impl Tree {
    pub fn paint(&mut self, canvas: Canvas) {
        let root = self.root;
        self.paint_node(root, canvas);
    }

    fn paint_node(&mut self, id: NodeId, canvas: Canvas) {
        let index = id as usize;
        if !self.nodes[index].visible {
            return;
        }
        if self.nodes[index].visual.show_on_hover {
            let t = self.ancestor_hover_t(id);
            if t <= 0.003 {
                return;
            }
            if t < 0.997 {
                let b = self.nodes[index].bounds;
                canvas.save_layer_alpha(
                    Rect::init(b.x - 4.0, b.y - 4.0, b.width + 8.0, b.height + 8.0),
                    t,
                );
                self.paint_node_inner(id, canvas);
                canvas.restore();
                return;
            }
        }
        self.paint_node_inner(id, canvas);
    }

    /// Hover progress (0…1) of the nearest ancestor with a hover look.
    fn ancestor_hover_t(&self, id: NodeId) -> f32 {
        let mut parent = self.nodes[id as usize].parent;
        while let Some(parent_id) = parent {
            let p = &self.nodes[parent_id as usize];
            if let NodeData::Container(data) = &p.data
                && data.hover_visual.is_some()
            {
                return data.hover_t;
            }
            parent = p.parent;
        }
        1.0
    }

    fn paint_node_inner(&mut self, id: NodeId, canvas: Canvas) {
        let index = id as usize;
        let bounds = self.nodes[index].bounds;
        let Some(anim) = self.nodes[index].anim else {
            self.paint_node_content(id, canvas, bounds);
            return;
        };
        // FLIP offset applies to the node's own box (leaf containers)
        let mut rect = bounds;
        if anim.flip_active {
            rect.x += anim.flip_delta[0];
            rect.y += anim.flip_delta[1];
            rect.width = (rect.width + anim.flip_delta[2]).max(0.0);
            rect.height = (rect.height + anim.flip_delta[3]).max(0.0);
        }
        let fx = anim.fx;
        if fx.is_identity() {
            self.paint_node_content(id, canvas, rect);
            return;
        }
        if fx.opacity <= 0.003 {
            return;
        }
        let motion_style = self.nodes[index].motion;
        let origin = motion::origin_point(
            motion::presence_origin(motion_style.presence, motion_style.origin),
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        );
        canvas.save();
        canvas.translate(origin[0] + fx.dx, origin[1] + fx.dy);
        canvas.scale(fx.scale_x, fx.scale_y);
        canvas.translate(-origin[0], -origin[1]);
        // an offscreen layer only while fading (opacity 1 draws directly)
        let layer = fx.opacity < 0.997;
        if layer {
            let visual = self.nodes[index].visual;
            let pad = (3.0 * visual.shadow_blur + visual.shadow_offset_y.abs()).max(8.0);
            canvas.save_layer_alpha(
                Rect::init(
                    bounds.x - pad,
                    bounds.y - pad,
                    bounds.width + pad * 2.0,
                    bounds.height + pad * 2.0,
                ),
                fx.opacity,
            );
        }
        self.paint_node_content(id, canvas, rect);
        if layer {
            canvas.restore();
        }
        canvas.restore();
    }

    fn paint_node_content(&mut self, id: NodeId, canvas: Canvas, rect: Rect) {
        let index = id as usize;
        let visual = self.nodes[index].visual;
        let kind = self.nodes[index].data.kind();
        let theme = self.theme;
        let selection_color = self.selection_color;
        let text_sel = self.nodes[index].text_sel;
        if let Some(shadow) = visual.shadow_color {
            let mut shadow_rect = rect;
            shadow_rect.y += visual.shadow_offset_y;
            canvas.shadow_round_rect(shadow_rect, visual.radius, shadow, visual.shadow_blur);
        }
        // buttons, inputs and selects paint their own (state-dependent)
        // background; filling it here too would double translucent fills
        let self_filled = matches!(kind, NodeKind::Button | NodeKind::Input | NodeKind::Select);
        let mut hover_fill: Option<u32> = None;
        let mut hover_border: Option<u32> = None;
        if let NodeData::Container(data) = &self.nodes[index].data
            && let Some(hover_visual) = data.hover_visual
            && data.hover_t > 0.001
        {
            if let Some(background) = hover_visual.background {
                hover_fill = Some(motion::mix_color(
                    visual.background.unwrap_or(background & 0x00ff_ffff),
                    background,
                    data.hover_t,
                ));
            }
            if let Some(border_color) = hover_visual.border_color {
                hover_border = Some(motion::mix_color(
                    visual.border_color.unwrap_or(border_color & 0x00ff_ffff),
                    border_color,
                    data.hover_t,
                ));
            }
        }
        if (visual.background.is_some() || hover_fill.is_some()) && !self_filled {
            let color = hover_fill.or(visual.background).unwrap_or(0);
            if visual.radius > 0.0 {
                canvas.fill_round_rect(rect, visual.radius, color);
            } else {
                canvas.fill_rect(rect, color);
            }
        }
        match kind {
            NodeKind::Root | NodeKind::Container => {}
            NodeKind::Text => {
                if let NodeData::Text(data) = &mut self.nodes[index].data {
                    let max_width = data.layout_width(rect.width);
                    if let Some(layout) = data.ensure_layout(max_width) {
                        if let Some(range) = text_sel {
                            paint_selection(canvas, layout, range, rect.x, rect.y, selection_color);
                        }
                        layout.paint(canvas, rect.x, rect.y);
                    }
                }
            }
            NodeKind::Button => {
                crate::components::button::paint(&mut self.nodes[index], canvas, theme);
            }
            // WP-2.1 / WP-2.2 own the paint of these components; until their
            // modules land only the generic background / border above is drawn.
            NodeKind::Input | NodeKind::Toggle | NodeKind::Image | NodeKind::Select => {}
            NodeKind::RichText => {
                if let NodeData::RichText(data) = &mut self.nodes[index].data {
                    // Rich-text selection painting needs the range-rect helper
                    // of `renderer::rich_text::Layout` (WP-1.1c only wraps
                    // `TextLayout::range_rects`); deferred with text selection.
                    data.paint_at(canvas, rect.x, rect.y, rect.width);
                }
            }
            NodeKind::Vector => {
                if let NodeData::Vector(data) = &self.nodes[index].data {
                    if text_sel.is_some() {
                        canvas.fill_rect(rect, selection_color);
                    }
                    vector::paint(canvas, &data.drawing, rect.x, rect.y + data.drawing.ascent);
                }
            }
        }

        let should_clip = visual.clip || kind == NodeKind::Input;
        if should_clip {
            canvas.save();
            canvas.clip_rect(rect);
        }
        let children = self.nodes[index].children.clone();
        if self.nodes[index].style.scroll {
            // scroll containers: skip children far outside the viewport
            let bounds = self.nodes[index].bounds;
            let top = bounds.y - 64.0;
            let bottom = bounds.y + bounds.height + 64.0;
            for child in children {
                let child_bounds = self.nodes[child as usize].bounds;
                if child_bounds.y > bottom || child_bounds.y + child_bounds.height < top {
                    continue;
                }
                self.paint_node(child, canvas);
            }
        } else {
            for child in children {
                self.paint_node(child, canvas);
            }
        }
        // borders frame the content: drawn after children so filled children
        // (headers, cells) never cover them
        if let Some(border) = hover_border.or(visual.border_color)
            && visual.border_width > 0.0
        {
            canvas.stroke_round_rect(
                Rect::init(
                    rect.x + 0.5,
                    rect.y + 0.5,
                    (rect.width - 1.0).max(0.0),
                    (rect.height - 1.0).max(0.0),
                ),
                visual.radius,
                border,
                visual.border_width,
            );
        }
        if self.nodes[index].style.scroll {
            paint_scroll_indicator(
                &self.nodes[index],
                canvas,
                tokens::with_alpha(theme.color_text, 0x40),
            );
        }
        if should_clip {
            canvas.restore();
        }
    }
}

fn paint_selection(
    canvas: Canvas,
    layout: &TextLayout,
    range: [u32; 2],
    x: f32,
    y: f32,
    color: u32,
) {
    if range[0] >= range[1] {
        return;
    }
    let mut rects = [ffi::ZantRect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    }; MAX_SELECTION_RECTS];
    let count = layout.range_rects(range[0] as usize, range[1] as usize, &mut rects);
    for rect in &rects[..count] {
        canvas.fill_rect(
            Rect::init(x + rect.x, y + rect.y, rect.width, rect.height),
            color,
        );
    }
}

fn paint_scroll_indicator(node: &crate::core::node::RetainedNode, canvas: Canvas, color: u32) {
    let NodeData::Container(data) = &node.data else {
        return;
    };
    let view = node.bounds.height;
    if view <= 0.0 || data.content_height <= view + 0.5 {
        return;
    }
    let track = view - 8.0;
    let thumb = (track * view / data.content_height).max(28.0);
    let progress = data.scroll_offset / data.max_scroll(view);
    let y = node.bounds.y + 4.0 + (track - thumb) * progress;
    let x = node.bounds.right() - 7.0;
    canvas.fill_round_rect(Rect::init(x, y, 3.5, thumb), 1.75, color);
}
