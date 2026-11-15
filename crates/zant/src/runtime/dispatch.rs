//! Event dispatch.
//!
//! [`Runtime::dispatch`] is the entry point of the native event callback: it
//! samples the clock, lets the event handlers run, reports input selection
//! changes, and tells the window whether to redraw. Pointer / wheel events
//! return non-zero only when something must be redrawn, so plain mouse movement
//! over static UI paints nothing.
#![forbid(unsafe_code)]

use crate::core::events::Event;
use crate::core::node::NodeData;
use crate::ui::actions::PointerPhase;

use super::{Runtime, window};

impl Runtime {
    /// Handles `event` and returns the native callback's
    /// "redraw needed" flag. The native host calls it from
    /// `HostHandler::event`; tests inject events directly.
    pub fn dispatch(&mut self, event: Event) -> i32 {
        self.dispatch_depth += 1;
        let prev_owner = crate::ui::set_command_owner(Some(self.owner_ptr()));
        self.sample_clock();
        let outer_local = super::window::take_local_rebuild();
        let result = self.dispatch_event(&event);
        let local_only = super::window::take_local_rebuild();
        if outer_local {
            super::window::rebuild_only_this_window();
        }
        if super::input_trace() && matches!(event, Event::PointerDown(_) | Event::PointerUp(_)) {
            eprintln!(
                "[trace] {:.1} {} needs_build={} result={}",
                crate::platform::platform::monotonic_us() as f64 / 1000.0,
                if matches!(event, Event::PointerDown(_)) {
                    "down"
                } else {
                    "up"
                },
                self.needs_build,
                result
            );
        }
        self.report_selection();
        // the app's state may have changed: other windows rebuild too (the
        // dispatching window itself is already borrowed, so `invalidate_all`
        // skips it)
        if self.needs_build && !local_only && window::open_count() > 1 {
            window::invalidate_all();
        }
        let redraw = match &event {
            Event::PointerMove(_)
            | Event::PointerDown(_)
            | Event::PointerUp(_)
            | Event::PointerEnter(_)
            | Event::PointerLeave(_)
            | Event::Scroll(_) => {
                i32::from(self.needs_build || self.tree.needs_paint() || self.tree.needs_layout())
            }
            _ => result,
        };
        crate::ui::set_command_owner(prev_owner);
        self.dispatch_depth -= 1;
        redraw
    }

