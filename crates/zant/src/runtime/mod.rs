//! Port of `src/ui_runtime.zig` — WP-1.4: the application runtime.
//!
//! [`Runtime`] owns the retained tree, the application state, the frame clock
//! and every pointer / keyboard / IME / focus / selection / overlay state of one
//! window. It builds the declarative tree (`build`), reconciles it into the
//! retained tree (`reconcile`), patches changed nodes (`patch`), dispatches
//! native events (`dispatch`, `pointer`, `keyboard`, `ime`, `focus`,
//! `selection`) and paints a frame (`frame`, `overlay`, `animate`) with the
//! `ZANT_*` debug switches of `debug`.
//!
//! The native window drives the runtime through [`host::RuntimeHost`], which
//! implements `platform::HostHandler`: `run()` (in `ui::app`) puts the runtime
//! in an `Rc<RefCell<…>>`, hands a host that borrows it to the window and calls
//! the native event loop. Headless tests build a [`Runtime`] directly, call
//! [`Runtime::rebuild_for_test`] / [`Runtime::render_frame`] on a raster canvas
//! and inject a fake clock through [`Runtime::clock`].
//!
//! Deviations from Zig (see the WP-1.4 report in docs/rust-port/status.md):
//! one window per runtime — Zig's `open_runtimes` list, `openWindow` /
//! `closeWindow` and `invalidateAll` belong to WP-2.8; Windows caption buttons
//! (`ui.windowControls`) to WP-2.5; the select popup paint / hit test and the
//! input / toggle / select component animation to WP-2.1 / WP-2.2.
#![forbid(unsafe_code)]

pub mod animate;
pub mod build;
pub mod debug;
pub mod dispatch;
pub mod focus;
pub mod frame;
pub mod host;
pub mod ime;
pub mod keyboard;
pub mod overlay;
pub mod patch;
pub mod pointer;
pub mod reconcile;
pub mod selection;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::geometry::{Rect, Size};
use crate::core::local_state::Store;
use crate::core::node::NodeId;
use crate::core::text_select;
use crate::core::tree::Tree;
use crate::motion::motion::Animated;
use crate::platform::platform::{self, Window};
use crate::renderer::text::TextLayout;
use crate::theme::tokens::{self, Theme};
use crate::ui::actions::ActionCx;
use crate::ui::app::{App, RunOptions};

use self::debug::FrameStats;

/// Zig `Runtime.Config`; the same struct the application entry point takes.
pub type Config = RunOptions;

/// How a multi-click pointer press extends the selection (Zig's anonymous
/// `enum { char, word, block }`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelMode {
    #[default]
    Char,
    Word,
    Block,
}

/// The clock sampled once per frame / event (Zig `monotonicMs`); tests inject
/// a fake one.
pub type Clock = fn() -> i64;

