//! The clipboard: text, images (PNG), files, change watching and the change
//! counter.
//!
//! Read the order the docs suggest: files first (Finder also puts the file
//! names on the clipboard as text), then text (copied text often comes with an
//! image too), then the image.
#![forbid(unsafe_code)]

use crate::platform::platform;

/// Sends [`crate::desktop::AppEvent::Clipboard`] after each clipboard change
/// (any app, this one included): clipboard managers, sync tools. Read the text
/// with [`clipboard_text`]; compare [`clipboard_change_count`] to skip your own
/// write.
pub fn watch_clipboard(enable: bool) {
    platform::watch_clipboard(enable);
}

/// Text on the clipboard (UTF-8; empty when it holds no text).
pub fn clipboard_text() -> String {
    platform::clipboard_text().unwrap_or_default()
}

/// The image on the clipboard as PNG bytes, or `None` when it holds no image.
/// Read the text first: copied text often comes with an image too (a rendering
/// of it).
///
/// May be called from a worker thread, and a large image should be: when the
/// clipboard holds a bitmap (Windows screenshots, many apps), it is encoded to
/// PNG here, about a second for a 12-megapixel photo.
pub fn clipboard_image() -> Option<Vec<u8>> {
    platform::clipboard_image_png()
}

/// Files and folders on the clipboard (copied in Finder / Explorer): their
/// absolute paths, one per line, or `None` when there are none. Check it before
/// the text: Finder also puts the names there as text.
pub fn clipboard_files() -> Option<String> {
    platform::clipboard_files()
}

/// Puts files on the clipboard (absolute paths, one per line), to paste in
/// Finder / Explorer like copied ones.
pub fn set_clipboard_files(paths: &str) -> bool {
    platform::set_clipboard_files(paths)
}

/// Puts a PNG image on the clipboard in the formats other apps paste.
pub fn set_clipboard_image(png_bytes: &[u8]) -> bool {
    platform::set_clipboard_image_png(png_bytes)
}

/// Puts `text` on the clipboard (plain text).
pub fn set_clipboard_text(text: &str) -> bool {
    platform::set_clipboard(text)
}

/// Increments whenever any app writes to the clipboard: poll it (cheap) to
/// notice copies, then read the text with [`clipboard_text`].
pub fn clipboard_change_count() -> u64 {
    platform::clipboard_change_count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trips_through_the_clipboard() {
        let _guard = crate::platform::platform::native_test_lock();
        // keep the user's clipboard: the test writes to the real one
        let previous = clipboard_text();
        assert!(set_clipboard_text("zant clipboard test"));
        assert_eq!(clipboard_text(), "zant clipboard test");
        let before = clipboard_change_count();
        assert!(set_clipboard_text("zant clipboard test 2"));
        assert!(clipboard_change_count() >= before, "the counter advances");
        assert_eq!(clipboard_text(), "zant clipboard test 2");
        // restore whatever was there before the test
        set_clipboard_text(&previous);
    }

    #[test]
    fn clipboard_queries_are_safe_without_content() {
        let _guard = crate::platform::platform::native_test_lock();
        let _ = clipboard_image();
        let _ = clipboard_files();
        watch_clipboard(false);
        let _ = clipboard_change_count();
    }
}
