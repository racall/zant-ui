//! Port of `src/theme/default.zig`: the default theme handed to the runtime.
#![forbid(unsafe_code)]

use crate::theme::tokens;

pub use crate::theme::tokens::Theme;

pub fn light() -> Theme {
    tokens::ant_light()
}
