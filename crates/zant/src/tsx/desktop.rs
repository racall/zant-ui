//! Desktop hooks of TSX apps: the tray (`useTray`) and global hotkeys
//! (`useHotkey`), port of `src/tsx_desktop.zig`.
//!
//! Both are applied while rendering, only when something changed, and their
//! callbacks run from app events (`tsx::app::Shell` forwards them here through
//! [`handle_app_event`]).
//!
//! ```ignore
//! // useTray({ icon: trayIcon, tooltip: "Clips", items: [{ title: "Open", onSelect: () => {} }] })
//! let shown = tsx::desktop::update(tsx::desktop::TrayOptions { icon: ICON, items: vec![…], ..Default::default() });
//! // useHotkey("ctrl+cmd+v", () => panel.toggle())
//! let ok = self._hotkey1.update("ctrl+cmd+v", action);
//! ```
//!
//! Zig's `Tray` / `Hotkey` were plain values with a manual `deinit`; here the
//! applied tray lives in a thread-local (one tray per app) and a `Hotkey`
//! shares its registration through an `Rc` that unregisters on drop (the
//! component's `local_state` slot going away is the unmount).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::Str;
use crate::desktop;
use crate::ui::{Action, ActionCx, HotkeyAction, TextAction};

// Tests drive TSX apps headless: they must not add a tray icon or take global
// hotkeys on the machine running them (Zig: `const system = !is_test`).
thread_local! {
    static SYSTEM: Cell<bool> = const { Cell::new(!cfg!(test)) };
}

/// Enables / disables the real tray and system hotkeys (tests turn them off;
/// an app never needs to).
pub fn set_system_enabled(enabled: bool) {
    SYSTEM.with(|system| system.set(enabled));
}

fn system_enabled() -> bool {
    SYSTEM.with(Cell::get)
}

// -------------------------------------------------------------------- tray

/// One menu item of `useTray`.
#[derive(Clone, Default)]
pub struct TrayItem {
    pub title: Str,
    pub checked: bool,
    pub disabled: bool,
    pub separator: bool,
    pub on_select: Option<Action>,
}

/// The options of `useTray` (`tsx.desktop.TrayOptions`).
#[derive(Clone, Default)]
pub struct TrayOptions {
    /// PNG bytes of an imported asset (`&'static [u8]`).
    pub icon: &'static [u8],
    pub icon_light: Option<&'static [u8]>,
    pub template: bool,
    pub tooltip: Str,
    pub items: Vec<TrayItem>,
    /// Windows: a left click on the icon.
    pub on_click: Option<Action>,
}

/// What `desktop.set_tray` last got (its own copy), null before.
struct AppliedItem {
    title: String,
    checked: bool,
    disabled: bool,
    separator: bool,
}

struct Applied {
    icon: Vec<u8>,
    icon_light: Option<Vec<u8>>,
    template: bool,
    tooltip: String,
    items: Vec<AppliedItem>,
}

/// The app's tray icon (one per app).
#[derive(Default)]
pub struct Tray {
    applied: Option<Applied>,
    /// This build's callbacks (the node owns them while the frame lives).
    selects: Vec<Option<Action>>,
    click_action: Option<Action>,
    /// The system refused the icon (no tray): `useTray` returns false.
    failed: bool,
}

