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

mod keyboard;
mod pointer;

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
