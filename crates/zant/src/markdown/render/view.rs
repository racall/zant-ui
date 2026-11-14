//! The Streamdown component: renders Markdown with block-level memoisation.
//!
//! The view state of a cached block lives in `ui::local_state` keyed by the
//! block's UI key, and the memo builder closure captures `Rc`s. The parsed
//! document is cloned only when the
//! block actually changes; unchanged blocks only build a fresh closure around
//! the same `Rc<Document>` (and the runtime skips even that builder).

use std::cell::Cell;
use std::rc::Rc;

use crate::Str;
use crate::core::hash::{self, Wyhash};
use crate::layout::flex;
use crate::markdown::ast;
use crate::markdown::cache::{self, Cache};
use crate::markdown::parser;
use crate::markdown::remend;
use crate::markdown::render::blocks;
use crate::markdown::render::context::Ctx;
use crate::markdown::render::options::{Caret, Options};
use crate::ui;
use crate::ui::options::{BoxOptions, MemoOptions};

/// Renders a parsed document without caching (everything in the frame).
pub fn document(doc: &ast::Document, options: Options) -> ui::Node {
    let options = Rc::new(options);
    let caret = options.caret != Caret::None && options.is_animating;
    let built = build_document(doc, &options, caret, &options.key_prefix);
    if !options.selectable {
        return built;
    }
    ui::column(BoxOptions {
        width_percent: Some(100.0),
        align_items: flex::Align::Stretch,
        selectable: true,
        children: vec![built],
        ..Default::default()
    })
}

fn build_document(doc: &ast::Document, options: &Rc<Options>, caret: bool, key: &str) -> ui::Node {
    let mut ctx = Ctx::init(options);
    ctx.caret = caret;
    ctx.key = key.to_owned();
    let gap = options.theme.block_gap * options.font_size;
    let body = blocks::sequence(&ctx, &doc.blocks, gap);
    if doc.footnotes.is_empty() {
        return body;
    }
    ui::column(BoxOptions {
        key: Some(Str::copy(key)),
        width_percent: Some(100.0),
        gap,
        align_items: flex::Align::Stretch,
        children: vec![body, blocks::footnotes(&ctx, &doc.footnotes)],
        ..Default::default()
    })
}

#[derive(Clone)]
pub struct Props {
    pub render: Options,
    pub mode: cache::Mode,
    pub parse_incomplete: bool,
    pub parse: parser::Options,
    pub remend: remend::Options,
}

impl Default for Props {
    fn default() -> Self {
        Props {
            render: Options::default(),
            mode: cache::Mode::Static,
            parse_incomplete: true,
            parse: parser::Options::default(),
            remend: remend::Options::default(),
        }
    }
}

/// Per-block view state kept in `ui::local_state`. `hash` is the cache entry
/// hash the stored document belongs to.
#[derive(Default)]
struct BlockView {
    doc: Option<Rc<ast::Document>>,
    hash: u64,
}

/// Local-state id of a block's view: the same identity as its UI key
/// (`{key_prefix}-e{id}`), mixed into a `u64`.
fn view_slot_id(key_prefix: &str, entry_id: u64) -> u64 {
    let mut hasher = Wyhash::init(0x6d645f76);
    hasher.update(key_prefix.as_bytes());
    hasher.update(&entry_id.to_le_bytes());
    hasher.finish()
}

fn hash_images(h: &mut Wyhash, options: &Options, list: &[ast::Block]) {
    for block in list {
        match block {
            ast::Block::Image(image) => {
                let loader = options
                    .image_loader
                    .as_ref()
                    .expect("only called with an image loader");
                let id: u64 = loader(&image.url).map_or(0, |decoded| decoded.id);
                h.update(&id.to_le_bytes());
            }
            ast::Block::Quote(quote) => hash_images(h, options, quote),
            ast::Block::Callout(callout) => hash_images(h, options, &callout.blocks),
            ast::Block::List(list) => {
                for item in &list.items {
                    hash_images(h, options, &item.blocks);
                }
            }
            _ => {}
        }
    }
}

/// UI key of a block (stable while its cache entry lives).
pub fn block_key(key_prefix: &str, entry: &cache::Entry) -> String {
    format!("{key_prefix}-e{}", entry.id)
}