impl Tray {
    /// Applies `options` when they differ from the last call; true while the
    /// icon is shown.
    pub fn update(&mut self, options: TrayOptions) -> bool {
        self.selects = options
            .items
            .iter()
            .map(|item| item.on_select.clone())
            .collect();
        self.click_action = options.on_click.clone();
        if let Some(last) = &self.applied
            && same(last, &options)
        {
            return !self.failed;
        }
        let applied = Applied {
            icon: options.icon.to_vec(),
            icon_light: options.icon_light.map(<[u8]>::to_vec),
            template: options.template,
            tooltip: options.tooltip.to_string(),
            items: options
                .items
                .iter()
                .map(|item| AppliedItem {
                    title: item.title.to_string(),
                    checked: item.checked,
                    disabled: item.disabled,
                    separator: item.separator,
                })
                .collect(),
        };
        self.applied = Some(applied);
        if !system_enabled() {
            return true;
        }
        let applied = self.applied.as_ref().expect("just stored");
        let items: Vec<desktop::TrayItem> = applied
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| desktop::TrayItem {
                id: u32::try_from(index + 1).unwrap_or(u32::MAX),
                title: &item.title,
                checked: item.checked,
                disabled: item.disabled,
                separator: item.separator,
            })
            .collect();
        let result = desktop::set_tray(desktop::TrayOptions {
            icon: &applied.icon,
            icon_light: applied.icon_light.as_deref(),
            template: applied.template,
            tooltip: &applied.tooltip,
            items: &items,
        });
        match result {
            Ok(()) => {
                self.failed = false;
                true
            }
            Err(error) => {
                eprintln!("zant: tray: {error}");
                self.failed = true;
                false
            }
        }
    }

    /// The menu item with `id` (1-based) was chosen.
    pub fn select(&mut self, cx: &mut ActionCx<'_>, id: u32) {
        if id == 0 || id as usize > self.selects.len() {
            return;
        }
        let action = self.selects[id as usize - 1].clone();
        if let Some(action) = action {
            action.call(cx);
        }
    }

    pub fn click(&mut self, cx: &mut ActionCx<'_>) {
        let action = self.click_action.clone();
        if let Some(action) = action {
            action.call(cx);
        }
    }
}

fn same(last: &Applied, options: &TrayOptions) -> bool {
    if last.icon.as_slice() != options.icon || last.template != options.template {
        return false;
    }
    if last.tooltip != options.tooltip.as_str() {
        return false;
    }
    let light_same = match (&last.icon_light, options.icon_light) {
        (Some(current), Some(next)) => current.as_slice() == next,
        (None, None) => true,
        _ => false,
    };
    if !light_same || last.items.len() != options.items.len() {
        return false;
    }
    for (current, next) in last.items.iter().zip(&options.items) {
        if current.title != next.title.as_str() || current.checked != next.checked {
            return false;
        }
        if current.disabled != next.disabled || current.separator != next.separator {
            return false;
        }
    }
    true
}

thread_local! {
    /// The app's tray (Zig `tsx_desktop.tray`).
    static TRAY: RefCell<Tray> = RefCell::new(Tray::default());
}

/// `useTray(options)`: applies the tray when it changed; true while shown.
pub fn update(options: TrayOptions) -> bool {
    TRAY.with(|tray| tray.borrow_mut().update(options))
}

/// The tray menu item with `id` was chosen (app event dispatch).
pub fn tray_select(cx: &mut ActionCx<'_>, id: u32) {
    TRAY.with(|tray| tray.borrow_mut().select(cx, id));
}

/// The tray icon was clicked (Windows left click).
pub fn tray_click(cx: &mut ActionCx<'_>) {
    TRAY.with(|tray| tray.borrow_mut().click(cx));
}

// ---------------------------------------------------------------- shortcuts

/// `HotkeyInput`'s shortcut type (the temporary duplicate of
/// `desktop.Shortcut` in `components/hotkey_input.rs`, see status.md 待决问题 5).
pub type Shortcut = crate::components::hotkey_input::Shortcut;

/// The control-side shortcut of a `desktop.Shortcut`.
pub fn hotkey_shortcut(shortcut: desktop::Shortcut) -> Shortcut {
    Shortcut {
        key: shortcut.key,
        modifiers: shortcut.modifiers,
    }
}

/// The framework-side shortcut of a control-side one.
pub fn desktop_shortcut(shortcut: Shortcut) -> desktop::Shortcut {
    desktop::Shortcut::new(shortcut.key, shortcut.modifiers)
}

/// `<HotkeyInput value="ctrl+cmd+v">`: `None` for "" or an invalid string.
pub fn parse_shortcut(text: &str) -> Option<Shortcut> {
    desktop::Shortcut::parse(text).map(hotkey_shortcut)
}

