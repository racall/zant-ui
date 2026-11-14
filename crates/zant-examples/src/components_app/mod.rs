//! `zant-components`: port of the Zig `zant-components` example,
//! `examples/components.zig` (the `zant-components` executable target of
//! `build.zig`).
//!
//! Every `ui.*` component in one window. Each interaction prints
//! `[event] ...` to stderr and shows it under the title, which
//! `scripts/win/components-check.ps1` uses (with `ZANT_LAYOUT_FILE`) to click
//! every component and check the result. `ZANT_DARK=1` starts in dark mode.
//! `ZANT_AUTOPLAY=1` cycles through the animated states by itself (menu, modal,
//! message, theme, tabs...) so frame rates can be measured without input, e.g.
//! with `ZANT_FRAME_STATS=1`; `ZANT_AUTOPLAY=quiet` runs the same cycle without
//! the `[event]` lines (stderr writes are slow on Windows).
//!
//! Deviations from the Zig source (see the WP-2.9 report):
//!
//! * `ui.pdfViewer` is not ported yet: `src/components/widgets.zig`'s PDF half
//!   belongs to WP-3.9 (`pdf/viewer.zig`; the Rust `pdf/viewer` modules are
//!   still stubs on this branch). The documents section keeps the slot
//!   (key / size / border) as a placeholder and the `on_change` callback that
//!   would log "pdf changed" is omitted until the widget lands.
//! * Zig stores the last event in a fixed 96-byte buffer with `bufPrint`, so a
//!   longer message is dropped entirely; the port keeps that rule (no log, the
//!   displayed text keeps its previous value).
//! * `ui.textFmt` has no Rust counterpart: call sites format with `format!`.
#![forbid(unsafe_code)]

pub mod pdf_viewer;
mod view;

use std::cell::RefCell;
use std::rc::Rc;

use zant::core::events::KeyEvent;
use zant::core::keys;
use zant::core::node::Icon;
use zant::platform::platform::{self, WindowOptions};
use zant::theme::tokens::{self, Theme};
use zant::ui::{self, ListState, RunOptions};

use self::view::render;

/// Zig `State.last_event` capacity (`[96]u8`).
const LAST_EVENT_CAPACITY: usize = 96;
/// Zig `State.autoplayTick`: one state change every 700 ms.
const AUTOPLAY_STEP_MS: i64 = 700;

/// Zig `State`: the whole application state of the components example.
pub struct ComponentsApp {
    pub(crate) dark: bool,
    pub(crate) widget_chunk: bool,
    /// The last `[event]` line, shown under the title (`last_len = 0` in Zig
    /// while nothing happened).
    pub(crate) last_event: String,
    pub(crate) notifications: bool,
    pub(crate) beta: bool,
    pub(crate) plan: usize,
    pub(crate) region: usize,
    pub(crate) view: usize,
    pub(crate) tab: usize,
    pub(crate) progress: f32,
    pub(crate) menu_open: bool,
    pub(crate) modal_open: bool,
    pub(crate) message_until_ms: i64,
    pub(crate) autoplay: bool,
    /// `ZANT_AUTOPLAY=quiet`: no `[event]` lines.
    pub(crate) quiet: bool,
    pub(crate) autoplay_step: usize,
    pub(crate) autoplay_at_ms: i64,
    /// Pointer area demo: knob position (0..1) and the drag start.
    pub(crate) knob: f32,
    pub(crate) knob_drag: Option<[f32; 2]>,
    /// Virtual list demo (10 000 rows).
    pub(crate) rows: Rc<RefCell<ListState>>,
    /// `src` of the PDF viewer widget (the Zig example uses the working
    /// directory relative fixture path; the headless tests point it at the
    /// repository fixture).
    pub(crate) pdf_src: String,
}

impl ComponentsApp {
    /// Zig `State{ .dark = .., .autoplay = .., .quiet = .. }`.
    pub fn new(dark: bool, autoplay: bool, quiet: bool) -> ComponentsApp {
        ComponentsApp {
            dark,
            widget_chunk: false,
            last_event: String::new(),
            notifications: true,
            beta: false,
            plan: 0,
            region: 0,
            view: 0,
            tab: 0,
            progress: 40.0,
            menu_open: false,
            modal_open: false,
            message_until_ms: 0,
            autoplay,
            quiet,
            autoplay_step: 0,
            autoplay_at_ms: 0,
            knob: 0.3,
            knob_drag: None,
            rows: Rc::new(RefCell::new(ListState::new())),
            pdf_src: String::from("tests/fixtures/widgets.pdf"),
        }
    }

