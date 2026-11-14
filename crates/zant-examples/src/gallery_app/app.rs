//! The imperative gallery application, with its window / snapshot wiring.
//!
//! The gallery does not use the declarative runtime: [`GalleryApp`] owns a
//! retained [`Tree`] built by hand (see [`super::gallery`]), paints it and
//! handles the native events itself. The runtime (`ui::App` /
//! `runtime::Runtime`) builds a *declarative* tree from `render()` on every
//! state change and would free the hand-built nodes, so this app implements
//! [`HostHandler`] (the native callback table) directly.
//!
//! Notes:
//!
//! * The window handle is an `Rc<Window>` the app only holds weakly: the host
//!   handler owns an `Rc<RefCell<GalleryApp>>`, so a strong window reference
//!   here would form a cycle.
//! * `TextAction` has no node-id parameter, so the input's action captures
//!   the id of the input it belongs to when the callback is wired after the
//!   tree was built.
#![forbid(unsafe_code)]

use std::any::Any;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use zant::Error;
use zant::core::events::{self, Composition, Event, Key, KeyEvent, PointerEvent};
use zant::core::node::{NodeData, NodeId};
use zant::core::tree::input_ops::Move;
use zant::platform::platform::{self, Cursor, HostHandler, Window, WindowOptions};
use zant::renderer::canvas::Canvas;
use zant::theme::tokens::{self, Theme};
use zant::ui;

use super::gallery::{GalleryIds, GalleryState};

/// The gallery's retained tree plus the event / frame state.
pub struct GalleryApp {
    pub theme: Theme,
    pub tree: zant::core::tree::Tree,
    gallery: GalleryState,
    ids: GalleryIds,
    pub window: Option<Weak<Window>>,
    pub viewport: zant::core::geometry::Size,
    pub last_width: f32,
    pub last_height: f32,
    pub last_tick_ms: i64,
    pub now_ms: i64,
    pub next_delay_ms: i32,
    pub any_animation: bool,
    pub pressed_button: Option<NodeId>,
    pub drag_input: Option<NodeId>,
    pub hovered: Option<NodeId>,
    pub active: bool,
}

impl GalleryApp {
    /// Theme, retained tree with the gallery, and the callbacks that mutate
    /// it.
    pub fn new() -> GalleryApp {
        let theme = tokens::ant_light();
        let mut tree = zant::core::tree::Tree::init(theme);
        let mut gallery = GalleryState::new(theme);
        let ids = gallery.build(&mut tree);
        let mut app = GalleryApp {
            theme,
            tree,
            gallery,
            ids,
            window: None,
            viewport: zant::core::geometry::Size::default(),
            last_width: 0.0,
            last_height: 0.0,
            last_tick_ms: platform::monotonic_ms(),
            now_ms: 0,
            next_delay_ms: -1,
            any_animation: false,
            pressed_button: None,
            drag_input: None,
            hovered: None,
            active: true,
        };
        app.wire_callbacks();
        app.tree.mark_layout_dirty();
        app
    }

    /// Attaches the increment / input-changed callbacks: the node ids exist
    /// only after the tree was built, so they are wired here.
    fn wire_callbacks(&mut self) {
        let action = ui::action(|app: &mut GalleryApp| app.increment());
        if let NodeData::Button(data) = &mut self.tree.get_mut(self.ids.increment_button).data {
            data.action = Some(action);
        }
        for &input in &self.ids.inputs {
            let action =
                ui::text_action(move |app: &mut GalleryApp, _value| app.input_changed(input));
            if let NodeData::Input(data) = &mut self.tree.get_mut(input).data {
                data.text_action = Some(action);
            }
        }
    }

    /// Click callback of the counter button.
    pub fn increment(&mut self) {
        self.gallery.counter += 1;
        let value = format!("Clicked: {}", self.gallery.counter);
        self.tree.set_text(self.ids.counter_text, &value);
        self.tree.mark_paint_dirty();
    }

    /// Change callback of the inputs.
    pub fn input_changed(&mut self, node_id: NodeId) {
        let value = {
            let NodeData::Input(data) = &self.tree.get(node_id).data else {
                return;
            };
            data.text.clone()
        };
        let display = if value.is_empty() {
            String::from("Value: —")
        } else {
            format!("Value: {value}")
        };
        self.tree.set_text_no_layout(self.ids.value_text, &display);
        self.tree.mark_paint_dirty();
    }

    /// The counter shown by the interactive card (tests).
    pub fn counter(&self) -> u32 {
        self.gallery.counter
    }

