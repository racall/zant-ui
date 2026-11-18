//! The frame loop.
//!
//! One frame: sample the clock, finish `ui::wake_after` timers, rebuild when
//! needed, advance transitions (`tick`), lay out (up to three passes while
//! `on_resize` changes the tree), run queued commands, paint, then compute the
//! delay until the next frame and write `ZANT_LAYOUT_FILE`.
#![forbid(unsafe_code)]

use crate::core::geometry::Size;
use crate::core::node::{NodeData, NodeId};
use crate::motion::easing::Easing;
use crate::motion::motion::{self, Animated, Transition};
use crate::renderer::canvas::Canvas;
use crate::ui::CommandKind;

use super::Runtime;
use super::debug::{self, Phase};

impl Runtime {
    /// The timestamp of this frame / event.
    pub(crate) fn sample_clock(&mut self) {
        self.now_ms = (self.clock)();
        self.tree.now_ms = self.now_ms;
        crate::ui::set_now(self.now_ms);
    }

    /// Microsecond timestamp for `ZANT_FRAME_STATS`, 0 when stats are off.
    fn stats_clock(&self) -> i64 {
        if self.frame_stats.is_some() {
            crate::platform::platform::monotonic_us()
        } else {
            0
        }
    }

    /// Draws one frame into `canvas`. The native host
    /// calls it from `HostHandler::draw`; headless tests call it with a raster
    /// canvas and a fake [`Runtime::clock`].
    pub fn render_frame(&mut self, canvas: Canvas, width: f32, height: f32, dpr: f32) {
        crate::ui::set_display_scale(dpr);
        let stats_start = self.stats_clock();
        self.sample_clock();
        if let Some(at) = self.wake_at_ms
            && self.now_ms >= at
        {
            self.wake_at_ms = None;
            self.needs_build = true;
        }
        // a frame painted from inside an event callback (modal dialog loop)
        // shows the current tree as is; the rebuild waits for the event to end
        let nested = self.nested_frame();
        if super::input_trace() {
            eprintln!(
                "[trace] {:.1} frame needs_build={}",
                crate::platform::platform::monotonic_us() as f64 / 1000.0,
                self.needs_build
            );
        }
        if self.needs_build && !nested {
            let start = self.stats_clock();
            self.rebuild();
            if let Some(stats) = self.frame_stats.as_mut() {
                stats.phase(Phase::Rebuild, start);
            }
        }
        if !nested {
            self.tick();
        }
        self.viewport = Size { width, height };
        let mut resized = false;
        if width != self.last_width || height != self.last_height {
            self.last_width = width;
            self.last_height = height;
            self.tree.mark_layout_dirty();
            resized = true;
        }
        if self.tree.needs_layout() {
            // a window resize moves things without a state change: no FLIP
            self.tree.flip_enabled = !resized;
            let start = self.stats_clock();
            self.tree.layout(self.viewport);
            self.refresh_key_bounds();
            // size-dependent UI (on_resize): rebuild and lay out again now, so
            // the new size never shows a frame of the old layout. Up to three
            // passes: a virtual list measures the items it just built, places
            // them, and may then build the ones that came into view.
            let mut passes: usize = 0;
            while !nested && passes < 3 && self.notify_resizes() {
                passes += 1;
                self.rebuild();
                if self.tree.needs_layout() {
                    self.tree.layout(self.viewport);
                    self.refresh_key_bounds();
                }
            }
            if let Some(stats) = self.frame_stats.as_mut() {
                stats.phase(Phase::Layout, start);
            }
            self.tree.flip_enabled = true;
        }
        if resized && let Some(stats) = self.frame_stats.as_mut() {
            stats.resized(crate::platform::platform::monotonic_us());
        }
        if !nested {
            self.apply_view_state();
        }
        self.run_commands();
        if !nested {
            self.update_focus_scopes();
        }
        let paint_start = self.stats_clock();
        canvas.clear(self.clear_color);
        self.tree.paint(canvas);
        self.paint_select_popup(canvas);
        self.paint_tooltip(canvas);
        if let Some(stats) = self.frame_stats.as_mut() {
            stats.phase(Phase::Paint, paint_start);
        }
        self.tree.clear_paint();
        self.compute_next_delay();
        if let Some(path) = self.layout_file.clone() {
            debug::write_layout_file(&self.tree, &path);
        }
        if let Some(stats) = self.frame_stats.as_mut() {
            let end = crate::platform::platform::monotonic_us();
            stats.frame(stats_start, end, width, height);
        }
    }