/// `<HotkeyInput onRecord={...}>`: the recorded shortcut as its string form
/// ("ctrl+cmd+v", what `useHotkey` takes).
pub fn shortcut_action(target: TextAction) -> HotkeyAction {
    HotkeyAction::new(move |cx, shortcut| {
        let text = desktop_shortcut(shortcut).format();
        target.call(cx, &text);
    })
}

// ------------------------------------------------------------------ hotkeys

/// Global ids from here up are TSX hotkeys (apps' own ids stay below).
const FIRST_ID: u32 = 0x7000_0000;

thread_local! {
    /// Registrations of this app (weak, so a dropped component unregisters).
    static HOTKEYS: RefCell<Vec<Weak<RefCell<HotkeyState>>>> = const { RefCell::new(Vec::new()) };
    static NEXT_ID: Cell<u32> = const { Cell::new(FIRST_ID) };
}

/// One `useHotkey`: its shortcut, registered while it is valid.
#[derive(Clone, Default)]
pub struct Hotkey {
    inner: Option<Rc<RefCell<HotkeyState>>>,
}

struct HotkeyState {
    /// App-wide id (assigned on first use).
    id: u32,
    shortcut: String,
    registered: bool,
    action: Option<Action>,
}

impl Hotkey {
    /// Registers `shortcut` ("ctrl+cmd+v"; "" for none) when it changed; true
    /// while registered (false: taken by another app, or invalid).
    pub fn update(&mut self, shortcut: &str, action: Action) -> bool {
        let inner = Rc::clone(self.inner.get_or_insert_with(|| {
            let id = NEXT_ID.with(|next| {
                let id = next.get();
                next.set(id.wrapping_add(1));
                id
            });
            let state = Rc::new(RefCell::new(HotkeyState {
                id,
                shortcut: String::new(),
                registered: false,
                action: None,
            }));
            HOTKEYS.with(|hotkeys| hotkeys.borrow_mut().push(Rc::downgrade(&state)));
            state
        }));
        let mut state = inner.borrow_mut();
        state.action = Some(action);
        let same = state.shortcut == shortcut;
        if same && !shortcut.is_empty() {
            return state.registered;
        }
        if state.registered && system_enabled() {
            desktop::unregister_hotkey(state.id);
        }
        state.registered = false;
        state.shortcut = shortcut.to_owned();
        let Some(parsed) = desktop::Shortcut::parse(shortcut) else {
            return false;
        };
        state.registered = !system_enabled() || desktop::register_hotkey(state.id, parsed);
        state.registered
    }

    /// The app-wide registration id (0 before the first `update`).
    pub fn id(&self) -> u32 {
        self.inner.as_ref().map_or(0, |inner| inner.borrow().id)
    }

    pub fn registered(&self) -> bool {
        self.inner
            .as_ref()
            .is_some_and(|inner| inner.borrow().registered)
    }
}

impl Drop for HotkeyState {
    fn drop(&mut self) {
        if self.registered && system_enabled() {
            desktop::unregister_hotkey(self.id);
        }
    }
}

/// A hotkey app event: runs the `useHotkey` callback with that id.
pub fn hotkey_pressed(cx: &mut ActionCx<'_>, id: u32) -> bool {
    let action = HOTKEYS.with(|hotkeys| {
        let mut hotkeys = hotkeys.borrow_mut();
        hotkeys.retain(|weak| weak.strong_count() > 0);
        for weak in hotkeys.iter() {
            let Some(state) = weak.upgrade() else {
                continue;
            };
            let state = state.borrow();
            if state.id == id {
                return state.action.clone();
            }
        }
        None
    });
    match action {
        Some(action) => {
            action.call(cx);
            true
        }
        None => false,
    }
}

