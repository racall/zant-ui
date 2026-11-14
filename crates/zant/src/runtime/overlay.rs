//! Port of `src/ui_runtime.zig:1940-2011` (tooltip painting) and the select
//! popup bookkeeping of `paintSelectPopup` / `closeSelect`.
//!
//! The tooltip is drawn by the runtime itself (it is not a retained node):
//! laid out once when it appears, animated with antd's `zoom-big-fast`
//! presence, centred below its target and flipped above near the window bottom.
//!
//! The select popup is painted by `components/select.zig:paintPopup` in Zig;
//! that component is not ported yet (WP-2.2), so this module keeps the popup's
//! `open_select` / `popup_select` bookkeeping but paints nothing yet.
#![forbid(unsafe_code)]

use crate::core::geometry::Rect;
use crate::core::node::NodeData;
use crate::motion::motion::{self, Animated, Presence};
use crate::renderer::canvas::Canvas;
use crate::renderer::text::{self, TextLayout};

use super::Runtime;

/// antd colorBgSpotlight (tooltip background).
const TOOLTIP_BACKGROUND: u32 = 0xd900_0000;

impl Runtime {
    /// Zig `closeSelect`: starts the closing transition of the open popup.
    pub(crate) fn close_select(&mut self) {
        if let Some(id) = self.open_select {
            if let NodeData::Select(data) = &mut self.tree.get_mut(id).data {
                data.open = false;
                data.hovered_option = None;
            }
            self.tree.poke(id);
            self.open_select = None;
        }
    }

    /// Zig `paintSelectPopup`: keeps `popup_select` alive while the popup's
    /// closing transition runs, then forgets it.
    pub(crate) fn paint_select_popup(&mut self, _canvas: Canvas) {
        if let Some(id) = self.open_select {
            self.popup_select = Some(id);
        }
        let Some(id) = self.popup_select else {
            return;
        };
        let (alive, is_select, open, open_t) = {
            let node = self.tree.get(id);
            match &node.data {
                NodeData::Select(data) => (node.alive, true, data.open, data.open_t),
                _ => (node.alive, false, false, 0.0),
            }
        };
        if !alive || !is_select {
            self.popup_select = None;
            return;
        }
        // painted until the closing transition has finished
        if !open && open_t <= 0.003 {
            self.popup_select = None;
            return;
        }
        // TODO(WP-2.2): `components::select::paint_popup(node, canvas, theme,
        // viewport)` is not ported yet, so an open select paints no popup.
        let _ = (open, open_t);
    }

    /// The tooltip state for tests: `(visible, rect)`.
    #[cfg(test)]
    pub(crate) fn tooltip_state(&self) -> (bool, Rect) {
        (self.tooltip_visible, self.tooltip_rect)
    }

    /// Zig `clearTooltip`: hides the tooltip; a visible one zooms out (its text
    /// layout lives until the transition ends), a pending one is cancelled.
    pub(crate) fn clear_tooltip(&mut self) {
        if self.tooltip_visible {
            self.tree.mark_paint_dirty();
            self.tooltip_anim.retarget(
                0.0,
                self.now_ms,
                motion::presence_exit(Presence::ZoomBigFast),
            );
            self.tooltip_hiding = true;
        } else if !self.tooltip_hiding {
            self.drop_tooltip_layout();
        }
        self.tooltip_target = None;
        self.tooltip_visible = false;
        self.tooltip_show_at = 0;
    }

    pub(crate) fn drop_tooltip_layout(&mut self) {
        self.tooltip_layout = None;
        self.tooltip_hiding = false;
        self.tooltip_anim = Animated::default();
    }

    /// Zig `showTooltip`: lays out the pending tooltip once and starts its
    /// enter transition (centred below the target, flipped above near the
    /// window bottom).
    pub(crate) fn show_tooltip(&mut self) {
        let Some(id) = self.tooltip_target else {
            return;
        };
        let (content, bounds, in_flow) = {
            let node = self.tree.get(id);
            (node.tooltip.clone(), node.bounds, node.in_flow())
        };
        let Some(content) = content else {
            return;
        };
        if !in_flow {
            return;
        }
        // a new tooltip replaces one that is still zooming out
        self.drop_tooltip_layout();
        let style = text::Style {
            font_size: 12.0,
            color: 0xffff_ffff,
            wrap: false,
            ..text::Style::default()
        };
        self.tooltip_layout = TextLayout::init(&content, style, 1.0e6).ok();
        let Some(layout) = &self.tooltip_layout else {
            return;
        };
        let metrics = layout.metrics();
        let width = metrics.longest_line + 16.0;
        let height = metrics.height + 8.0;
        let mut x = bounds.x + (bounds.width - width) * 0.5;
        let mut y = bounds.bottom() + 6.0;
        x = x.clamp(8.0, (self.viewport.width - width - 8.0).max(8.0));
        self.tooltip_above = false;
        if y + height > self.viewport.height - 8.0 {
            y = (bounds.y - height - 6.0).max(8.0);
            self.tooltip_above = true;
        }
        self.tooltip_rect = Rect::init(x, y, width, height);
        self.tooltip_anim = Animated::start(
            0.0,
            1.0,
            self.now_ms,
            motion::presence_enter(Presence::ZoomBigFast),
        );
        self.tooltip_visible = true;
        self.tree.mark_paint_dirty();
    }

