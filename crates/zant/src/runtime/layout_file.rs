//! `ZANT_LAYOUT_FILE` writer: a background thread writes the latest layout
//! dump, so a slow disk, a virus scanner or an automation script reading the
//! file never stalls a frame (a synchronous write took 150–180 ms now and then
//! on Windows while a test driver polled the file).
//!
//! Only the newest dump is kept: a frame publishes its contents and returns;
//! the writer skips contents equal to what it wrote last, writes a temporary
//! file next to the target and renames it over the target, so a reader never
//! sees a half-written file. A failed rename (the reader holds the file open
//! on Windows) is retried with the next dump.
#![forbid(unsafe_code)]

use std::sync::{Condvar, Mutex, OnceLock};

struct Slot {
    pending: Mutex<Option<(String, String)>>,
    ready: Condvar,
}

fn slot() -> &'static Slot {
    static SLOT: OnceLock<Slot> = OnceLock::new();
    SLOT.get_or_init(|| {
        std::thread::Builder::new()
            .name("zant-layout-file".into())
            .spawn(writer)
            .expect("spawn the layout file writer");
        Slot {
            pending: Mutex::new(None),
            ready: Condvar::new(),
        }
    })
}

/// Hands the dump of this frame to the writer (replacing one it has not
/// written yet).
pub(crate) fn publish(path: &str, contents: String) {
    let slot = slot();
    let mut pending = slot.pending.lock().unwrap_or_else(|e| e.into_inner());
    *pending = Some((path.to_owned(), contents));
    slot.ready.notify_one();
}

fn writer() {
    let slot = slot();
    let mut written: Option<(String, String)> = None;
    loop {
        let (path, contents) = {
            let mut pending = slot.pending.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if let Some(next) = pending.take() {
                    break next;
                }
                pending = slot.ready.wait(pending).unwrap_or_else(|e| e.into_inner());
            }
        };
        if written
            .as_ref()
            .is_some_and(|(p, c)| *p == path && *c == contents)
        {
            continue;
        }
        if write_replace(&path, &contents) {
            written = Some((path, contents));
        }
    }
}

/// Writes `path` through a temporary file in the same directory; false when
/// the file could not be replaced (retried with the next dump).
fn write_replace(path: &str, contents: &str) -> bool {
    let tmp = format!("{path}.tmp");
    if std::fs::write(&tmp, contents).is_err() {
        return false;
    }
    if std::fs::rename(&tmp, path).is_ok() {
        return true;
    }
    let _ = std::fs::remove_file(&tmp);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_writer_replaces_the_file_with_the_latest_dump() {
        let dir = std::env::temp_dir().join(format!("zant-layout-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("layout.txt");
        let path = path.to_str().expect("utf-8 path");
        publish(path, "a\t1.0\t2.0\t3.0\t4.0\n".to_owned());
        publish(path, "b\t5.0\t6.0\t7.0\t8.0\n".to_owned());
        let mut contents = String::new();
        for _ in 0..200 {
            contents = std::fs::read_to_string(path).unwrap_or_default();
            if contents.starts_with('b') {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(contents, "b\t5.0\t6.0\t7.0\t8.0\n");
        assert!(!std::path::Path::new(&format!("{path}.tmp")).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
