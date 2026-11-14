//! Port of `src/pdf/document.zig` — WP-3.8.
//!
//! An open PDF document: page sizes, text geometry, links and outline,
//! backed by PDFKit (macOS) or PDFium (Windows).
//!
//! Text, links and the outline are loaded lazily and cached; in Zig they are
//! for the UI thread while `renderTile` is thread-safe (the viewer's render
//! worker calls it).
//!
//! Threading choice: `Document` is `Send + Sync`. The native handle is
//! immutable after `open` and the C header documents every call as
//! thread-safe (the bridge serialises per backend: PDFKit with an NSLock,
//! PDFium with its global lock), so `render_tile` needs no lock of ours.
//! The lazy caches are a `Mutex<Cache>` only because Zig's `*Document` +
//! `[]?PageText` lazy mutation becomes `&self` here; the mutex is not needed
//! for the native calls themselves and never wraps `render_tile`.
//!
//! Zig `!Document` errors are [`OpenError`]; OOM disappears (Rust aborts).
//! Zig returns borrowed slices into an arena; Rust returns owned `String`s
//! and `Arc<[T]>` snapshots so the caches can sit behind a mutex.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::image::Image;
use crate::pdf::native;
use crate::pdf::text::{self, PageText};

/// Displayed page size in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// A link area on a page.
#[derive(Clone, Debug, Default)]
pub struct Link {
    /// Area on the page (points, top-left origin).
    pub rect: text::Rect,
    /// Internal target page (`None` for external links).
    pub dest_page: Option<usize>,
    /// Target y on `dest_page` in points from the top (`None`: page top).
    pub dest_y: Option<f32>,
    /// External URL (`""` for internal links).
    pub uri: String,
}

/// One bookmark, flattened depth-first.
#[derive(Clone, Debug)]
pub struct OutlineEntry {
    pub depth: u32,
    pub page: Option<usize>,
    pub title: String,
}

/// Errors of [`Document::open`] (Zig: `error.FileNotFound`,
/// `error.PasswordRequired`, `error.OutOfMemory`).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OpenError {
    #[error("PDF file not found or cannot be opened")]
    FileNotFound,
    #[error("password required")]
    PasswordRequired,
}

/// Lazy per-page caches (Zig: `texts`, `links`, `outline_items`, `strings`).
struct Cache {
    texts: Vec<Option<Arc<PageText>>>,
    links: Vec<Option<Arc<[Link]>>>,
    outline: Option<Arc<[OutlineEntry]>>,
}

/// An open PDF document. Closed on drop (Zig: `Document.close`).
pub struct Document {
    handle: native::Doc,
    sizes: Vec<Size>,
    cache: Mutex<Cache>,
}

