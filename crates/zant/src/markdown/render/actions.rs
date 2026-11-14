//! Callback contexts for interactive pieces (copy, download, links).
//!
//! The closures capture owned values, so the contexts outlive frames while
//! memoised block UI is reused.

use crate::platform::dialog;
use crate::platform::platform;
use crate::ui::actions::{Action, TextAction};

/// Copies `text` to the clipboard.
#[derive(Clone, Debug)]
pub struct Copy {
    pub text: String,
}

impl Copy {
    pub fn run(&self) {
        let _ = platform::set_clipboard(&self.text);
    }

    pub fn action(text: &str) -> Action {
        let context = Copy {
            text: text.to_owned(),
        };
        Action::new(move |_cx| context.run())
    }
}

/// Save panel + write. With two formats, the chosen extension decides.
#[derive(Clone, Debug, Default)]
pub struct Download {
    pub filename: String,
    pub extensions: String,
    pub text: String,
    /// Optional alternative content for a second extension (table: md).
    pub alt_extension: String,
    pub alt_text: String,
}

impl Download {
    pub fn run(&self) {
        let Some(path) = dialog::save_file_as(&self.filename, &self.extensions) else {
            return;
        };
        let data = if !self.alt_extension.is_empty() && path.ends_with(&self.alt_extension) {
            &self.alt_text
        } else {
            &self.text
        };
        let _ = dialog::write_file(&path, data.as_bytes());
    }

    pub fn action(value: Download) -> Action {
        Action::new(move |_cx| value.run())
    }
}

/// Rich-text link activation: forwards to the app (link safety) or opens
/// the URL directly.
#[derive(Clone)]
pub struct Link {
    pub forward: Option<TextAction>,
}

impl Link {
    pub fn run(&self, cx: &mut crate::ui::actions::ActionCx<'_>, url: &str) {
        match &self.forward {
            Some(action) => action.call(cx, url),
            None => {
                let _ = dialog::open_url(url);
            }
        }
    }

    pub fn action(forward: Option<TextAction>) -> TextAction {
        let context = Link { forward };
        TextAction::new(move |cx, url| context.run(cx, url))
    }
}
