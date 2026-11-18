//! The build stage.
//!
//! [`Runtime::rebuild`] runs the app's `render`, expands portals, performs the
//! `ui::wake_after` / `ui::document_edited` bookkeeping and reconciles the
//! declarative root into the retained tree; then it runs the queued effects.
//! [`Runtime::set_theme`] switches the theme and drops every cached text
//! layout.
//!
//! **Hard rule:** the build context borrow is released before
//! `App::render` runs — every `ui::*` call borrows it again.
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::core::node::{NodeData, NodeKind};
use crate::theme::tokens::{self, Theme};
use crate::ui::element::Node;

use super::{Runtime, action_cx};

#[cfg(test)]
mod tests;

impl Runtime {
    /// Runs one rebuild: `build_tree` plus the `ui::effect` callbacks. An effect
    /// that changed state asks for another build (`needs_build`).
    pub(crate) fn rebuild(&mut self) {
        // the app state is held by a hook running a modal loop: later
        if self.app.try_borrow_mut().is_none() {
            self.needs_build = true;
            if let Some(window) = &self.window {
                window.request_redraw(100);
            }
            return;
        }
        // the render sees every change so far; effects below may note more
        super::clear_handler_ran();
        self.build_tree();
        // `ui::effect` callbacks: after the tree is in place; they may set
        // state, so another build follows
        let requested = {
            let mut app = self.app.borrow_mut();
            let mut cx = action_cx(&mut app);
            crate::ui::run_effects(&mut cx)
        };
        if requested {
            // an effect changed state: rendered before the next discrete event
            super::note_handler();
            self.needs_build = true;
            self.request_redraw();
        }
    }

    /// A hidden-titlebar window on Windows draws
    /// its own caption buttons (macOS keeps the traffic lights).
    fn draws_caption_buttons(&self) -> bool {
        cfg!(target_os = "windows")
            && self.config.window.style == crate::platform::platform::WindowStyle::HiddenTitlebar
    }

    /// Height of the app-drawn title bar (Windows 11 default: 32).
    fn caption_height(&self) -> f32 {
        let height = self.config.window.titlebar_height;
        if height > 0.0 { height } else { 32.0 }
    }

    /// Theme / window style / build states, then `render` and
    /// [`Runtime::reconcile_root`].
    pub(crate) fn build_tree(&mut self) {
        let _lists = super::view_state::ListBuildGuard::enter(Rc::clone(&self.view_lists));
        let theme = self.app.borrow().theme();
        if let Some(theme) = theme {
            self.set_theme(theme);
        }
        crate::ui::set_build_theme(self.theme);
        crate::ui::set_build_window_style(self.config.window.style);
        // memo subtrees build during reconcile, so the store stays set (and
        // sweeps unmounted slots) until the end
        self.local_states.borrow_mut().begin_build();
        crate::ui::set_build_states(Some(Rc::clone(&self.local_states)));
        crate::ui::begin_frame();
        // `ui::key_bounds` answers from this window's snapshot while it builds
        // (with several windows the hook has to follow the window being
        // built).
        let bounds = Rc::clone(&self.key_bounds);
        crate::ui::set_key_bounds_hook(Some(Rc::new(move |key: &str| {
            bounds.borrow().get(key).copied()
        })));
        let prev_owner = crate::ui::set_command_owner(Some(self.owner_ptr()));

        let mut root = match &self.window_hooks {
            Some(hooks) => (hooks.render)(),
            None => self.app.borrow_mut().render(),
        };
        let portals = crate::ui::take_portals();
        if !portals.is_empty() && root.element().kind == NodeKind::Container {
            root = crate::ui::append_children(root, &portals);
        }
        if let Some(text) = &self.hot_notice
            && root.element().kind == NodeKind::Container
        {
            root = crate::ui::append_children(root, &[hot_notice(text)]);
        }
        // App-drawn Windows caption buttons: a
        // hidden-titlebar window on Windows gets the minimize / maximize /
        // close buttons in the top-right corner of its own title bar.
        if self.draws_caption_buttons() && root.element().kind == NodeKind::Container {
            let options = crate::ui::WindowControlsOptions {
                height: self.caption_height(),
                maximized: self
                    .window
                    .as_ref()
                    .is_some_and(|window| window.is_maximized()),
                active: self.active,
                on_minimize: Some(crate::ui::Action::new(|_cx| {
                    crate::ui::window_minimize();
                })),
                on_maximize: Some(crate::ui::Action::new(|_cx| {
                    crate::ui::window_maximize();
                })),
                on_close: Some(crate::ui::Action::new(|_cx| {
                    crate::ui::window_close();
                })),
            };
            root = crate::ui::append_children(root, &[crate::ui::window_controls(options)]);
        }
        self.wake_at_ms = crate::ui::take_wake().map(|ms| self.now_ms + i64::from(ms));
        if let Some(edited) = crate::ui::take_document_edited()
            && edited != self.document_edited
        {
            self.document_edited = edited;
            if let Some(window) = &self.window {
                window.set_document_edited(edited);
            }
        }
        if root.element().kind == NodeKind::Container {
            self.reconcile_root(root);
        } else {
            eprintln!("Zant UI: render() must return a container (ui.column/ui.row/ui.card)");
        }
        self.view_lists
            .borrow_mut()
            .retain(|key, state| state.strong_count() > 0 && self.tree.id_of_key(key).is_some());
        self.needs_build = false;
        // Keep the hook installed for the rest of the frame: layout followers
        // (`notify_resizes`, app `on_resize` callbacks) still ask `ui::key_bounds`
        // for the boxes just laid out. The snapshot is refreshed after every
        // layout and cleared when the runtime is dropped.
        crate::ui::set_command_owner(prev_owner);
        crate::ui::set_build_states(None);
        crate::ui::end_frame();
        self.local_states.borrow_mut().sweep();
    }

