//! Background page rendering with a tile cache (EmbedPDF's render plugin
//! and tiling layer: a low-resolution base image per page, sharp tiles for
//! the visible area at the current zoom).
//!
//! The UI thread asks for tiles every build (`tile`); missing tiles are
//! queued for one worker thread, which renders them, stores the image and
//! wakes the UI thread so the next build shows them. Requests not repeated
//! by later builds are dropped (scrolled away), and the cache evicts the
//! least recently used tiles over its memory budget.
//!
//! The renderer takes its wake callback at creation (`create(doc, wake)`):
//! the viewer passes `crate::ui::post_wake`, the tests a channel sender.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use crate::image::Image;
use crate::pdf::document::Document;

/// Device pixels per tile edge.
pub const TILE_SIZE: u32 = 1024;
/// Width in device pixels of the base image of a page.
pub const BASE_WIDTH: u32 = 360;
/// Tile cache memory budget.
const BUDGET: usize = 384 * 1024 * 1024;

/// Cache key: a tile of a page at a zoom level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub page: u32,
    /// Scaled page width in device pixels (identifies the zoom level).
    pub page_w: u32,
    pub tx: u16,
    pub ty: u16,
}

#[derive(Default)]
struct Entry {
    image: Option<Image>,
    /// Frame that last asked for it (LRU and request freshness).
    used: u64,
    queued: bool,
    failed: bool,
    bytes: usize,
}

struct State {
    stop: bool,
    entries: HashMap<Key, Entry>,
    /// Pending keys, newest last (the worker takes the newest first).
    queue: Vec<Key>,
    frame: u64,
    bytes: usize,
    budget: usize,
}

impl Default for State {
    fn default() -> State {
        State {
            stop: false,
            entries: HashMap::new(),
            queue: Vec::new(),
            frame: 1,
            bytes: 0,
            budget: BUDGET,
        }
    }
}

/// Parts shared with the worker thread.
struct Shared {
    doc: Arc<Document>,
    /// Called from the worker after a tile is stored.
    wake: Box<dyn Fn() + Send + Sync>,
    state: Mutex<State>,
    cond: Condvar,
}