    /// `pdf_src` of the PDF viewer widget (headless tests resolve the fixture
    /// from the crate directory instead of the working directory).
    pub fn with_pdf_src(mut self, src: impl Into<String>) -> ComponentsApp {
        self.pdf_src = src.into();
        self
    }

    /// The last `[event]` message (`None` while nothing happened).
    pub fn last_event(&self) -> Option<&str> {
        if self.last_event.is_empty() {
            None
        } else {
            Some(&self.last_event)
        }
    }

    /// Zig `State.autoplayTick`: one state change every 700 ms, so each enter /
    /// exit animation runs to the end before the next one starts.
    fn autoplay_tick(&mut self) {
        let now = ui::now_ms();
        if self.autoplay_at_ms == 0 {
            self.autoplay_at_ms = now + AUTOPLAY_STEP_MS;
        }
        if now >= self.autoplay_at_ms {
            match self.autoplay_step % 12 {
                0 => self.open_menu(),
                1 => self.close_menu(),
                2 => self.open_modal(),
                3 => self.close_modal(),
                4 => self.show_message(),
                5 => self.toggle_dark(),
                6 => self.on_tab(1),
                7 => self.on_view(2),
                8 => self.more_progress(),
                9 => self.toggle_dark(),
                10 => self.on_tab(0),
                _ => self.on_view(0),
            }
            self.autoplay_step += 1;
            self.autoplay_at_ms = now + AUTOPLAY_STEP_MS;
        }
        ui::wake_after((self.autoplay_at_ms - now).max(1) as u32);
    }

    /// Zig `State.event`: `bufPrint` into a 96-byte buffer — an over-long
    /// message is dropped entirely (no store, no log).
    fn event(&mut self, message: String) {
        if message.len() > LAST_EVENT_CAPACITY {
            return;
        }
        self.last_event = message.clone();
        if !self.quiet {
            eprintln!("[event] {message}");
        }
    }

    fn append_widget(&mut self) {
        self.widget_chunk = !self.widget_chunk;
        self.event(String::from("markdown chunk"));
    }

    /// Zig `State.pdfChanged`.
    fn pdf_changed(&mut self) {
        self.event(String::from("pdf changed"));
    }

    /// `ui.pointerArea`: drag the knob along the track; double click resets it.
    fn on_track(&mut self, info: ui::PointerInfo) -> bool {
        match info.phase {
            ui::PointerPhase::Down => {
                if info.click_count == 2 {
                    self.knob = 0.5;
                    self.event(String::from("pointer double"));
                    return true;
                }
                self.knob = (info.x / info.width.max(1.0)).clamp(0.0, 1.0);
                self.knob_drag = Some([info.window_x, self.knob]);
                self.event(String::from("pointer down"));
            }
            ui::PointerPhase::Drag => {
                let Some(start) = self.knob_drag else {
                    return false;
                };
                self.knob =
                    (start[1] + (info.window_x - start[0]) / info.width.max(1.0)).clamp(0.0, 1.0);
            }
            ui::PointerPhase::Up => {
                if self.knob_drag.is_none() {
                    return false;
                }
                self.knob_drag = None;
                self.event(String::from("pointer up"));
            }
            _ => return false,
        }
        true
    }

    fn list_scrolled(&mut self) {
        self.event(String::from("list scrolled"));
    }