    /// One frame into `canvas`.
    pub fn draw(&mut self, canvas: Canvas, width: f32, height: f32, dpr: f32) {
        let _ = dpr;
        self.tick();
        self.viewport = zant::core::geometry::Size { width, height };
        if width != self.last_width || height != self.last_height {
            self.last_width = width;
            self.last_height = height;
            self.tree.mark_layout_dirty();
        }
        if self.tree.needs_layout() {
            self.tree.layout(self.viewport);
        }
        // A draw callback already implies that a frame is needed. GPU backends
        // hand us a fresh drawable each time, so the tree must be replayed even
        // when no new invalidation arrived since the previous frame.
        canvas.clear(self.theme.color_bg_base);
        self.tree.paint(canvas);
        self.tree.clear_paint();
        self.compute_next_delay();
    }

    /// The button hover / press transitions and the caret blink.
    fn tick(&mut self) {
        self.now_ms = platform::monotonic_ms();
        let mut dt: f32 = (self.now_ms - self.last_tick_ms).max(0) as f32;
        dt = dt.min(50.0);
        let _ = dt;
        self.last_tick_ms = self.now_ms;
        self.tree.now_ms = self.now_ms;
        self.any_animation = false;
        let now = platform::monotonic_ms();
        let theme = self.theme;
        let focused = self.tree.focused;
        for index in 0..self.tree.nodes.len() {
            let enabled = self.tree.nodes[index].enabled;
            let active = match &mut self.tree.nodes[index].data {
                NodeData::Button(data) => {
                    if enabled {
                        zant::components::button::animate(data, now, theme)
                    } else {
                        false
                    }
                }
                NodeData::Input(data) => {
                    if focused == Some(index as NodeId) && enabled {
                        zant::components::input::update_blink(data, enabled, self.now_ms)
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if active {
                self.any_animation = true;
                self.tree.mark_dirty(index as NodeId, false, true);
            }
        }
    }

    /// When the next frame is needed (animation, caret blink or none).
    fn compute_next_delay(&mut self) {
        let mut delay: i32 = -1;
        if self.any_animation {
            delay = 16;
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
        self.next_delay_ms = delay;
    }

    /// Handles a translated native event; 1 = consumed.
    pub fn handle(&mut self, event: Event) -> i32 {
        match event {
            Event::PointerMove(p) | Event::PointerEnter(p) => {
                self.handle_pointer_move(p);
                1
            }
            Event::PointerDown(p) => {
                self.handle_pointer_down(p);
                1
            }
            Event::PointerUp(p) => {
                self.handle_pointer_up(p);
                1
            }
            Event::PointerLeave(_) => {
                self.set_hover(None);
                self.drag_input = None;
                if let Some(id) = self.pressed_button {
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        data.pressed = false;
                    }
                    self.pressed_button = None;
                    self.tree.mark_paint_dirty();
                }
                self.set_cursor(Cursor::Arrow);
                1
            }
            Event::KeyDown(k) => self.handle_key_down(k),
            Event::KeyUp(_) => 0,
            Event::TextInput(value) => self.handle_text_input(value),
            Event::Composition(composition) => self.handle_composition(composition),
            Event::WindowBlur => {
                self.active = false;
                self.cancel_focused_composition();
                self.tree.set_focus(None);
                self.set_hover(None);
                if let Some(id) = self.pressed_button {
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        data.pressed = false;
                    }
                    self.pressed_button = None;
                }
                self.tree.mark_paint_dirty();
                0
            }
            Event::WindowFocus => {
                self.active = true;
                self.tree.mark_paint_dirty();
                0
            }
            Event::WindowResize(_) => {
                self.tree.mark_layout_dirty();
                0
            }
            Event::WindowClose
            | Event::WindowCloseRequest
            | Event::AppQuitRequest
            | Event::AppearanceChange => 0,
            Event::Scroll(_) => 0,
        }
    }

    fn handle_pointer_move(&mut self, p: PointerEvent) {
        let target = self.tree.hit_test(p.x, p.y);
        self.set_hover(target);
        if let Some(id) = self.pressed_button {
            let (inside, pressed) = {
                let node = self.tree.get(id);
                let pressed = match &node.data {
                    NodeData::Button(data) => data.pressed,
                    _ => false,
                };
                (node.bounds.contains(p.x, p.y), pressed)
            };
            if pressed != inside
                && let NodeData::Button(data) = &mut self.tree.get_mut(id).data
            {
                data.pressed = inside;
                self.tree.mark_dirty(id, false, true);
            }
        }
        if let Some(id) = self.drag_input {
            self.set_input_cursor(id, p.x, p.y, true);
        }
        self.update_cursor(target);
    }

    fn set_hover(&mut self, target: Option<NodeId>) {
        if self.hovered == target {
            return;
        }
        if let Some(previous) = self.hovered {
            if let NodeData::Button(data) = &mut self.tree.get_mut(previous).data {
                data.hovered = false;
            } else if let NodeData::Input(data) = &mut self.tree.get_mut(previous).data {
                data.hovered = false;
            }
            self.tree.mark_dirty(previous, false, true);
        }
        self.hovered = target;
        if let Some(id) = target {
            let enabled = self.tree.get(id).enabled;
            if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                data.hovered = enabled;
            } else if let NodeData::Input(data) = &mut self.tree.get_mut(id).data {
                data.hovered = enabled;
            }
            self.tree.mark_dirty(id, false, true);
        }
        self.tree.mark_paint_dirty();
    }

    /// The pointer cursor for the node under the pointer.
    fn update_cursor(&mut self, target: Option<NodeId>) {
        let cursor = match target {
            Some(id) => match &self.tree.get(id).data {
                NodeData::Input(_) => Cursor::Ibeam,
                NodeData::Button(_) => Cursor::PointingHand,
                _ => Cursor::Arrow,
            },
            None => Cursor::Arrow,
        };
        self.set_cursor(cursor);
    }

    fn set_cursor(&self, cursor: Cursor) {
        if let Some(window) = self.window.as_ref().and_then(Weak::upgrade) {
            window.set_cursor(cursor);
        }
    }

    fn handle_pointer_down(&mut self, p: PointerEvent) {
        let target = self.tree.hit_test(p.x, p.y);
        self.set_hover(target);
        if let Some(id) = target {
            let enabled = self.tree.get(id).enabled;
            match &self.tree.get(id).data {
                NodeData::Button(_) => {
                    self.focus_node(Some(id));
                    if !enabled {
                        return;
                    }
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        data.pressed = true;
                        data.hovered = true;
                    }
                    self.pressed_button = Some(id);
                    self.tree.mark_dirty(id, false, true);
                }
                NodeData::Input(_) => {
                    if !enabled {
                        return;
                    }
                    self.focus_node(Some(id));
                    self.drag_input = Some(id);
                    self.set_input_cursor(id, p.x, p.y, p.modifiers.shift);
                }
                _ => {}
            }
            self.update_cursor(target);
        } else {
            self.focus_node(None);
            self.tree.mark_paint_dirty();
        }
    }