    /// Zig `paintTooltip`.
    pub(crate) fn paint_tooltip(&mut self, canvas: Canvas) {
        if !self.tooltip_visible && !self.tooltip_hiding {
            return;
        }
        let Some(layout) = self.tooltip_layout.as_ref() else {
            return;
        };
        let rect = self.tooltip_rect;
        let fx = motion::presence_fx(Presence::ZoomBigFast, self.tooltip_anim.value(self.now_ms));
        if fx.opacity <= 0.003 {
            return;
        }
        let origin = motion::origin_point(
            if self.tooltip_above {
                motion::Origin::Bottom
            } else {
                motion::Origin::Top
            },
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        );
        canvas.save();
        canvas.translate(origin[0], origin[1]);
        canvas.scale(fx.scale_x, fx.scale_y);
        canvas.translate(-origin[0], -origin[1]);
        let layer = fx.opacity < 0.997;
        if layer {
            canvas.save_layer_alpha(
                Rect::init(
                    rect.x - 4.0,
                    rect.y - 4.0,
                    rect.width + 8.0,
                    rect.height + 8.0,
                ),
                fx.opacity,
            );
        }
        canvas.fill_round_rect(rect, 6.0, TOOLTIP_BACKGROUND);
        layout.paint(canvas, rect.x + 8.0, rect.y + 4.0);
        if layer {
            canvas.restore();
        }
        canvas.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::{Event, Modifiers, PointerButton, PointerEvent};
    use crate::core::node::{FlexStyle, NodeKind};
    use crate::renderer::renderer::Renderer;
    use crate::ui::RunOptions;
    use crate::ui::app::App;
    use crate::ui::builders::column;
    use crate::ui::element::{Element, Node};
    use crate::ui::options::BoxOptions;
    use std::sync::atomic::{AtomicI64, Ordering};

    fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 300.0, 200.0, 1.0);
        renderer.end();
    }

    fn pointer(x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            x,
            y,
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
        }
    }

    struct Tip;

    impl App for Tip {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![Node::new(Element {
                    kind: NodeKind::Button,
                    content: "Save".into(),
                    tooltip: Some("Save changes".into()),
                    focusable: true,
                    layout: FlexStyle {
                        width: Some(100.0),
                        height: Some(32.0),
                        shrink: 0.0,
                        ..Default::default()
                    },
                    ..Element::default()
                })],
                ..Default::default()
            })
        }
    }

    #[test]
    fn hover_tooltip_appears_after_the_delay_and_zooms_out_on_leave() {
        static NOW: AtomicI64 = AtomicI64::new(1_000);
        fn clock() -> i64 {
            NOW.load(Ordering::Relaxed)
        }
        NOW.store(1_000, Ordering::Relaxed);
        let mut runtime = Runtime::init(Tip, RunOptions::default());
        runtime.clock = clock;
        let mut renderer = Renderer::create_raster(300, 200).expect("renderer");
        frame(&mut runtime, &mut renderer);
        let button = runtime
            .tree
            .nodes
            .iter()
            .find(|node| node.alive && node.kind() == NodeKind::Button)
            .map(|node| node.id)
            .expect("button");
        let bounds = runtime.tree.get(button).bounds;
        let center = pointer(
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        );
        runtime.dispatch(Event::PointerMove(center));
        assert_eq!(runtime.tooltip_target, Some(button));
        assert_eq!(runtime.tooltip_show_at, 1_450, "450 ms hover delay");
        // the frame after the hover animates the button hover look
        frame(&mut runtime, &mut renderer);
        assert!(!runtime.tooltip_state().0);
        assert_eq!(runtime.next_delay_ms, 16, "the button hover transition");
        // once the hover transition settled, the tooltip wake-up is scheduled
        NOW.store(1_210, Ordering::Relaxed);
        frame(&mut runtime, &mut renderer);
        assert_eq!(runtime.next_delay_ms, 240);
        assert!(!runtime.tooltip_state().0);
        // once the delay passed the tooltip appears above the target
        NOW.store(2_000, Ordering::Relaxed);
        frame(&mut runtime, &mut renderer);
        let (visible, rect) = runtime.tooltip_state();
        assert!(visible, "the tooltip is shown");
        assert!(rect.width >= 16.0 && rect.height >= 8.0);
        assert!(rect.x >= 8.0 && rect.bottom() <= 200.0 - 8.0);
        assert_eq!(runtime.next_delay_ms, 16, "the enter transition runs");
        NOW.store(2_200, Ordering::Relaxed);
        frame(&mut runtime, &mut renderer);
        assert_eq!(runtime.next_delay_ms, -1, "the tooltip settled");
        // leaving starts the zoom-out; the layout is dropped when it ends
        runtime.dispatch(Event::PointerLeave(center));
        assert!(!runtime.tooltip_state().0);
        assert!(runtime.tooltip_hiding);
        frame(&mut runtime, &mut renderer);
        NOW.store(2_400, Ordering::Relaxed);
        frame(&mut runtime, &mut renderer);
        assert!(!runtime.tooltip_hiding);
        assert!(runtime.tooltip_layout.is_none());
        assert_eq!(runtime.next_delay_ms, -1);
    }
}