    /// Calls `on_resize` of containers whose size changed
    /// in the last layout; true when any was called. Callbacks may read the new
    /// boxes through `ui::key_bounds` (the snapshot was refreshed after layout).
    fn notify_resizes(&mut self) -> bool {
        let mut any = false;
        let mut index = 0usize;
        while index < self.tree.nodes.len() {
            let id = index as NodeId;
            index += 1;
            let Some((action, width, height)) = (|| {
                let node = self.tree.get(id);
                if !node.alive || !node.in_flow() {
                    return None;
                }
                let NodeData::Container(data) = &node.data else {
                    return None;
                };
                let action = data.resize_action.clone()?;
                let bounds = node.bounds;
                let reported = (data.reported_width, data.reported_height);
                if !data.resize_step.map_or_else(
                    || {
                        (bounds.width - reported.0).abs() >= 0.5
                            || (bounds.height - reported.1).abs() >= 0.5
                    },
                    |step| step.changed(reported, (bounds.width, bounds.height)),
                ) {
                    return None;
                }
                Some((action, bounds.width, bounds.height))
            })() else {
                continue;
            };
            if let NodeData::Container(data) = &mut self.tree.get_mut(id).data {
                data.reported_width = width;
                data.reported_height = height;
            }
            let mut app = self.app.borrow_mut();
            let mut cx = super::action_cx(&mut app);
            action.call(&mut cx);
            any = true;
        }
        any
    }

    /// Runs `ui::focus` / `ui::scroll_into_view` /
    /// `ui::scroll_to_end` after layout, when the nodes they name have bounds.
    fn run_commands(&mut self) {
        let commands = crate::ui::take_commands_for(self.owner_ptr());
        for command in commands {
            // window operations carry no node key: run them directly
            match command.kind {
                CommandKind::WindowMinimize => {
                    if let Some(window) = &self.window {
                        window.minimize();
                    }
                    continue;
                }
                CommandKind::WindowMaximize => {
                    if let Some(window) = &self.window {
                        window.toggle_maximize();
                    }
                    continue;
                }
                CommandKind::WindowClose => {
                    if let Some(window) = &self.window {
                        window.close();
                    }
                    continue;
                }
                _ => {}
            }
            let Some(id) = self.find_key(command.key()) else {
                crate::ui::requeue_command(command);
                continue;
            };
            match command.kind {
                CommandKind::Focus => {
                    let node = self.tree.get(id);
                    if node.focusable && node.enabled && node.in_flow() {
                        self.focus_node(Some(id));
                    }
                }
                CommandKind::ScrollIntoView => self.scroll_into_view(id),
                // handled above (no node key)
                CommandKind::WindowMinimize
                | CommandKind::WindowMaximize
                | CommandKind::WindowClose => {}
                CommandKind::ScrollToEnd => {
                    let (scroll, target) = {
                        let node = self.tree.get(id);
                        match &node.data {
                            NodeData::Container(data) if node.style.scroll => (
                                true,
                                data.max_scroll(node.bounds.height) - data.scroll_offset,
                            ),
                            _ => (false, 0.0),
                        }
                    };
                    if scroll {
                        if let NodeData::Container(data) = &mut self.tree.get_mut(id).data {
                            data.scroll_animating = false;
                        }
                        self.tree.scroll_by(id, target);
                    }
                }
            }
        }
    }

    /// The live node with `key`.
    pub fn find_key(&self, key: &str) -> Option<NodeId> {
        self.tree.id_of_key(key)
    }

