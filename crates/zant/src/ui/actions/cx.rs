//! The context callbacks run in (`ActionCx`) and what an `on_resize`
//! report asks the runtime to rebuild.

use std::any::Any;

/// The runtime context a callback runs in: the root application state plus,
/// later, whatever the dispatcher needs.
///
/// Tests construct it from a `&mut dyn Any` root. `pub(crate)` fields
/// (window, dispatcher, …) and extra constructors may be added without
/// changing `new` / `state` / `Action::call`.
pub struct ActionCx<'a> {
    root: &'a mut dyn Any,
    rebuild: Rebuild,
}

/// When an `on_resize` report rebuilds the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rebuild {
    /// In the same frame, before it is painted (the default).
    Now,
    /// In the next frame (`ActionCx::defer_rebuild`).
    NextFrame,
    /// Not at all (`ActionCx::skip_rebuild`).
    Skip,
}

impl<'a> ActionCx<'a> {
    /// Builds the context around the current window's root state.
    pub fn new(root: &'a mut dyn Any) -> ActionCx<'a> {
        ActionCx {
            root,
            rebuild: Rebuild::Now,
        }
    }

    /// For an `on_resize` callback: it changed nothing the UI tree is built
    /// from (it only recorded the new size, or the layout already places
    /// everything right), so the runtime does not rebuild the app for this
    /// report. Without it every size report rebuilds and lays out again in
    /// the same frame. Other callbacks ignore it.
    pub fn skip_rebuild(&mut self) {
        if self.rebuild == Rebuild::Now {
            self.rebuild = Rebuild::Skip;
        }
    }

    /// For an `on_resize` callback: what it changed can wait one frame (a
    /// scroll bar thumb, a counter), so this frame is painted without
    /// rebuilding and the next frame rebuilds. Other callbacks ignore it.
    pub fn defer_rebuild(&mut self) {
        self.rebuild = Rebuild::NextFrame;
    }

    /// What the callback asked for (runtime).
    pub(crate) fn rebuild(&self) -> Rebuild {
        self.rebuild
    }

    /// The root application state; panics when the callback expects another
    /// type (the state type is only known at run time, through `dyn Any`).
    pub fn state<S: 'static>(&mut self) -> &mut S {
        match self.root.downcast_mut::<S>() {
            Some(state) => state,
            None => panic!(
                "Zant UI: action expects the root state to be `{}`, got another type",
                std::any::type_name::<S>()
            ),
        }
    }
}
