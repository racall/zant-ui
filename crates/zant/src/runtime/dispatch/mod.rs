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

#[cfg(test)]
mod tests;

impl Runtime {
    /// Handles `event` and returns the native callback's
    /// "redraw needed" flag. The native host calls it from
    /// `HostHandler::event`; tests inject events directly.
    pub fn dispatch(&mut self, event: Event) -> i32 {
        self.dispatch_depth += 1;
        let prev_owner = crate::ui::set_command_owner(Some(self.owner_ptr()));
        self.sample_clock();
        let outer_local = super::window::take_local_rebuild();
        if self.dispatch_depth == 1 && discrete(&event) && self.needs_build && super::handler_ran()
        {
            self.flush_build();
        }
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

    /// Renders the state changes of the previous event now, before a
    /// discrete event is handled (React flushes discrete updates the same
    /// way): handlers read the state of the render they come from, so a key
    /// press right after typing, within one frame, must see the typed text.
    /// Hover, scroll and press looks alone never get here (no app code ran).
    fn flush_build(&mut self) {
        self.rebuild();
        if self.last_width > 0.0 && self.tree.needs_layout() {
            self.tree.layout(self.viewport);
            self.refresh_key_bounds();
        }
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
                    super::note_handler();
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

/// A discrete user action: each one sees the result of the one before.
fn discrete(event: &Event) -> bool {
    matches!(
        event,
        Event::PointerDown(_)
            | Event::PointerUp(_)
            | Event::KeyDown(_)
            | Event::TextInput(_)
            | Event::Composition(_)
    )
}
