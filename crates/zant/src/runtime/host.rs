//! The native `platform::HostHandler` of a [`Runtime`].
//!
//! Zig passed a raw `*Runtime` as the `ZantHost.userdata` and installed plain
//! `callconv(.c)` callbacks (`drawCallback`, `eventCallback`, …,
//! `ui_runtime.zig:2666-2721`). Rust's platform layer forwards the C table to a
//! `Box<dyn HostHandler>`; this type is that handler: it holds the
//! `Rc<RefCell<Runtime>>` the runtime lives in and borrows it for each
//! callback.
//!
//! The borrow is released when the callback returns, so a native modal loop
//! that paints from inside an event callback cannot re-borrow the runtime; such
//! a nested frame is skipped (Zig painted the unchanged tree instead — see the
//! WP-1.4 report).
#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::events;
use crate::platform::platform::{self, HostHandler};

use super::Runtime;

/// Forwards the native host callbacks into the runtime.
pub struct RuntimeHost {
    core: Rc<RefCell<Runtime>>,
}

impl RuntimeHost {
    /// The `Box<dyn HostHandler>` a `Window` takes, wired to `core`.
    pub fn boxed(core: Rc<RefCell<Runtime>>) -> Box<dyn HostHandler> {
        Box::new(RuntimeHost { core })
    }
}

impl HostHandler for RuntimeHost {
    fn draw(&mut self, canvas: crate::renderer::canvas::Canvas, width: f32, height: f32, dpr: f32) {
        // A nested draw (modal native loop) while an event is dispatching: the
        // runtime is already borrowed, so the frame is skipped.
        if let Ok(mut runtime) = self.core.try_borrow_mut() {
            runtime.render_frame(canvas, width, height, dpr);
        }
    }

    fn event(&mut self, event: &platform::Event) -> bool {
        let Ok(mut runtime) = self.core.try_borrow_mut() else {
            return false;
        };
        runtime.dispatch(events::translate(event)) != 0
    }

    fn ime_rect(&mut self) -> Option<(f32, f32, f32, f32)> {
        let mut runtime = self.core.try_borrow_mut().ok()?;
        let rect = runtime.ime_rect()?;
        Some((rect.x, rect.y, rect.width, rect.height))
    }

    fn next_frame_delay(&mut self) -> i32 {
        self.core
            .try_borrow_mut()
            .map_or(-1, |runtime| runtime.next_delay_ms)
    }

    fn titlebar_hit(&mut self, x: f32, y: f32) -> i32 {
        self.core
            .try_borrow_mut()
            .map_or(platform::hit::CAPTION, |mut runtime| {
                runtime.titlebar_hit(x, y)
            })
    }

    fn wake(&mut self) {
        if let Ok(mut runtime) = self.core.try_borrow_mut() {
            runtime.handle_wake();
        }
    }
}