fn lock(cache: &Mutex<Cache>) -> MutexGuard<'_, Cache> {
    // A panic while a cache update is in flight cannot corrupt the handle;
    // keep the cache usable like Zig's plain (lock-free) struct.
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Document {
    /// Opens a PDF file. `password` unlocks encrypted documents.
    pub fn open(path: &str, password: Option<&str>) -> Result<Document, OpenError> {
        let mut locked = false;
        let Some(handle) = native::Doc::open(path, password, &mut locked) else {
            return Err(if locked {
                OpenError::PasswordRequired
            } else {
                OpenError::FileNotFound
            });
        };
        let count = handle.count().max(0) as usize;
        let mut sizes: Vec<Size> = Vec::with_capacity(count);
        for page in 0..count {
            let (mut width, mut height) = handle.size(page as i32).unwrap_or((612.0, 792.0));
            if width <= 0.0 || height <= 0.0 {
                width = 612.0;
                height = 792.0;
            }
            sizes.push(Size { width, height });
        }
        let mut texts: Vec<Option<Arc<PageText>>> = Vec::new();
        texts.resize_with(count, || None);
        let mut links: Vec<Option<Arc<[Link]>>> = Vec::new();
        links.resize_with(count, || None);
        Ok(Document {
            handle,
            sizes,
            cache: Mutex::new(Cache {
                texts,
                links,
                outline: None,
            }),
        })
    }

    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }

    /// Displayed page size in points (rotation applied).
    pub fn page_size(&self, page: usize) -> Size {
        self.sizes[page]
    }

    /// Text geometry of a page (loaded on first use; `None` for a page index
    /// past the end, an empty `PageText` when the page has no text layer or
    /// cannot be read).
    pub fn page_text(&self, page: usize) -> Option<Arc<PageText>> {
        if page >= self.sizes.len() {
            return None;
        }
        let mut cache = lock(&self.cache);
        if cache.texts[page].is_none() {
            cache.texts[page] = Some(Arc::new(self.load_text(page)));
        }
        cache.texts[page].clone()
    }

    fn load_text(&self, page: usize) -> PageText {
        let raw = self.handle.glyphs(page as i32);
        if raw.is_empty() {
            return PageText::default();
        }
        PageText::build(&raw)
    }

    /// Links of a page (loaded on first use).
    pub fn page_links(&self, page: usize) -> Arc<[Link]> {
        if page >= self.sizes.len() {
            return Arc::from(Vec::new());
        }
        let mut cache = lock(&self.cache);
        if cache.links[page].is_none() {
            cache.links[page] = Some(Arc::from(self.load_links(page)));
        }
        cache.links[page].clone().expect("link cache filled above")
    }

    fn load_links(&self, page: usize) -> Vec<Link> {
        let mut out: Vec<Link> = Vec::new();
        for link in self.handle.links(page as i32) {
            let uri = native::utf8_until_nul(&link.uri);
            out.push(Link {
                rect: text::Rect {
                    x: link.x,
                    y: link.y,
                    width: link.width,
                    height: link.height,
                },
                dest_page: if link.dest_page >= 0 && (link.dest_page as usize) < self.sizes.len() {
                    Some(link.dest_page as usize)
                } else {
                    None
                },
                dest_y: if link.dest_y >= 0.0 {
                    Some(link.dest_y)
                } else {
                    None
                },
                uri: if uri.is_empty() {
                    String::new()
                } else {
                    uri.into_owned()
                },
            });
        }
        out
    }

    /// Bookmarks, flattened depth-first (loaded on first use).
    pub fn outline(&self) -> Arc<[OutlineEntry]> {
        let mut cache = lock(&self.cache);
        if cache.outline.is_none() {
            cache.outline = Some(Arc::from(self.load_outline()));
        }
        cache.outline.clone().expect("outline cache filled above")
    }

    fn load_outline(&self) -> Vec<OutlineEntry> {
        let mut out: Vec<OutlineEntry> = Vec::new();
        for item in self.handle.outline() {
            out.push(OutlineEntry {
                depth: item.depth.max(0) as u32,
                page: if item.page >= 0 && (item.page as usize) < self.sizes.len() {
                    Some(item.page as usize)
                } else {
                    None
                },
                title: native::utf8_until_nul(&item.title).into_owned(),
            });
        }
        out
    }

    /// UTF-8 text of a page.
    pub fn page_text_string(&self, page: usize) -> String {
        let mut out = String::new();
        if let Some(text) = self.page_text(page)
            && text.count() > 0
        {
            text.append_text(&mut out, 0, text.count() - 1);
        }
        out
    }

    /// Renders region (x, y, w, h) of the page scaled to `page_w` × `page_h`
    /// pixels. Thread-safe.
    #[allow(clippy::too_many_arguments)]
    pub fn render_tile(
        &self,
        page: usize,
        page_w: u32,
        page_h: u32,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Option<Image> {
        if w == 0 || h == 0 {
            return None;
        }
        let stride = w * 4;
        let mut pixels = vec![0u8; stride as usize * h as usize];
        if !self.handle.render(
            page as i32,
            page_w as i32,
            page_h as i32,
            x as i32,
            y as i32,
            w as i32,
            h as i32,
            &mut pixels,
            stride as i32,
        ) {
            return None;
        }
        Image::from_pixels(&pixels, w, h, stride)
    }
}