    /// The root always fills the window and needs a
    /// background, so the spec is normalised before it is compared.
    pub(crate) fn reconcile_root(&mut self, root_spec: Node) -> bool {
        let root_id = self.tree.root;
        let spec = root_spec.element();
        let mut want = spec.layout;
        want.width = None;
        want.height = None;
        want.grow = 0.0;
        let mut want_visual = spec.visual;
        if want_visual.background.is_none() {
            want_visual.background = Some(self.theme.color_bg_base);
        }
        let mut changed_layout = {
            let root = self.tree.get(root_id);
            root.style != want || root.visual != want_visual
        };
        {
            let root = self.tree.get_mut(root_id);
            root.style = want;
            root.visual = want_visual;
            root.enabled = true;
            root.focusable = false;
        }
        // keep the window's own background (visible while resizing) and the
        // frame clear colour in sync with the app's root background
        if let Some(background) = want_visual.background
            && background != self.clear_color
            && (background >> 24) == 0xff
        {
            self.clear_color = background;
            if let Some(window) = &self.window {
                window.set_background(background);
            }
        }
        if self.reconcile_children(root_id, &spec.children) {
            changed_layout = true;
        }
        if changed_layout {
            self.tree.mark_dirty(root_id, true, true);
        }
        changed_layout
    }

    /// Switches the active theme and invalidates every cached
    /// text layout (the colours are baked into the shaped paragraphs).
    pub(crate) fn set_theme(&mut self, theme: Theme) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        self.tree.theme = theme;
        self.tree.selection_color = tokens::with_alpha(theme.color_primary, 0x4d);
        for node in &mut self.tree.nodes {
            match &mut node.data {
                NodeData::Text(data) => data.invalidate(),
                NodeData::Button(data) => data.invalidate_layout(),
                NodeData::Input(data) => {
                    data.invalidate_layout();
                    data.placeholder_layout = None;
                }
                NodeData::Toggle(data) => data.invalidate_layout(),
                NodeData::Select(data) => {
                    data.layout = None;
                    data.option_layouts.clear();
                    data.layout_dirty = true;
                }
                NodeData::RichText(data) => data.invalidate_layout(),
                _ => {}
            }
        }
        self.tree.mark_layout_dirty();
        self.tree.mark_paint_dirty();
    }
}

/// The hot-reload message panel: the build error of the version that could
/// not be loaded, over the bottom of the still-running UI.
fn hot_notice(text: &str) -> crate::ui::Node {
    use crate::ui::{BoxOptions, Style, TextOptions};
    let body = crate::ui::text(
        crate::Str::copy(text),
        TextOptions {
            size: 12.0,
            color: Some(0xffff_ffff),
            wrap: true,
            line_height: 1.5,
            family: crate::renderer::text::Family::Mono,
            ..Default::default()
        },
    );
    crate::ui::styled(
        crate::ui::column(BoxOptions {
            key: Some(crate::Str::Static("zant-hot-notice")),
            children: vec![body],
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(12.0),
            right: Some(12.0),
            bottom: Some(12.0),
            max_height: Some(320.0),
            padding: Some(crate::core::geometry::Edges::all(12.0)),
            background: Some(0xf2a8_071a),
            radius: Some(8.0),
            clip: Some(true),
            block_pointer: Some(true),
            shadow_color: Some(0x4000_0000),
            shadow_blur: Some(16.0),
            shadow_offset_y: Some(4.0),
            ..Default::default()
        },
    )
}