    fn click_primary(&mut self) {
        self.event(String::from("click primary"));
    }
    fn click_default(&mut self) {
        self.event(String::from("click default"));
    }
    fn click_text(&mut self) {
        self.event(String::from("click text"));
    }
    fn click_link_button(&mut self) {
        self.event(String::from("click link-button"));
    }
    fn click_disabled(&mut self) {
        self.event(String::from("click disabled"));
    }
    fn click_icon(&mut self) {
        self.event(String::from("click icon"));
    }
    fn click_link(&mut self) {
        self.event(String::from("click link"));
    }
    fn on_notify(&mut self, value: bool) {
        self.notifications = value;
        self.event(format!("checkbox {value}"));
    }
    fn on_beta(&mut self, value: bool) {
        self.beta = value;
        self.event(format!("switch {value}"));
    }
    fn on_plan(&mut self, index: usize) {
        self.plan = index;
        self.event(format!("radio {index}"));
    }
    fn on_region(&mut self, index: usize) {
        self.region = index;
        self.event(format!("select {index}"));
    }
    fn on_view(&mut self, index: usize) {
        self.view = index;
        self.event(format!("segmented {index}"));
    }
    fn on_tab(&mut self, index: usize) {
        self.tab = index;
        self.event(format!("tab {index}"));
    }
    fn on_input(&mut self, text: &str) {
        self.event(format!("input {text}"));
    }
    fn on_submit(&mut self) {
        self.event(String::from("submit"));
    }
    fn more_progress(&mut self) {
        self.progress = if self.progress >= 100.0 {
            0.0
        } else {
            self.progress + 20.0
        };
        self.event(format!("progress {}", self.progress as u32));
    }
    fn toggle_dark(&mut self) {
        self.dark = !self.dark;
        self.event(format!("dark {}", self.dark));
    }
    fn open_menu(&mut self) {
        self.menu_open = true;
        self.event(String::from("menu open"));
    }
    fn close_menu(&mut self) {
        self.menu_open = false;
        self.event(String::from("menu dismiss"));
    }
    fn menu_new(&mut self) {
        self.menu_open = false;
        self.event(String::from("menu new"));
    }
    fn menu_delete(&mut self) {
        self.menu_open = false;
        self.event(String::from("menu delete"));
    }
    fn open_modal(&mut self) {
        self.modal_open = true;
        self.event(String::from("modal open"));
    }
    fn close_modal(&mut self) {
        self.modal_open = false;
        self.event(String::from("modal close"));
    }
    fn show_message(&mut self) {
        // antd's default message duration
        self.message_until_ms = ui::now_ms() + 3000;
        self.event(String::from("message show"));
    }
}

impl ui::App for ComponentsApp {
    fn render(&mut self) -> ui::Node {
        render(self)
    }

    /// Zig `State.theme`.
    fn theme(&self) -> Option<Theme> {
        Some(if self.dark {
            tokens::ant_dark()
        } else {
            tokens::ant_light()
        })
    }

    /// Zig `State.onKey`: Esc closes the menu and the modal.
    fn on_key(&mut self, key: &KeyEvent) -> bool {
        if key.key == zant::core::events::Key::Escape && (self.menu_open || self.modal_open) {
            self.menu_open = false;
            self.modal_open = false;
            self.event(String::from("escape"));
            return true;
        }
        false
    }
}

/// Zig `Runtime.Config` of the example (`zant.launch.run` options).
pub fn run_options() -> RunOptions {
    RunOptions {
        title: String::from("Zant 组件总览"),
        window: WindowOptions {
            width: 1100.0,
            height: 820.0,
            min_width: 900.0,
            min_height: 600.0,
            ..Default::default()
        },
        theme: Theme::default(),
    }
}

/// `ZANT_AUTOPLAY` / `ZANT_DARK` of the Zig `main`.
pub fn state_from_env() -> ComponentsApp {
    let dark = platform::getenv("ZANT_DARK").is_some();
    let autoplay = platform::getenv("ZANT_AUTOPLAY");
    let quiet = autoplay.as_deref() == Some("quiet");
    ComponentsApp::new(dark, autoplay.is_some(), quiet)
}

/// The example entry point (`pub fn main` of the Zig source).
pub fn run() -> zant::Result<()> {
    ui::run(state_from_env(), run_options())
}

/// The `[event]` line of the keyboard-shortcut demo.
fn shortcut_groups() -> Vec<ui::ShortcutGroup> {
    vec![
        ui::ShortcutGroup {
            title: "文件".into(),
            items: vec![
                ui::Shortcut {
                    label: "保存".into(),
                    keys: keys::label("mod+s").into(),
                },
                ui::Shortcut {
                    label: "另存为".into(),
                    keys: keys::label("mod+shift+s").into(),
                },
            ],
        },
        ui::ShortcutGroup {
            title: "编辑".into(),
            items: vec![ui::Shortcut {
                label: "撤销 / 重做".into(),
                keys: keys::pick(keys::label("mod+z mod+shift+z"), keys::label("mod+z mod+y"))
                    .into(),
            }],
        },
    ]
}

/// Icons of the display section.
pub(crate) const DISPLAY_ICONS: [Icon; 12] = [
    Icon::Search,
    Icon::Settings,
    Icon::Folder,
    Icon::Save,
    Icon::Copy,
    Icon::Trash,
    Icon::Sun,
    Icon::Moon,
    Icon::Info,
    Icon::Alert,
    Icon::Check,
    Icon::Close,
];
