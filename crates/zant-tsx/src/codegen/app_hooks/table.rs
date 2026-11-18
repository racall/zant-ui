//! The app hook table (`useTray`, `useHotkey`, `useClipboardChange`, …).

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum HookParam {
    None,
    Key,
    Focused,
}

pub(super) struct Hook {
    pub(super) name: &'static str,
    /// The `ui::App` / `tsx::window::Component` method the hook forwards to.
    pub(super) rust: &'static str,
    pub(super) param: HookParam,
    pub(super) returns: bool,
    /// Runs in the root component (`impl ui::App` forwards it).
    pub(super) root: bool,
    /// Runs in a `createWindow` component (`tsx::window::Component`
    /// forwards it).
    pub(super) window: bool,
}

pub(super) const HOOKS: [Hook; 8] = [
    Hook {
        name: "useCloseRequest",
        rust: "on_close_request",
        param: HookParam::None,
        returns: true,
        root: true,
        window: true,
    },
    Hook {
        name: "useQuitRequest",
        rust: "on_quit_request",
        param: HookParam::None,
        returns: true,
        root: true,
        window: false,
    },
    Hook {
        name: "useKeyDown",
        rust: "on_key",
        param: HookParam::Key,
        returns: true,
        root: true,
        window: true,
    },
    Hook {
        name: "useWindowFocus",
        rust: "on_focus",
        param: HookParam::Focused,
        returns: false,
        root: false,
        window: true,
    },
    Hook {
        name: "useClipboardChange",
        rust: "on_clipboard",
        param: HookParam::None,
        returns: false,
        root: true,
        window: false,
    },
    Hook {
        name: "useReopen",
        rust: "on_reopen",
        param: HookParam::None,
        returns: false,
        root: true,
        window: false,
    },
    Hook {
        name: "useWake",
        rust: "on_wake",
        param: HookParam::None,
        returns: false,
        root: true,
        window: false,
    },
    Hook {
        name: "useAppExit",
        rust: "on_exit",
        param: HookParam::None,
        returns: false,
        root: true,
        window: false,
    },
];

pub(super) fn find(name: &str) -> Option<&'static Hook> {
    HOOKS.iter().find(|hook| hook.name == name)
}

pub fn is_app_hook(name: &str) -> bool {
    find(name).is_some()
}