    fn handle_pointer_up(&mut self, p: PointerEvent) {
        self.drag_input = None;
        if let Some(id) = self.pressed_button {
            let (inside, enabled) = {
                let node = self.tree.get(id);
                (node.bounds.contains(p.x, p.y), node.enabled)
            };
            let action = if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                data.pressed = false;
                data.action.clone()
            } else {
                None
            };
            if inside
                && enabled
                && let Some(action) = action
            {
                let mut cx = ui::ActionCx::new(self as &mut dyn Any);
                action.call(&mut cx);
            }
            self.pressed_button = None;
            self.tree.mark_dirty(id, false, true);
        }
        self.update_cursor(self.tree.hit_test(p.x, p.y));
    }

    fn focus_node(&mut self, id: Option<NodeId>) {
        if let Some(old) = self.tree.focused
            && Some(old) != id
        {
            self.cancel_composition_for(old);
        }
        self.tree.set_focus(id);
        self.tree.mark_paint_dirty();
    }

    fn cancel_composition_for(&mut self, id: NodeId) {
        let has_composition =
            matches!(&self.tree.get(id).data, NodeData::Input(data) if data.has_composition);
        if has_composition {
            self.tree.input_cancel_composition(id);
        }
    }

    fn cancel_focused_composition(&mut self) {
        if let Some(id) = self.tree.focused {
            self.cancel_composition_for(id);
        }
    }

    /// Places the caret under the pointer.
    fn set_input_cursor(&mut self, id: NodeId, x: f32, y: f32, extend: bool) {
        let (bounds, scroll_x) = {
            let node = self.tree.get(id);
            let scroll_x = match &node.data {
                NodeData::Input(data) => data.scroll_x,
                _ => return,
            };
            (node.bounds, scroll_x)
        };
        let byte = {
            let node = self.tree.get_mut(id);
            let NodeData::Input(data) = &mut node.data else {
                return;
            };
            if data.has_composition {
                return;
            }
            if !zant::components::input::ensure_layout(data) {
                return;
            }
            let Some(layout) = data.layout.as_ref() else {
                return;
            };
            let origin_x = bounds.x + zant::components::input::PADDING_X - scroll_x;
            let origin_y = bounds.y + (bounds.height - layout.metrics().height) * 0.5;
            let byte = layout.index_at(x - origin_x, y - origin_y);
            byte.min(data.text.len())
        };
        if let NodeData::Input(data) = &mut self.tree.get_mut(id).data {
            data.cursor = byte;
            if !extend {
                data.anchor = byte;
            }
        }
        self.tree.mark_dirty(id, false, true);
    }

    fn handle_key_down(&mut self, k: KeyEvent) -> i32 {
        let shift = k.modifiers.shift;
        match k.key {
            Key::Tab => {
                self.cancel_focused_composition();
                self.tree.tab_focus(shift);
                return 1;
            }
            Key::Escape => {
                if let Some(id) = self.tree.focused
                    && matches!(&self.tree.get(id).data, NodeData::Input(data) if data.has_composition)
                {
                    self.tree.input_cancel_composition(id);
                    return 1;
                }
                self.focus_node(None);
                return 1;
            }
            _ => {}
        }

        if let Some(id) = self.tree.focused {
            let is_button = matches!(self.tree.get(id).data, NodeData::Button(_));
            let is_input = matches!(self.tree.get(id).data, NodeData::Input(_));
            if is_button {
                let data = match &self.tree.get(id).data {
                    NodeData::Button(data) => data,
                    _ => unreachable!(),
                };
                if k.key == Key::Enter || events::character_equals(k.key, b' ') {
                    if let Some(action) = data.action.clone() {
                        let mut cx = ui::ActionCx::new(self as &mut dyn Any);
                        action.call(&mut cx);
                    }
                    return 1;
                }
            } else if is_input {
                let meta_or_control = k.modifiers.meta || k.modifiers.control;
                let has_composition = matches!(&self.tree.get(id).data, NodeData::Input(data) if data.has_composition);
                if has_composition {
                    return 0;
                }
                if meta_or_control {
                    if events::character_equals(k.key, b'a') {
                        self.tree.input_select_all(id);
                        return 1;
                    }
                    if events::character_equals(k.key, b'c') {
                        let _ = self.copy_selection(id);
                        return 1;
                    }
                    if events::character_equals(k.key, b'x') {
                        if self.copy_selection(id) && self.tree.input_delete_backward(id) {
                            self.notify_text_action(id);
                        }
                        return 1;
                    }
                    if events::character_equals(k.key, b'v') {
                        self.paste_into(id);
                        return 1;
                    }
                    if events::character_equals(k.key, b'a')
                        || events::character_equals(k.key, b'A')
                    {
                        self.tree.input_select_all(id);
                        return 1;
                    }
                    return 0;
                }
                match k.key {
                    Key::Left => {
                        if k.modifiers.alt {
                            self.tree.input_move(id, Move::WordLeft, shift);
                        } else {
                            self.tree.input_move(id, Move::Left, shift);
                        }
                        return 1;
                    }
                    Key::Right => {
                        if k.modifiers.alt {
                            self.tree.input_move(id, Move::WordRight, shift);
                        } else {
                            self.tree.input_move(id, Move::Right, shift);
                        }
                        return 1;
                    }
                    Key::Home => {
                        self.tree.input_move(id, Move::Home, shift);
                        return 1;
                    }
                    Key::End => {
                        self.tree.input_move(id, Move::End, shift);
                        return 1;
                    }
                    Key::Backspace => {
                        if self.tree.input_delete_backward(id) {
                            self.notify_text_action(id);
                        }
                        return 1;
                    }
                    Key::Delete => {
                        if self.tree.input_delete_forward(id) {
                            self.notify_text_action(id);
                        }
                        return 1;
                    }
                    Key::Enter => {
                        let submit = match &self.tree.get(id).data {
                            NodeData::Input(data) => data.submit_action.clone(),
                            _ => None,
                        };
                        if let Some(action) = submit {
                            let mut cx = ui::ActionCx::new(self as &mut dyn Any);
                            action.call(&mut cx);
                        }
                        return 1;
                    }
                    _ => return 0,
                }
            }
        }
        0
    }

    fn handle_text_input(&mut self, value: String) -> i32 {
        if let Some(id) = self.tree.focused
            && matches!(self.tree.get(id).data, NodeData::Input(_))
            && self.tree.input_insert(id, &value)
        {
            self.notify_text_action(id);
            return 1;
        }
        0
    }

    fn handle_composition(&mut self, composition: Composition) -> i32 {
        if let Some(id) = self.tree.focused
            && matches!(self.tree.get(id).data, NodeData::Input(_))
        {
            if composition.text.is_empty() {
                self.tree.input_cancel_composition(id);
            } else {
                self.tree.input_composition(
                    id,
                    &composition.text,
                    composition.selection_start,
                    composition.selection_end,
                );
            }
            return 1;
        }
        0
    }

    /// Runs the input's `text_action` after a successful edit. The tree only
    /// changes state; normally the runtime notifies the action (see
    /// `runtime/keyboard.rs:notify_text_action`), but the gallery drives the
    /// tree itself, so it notifies here.
    fn notify_text_action(&mut self, id: NodeId) {
        let action = match &self.tree.get(id).data {
            NodeData::Input(data) => data.text_action.clone(),
            _ => None,
        };
        let Some(action) = action else {
            return;
        };
        let text = match &self.tree.get(id).data {
            NodeData::Input(data) => data.text.clone(),
            _ => return,
        };
        let mut cx = ui::ActionCx::new(self as &mut dyn Any);
        action.call(&mut cx, &text);
    }

    /// Copies the input's selection to the clipboard.
    fn copy_selection(&mut self, id: NodeId) -> bool {
        let text = {
            let node = self.tree.get(id);
            let NodeData::Input(data) = &node.data else {
                return false;
            };
            if !zant::components::input::has_selection(data) {
                return false;
            }
            let (start, end) = zant::components::input::selection_range(data);
            if start == end {
                return false;
            }
            data.text[start..end].to_string()
        };
        platform::set_clipboard(&text)
    }

    /// Pastes the clipboard text into the input.
    fn paste_into(&mut self, id: NodeId) {
        let Some(clipboard) = platform::clipboard_text() else {
            return;
        };
        if clipboard.is_empty() {
            return;
        }
        if self.tree.input_insert(id, &clipboard) {
            self.notify_text_action(id);
        }
    }

    /// The caret rectangle of the focused input, for the input method.
    pub fn ime_rect(&mut self) -> Option<zant::core::geometry::Rect> {
        let id = self.tree.focused?;
        if !matches!(self.tree.get(id).data, NodeData::Input(_)) {
            return None;
        }
        self.tree.input_caret_rect(id)
    }

    pub fn next_frame_delay(&self) -> i32 {
        self.next_delay_ms
    }
}