    /// The per-event handler.
    fn dispatch_event(&mut self, event: &Event) -> i32 {
        match event {
            Event::PointerMove(p) | Event::PointerEnter(p) => {
                self.pointer_position = Some([p.x, p.y]);
            }
            Event::PointerDown(p) | Event::PointerUp(p) => {
                self.pointer_position = Some([p.x, p.y]);
                self.pointer_pressed = matches!(event, Event::PointerDown(_));
            }
            Event::Scroll(p) => self.pointer_position = Some([p.x, p.y]),
            Event::PointerLeave(_) | Event::WindowBlur => {
                self.pointer_position = None;
                self.pointer_pressed = false;
            }
            _ => {}
        }
        match event {
            Event::PointerMove(p) => {
                self.handle_pointer_move(*p);
                1
            }
            Event::PointerEnter(p) => {
                self.handle_pointer_move(*p);
                1
            }
            Event::PointerDown(p) => {
                self.tree.keyboard_modality = false;
                self.handle_pointer_down(*p);
                self.needs_build = true;
                1
            }
            Event::PointerUp(p) => {
                self.handle_pointer_up(*p);
                self.needs_build = true;
                1
            }
            Event::PointerLeave(p) => {
                self.set_hover_box(None);
                if let Some(previous) = self.pointer_hover_area {
                    self.pointer_hover_area = None;
                    self.send_pointer(previous, PointerPhase::Leave, *p, 0.0, 0.0);
                }
                self.set_hover(None);
                self.drag_input = None;
                self.clear_pressed();
                if let Some(window) = &self.window {
                    window.set_cursor(crate::platform::platform::Cursor::Arrow);
                }
                1
            }
            Event::KeyDown(key) => {
                self.tree.keyboard_modality = true;
                let consumed = !self.focused_has_composition()
                    && match &self.window_hooks {
                        // an extra window sees its own keys before the widgets
                        Some(hooks) => hooks.on_key.as_ref().is_some_and(|hook| hook(key)),
                        None => self.app.borrow_mut().on_key(key),
                    };
                if consumed {
                    self.needs_build = true;
                    return 1;
                }
                let result = self.handle_key_down(*key);
                self.needs_build = true;
                result
            }
            Event::KeyUp(_) => 0,
            Event::TextInput(value) => {
                let result = self.handle_text_input(value);
                self.needs_build = true;
                result
            }
            Event::Composition(composition) => {
                let result = self.handle_composition(composition);
                self.needs_build = true;
                result
            }
            Event::WindowBlur => {
                if let Some(hooks) = &self.window_hooks
                    && let Some(hook) = &hooks.on_focus
                    && hook(false)
                {
                    self.needs_build = true;
                }
                self.active = false;
                self.sel_dragging = false;
                self.release_pointer_capture(crate::core::events::PointerEvent {
                    x: self.last_click_x,
                    y: self.last_click_y,
                    ..Default::default()
                });
                self.cancel_focused_composition();
                self.close_select();
                self.tree.set_focus(None);
                self.set_hover(None);
                self.clear_pressed();
                self.tree.mark_paint_dirty();
                0
            }
            Event::WindowFocus => {
                if let Some(hooks) = &self.window_hooks
                    && let Some(hook) = &hooks.on_focus
                    && hook(true)
                {
                    self.needs_build = true;
                }
                self.active = true;
                0
            }
            Event::WindowResize(_) => {
                if let Some(stats) = self.frame_stats.as_mut() {
                    stats.resize_requested(crate::platform::platform::monotonic_us());
                }
                self.tree.mark_layout_dirty();
                0
            }
            Event::WindowClose => {
                // an extra window was closed by the system (its close button
                // is not vetoed here): free it on a fresh event-loop turn,
                // never from inside this callback
                if self.window_hooks.is_some() {
                    window::request_close(self.owner_ptr());
                }
                0
            }
            Event::AppearanceChange => {
                // theme hooks that follow the system re-evaluate
                self.needs_build = true;
                1
            }
            Event::WindowCloseRequest => {
                let allowed = match &self.window_hooks {
                    Some(hooks) => hooks.on_close_request.as_ref().is_none_or(|hook| hook()),
                    None => self.app.borrow_mut().on_close_request(),
                };
                self.needs_build = true;
                i32::from(!allowed)
            }
            Event::AppQuitRequest => {
                if self.window_hooks.is_some() {
                    // only the main window decides about quitting, so a
                    // panel simply allows the app to quit
                    return 0;
                }
                let allowed = self.app.borrow_mut().on_quit_request();
                self.needs_build = true;
                i32::from(!allowed)
            }
            Event::Scroll(scroll) => self.handle_scroll(*scroll),
        }
    }