/// One window's runtime: tree, application state, frame loop and input state.
///
/// Field names follow the Zig `Runtime` (the struct is split across the
/// submodules of this directory but keeps the same state).
pub struct Runtime {
    pub config: RunOptions,
    pub app: Box<dyn App>,
    pub theme: Theme,
    /// The retained tree (`runtime.tree` in Zig).
    pub tree: Tree,
    /// `ui::local_state` slots of this window's components (Zig
    /// `local_states`).
    pub local_states: Rc<RefCell<Store>>,
    pub needs_build: bool,
    /// Greater than 0 while an event is being dispatched. Callbacks can run a
    /// native modal loop (file dialogs, confirm alerts) that paints the window
    /// meanwhile; such nested frames must not rebuild the tree or finish exit
    /// animations, because the dispatching code still holds node ids.
    pub dispatch_depth: u32,
    /// `ZANT_LAYOUT_FILE`: after every frame, write "key x y width height"
    /// (logical px, tab separated) of each keyed in-flow node, for automation.
    pub layout_file: Option<String>,
    /// `ZANT_FRAME_STATS=1`: once a second print frames drawn, the slowest
    /// frame and the longest gap between frames.
    pub frame_stats: Option<FrameStats>,
    pub viewport: Size,
    pub last_width: f32,
    pub last_height: f32,
    /// Time source (ms). Tests inject a fake clock.
    pub clock: Clock,
    /// Timestamp of the current frame / event, sampled from `clock`.
    pub now_ms: i64,
    pub next_delay_ms: i32,
    pub any_animation: bool,
    pub hovered: Option<NodeId>,
    pub pressed_button: Option<NodeId>,
    pub pressed_toggle: Option<NodeId>,
    pub drag_input: Option<NodeId>,
    pub active: bool,
    pub tooltip_target: Option<NodeId>,
    pub tooltip_show_at: i64,
    pub tooltip_visible: bool,
    pub tooltip_layout: Option<TextLayout>,
    /// Show / hide transition (antd zoom-big-fast) and the box computed when
    /// the tooltip appeared (kept while it zooms out).
    pub tooltip_anim: Animated,
    pub tooltip_rect: Rect,
    pub tooltip_above: bool,
    pub tooltip_hiding: bool,
    /// Select whose popup is painted (stays set while the popup closes).
    pub popup_select: Option<NodeId>,
    /// Rebuild requested by `ui::wake_after` (absolute ms).
    pub wake_at_ms: Option<i64>,
    /// Last `ui::document_edited` value sent to the window.
    pub document_edited: bool,
    pub open_select: Option<NodeId>,
    pub pressed_link: Option<NodeId>,
    /// Pointer area holding the pointer between `down` and `up`.
    pub pointer_capture: Option<NodeId>,
    /// Container whose hover look is shown (`BoxOptions::hover_*`).
    pub hover_box: Option<NodeId>,
    /// Pointer area last sent `hover` (gets `leave` when the pointer goes).
    pub pointer_hover_area: Option<NodeId>,
    /// Multi-click detection (double / triple click).
    pub last_click_ms: i64,
    pub last_click_x: f32,
    pub last_click_y: f32,
    pub click_count: u8,
    /// Text selection in a `selectable` container.
    pub sel_root: Option<NodeId>,
    pub sel_anchor: Option<text_select::Point>,
    pub sel_focus: Option<text_select::Point>,
    pub sel_dragging: bool,
    pub sel_mode: SelMode,
    /// Word / block under the pointer when a multi-click drag started.
    pub sel_unit: [text_select::Point; 2],
    /// Nodes whose `text_sel` is set (cleared on the next change).
    pub sel_marked: Vec<NodeId>,
    /// Window clear colour; follows the root background (dark mode).
    pub clear_color: u32,
    /// The native window while `ui::app::run` drives the loop (`None` in
    /// headless tests and before the native window exists).
    pub window: Option<Rc<Window>>,
    /// Previous-frame boxes of keyed nodes (the `ui::key_bounds` hook; Zig
    /// pointed at the live tree — a snapshot avoids re-borrowing the tree from
    /// user code).
    pub key_bounds: Rc<RefCell<HashMap<String, Rect>>>,
}

impl Runtime {
    /// Creates a runtime for `app` (`Runtime.init` in Zig). No native window is
    /// created; `ui::app::run` attaches one.
    pub fn init(app: impl App, config: RunOptions) -> Runtime {
        let theme = config.theme;
        let window_background = config.window.background;
        let mut tree = Tree::init(theme);
        {
            let root = tree.root;
            let root = tree.get_mut(root);
            root.style.direction = crate::layout::flex::Direction::Column;
            root.style.align_items = crate::layout::flex::Align::Stretch;
            root.visual.background = Some(window_background);
        }
        tree.selection_color = tokens::with_alpha(theme.color_primary, 0x4d);
        let key_bounds: Rc<RefCell<HashMap<String, Rect>>> = Rc::new(RefCell::new(HashMap::new()));
        let hook = Rc::clone(&key_bounds);
        crate::ui::set_key_bounds_hook(Some(Rc::new(move |key: &str| {
            hook.borrow().get(key).copied()
        })));
        Runtime {
            config,
            app: Box::new(app),
            theme,
            tree,
            local_states: Rc::new(RefCell::new(Store::new())),
            needs_build: true,
            dispatch_depth: 0,
            layout_file: None,
            frame_stats: None,
            viewport: Size::default(),
            last_width: 0.0,
            last_height: 0.0,
            clock: platform::monotonic_ms,
            now_ms: 0,
            next_delay_ms: -1,
            any_animation: false,
            hovered: None,
            pressed_button: None,
            pressed_toggle: None,
            drag_input: None,
            active: true,
            tooltip_target: None,
            tooltip_show_at: 0,
            tooltip_visible: false,
            tooltip_layout: None,
            tooltip_anim: Animated::default(),
            tooltip_rect: Rect::default(),
            tooltip_above: false,
            tooltip_hiding: false,
            popup_select: None,
            wake_at_ms: None,
            document_edited: false,
            open_select: None,
            pressed_link: None,
            pointer_capture: None,
            hover_box: None,
            pointer_hover_area: None,
            last_click_ms: 0,
            last_click_x: 0.0,
            last_click_y: 0.0,
            click_count: 0,
            sel_root: None,
            sel_anchor: None,
            sel_focus: None,
            sel_dragging: false,
            sel_mode: SelMode::Char,
            sel_unit: [
                text_select::Point { node: 0, byte: 0 },
                text_select::Point { node: 0, byte: 0 },
            ],
            sel_marked: Vec::new(),
            clear_color: window_background,
            window: None,
            key_bounds,
        }
    }