impl Default for GalleryApp {
    fn default() -> Self {
        GalleryApp::new()
    }
}

/// The native host callbacks of the gallery.
pub struct GalleryHost {
    app: Rc<RefCell<GalleryApp>>,
}

impl GalleryHost {
    /// The `Box<dyn HostHandler>` a [`Window`] takes, wired to `app`.
    pub fn boxed(app: Rc<RefCell<GalleryApp>>) -> Box<dyn HostHandler> {
        Box::new(GalleryHost { app })
    }
}

impl HostHandler for GalleryHost {
    fn draw(&mut self, canvas: Canvas, width: f32, height: f32, dpr: f32) {
        if let Ok(mut app) = self.app.try_borrow_mut() {
            app.draw(canvas, width, height, dpr);
        }
    }

    fn event(&mut self, event: &platform::Event) -> bool {
        let event = events::translate(event);
        self.app
            .try_borrow_mut()
            .is_ok_and(|mut app| app.handle(event) != 0)
    }

    fn ime_rect(&mut self) -> Option<(f32, f32, f32, f32)> {
        let mut app = self.app.try_borrow_mut().ok()?;
        let rect = app.ime_rect()?;
        Some((rect.x, rect.y, rect.width, rect.height))
    }

    fn next_frame_delay(&mut self) -> i32 {
        self.app
            .try_borrow_mut()
            .map_or(-1, |app| app.next_frame_delay())
    }
}

/// The `zant-gallery` entry point.
pub fn run() -> Result<(), Error> {
    let app = Rc::new(RefCell::new(GalleryApp::new()));
    let window = Rc::new(
        Window::create(
            "Zant UI — Component Gallery",
            WindowOptions {
                width: 900.0,
                height: 640.0,
                ..Default::default()
            },
            GalleryHost::boxed(Rc::clone(&app)),
        )
        .map_err(|_| Error::Message(String::from("window creation failed")))?,
    );
    app.borrow_mut().window = Some(Rc::downgrade(&window));
    eprintln!("Zant UI running — renderer: {}", window.renderer_name());
    if let Some(path) = platform::getenv("ZANT_SNAPSHOT") {
        window.schedule_snapshot(&path, 600);
    }
    window.run();
    app.borrow_mut().window = None;
    Ok(())
}
