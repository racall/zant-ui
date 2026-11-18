//! The `zant` API table: namespaces, functions and constants TSX can import from `zant`.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Void,
    String,
    Boolean,
    Number,
    Bytes,
    OptString,
    OptBytes,
    OptImage,
}

pub(super) struct Function {
    pub(super) name: &'static str,
    pub(super) rust: &'static str,
    pub(super) params: &'static [Kind],
    pub(super) ret: Kind,
    /// Changes the clipboard, a window or the app: not while rendering.
    pub(super) effect: bool,
}

pub(super) struct Namespace {
    pub(super) name: &'static str,
    pub(super) rust: &'static str,
    pub(super) functions: &'static [Function],
}

pub(super) const NAMESPACES: [Namespace; 3] = [
    Namespace {
        name: "clipboard",
        rust: "tsx::platform::clipboard",
        functions: &[
            Function {
                name: "text",
                rust: "text",
                params: &[],
                ret: Kind::String,
                effect: false,
            },
            Function {
                name: "setText",
                rust: "set_text",
                params: &[Kind::String],
                ret: Kind::Boolean,
                effect: true,
            },
            Function {
                name: "image",
                rust: "image",
                params: &[],
                ret: Kind::OptBytes,
                effect: false,
            },
            Function {
                name: "setImage",
                rust: "set_image",
                params: &[Kind::Bytes],
                ret: Kind::Boolean,
                effect: true,
            },
            Function {
                name: "files",
                rust: "files",
                params: &[],
                ret: Kind::OptString,
                effect: false,
            },
            Function {
                name: "setFiles",
                rust: "set_files",
                params: &[Kind::String],
                ret: Kind::Boolean,
                effect: true,
            },
            Function {
                name: "changeCount",
                rust: "change_count",
                params: &[],
                ret: Kind::Number,
                effect: false,
            },
        ],
    },
    Namespace {
        name: "app",
        rust: "tsx::platform::app",
        functions: &[
            Function {
                name: "quit",
                rust: "quit",
                params: &[],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "launchAtLogin",
                rust: "launch_at_login",
                params: &[],
                ret: Kind::String,
                effect: false,
            },
            Function {
                name: "setLaunchAtLogin",
                rust: "set_launch_at_login",
                params: &[Kind::Boolean],
                ret: Kind::Boolean,
                effect: true,
            },
            Function {
                name: "launchedAtLogin",
                rust: "launched_at_login",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
            Function {
                name: "canLaunchAtLogin",
                rust: "can_launch_at_login",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
            Function {
                name: "openLoginItemSettings",
                rust: "open_login_item_settings",
                params: &[],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "deviceName",
                rust: "device_name",
                params: &[],
                ret: Kind::String,
                effect: false,
            },
            Function {
                name: "downloadsDir",
                rust: "downloads_dir",
                params: &[],
                ret: Kind::OptString,
                effect: false,
            },
            Function {
                name: "dataDir",
                rust: "data_dir",
                params: &[],
                ret: Kind::OptString,
                effect: false,
            },
        ],
    },
    Namespace {
        name: "mainWindow",
        rust: "tsx::platform::main_window",
        functions: &[
            Function {
                name: "show",
                rust: "show",
                params: &[],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "hide",
                rust: "hide",
                params: &[],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "activate",
                rust: "activate",
                params: &[],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "focus",
                rust: "focus",
                params: &[Kind::String],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "setTitle",
                rust: "set_title",
                params: &[Kind::String],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "setSize",
                rust: "set_size",
                params: &[Kind::Number, Kind::Number],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "setMinSize",
                rust: "set_min_size",
                params: &[Kind::Number, Kind::Number],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "setMaxSize",
                rust: "set_max_size",
                params: &[Kind::Number, Kind::Number],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "setAlwaysOnTop",
                rust: "set_always_on_top",
                params: &[Kind::Boolean],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "setFullscreen",
                rust: "set_fullscreen",
                params: &[Kind::Boolean],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "fullscreen",
                rust: "fullscreen",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
            Function {
                name: "visible",
                rust: "visible",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
            Function {
                name: "active",
                rust: "active",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
            Function {
                name: "minimized",
                rust: "minimized",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
        ],
    },
];

/// Functions imported by name.
pub(super) const FUNCTIONS: [Function; 3] = [
    Function {
        name: "wakeAfter",
        rust: "tsx::platform::wake_after",
        params: &[Kind::Number],
        ret: Kind::Void,
        effect: false,
    },
    Function {
        name: "shortcutLabel",
        rust: "tsx::platform::shortcut_label",
        params: &[Kind::String],
        ret: Kind::String,
        effect: false,
    },
    Function {
        name: "assetImage",
        rust: "tsx::platform::asset_image",
        params: &[Kind::Bytes],
        ret: Kind::OptImage,
        effect: false,
    },
];

/// Constants imported by name (`tsx::platform::IS_MACOS`).
pub(super) const CONSTANTS: [(&str, &str); 2] = [
    ("isMacos", "zant::tsx::platform::IS_MACOS"),
    ("isWindows", "zant::tsx::platform::IS_WINDOWS"),
];

pub(super) fn names(namespace: &Namespace) -> String {
    namespace
        .functions
        .iter()
        .map(|function| function.name)
        .collect::<Vec<_>>()
        .join(", ")
}
