//! Port of `src/ui_runtime.zig:292-382` + `setTheme`: the build stage.
//!
//! [`Runtime::rebuild`] runs the app's `render`, expands portals, performs the
//! `ui::wake_after` / `ui::document_edited` bookkeeping and reconciles the
//! declarative root into the retained tree; then it runs the queued effects.
//! [`Runtime::set_theme`] switches the theme and drops every cached text
//! layout.
//!
//! **Hard rule (plan.md §4):** the build context borrow is released before
//! `App::render` runs — every `ui::*` call borrows it again.
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::core::node::{NodeData, NodeKind};
use crate::theme::tokens::{self, Theme};
use crate::ui::element::Node;

use super::{Runtime, action_cx};

impl Runtime {
    /// Runs one rebuild: `buildTree` plus the `ui::effect` callbacks. An effect
    /// that changed state asks for another build (`needs_build`).
    pub(crate) fn rebuild(&mut self) {
        self.build_tree();
        // `ui::effect` callbacks: after the tree is in place; they may set
        // state, so another build follows
        let requested = {
            let mut cx = action_cx(&mut self.app);
            crate::ui::run_effects(&mut cx)
        };
        if requested {
            self.needs_build = true;
            self.request_redraw();
        }
    }

    /// Zig `buildTree`: theme / window style / build states, then `render` and
    /// [`Runtime::reconcile_root`].
    pub(crate) fn build_tree(&mut self) {
        if let Some(theme) = self.app.theme() {
            self.set_theme(theme);
        }
        crate::ui::set_build_theme(self.theme);
        crate::ui::set_build_window_style(self.config.window.style);
        // memo subtrees build during reconcile, so the store stays set (and
        // sweeps unmounted slots) until the end
        self.local_states.borrow_mut().begin_build();
        crate::ui::set_build_states(Some(Rc::clone(&self.local_states)));
        crate::ui::begin_frame();
        let prev_owner = crate::ui::set_command_owner(Some(self.owner_ptr()));

        let mut root = self.app.render();
        let portals = crate::ui::take_portals();
        if !portals.is_empty() && root.element().kind == NodeKind::Container {
            root = crate::ui::append_children(root, &portals);
        }
        // Windows caption buttons (`ui.windowControls`) are appended here in
        // Zig; the Rust builder arrives with WP-2.5 (`components::window_controls`).
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
        self.needs_build = false;
        crate::ui::set_command_owner(prev_owner);
        crate::ui::set_build_states(None);
        crate::ui::end_frame();
        self.local_states.borrow_mut().sweep();
    }

    /// Zig `reconcileRoot`: the root always fills the window and needs a
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

    /// Zig `setTheme`: switches the active theme and invalidates every cached
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