    /// Wheel events: pointer areas with `capture_scroll` first, then
    /// multiline input scrolling, then the innermost scroll container.
    fn handle_scroll(&mut self, scroll: crate::core::events::Scroll) -> i32 {
        if let Some(target) = self.tree.wheel_target_at(scroll.x, scroll.y) {
            let capture = match &self.tree.get(target).data {
                NodeData::Container(data) => data
                    .pointer
                    .as_ref()
                    .is_some_and(|area| area.capture_scroll),
                _ => false,
            };
            if capture {
                self.send_pointer(
                    target,
                    PointerPhase::Scroll,
                    crate::core::events::PointerEvent {
                        x: scroll.x,
                        y: scroll.y,
                        button: crate::core::events::PointerButton::Left,
                        modifiers: scroll.modifiers,
                    },
                    scroll.dx,
                    scroll.dy,
                );
                return 1;
            }
        }
        if let Some(id) = self.tree.hit_test(scroll.x, scroll.y) {
            let (multiline, view_height) = {
                let node = self.tree.get(id);
                (
                    matches!(&node.data, NodeData::Input(data) if data.multiline),
                    (node.bounds.height - 16.0).max(0.0),
                )
            };
            if multiline {
                let max = {
                    let NodeData::Input(data) = &mut self.tree.get_mut(id).data else {
                        unreachable!()
                    };
                    super::ime::input_max_scroll_y(data, view_height)
                };
                if let NodeData::Input(data) = &mut self.tree.get_mut(id).data {
                    data.scroll_y = (data.scroll_y - scroll.dy).clamp(0.0, max);
                }
                self.tree.mark_dirty(id, false, true);
                return 1;
            }
        }
        if self.tree.scroll_at(scroll.x, scroll.y, scroll.dy) {
            self.update_cursor(self.tree.hit_test(scroll.x, scroll.y));
            return 1;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::{
        Composition, Event, Key, KeyEvent, Modifiers, PointerButton, PointerEvent,
    };
    use crate::core::node::{FlexStyle, NodeKind};
    use crate::renderer::renderer::Renderer;
    use crate::ui::RunOptions;
    use crate::ui::actions::{action, text_action};
    use crate::ui::app::App;
    use crate::ui::builders::{button, column};
    use crate::ui::element::{Element, Node};
    use crate::ui::options::{BoxOptions, ButtonOptions};

    fn frame(runtime: &mut Runtime, renderer: &mut Renderer, width: f32, height: f32) {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, width, height, 1.0);
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

    fn key(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            modifiers: Modifiers::default(),
            repeat: false,
        }
    }

    struct Clicks {
        clicks: u32,
    }

    impl App for Clicks {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![button(ButtonOptions {
                    key: Some("ok".into()),
                    text: "OK".into(),
                    on_click: Some(action(|state: &mut Clicks| state.clicks += 1)),
                    ..Default::default()
                })],
                ..Default::default()
            })
        }
    }

    #[test]
    fn pointer_click_runs_the_button_action_once() {
        let mut runtime = Runtime::init(Clicks { clicks: 0 }, RunOptions::default());
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        frame(&mut runtime, &mut renderer, 200.0, 120.0);
        let id = runtime.find_key("ok").expect("button");
        let bounds = runtime.tree.get(id).bounds;
        let center = pointer(
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        );
        assert_eq!(runtime.dispatch(Event::PointerMove(center)), 1);
        assert_eq!(runtime.dispatch(Event::PointerDown(center)), 1);
        assert!(runtime.needs_build, "a press rebuilds");
        assert_eq!(runtime.dispatch(Event::PointerUp(center)), 1);
        assert_eq!(runtime.app_state_mut::<Clicks>().clicks, 1);
        // the next frame rebuilds and must not click again
        frame(&mut runtime, &mut renderer, 200.0, 120.0);
        assert_eq!(runtime.app_state_mut::<Clicks>().clicks, 1);
        assert_eq!(
            runtime.tree.hit_test(bounds.x + 1.0, bounds.y + 1.0),
            Some(id)
        );
    }

    #[test]
    fn hover_over_static_ui_does_not_request_a_redraw() {
        struct Plain;
        impl App for Plain {
            fn render(&mut self) -> Node {
                column(BoxOptions {
                    children: crate::nodes![crate::ui::text("hi", Default::default())],
                    ..Default::default()
                })
            }
        }
        let mut runtime = Runtime::init(Plain, RunOptions::default());
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        frame(&mut runtime, &mut renderer, 200.0, 120.0);
        assert_eq!(runtime.dispatch(Event::PointerMove(pointer(5.0, 5.0))), 0);
        assert!(!runtime.needs_build);
    }

    #[test]
    fn tab_moves_focus_and_enter_activates_the_focused_button() {
        struct Two {
            first: u32,
            second: u32,
        }
        impl App for Two {
            fn render(&mut self) -> Node {
                column(BoxOptions {
                    children: crate::nodes![
                        button(ButtonOptions {
                            key: Some("a".into()),
                            text: "A".into(),
                            on_click: Some(action(|state: &mut Two| state.first += 1)),
                            ..Default::default()
                        }),
                        button(ButtonOptions {
                            key: Some("b".into()),
                            text: "B".into(),
                            on_click: Some(action(|state: &mut Two| state.second += 1)),
                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                })
            }
        }
        let mut runtime = Runtime::init(
            Two {
                first: 0,
                second: 0,
            },
            RunOptions::default(),
        );
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        frame(&mut runtime, &mut renderer, 200.0, 120.0);
        let first = runtime.find_key("a").expect("a");
        let second = runtime.find_key("b").expect("b");
        assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Tab))), 1);
        assert_eq!(
            runtime.tree.focused,
            Some(first),
            "Tab focuses the first button"
        );
        assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Tab))), 1);
        assert_eq!(runtime.tree.focused, Some(second));
        assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Enter))), 1);
        assert_eq!(runtime.app_state_mut::<Two>().second, 1);
        assert_eq!(runtime.app_state_mut::<Two>().first, 0);
        // Escape drops the focus
        assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Escape))), 1);
        assert_eq!(runtime.tree.focused, None);
    }

    /// An input built from a raw `Element`, without the `ui::input` builder.
    fn input_node(value: &str, on_text: crate::ui::TextAction) -> Node {
        Node::new(Element {
            kind: NodeKind::Input,
            content: crate::Str::copy(value),
            placeholder: "name".into(),
            layout: FlexStyle {
                width: Some(150.0),
                height: Some(32.0),
                shrink: 0.0,
                ..Default::default()
            },
            on_text: Some(on_text),
            // The `ui::input` builder marks the element focusable; the raw
            // element used here has to say it.
            focusable: true,
            ..Element::default()
        })
    }

    struct Form {
        text: String,
        edits: u32,
    }

    impl App for Form {
        fn render(&mut self) -> Node {
            let text = self.text.clone();
            column(BoxOptions {
                children: crate::nodes![input_node(
                    &text,
                    text_action(|state: &mut Form, value| {
                        state.text = value.to_string();
                        state.edits += 1;
                    })
                )],
                ..Default::default()
            })
        }
    }

    fn click_into_input(runtime: &mut Runtime) {
        let id = runtime
            .tree
            .nodes
            .iter()
            .find(|node| node.alive && node.kind() == NodeKind::Input)
            .map(|node| node.id)
            .expect("input");
        let bounds = runtime.tree.get(id).bounds;
        let point = pointer(bounds.x + 20.0, bounds.y + bounds.height * 0.5);
        runtime.dispatch(Event::PointerMove(point));
        runtime.dispatch(Event::PointerDown(point));
        runtime.dispatch(Event::PointerUp(point));
        assert_eq!(runtime.tree.focused, Some(id));
    }

    #[test]
    fn text_input_reaches_the_focused_input_and_its_action() {
        let mut runtime = Runtime::init(
            Form {
                text: String::new(),
                edits: 0,
            },
            RunOptions::default(),
        );
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        frame(&mut runtime, &mut renderer, 200.0, 120.0);
        click_into_input(&mut runtime);
        assert_eq!(runtime.dispatch(Event::TextInput(String::from("Zant"))), 1);
        assert_eq!(runtime.app_state_mut::<Form>().text, "Zant");
        assert_eq!(runtime.app_state_mut::<Form>().edits, 1);
        // a second event appends and reports through the action again
        assert_eq!(runtime.dispatch(Event::TextInput(String::from(" UI"))), 1);
        assert_eq!(runtime.app_state_mut::<Form>().text, "Zant UI");
        frame(&mut runtime, &mut renderer, 200.0, 120.0);
        let id = runtime.tree.focused.expect("still focused");
        match &runtime.tree.get(id).data {
            NodeData::Input(data) => assert_eq!(data.text, "Zant UI"),
            _ => panic!("expected an input"),
        }
    }

    #[test]
    fn composition_updates_the_input_and_an_empty_text_cancels_it() {
        let mut runtime = Runtime::init(
            Form {
                text: String::new(),
                edits: 0,
            },
            RunOptions::default(),
        );
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        frame(&mut runtime, &mut renderer, 200.0, 120.0);
        click_into_input(&mut runtime);
        let id = runtime.tree.focused.expect("focused input");
        assert_eq!(
            runtime.dispatch(Event::Composition(Composition {
                text: String::from("zhong"),
                selection_start: 0,
                selection_end: 3,
            })),
            1
        );
        match &runtime.tree.get(id).data {
            NodeData::Input(data) => {
                assert!(data.has_composition);
                assert_eq!(data.composition, "zhong");
                assert_eq!(data.display_text, "zhong");
            }
            _ => panic!("expected an input"),
        }
        // an empty composition cancels the marked text
        assert_eq!(
            runtime.dispatch(Event::Composition(Composition {
                text: String::new(),
                selection_start: 0,
                selection_end: 0,
            })),
            1
        );
        match &runtime.tree.get(id).data {
            NodeData::Input(data) => assert!(!data.has_composition),
            _ => panic!("expected an input"),
        }
    }

    #[test]
    fn close_and_quit_requests_are_vetoable_by_the_app() {
        struct Veto {
            closes: u32,
        }
        impl App for Veto {
            fn render(&mut self) -> Node {
                column(BoxOptions::default())
            }
            fn on_close_request(&mut self) -> bool {
                self.closes += 1;
                false
            }
        }
        let mut runtime = Runtime::init(Veto { closes: 0 }, RunOptions::default());
        assert_eq!(
            runtime.dispatch(Event::WindowCloseRequest),
            1,
            "veto redraws"
        );
        assert_eq!(runtime.app_state_mut::<Veto>().closes, 1);
        assert_eq!(runtime.dispatch(Event::AppQuitRequest), 1);
        assert_eq!(
            runtime.app_state_mut::<Veto>().closes,
            2,
            "quit asks on_close_request by default"
        );
    }
}