    /// Animates the nearest scroll container so `id` sits
    /// 12 px below its top edge.
    fn scroll_into_view(&mut self, id: NodeId) {
        let (mut current, target_y) = {
            let target = self.tree.get(id);
            (target.parent, target.bounds.y)
        };
        while let Some(pid) = current {
            let (is_scroll, container_y, max, offset) = {
                let container = self.tree.get(pid);
                match &container.data {
                    NodeData::Container(data) => (
                        container.style.scroll,
                        container.bounds.y,
                        data.max_scroll(container.bounds.height),
                        data.scroll_offset,
                    ),
                    _ => (false, 0.0, 0.0, 0.0),
                }
            };
            current = self.tree.get(pid).parent;
            if !is_scroll {
                continue;
            }
            let goal = (offset + (target_y - container_y) - 12.0).clamp(0.0, max);
            if (goal - offset).abs() < 0.5 {
                return;
            }
            if motion::enabled() {
                let now = self.now_ms;
                if let NodeData::Container(data) = &mut self.tree.get_mut(pid).data {
                    data.scroll_anim = Animated::start(
                        offset,
                        goal,
                        now,
                        Transition {
                            duration_ms: 250,
                            easing: Easing::EaseInOut,
                            delay_ms: 0,
                        },
                    );
                    data.scroll_animating = true;
                }
                self.tree.poke(pid);
            } else {
                self.tree.scroll_by(pid, goal - offset);
            }
            return;
        }
    }

