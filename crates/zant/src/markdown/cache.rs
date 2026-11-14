//! Port of `src/markdown/cache.zig` — WP-3.2.
//!
//! Streamdown's processing pipeline with block-level memoisation:
//!
//! ```text
//! source ─(streaming? remend)→ healed ─(parse-blocks)→ block strings
//!        ─(per block, cached by content)→ parsed documents
//! ```
//!
//! A block whose text did not change keeps its parsed document (and its
//! hash, which lets the view skip rebuilding its UI via `ui.memo`). While a
//! response streams in, only the last block or two are parsed again per tick.
//! The cache is owned by the caller (no global state).
//!
//! The Zig `Entry` also carries view-only scratch (`view_slot`, `view_arena`);
//! the Rust view keeps that state itself, so `Entry` only holds the parsed
//! block and its identity here.

use std::borrow::Cow;

use crate::core::hash::{self, Wyhash};
use crate::markdown::ast;
use crate::markdown::blocks;
use crate::markdown::parser;
use crate::markdown::remend;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    /// Complete documents (notes, files).
    #[default]
    Static,
    /// Content arriving incrementally (AI responses): incomplete syntax is
    /// healed with remend before parsing.
    Streaming,
}

#[derive(Clone)]
pub struct Options {
    pub mode: Mode,
    /// Streamdown `parseIncompleteMarkdown` (only used in streaming mode).
    pub parse_incomplete: bool,
    pub remend: remend::Options,
    pub parse: parser::Options,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Static,
            parse_incomplete: true,
            remend: remend::Options::default(),
            parse: parser::Options::default(),
        }
    }
}

pub struct Entry {
    /// Stable identity for the entry's lifetime (used for UI keys, so an
    /// unchanged block keeps its nodes when blocks are inserted before it).
    pub id: u64,
    pub raw: String,
    pub doc: ast::Document,
    /// Hash of `raw` and the parse options.
    pub hash: u64,
}

pub struct Cache {
    splitter: blocks::Splitter,
    pub entries: Vec<Entry>,
    options_hash: u64,
    /// Statistics of the last update (tests / profiling).
    pub parsed_last: usize,
    next_id: u64,
}

impl Default for Cache {
    fn default() -> Self {
        Cache::new()
    }
}

impl Cache {
    pub fn new() -> Cache {
        Cache {
            splitter: blocks::Splitter::new(),
            entries: Vec::new(),
            options_hash: 0,
            parsed_last: 0,
            next_id: 1,
        }
    }

    /// Brings the cache up to date with `source`; returns the block entries
    /// (valid until the next update).
    pub fn update(&mut self, source: &str, options: Options) -> &[Entry] {
        let healed: Cow<'_, str> = if options.mode == Mode::Streaming && options.parse_incomplete {
            Cow::Owned(remend::remend(source, &options.remend))
        } else {
            Cow::Borrowed(source)
        };

        let options_hash = hash::hash(1, &options.parse);
        if options_hash != self.options_hash {
            self.entries.clear();
            self.options_hash = options_hash;
        }

        self.splitter.set_options(options.parse);
        let raw_blocks = self.splitter.split(healed.as_ref());
        // keep the common prefix and suffix; only the middle is re-parsed
        let old_len = self.entries.len();
        let mut p: usize = 0;
        while p < old_len && p < raw_blocks.len() && self.entries[p].raw == raw_blocks[p] {
            p += 1;
        }
        let mut s: usize = 0;
        while s < old_len - p
            && s < raw_blocks.len() - p
            && self.entries[old_len - 1 - s].raw == raw_blocks[raw_blocks.len() - 1 - s]
        {
            s += 1;
        }

        let middle_len = raw_blocks.len() - s - p;
        let mut fresh: Vec<Entry> = Vec::with_capacity(middle_len);
        for raw in &raw_blocks[p..raw_blocks.len() - s] {
            fresh.push(self.create_entry(raw, options_hash, options.parse));
        }