/// Routes an app event to the tray / hotkey hooks; true when one matched.
pub fn handle_app_event(cx: &mut ActionCx<'_>, event: &desktop::AppEvent) -> bool {
    match event {
        desktop::AppEvent::TrayClick => {
            tray_click(cx);
            true
        }
        desktop::AppEvent::TrayMenu(id) => {
            tray_select(cx, *id);
            true
        }
        desktop::AppEvent::Hotkey(id) => hotkey_pressed(cx, *id),
        desktop::AppEvent::Reopen | desktop::AppEvent::Clipboard => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell as StdCell;

    fn eq_str(expected: &str, got: Str) {
        assert_eq!(expected, got.as_str());
    }

    #[test]
    fn tray_menu_items_run_their_callbacks_by_id() {
        let hits = Rc::new(StdCell::new([0u32; 3]));
        let counter = |index: usize| {
            let hits = Rc::clone(&hits);
            Action::new(move |_| {
                let mut values = hits.get();
                values[index] += 1;
                hits.set(values);
            })
        };
        let mut tray = Tray {
            applied: None,
            selects: vec![Some(counter(0)), None, Some(counter(1))],
            click_action: Some(counter(2)),
            failed: false,
        };
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        tray.select(&mut cx, 1);
        tray.select(&mut cx, 2);
        tray.select(&mut cx, 3);
        tray.select(&mut cx, 9);
        tray.click(&mut cx);
        assert_eq!(hits.get(), [1, 1, 1]);
    }

    #[test]
    fn tray_changes_are_detected_by_value() {
        let last = Applied {
            icon: b"png".to_vec(),
            icon_light: None,
            template: true,
            tooltip: String::from("App"),
            items: vec![AppliedItem {
                title: String::from("Open"),
                checked: false,
                disabled: false,
                separator: false,
            }],
        };
        let open = TrayOptions {
            icon: b"png",
            template: true,
            tooltip: Str::from("App"),
            items: vec![TrayItem {
                title: Str::from("Open"),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(same(&last, &open));
        let quit = TrayOptions {
            items: vec![TrayItem {
                title: Str::from("Quit"),
                ..Default::default()
            }],
            ..open.clone()
        };
        assert!(!same(&last, &quit));
        let renamed = TrayOptions {
            tooltip: Str::from("App 2"),
            ..open
        };
        assert!(!same(&last, &renamed));
        let light = TrayOptions {
            icon_light: Some(b"light"),
            ..quit
        };
        assert!(!same(&last, &light));
    }

    #[test]
    fn hotkey_events_reach_the_hook_with_that_id() {
        set_system_enabled(false);
        let hits = Rc::new(StdCell::new(0u32));
        let counter = Rc::clone(&hits);
        let mut hotkey = Hotkey::default();
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        assert!(hotkey.update(
            "ctrl+alt+k",
            Action::new(move |_| counter.set(counter.get() + 1)),
        ));
        let id = hotkey.id();
        assert!(hotkey_pressed(&mut cx, id));
        assert!(!hotkey_pressed(&mut cx, id + 100));
        assert_eq!(hits.get(), 1);
        // the same shortcut is not re-registered
        assert!(hotkey.update("ctrl+alt+k", Action::new(|_| {})));
        drop(hotkey);
        assert!(!hotkey_pressed(&mut cx, id), "dropping unregisters");
        assert!(!Hotkey::default().update("not a shortcut+", Action::new(|_| {})));
        assert!(!Hotkey::default().update("", Action::new(|_| {})));
    }

    #[test]
    fn a_recorded_shortcut_reaches_tsx_as_its_string() {
        let got = Rc::new(RefCell::new(String::new()));
        let sink = Rc::clone(&got);
        let action = shortcut_action(TextAction::new(move |_cx, text| {
            *sink.borrow_mut() = text.to_owned();
        }));
        let shortcut = parse_shortcut("ctrl+alt+k").expect("valid shortcut");
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        action.call(&mut cx, shortcut);
        let text = got.borrow().clone();
        eq_str(&desktop_shortcut(shortcut).format(), Str::copy(&text));
        let parsed = parse_shortcut(&text).expect("round trip");
        assert!(parsed.eql(&shortcut));
        assert!(parse_shortcut("").is_none());
    }
}
