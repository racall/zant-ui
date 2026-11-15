//! Opt-in native-callback replay for environments without desktop automation.
//! A FIFO at ZANT_HOT_CONTROL accepts `pointer type x y`, `scroll x y dy`,
//! `key code modifiers`, `text ...`, `snapshot` and `close` (logical points).
use std::io::BufRead;
use zant_sys::platform::{self as ffi, ZantEvent};

pub enum Action {
    Pointer(i32, f32, f32),
    Scroll(f32, f32, f32),
    Key(i32, i32),
    Text(String),
    Snapshot,
    Close,
}

pub fn start() {
    let Some(path) = std::env::var_os("ZANT_HOT_CONTROL") else {
        return;
    };
    std::thread::spawn(move || {
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("[zant] control: {error}");
                return;
            }
        };
        for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
            if let Some(action) = parse(&line) {
                super::enqueue(super::Command::Control(action));
            } else {
                eprintln!("[zant] invalid control command: {line}");
            }
        }
    });
}

fn parse(line: &str) -> Option<Action> {
    if let Some(text) = line.strip_prefix("text ") {
        return Some(Action::Text(text.into()));
    }
    let mut fields = line.split_whitespace();
    Some(match fields.next()? {
        "pointer" => Action::Pointer(
            fields.next()?.parse().ok()?,
            fields.next()?.parse().ok()?,
            fields.next()?.parse().ok()?,
        ),
        "scroll" => Action::Scroll(
            fields.next()?.parse().ok()?,
            fields.next()?.parse().ok()?,
            fields.next()?.parse().ok()?,
        ),
        "key" => Action::Key(fields.next()?.parse().ok()?, fields.next()?.parse().ok()?),
        "snapshot" => Action::Snapshot,
        "close" => Action::Close,
        _ => return None,
    })
}

pub fn dispatch(action: Action) {
    let window = super::WINDOW.load(std::sync::atomic::Ordering::Acquire);
    if matches!(action, Action::Snapshot) {
        super::debug::loaded(window);
        return;
    }
    if matches!(action, Action::Close) {
        // SAFETY: live host window, on its UI thread (normal close-request path).
        unsafe { ffi::zant_window_close(window) };
        return;
    }
    // SAFETY: C event has no invalid zero bit patterns. Text is borrowed only
    // for the synchronous callback, and never enters the cross-thread queue.
    let mut event: ZantEvent = unsafe { std::mem::zeroed() };
    match &action {
        Action::Pointer(kind, x, y) => {
            event.type_ = *kind;
            event.x = *x;
            event.y = *y;
        }
        Action::Scroll(x, y, dy) => {
            event.type_ = ffi::ZANT_EVENT_SCROLL as i32;
            event.x = *x;
            event.y = *y;
            event.scroll_y = *dy;
        }
        Action::Key(key, modifiers) => {
            event.type_ = ffi::ZANT_EVENT_KEY_DOWN as i32;
            event.key = *key;
            event.modifiers = *modifiers;
        }
        Action::Text(text) => {
            event.type_ = ffi::ZANT_EVENT_TEXT_INPUT as i32;
            event.text = text.as_ptr().cast();
            event.text_len = text.len();
        }
        _ => return,
    }
    // SAFETY: forwarding a fully owned debug event to the native callback.
    unsafe { super::on_event(std::ptr::null_mut(), &event) };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_commands_are_ignored() {
        for line in ["", "pointer bad 1 2", "scroll 1", "key nope", "unknown"] {
            assert!(parse(line).is_none());
        }
        assert!(matches!(parse("text 世界"), Some(Action::Text(text)) if text == "世界"));
        assert!(matches!(
            parse("pointer 2 10 20"),
            Some(Action::Pointer(2, 10.0, 20.0))
        ));
    }
}
