//! `markdown_bench.tsx` vs the hand-held Streamdown cache of
//! `bench_markdown.zig`: the same streamed text and full raster frames through
//! the generated `<Markdown>` component (`ui::markdown`, which owns its cache)
//! and through a hand-held `markdown::Cache`. Source assignment is outside the
//! timed render on both sides.
//!
//! The Zig benchmark grew the TSX root component's `source` state directly;
//! Rust codegen keeps root state private, so the driver owns the streamed text
//! and passes it to the generated `MarkdownView` component
//! (`tsx_views/MarkdownView.tsx`, same widget and options).
#![forbid(unsafe_code)]

use zant::Str;
use zant::ui::{self, App};

use super::controls::markdown::Manual;
use super::{Harness, Stats, markdown_case};

use markdown_case::markdown_view::{MarkdownView, MarkdownViewProps_};

/// The component instance id: `0` makes the generated `<Markdown>` instance
/// id `tsx::site_id(0, "Markdown", 1)`, the same `md-<hex>` node key prefix as
/// the hand-held control (and the Zig benchmark).
const MARKDOWN_VIEW_ID: u64 = 0;

/// The streamed reply of Zig `bench_markdown.run` (48 copies of one paragraph).
fn stream_input() -> String {
    const PARAGRAPH: &str = "## A streamed reply\n\nStable **paragraphs** stay cached while the tail grows. \
Inline `code` and [links](https://zant.dev) keep their nodes.\n\n\
```zig\nconst answer = 42;\n```\n\n";
    PARAGRAPH.repeat(48)
}

/// Driver side of the markdown scenario: it owns the text the Zig benchmark
/// wrote into the root state.
trait Source {
    fn set_source(&mut self, source: String);
}

/// The TSX side's app: owns the streamed text, renders the generated
/// `<Markdown>` view.
struct TsxMarkdown {
    source: String,
}

impl TsxMarkdown {
    fn new() -> TsxMarkdown {
        TsxMarkdown {
            source: String::new(),
        }
    }
}

impl App for TsxMarkdown {
    fn render(&mut self) -> ui::Node {
        MarkdownView::build(
            MarkdownViewProps_ {
                source: Str::copy(&self.source),
            },
            MARKDOWN_VIEW_ID,
        )
    }
}

impl Source for TsxMarkdown {
    fn set_source(&mut self, source: String) {
        self.source = source;
    }
}

impl Source for Manual {
    fn set_source(&mut self, source: String) {
        self.source = source;
    }
}

/// Zig `stream`: 24 bytes of source per timed frame.
fn run<A: App + Source + 'static>(app: A) -> Vec<Stats> {
    let input = stream_input();
    let mut harness = Harness::new(app);
    let mut stats = Stats::new();
    let mut end = 24usize;
    while end < input.len() + 24 {
        let upto = usize::min(end, input.len());
        harness.app_mut::<A>().set_source(input[..upto].to_string());
        harness.invalidate();
        stats.add(harness.frame());
        end += 24;
    }
    vec![stats]
}

/// The TSX side: the generated `<Markdown>` view fed by the driver.
pub fn tsx() -> Vec<Stats> {
    run(TsxMarkdown::new())
}

/// The hand-written control: `bench_markdown.zig`'s `Manual` struct ported.
pub fn manual() -> Vec<Stats> {
    run(Manual::new())
}