    /// Zig `Runtime.deinit`: Rust frees on `Drop`; also uninstalls this
    /// runtime's `ui::key_bounds` hook.
    pub fn deinit(&mut self) {}

    /// Rebuilds the UI on the next frame (`Runtime.invalidate`).
    pub fn invalidate(&mut self) {
        self.needs_build = true;
        self.request_redraw();
    }

    /// Asks the window for a frame after `delay_ms` (0 = next frame). A no-op
    /// without a window (headless tests).
    pub fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw(0);
        }
    }

    /// The window while the native loop drives this runtime.
    pub fn window(&self) -> Option<&Rc<Window>> {
        self.window.as_ref()
    }

    /// Called by `ui::app::run` once the window exists.
    pub fn attach_window(&mut self, window: Rc<Window>) {
        self.window = Some(window);
    }

    /// Drops the window handle (breaks the runtime ↔ host cycle) when the
    /// native event loop ends.
    pub fn detach_window(&mut self) {
        self.window = None;
    }

    /// A stable per-runtime address used as the owner of queued
    /// `ui::focus` / `ui::scroll_into_view` commands (Zig passed `self`).
    /// Only compared as an opaque pointer, never dereferenced.
    pub fn owner_ptr(&self) -> *const () {
        self as *const Runtime as *const ()
    }

    /// The application state, downcast to the concrete `App` type (tests and
    /// code that needs to inspect or mutate the state from outside the frame).
    pub fn app_state_mut<S: 'static>(&mut self) -> &mut S {
        let app: &mut dyn App = self.app.as_mut();
        let root: &mut dyn std::any::Any = app;
        root.downcast_mut::<S>()
            .expect("Zant UI: runtime app state has another type")
    }

    /// The runtime keeps its rebuild loop in a per-window thread; nothing to do
    /// for headless single-window use.
    pub fn run(&mut self) {}

    /// Window-drag hit test for app-drawn title bars: interactive widgets stay
    /// clickable, the rest of the bar moves the window. The maximize-button
    /// special case (`ui.caption_maximize_key`) lands with WP-2.5.
    pub fn titlebar_hit(&mut self, x: f32, y: f32) -> i32 {
        if self.tree.hit_test(x, y).is_some() {
            return platform::hit::CLIENT;
        }
        platform::hit::CAPTION
    }

    /// Runs one rebuild + reconcile without painting (tests; Zig
    /// `rebuildForTest`).
    pub fn rebuild_for_test(&mut self) {
        self.rebuild();
    }

    /// Turns on `ZANT_FRAME_STATS` (per-second stderr line, `[slow-frame]`).
    pub fn enable_frame_stats(&mut self) {
        self.frame_stats = Some(FrameStats::default());
        crate::renderer::text::LAYOUT_STATS.set_enabled(true);
    }

    /// Rebuilds the `ui::key_bounds` snapshot from the laid-out tree (called
    /// after every layout; Zig read the live tree).
    pub fn refresh_key_bounds(&mut self) {
        let mut bounds = self.key_bounds.borrow_mut();
        bounds.clear();
        for node in &self.tree.nodes {
            if !node.alive || !node.in_flow() {
                continue;
            }
            if let Some(key) = node.key.as_deref() {
                bounds.insert(key.to_string(), node.bounds);
            }
        }
    }

    /// A frame painted from inside an event callback (native modal loop) shows
    /// the current tree as is.
    pub(crate) fn nested_frame(&self) -> bool {
        self.dispatch_depth > 0
    }

    /// The host's wake callback: the app takes results published by background
    /// work, then the UI rebuilds.
    pub fn handle_wake(&mut self) {
        self.app.on_wake();
        self.needs_build = true;
        self.request_redraw();
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // The hook points at this runtime's snapshot; leave no dangling `Rc`.
        crate::ui::set_key_bounds_hook(None);
    }
}

/// Builds the runtime context of a callback: the root application state of
/// `action` / `text_action` / … closures (Zig handed around a `*anyopaque`).
///
/// Takes `&mut Box<dyn App>` instead of `&mut Runtime` so the borrow only
/// covers the `app` field and the caller can keep using the tree around it.
pub(crate) fn action_cx(app: &mut Box<dyn App>) -> ActionCx<'_> {
    let app: &mut dyn App = app.as_mut();
    let root: &mut dyn std::any::Any = app;
    ActionCx::new(root)
}