fn lock(shared: &Shared) -> MutexGuard<'_, State> {
    shared
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Background renderer of one document. Dropping it stops and joins the
/// worker and frees the cached images.
pub struct Renderer {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Renderer {
    /// Starts the worker thread for `doc`. `wake` asks the UI thread to
    /// rebuild soon; it is called from the worker thread after every tile.
    pub fn create(doc: Arc<Document>, wake: impl Fn() + Send + Sync + 'static) -> Renderer {
        let shared = Arc::new(Shared {
            doc,
            wake: Box::new(wake),
            state: Mutex::new(State::default()),
            cond: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        // A failed spawn is tolerated: without a worker nothing renders, but
        // the cache stays usable.
        let thread = std::thread::Builder::new()
            .spawn(move || worker(&worker_shared))
            .ok();
        Renderer { shared, thread }
    }

    /// Starts a build: tiles not asked for from now on are stale.
    pub fn begin_frame(&self) {
        let mut state = lock(&self.shared);
        state.frame += 1;
        // drop queued work nobody asked for in the previous build
        let frame = state.frame;
        let State { entries, queue, .. } = &mut *state;
        let mut i: usize = 0;
        while i < queue.len() {
            let key = queue[i];
            let mut remove = false;
            match entries.get(&key) {
                None => remove = true,
                Some(entry) => {
                    if entry.used + 1 < frame {
                        if let Some(entry) = entries.get_mut(&key) {
                            entry.queued = false;
                        }
                        if entries.get(&key).is_some_and(|entry| entry.image.is_none()) {
                            entries.remove(&key);
                        }
                        remove = true;
                    }
                }
            }
            if remove {
                queue.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// The tile's image, or `None` while it renders (it is queued). The image
    /// is a retained handle: the caller may keep it after the cache changes.
    pub fn tile(&self, key: Key) -> Option<Image> {
        let mut state = lock(&self.shared);
        let frame = state.frame;
        let mut queued_now = false;
        {
            let entry = state.entries.entry(key).or_default();
            entry.used = frame;
            if let Some(image) = &entry.image {
                return Some(image.clone());
            }
            if !entry.queued && !entry.failed {
                entry.queued = true;
                queued_now = true;
            }
        }
        if queued_now {
            state.queue.push(key);
            self.shared.cond.notify_one();
        }
        None
    }

    /// A ready image of the page at any zoom (shown, scaled, while the
    /// sharp tiles render): the whole-page image closest to `page_w`.
    pub fn any_page_image(&self, page: u32, page_w: u32) -> Option<Image> {
        let mut state = lock(&self.shared);
        let frame = state.frame;
        let mut best: Option<&Image> = None;
        let mut best_diff: u32 = u32::MAX;
        for (k, entry) in state.entries.iter_mut() {
            if k.page != page || k.tx != 0 || k.ty != 0 {
                continue;
            }
            let Some(image) = &entry.image else { continue };
            // whole-page images only (single tile pages)
            if k.page_w > TILE_SIZE {
                continue;
            }
            let diff = k.page_w.abs_diff(page_w);
            if diff < best_diff {
                best_diff = diff;
                best = Some(image);
                entry.used = frame;
            }
        }
        best.cloned()
    }

    /// Scaled page height in device pixels for a page width (tiles grid).
    pub fn page_height_for(&self, page: u32, page_w: u32) -> u32 {
        page_height_px(&self.shared.doc, page, page_w)
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        {
            let mut state = lock(&self.shared);
            state.stop = true;
            self.shared.cond.notify_one();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn page_height_px(doc: &Document, page: u32, page_w: u32) -> u32 {
    let size = doc.page_size(page as usize);
    let height = (page_w as f32 * size.height / size.width).round() as u32;
    height.max(1)
}

fn worker(shared: &Arc<Shared>) {
    loop {
        let key = {
            let mut state = lock(shared);
            while state.queue.is_empty() && !state.stop {
                state = shared
                    .cond
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            if state.stop {
                return;
            }
            state.queue.pop().expect("queued tile")
        };
        let page_h = page_height_px(&shared.doc, key.page, key.page_w);
        let x = u32::from(key.tx) * TILE_SIZE;
        let y = u32::from(key.ty) * TILE_SIZE;
        let w = TILE_SIZE.min(key.page_w.saturating_sub(x));
        let h = TILE_SIZE.min(page_h.saturating_sub(y));
        let rendered = if w > 0 && h > 0 {
            shared
                .doc
                .render_tile(key.page as usize, key.page_w, page_h, x, y, w, h)
        } else {
            None
        };
        {
            let mut state = lock(shared);
            let state = &mut *state;
            if let Some(entry) = state.entries.get_mut(&key) {
                entry.queued = false;
                if let Some(image) = rendered {
                    entry.bytes = image.width as usize * image.height as usize * 4;
                    state.bytes += entry.bytes;
                    entry.image = Some(image);
                } else {
                    entry.failed = true;
                }
            }
            evict(state);
        }
        (shared.wake)();
    }
}

/// Frees least recently used tiles while over budget (keeps tiles of the
/// current frame). Called with the mutex held.
fn evict(state: &mut State) {
    while state.bytes > state.budget {
        let mut oldest: Option<Key> = None;
        let mut oldest_used: u64 = u64::MAX;
        for (key, entry) in &state.entries {
            if entry.image.is_none() || entry.used + 1 >= state.frame {
                continue;
            }
            if entry.used < oldest_used {
                oldest_used = entry.used;
                oldest = Some(*key);
            }
        }
        let Some(key) = oldest else { return };
        if let Some(entry) = state.entries.remove(&key) {
            state.bytes -= entry.bytes;
        }
    }
}