/// Key of the document's `n`-th top-level heading (0-based, in source order)
/// as rendered by [`streamdown`], for `ui::scrollIntoView` — e.g. an outline
/// jump. Works even when the document is one block (footnotes).
pub fn heading_key(cache: &Cache, key_prefix: &str, n: usize) -> Option<String> {
    let mut seen: usize = 0;
    for entry in &cache.entries {
        for (i, block) in entry.doc.blocks.iter().enumerate() {
            if !matches!(block, ast::Block::Heading(_)) {
                continue;
            }
            if seen == n {
                return Some(format!("{key_prefix}-e{}-{i}", entry.id));
            }
            seen += 1;
        }
    }
    None
}

/// Key of the rendered block containing byte `offset` of the source, for
/// `ui::scrollIntoView` (e.g. jumping to a heading in the preview). The cache
/// must be the one passed to [`streamdown`] with the same `key_prefix`.
pub fn block_key_at_offset(cache: &Cache, key_prefix: &str, offset: usize) -> Option<String> {
    let index = cache.entry_at_offset(offset)?;
    Some(block_key(key_prefix, &cache.entries[index]))
}

thread_local! {
    /// Test hook: number of block memo builders that actually ran (a skipped
    /// `ui::memo` does not call its builder — see `markdown_render_test.rs`).
    static MEMO_BUILDS: Cell<usize> = const { Cell::new(0) };
}

/// Test hook: how many block builders ran since the last
/// [`reset_memo_build_count`] (memoised blocks that the runtime skipped do not
/// count).
#[doc(hidden)]
pub fn memo_build_count() -> usize {
    MEMO_BUILDS.with(Cell::get)
}

/// Test hook: resets [`memo_build_count`].
#[doc(hidden)]
pub fn reset_memo_build_count() {
    MEMO_BUILDS.with(|count| count.set(0));
}

/// Streamdown: heals (streaming), splits, parses changed blocks only and
/// rebuilds the UI of changed blocks only. `cache` is owned by the caller.
pub fn streamdown(cache: &mut Cache, source: &str, props: Props) -> ui::Node {
    let options = Rc::new(props.render);
    let options_hash = options.hash();
    let animating = options.is_animating
        && options.caret != Caret::None
        && props.mode == cache::Mode::Streaming;
    let entries = cache.update(
        source,
        cache::Options {
            mode: props.mode,
            parse_incomplete: props.parse_incomplete,
            parse: props.parse,
            remend: props.remend,
        },
    );
    let mut children: Vec<ui::Node> = Vec::with_capacity(entries.len());
    for (i, entry) in entries.iter().enumerate() {
        let caret = animating && i + 1 == entries.len();
        let key = block_key(&options.key_prefix, entry);
        let slot = ui::local_state(
            view_slot_id(&options.key_prefix, entry.id),
            BlockView::default,
        );
        let doc = {
            let mut view = slot.borrow_mut();
            if view.hash != entry.hash {
                view.doc = Some(Rc::new(entry.doc.clone()));
                view.hash = entry.hash;
            }
            view.doc
                .clone()
                .expect("a view slot always holds the document of its hash")
        };
        let mut hasher = hash::Wyhash::init(entry.hash);
        hasher.update(&options_hash.to_le_bytes());
        hasher.update(&[caret as u8]);
        // rebuild a block when one of its images finishes loading
        if options.image_loader.is_some() {
            hash_images(&mut hasher, &options, &entry.doc.blocks);
        }
        let memo_key = Str::copy(&key);
        let build_options = Rc::clone(&options);
        let build_key = key.clone();
        children.push(ui::memo(
            MemoOptions {
                key: Some(memo_key),
                hash: hasher.finish(),
            },
            move || {
                MEMO_BUILDS.with(|count| count.set(count.get() + 1));
                build_document(&doc, &build_options, caret, &build_key)
            },
        ));
    }
    if children.is_empty() && animating {
        let empty = ast::Document::default();
        let only = vec![document(&empty, (*options).clone())];
        return ui::column(BoxOptions {
            key: Some(Str::copy(&options.key_prefix)),
            width_percent: Some(100.0),
            align_items: flex::Align::Stretch,
            children: only,
            ..Default::default()
        });
    }
    ui::column(BoxOptions {
        key: Some(Str::copy(&options.key_prefix)),
        width_percent: Some(100.0),
        gap: options.theme.block_gap * options.font_size,
        align_items: flex::Align::Stretch,
        selectable: options.selectable,
        children,
        ..Default::default()
    })
}