    /// When the native side should ask for the next
    /// frame (-1 = no timer).
    fn compute_next_delay(&mut self) {
        let mut delay: i32 = if self.pending_view_state.is_some() || self.needs_build {
            16
        } else {
            -1
        };
        // transitions may also start after `tick` (FLIP in layout, scroll
        // commands), so the animating list counts too
        if self.any_animation || !self.tree.animating.is_empty() {
            delay = 16;
        }
        if let Some(at) = self.wake_at_ms {
            let wake: i32 = (at - self.now_ms).clamp(1, 60_000) as i32;
            if delay < 0 || wake < delay {
                delay = wake;
            }
        }
        if let Some(id) = self.tree.focused {
            let node = self.tree.get(id);
            if node.enabled
                && let NodeData::Input(data) = &node.data
            {
                let remaining: i64 = (data.next_blink_ms - self.now_ms).max(1);
                let blink_delay: i32 = remaining.min(2000) as i32;
                if delay < 0 || blink_delay < delay {
                    delay = blink_delay;
                }
            }
        }
        if self.tooltip_target.is_some() && !self.tooltip_visible {
            let remaining: i64 = (self.tooltip_show_at - self.now_ms).max(1);
            let tooltip_delay: i32 = remaining.min(1000) as i32;
            if delay < 0 || tooltip_delay < delay {
                delay = tooltip_delay;
            }
        }
        self.next_delay_ms = delay;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::NodeKind;
    use crate::renderer::renderer::Renderer;
    use crate::theme::tokens::ant_light;
    use crate::ui::actions::action;
    use crate::ui::app::App;
    use crate::ui::builders::{button, column, text};
    use crate::ui::options::{BoxOptions, ButtonOptions, TextOptions};
    use crate::ui::{Node, RunOptions};

    /// A small declarative app: white page, a heading and a primary button.
    pub(crate) struct Demo {
        pub clicks: u32,
    }

    impl App for Demo {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                padding: crate::ui::inset(8.0),
                gap: 6.0,
                background: Some(0xffff_ffff),
                children: crate::nodes![
                    text(
                        "Hello Zant",
                        TextOptions {
                            size: 20.0,
                            color: Some(0xff00_0000),
                            ..Default::default()
                        }
                    ),
                    button(ButtonOptions {
                        key: Some("go".into()),
                        text: "Go".into(),
                        variant: crate::core::node::ButtonType::Primary,
                        on_click: Some(action(|state: &mut Demo| state.clicks += 1)),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            })
        }
    }

    /// One frame of `Demo` into a 200×120 raster, as the headless smoke test.
    fn smoke_frame(runtime: &mut Runtime, renderer: &mut Renderer) {
        let canvas = renderer.begin(1.0).expect("a canvas");
        runtime.render_frame(canvas, 200.0, 120.0, 1.0);
        renderer.end();
    }

    #[test]
    fn headless_frame_builds_lays_out_and_paints_pixels() {
        let mut runtime = Runtime::init(Demo { clicks: 0 }, RunOptions::default());
        let mut renderer = Renderer::create_raster(200, 120).expect("raster renderer");
        smoke_frame(&mut runtime, &mut renderer);

        // the app's column becomes the window root; one text, one button
        assert_eq!(runtime.tree.live_ids().len(), 3);
        assert!(runtime.tree.id_of_key("go").is_some());
        assert!(!runtime.needs_build, "the frame rebuilt the tree");
        let button_id = runtime.tree.id_of_key("go").expect("button");
        assert_eq!(runtime.tree.get(button_id).kind(), NodeKind::Button);
        let bounds = runtime.tree.get(button_id).bounds;
        assert!(bounds.width > 0.0 && bounds.height > 0.0);

        // the raster was blank before: the frame paints the white page and the
        // primary button (a different colour)
        let page = renderer.pixel_at(2, 2).expect("pixel");
        assert_eq!(page & 0x00ff_ffff, 0x00ff_ffff, "white background");
        let mut distinct = std::collections::HashSet::new();
        for y in 0..120 {
            for x in 0..200 {
                if let Some(pixel) = renderer.pixel_at(x, y) {
                    distinct.insert(pixel);
                }
            }
        }
        assert!(
            distinct.len() >= 3,
            "background, text and button colours, got {distinct:?}"
        );
        // antd primary blue is painted somewhere inside the button box
        let primary = ant_light().color_primary;
        let mut saw_primary = false;
        for y in (bounds.y as usize)..((bounds.bottom() as usize).min(120)) {
            for x in (bounds.x as usize)..((bounds.right() as usize).min(200)) {
                if renderer
                    .pixel_at(x, y)
                    .is_some_and(|pixel| (pixel & 0x00ff_ffff) == (primary & 0x00ff_ffff))
                {
                    saw_primary = true;
                }
            }
        }
        assert!(saw_primary, "the button painted its primary background");
    }

    #[test]
    fn a_second_frame_without_changes_paints_again_without_rebuilding() {
        let mut runtime = Runtime::init(Demo { clicks: 0 }, RunOptions::default());
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        smoke_frame(&mut runtime, &mut renderer);
        runtime.tree.clear_paint();
        let revision = runtime.tree.live_ids();
        smoke_frame(&mut runtime, &mut renderer);
        assert_eq!(runtime.tree.live_ids(), revision, "no rebuild happened");
        assert!(runtime.now_ms >= 0);
    }

    #[test]
    fn next_frame_delay_is_unset_for_a_static_ui() {
        let mut runtime = Runtime::init(Demo { clicks: 0 }, RunOptions::default());
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        smoke_frame(&mut runtime, &mut renderer);
        assert_eq!(
            runtime.next_delay_ms, -1,
            "a static window does not poll for frames"
        );
    }

    #[test]
    fn wake_after_schedules_a_rebuild_frame() {
        use std::sync::atomic::{AtomicI64, Ordering};
        static NOW: AtomicI64 = AtomicI64::new(1_000);
        fn clock() -> i64 {
            NOW.load(Ordering::Relaxed)
        }
        struct Waker;
        impl App for Waker {
            fn render(&mut self) -> Node {
                crate::ui::wake_after(50);
                column(BoxOptions::default())
            }
        }
        NOW.store(1_000, Ordering::Relaxed);
        let mut runtime = Runtime::init(Waker, RunOptions::default());
        runtime.clock = clock;
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        smoke_frame(&mut runtime, &mut renderer);
        assert_eq!(runtime.wake_at_ms, Some(1_050));
        assert_eq!(runtime.next_delay_ms, 50);
        NOW.store(1_050, Ordering::Relaxed);
        smoke_frame(&mut runtime, &mut renderer);
        // the next build queues a new 50 ms wake-up
        assert_eq!(runtime.wake_at_ms, Some(1_100));
        assert!(!runtime.needs_build);
        assert_eq!(runtime.next_delay_ms, 50);
    }
}
