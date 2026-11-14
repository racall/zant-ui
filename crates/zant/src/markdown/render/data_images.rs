//! Port of `src/markdown/render/data_images.zig` — WP-3.3.
//!
//! Decoded `data:image/…;base64,…` URIs, cached by content hash (UI thread).
//! Zig kept a plain module-level map; Rust uses a thread-local map of retained
//! `Image` handles.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::core::hash::Wyhash;
use crate::image::Image;

const MAX_ENTRIES: usize = 64;

thread_local! {
    static ENTRIES: RefCell<HashMap<u64, Image>> = RefCell::new(HashMap::new());
}

pub fn get(uri: &str) -> Option<Image> {
    let mut hasher = Wyhash::init(0x64617461);
    hasher.update(uri.as_bytes());
    let key = hasher.finish();
    ENTRIES.with(|entries| {
        let mut entries = entries.borrow_mut();
        if let Some(image) = entries.get(&key) {
            return Some(image.retain());
        }
        let image = Image::from_data_uri(uri)?;
        if entries.len() >= MAX_ENTRIES {
            // drop one: nodes showing it keep their own reference
            if let Some(evict) = entries.keys().next().copied() {
                entries.remove(&evict);
            }
        }
        entries.insert(key, image.retain());
        Some(image)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_uris_are_decoded_and_cached() {
        // 1x1 transparent PNG.
        let uri = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let first = get(uri).expect("valid data URI");
        assert_eq!(first.width, 1);
        assert_eq!(first.height, 1);
        let second = get(uri).expect("cached");
        assert_eq!(first.id, second.id, "the cache reuses the decoded image");
        assert!(get("https://example.com/a.png").is_none());
    }
}