        // changed blocks keep the identity (UI key) of the block they
        // replace, position by position, like Streamdown's index keys: the
        // block that grows while streaming keeps its nodes, so unchanged
        // lines / cells keep their shaped text instead of being rebuilt
        let removed = old_len - s - p;
        let keep = removed.min(fresh.len());
        for (fresh_entry, old_entry) in fresh.iter_mut().zip(self.entries[p..p + keep].iter()) {
            fresh_entry.id = old_entry.id;
        }
        // splice: old[p .. old.len - s] → fresh
        self.entries.drain(p..p + removed);
        self.entries.splice(p..p, fresh);
        self.parsed_last = middle_len;
        &self.entries
    }

    fn create_entry(&mut self, raw: &str, options_hash: u64, parse: parser::Options) -> Entry {
        let mut h = Wyhash::init(options_hash);
        h.update(raw.as_bytes());
        let entry = Entry {
            id: self.next_id,
            raw: raw.to_string(),
            doc: parser::parse(raw, parse),
            hash: h.finish(),
        };
        self.next_id += 1;
        entry
    }

    /// Index of the entry containing byte `offset` of the (healed) source.
    pub fn entry_at_offset(&self, offset: usize) -> Option<usize> {
        let mut start: usize = 0;
        for (i, e) in self.entries.iter().enumerate() {
            let end = start + e.raw.len();
            if offset < end || (i + 1 == self.entries.len() && offset == end) {
                return Some(i);
            }
            start = end;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::ast::{Block, Span};

    fn paragraph_of(block: &Block) -> &[Span] {
        match block {
            Block::Paragraph(spans) => spans,
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    fn streaming() -> Options {
        Options {
            mode: Mode::Streaming,
            ..Options::default()
        }
    }

    #[test]
    fn streaming_reparses_only_the_tail() {
        let mut cache = Cache::new();
        let _ = cache.update("# Title\n\nFirst paragraph.\n\nSecond **bo", streaming());
        let (len, bold) = {
            let entries = cache.update(
                "# Title\n\nFirst paragraph.\n\nSecond **bold** text\n\nThird",
                streaming(),
            );
            // healed incomplete bold rendered as bold
            let bold = paragraph_of(&entries[2].doc.blocks[0])
                .iter()
                .any(|s| s.style.bold);
            (entries.len(), bold)
        };
        assert_eq!(len, 4);
        assert!(cache.parsed_last <= 2);
        assert!(bold);
    }

    #[test]
    fn a_block_growing_while_streaming_keeps_its_id() {
        let mut cache = Cache::new();
        let id = cache.update("# T\n\nPara one", streaming())[1].id;
        let grown_id = cache.update("# T\n\nPara one grows", streaming())[1].id;
        assert_eq!(id, grown_id);
        let (first_id, second_id) = {
            let c = cache.update("# T\n\nPara one grows\n\nNext", streaming());
            (c[1].id, c[2].id)
        };
        assert_eq!(id, first_id);
        assert_ne!(id, second_id);
    }

    #[test]
    fn inserting_a_block_in_the_middle_parses_only_that_block() {
        let mut cache = Cache::new();
        let ids = {
            let before = cache.update("# A\n\nOne.\n\nTwo.\n\nThree.", Options::default());
            assert_eq!(before.len(), 4);
            [before[0].id, before[1].id, before[2].id, before[3].id]
        };
        let after_ids = {
            let after = cache.update(
                "# A\n\nOne.\n\nInserted.\n\nTwo.\n\nThree.",
                Options::default(),
            );
            assert_eq!(after.len(), 5);
            [after[0].id, after[1].id, after[3].id, after[4].id]
        };
        assert_eq!(cache.parsed_last, 1);
        assert_eq!(ids[0], after_ids[0]);
        assert_eq!(ids[1], after_ids[1]);
        assert_eq!(ids[2], after_ids[2]);
        assert_eq!(ids[3], after_ids[3]);
        // deleting it again re-parses nothing
        let _ = cache.update("# A\n\nOne.\n\nTwo.\n\nThree.", Options::default());
        assert_eq!(cache.parsed_last, 0);
    }
}
