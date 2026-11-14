//! Headless render smoke tests for the markdown renderer (WP-3.3).
//!
//! Builds the node tree of the repository README and of a 10 000-line document
//! without a window, checks node kinds / keys, and drives a real `Runtime`
//! rebuild to prove that block memoisation skips unchanged blocks.

use zant::core::node::NodeKind;
use zant::markdown::cache::Cache;
use zant::markdown::parser::{self, Options as ParseOptions};
use zant::markdown::render::options::Options as RenderOptions;
use zant::markdown::render::view::{self, Props as ViewProps};
use zant::runtime::Runtime;
use zant::ui;
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::options::BoxOptions;

const README: &str = include_str!("../../../README.md");

/// Counts nodes of a declarative tree by kind.
fn count_kinds(node: &Node, counts: &mut [usize; 10]) {
    counts[node.element().kind as usize] += 1;
    for child in &node.element().children {
        count_kinds(child, counts);
    }
}

/// Collects every key of a declarative tree.
fn collect_keys(node: &Node, keys: &mut Vec<String>) {
    if let Some(key) = node.element().key.as_deref() {
        keys.push(key.to_owned());
    }
    for child in &node.element().children {
        collect_keys(child, keys);
    }
}

#[test]
fn readme_renders_a_complete_node_tree() {
    let doc = parser::parse(README, ParseOptions::default());
    assert!(
        doc.blocks.len() > 5,
        "the README should have several top-level blocks"
    );
    ui::begin_frame();
    let node = view::document(&doc, RenderOptions::default());
    ui::end_frame();

    // `document` wraps the body in a selectable full-width column.
    assert_eq!(node.element().kind, NodeKind::Container);
    assert!(node.element().visual.selectable);
    assert_eq!(node.element().children.len(), 1);
    let body = &node.element().children[0];
    assert_eq!(body.element().kind, NodeKind::Container);
    assert_eq!(body.element().key.as_deref(), Some("md"));
    // one child per top-level block; non-paragraph blocks carry their position
    // key (`md-<index>`), paragraphs are keyless rich text (like Zig)
    assert_eq!(body.element().children.len(), doc.blocks.len());
    let keyed = body
        .element()
        .children
        .iter()
        .filter(|child| child.element().key.is_some())
        .count();
    assert!(keyed > 0);
    for (i, child) in body.element().children.iter().enumerate() {
        if let Some(key) = child.element().key.as_deref() {
            assert_eq!(key, format!("md-{i}"));
        }
    }

    let mut counts = [0usize; 10];
    count_kinds(&node, &mut counts);
    assert!(counts[NodeKind::Container as usize] > 0);
    assert!(counts[NodeKind::Text as usize] > 0);
    assert!(counts[NodeKind::RichText as usize] > 0);
    assert!(counts[NodeKind::Button as usize] > 0, "code copy buttons");

    let mut keys = Vec::new();
    collect_keys(&node, &mut keys);
    assert!(keys.iter().any(|key| key.starts_with("md-")));
    assert!(keys.iter().all(|key| key != "Markdown render failed"));
}

#[test]
fn ten_thousand_line_document_renders_with_stable_keys() {
    let mut source = String::with_capacity(10_000 * 24);
    for i in 0..2_500 {
        source.push_str(&format!("## Section {i}\n\nParagraph number {i}.\n\n"));
    }
    assert_eq!(source.lines().count(), 10_000);
    let doc = parser::parse(&source, ParseOptions::default());
    assert_eq!(doc.blocks.len(), 5_000);

    ui::begin_frame();
    let node = view::document(&doc, RenderOptions::default());
    ui::end_frame();
    let body = &node.element().children[0];
    assert_eq!(body.element().children.len(), 5_000);
    assert_eq!(
        body.element().children[0].element().key.as_deref(),
        Some("md-0")
    );
    // the last block is a paragraph (keyless rich text); the heading before it
    // keeps its position key
    assert_eq!(
        body.element().children[4_998].element().key.as_deref(),
        Some("md-4998")
    );
    assert!(body.element().children[4_999].element().key.is_none());
    // headings and paragraphs alternate
    assert_eq!(
        body.element().children[0].element().kind,
        NodeKind::Container
    );
    assert_eq!(
        body.element().children[1].element().kind,
        NodeKind::RichText
    );
}

struct MemoDoc {
    cache: Cache,
    source: String,
}

impl App for MemoDoc {
    fn render(&mut self) -> Node {
        ui::column(BoxOptions {
            children: vec![view::streamdown(
                &mut self.cache,
                &self.source,
                ViewProps::default(),
            )],
            ..Default::default()
        })
    }
}

#[test]
fn memo_rebuild_skips_unchanged_blocks() {
    let source = "# A\n\nOne.\n\n## B\n\nTwo.".to_owned();
    let mut runtime = Runtime::init(
        MemoDoc {
            cache: Cache::new(),
            source,
        },
        RunOptions::default(),
    );
    view::reset_memo_build_count();
    runtime.rebuild_for_test();
    let block_count = runtime.app_state_mut::<MemoDoc>().cache.entries.len();
    assert_eq!(block_count, 4);
    assert_eq!(
        view::memo_build_count(),
        block_count,
        "the first build renders every block"
    );

    // same source: every memo hash is unchanged, no block builder runs
    view::reset_memo_build_count();
    runtime.needs_build = true;
    runtime.rebuild_for_test();
    assert_eq!(view::memo_build_count(), 0, "unchanged blocks are skipped");

    // inserting a block re-renders only that block (the unchanged suffix keeps
    // its cache entries and memo hashes)
    let source = runtime.app_state_mut::<MemoDoc>().source.clone();
    runtime.app_state_mut::<MemoDoc>().source = format!("Inserted.\n\n{source}");
    view::reset_memo_build_count();
    runtime.needs_build = true;
    runtime.rebuild_for_test();
    assert_eq!(
        view::memo_build_count(),
        1,
        "only the changed block rebuilds"
    );
}

#[test]
fn streaming_source_grows_one_block_at_a_time() {
    let mut cache = Cache::new();
    ui::begin_frame();
    let node = view::streamdown(
        &mut cache,
        "# Title\n\nfirst li",
        ViewProps {
            mode: zant::markdown::cache::Mode::Streaming,
            ..Default::default()
        },
    );
    ui::end_frame();
    assert_eq!(node.element().children.len(), 2);
    assert_eq!(
        node.element().children[0].element().key.as_deref(),
        Some("md-e1")
    );
    ui::begin_frame();
    let grown = view::streamdown(
        &mut cache,
        "# Title\n\nfirst line\n\nnext para",
        ViewProps {
            mode: zant::markdown::cache::Mode::Streaming,
            ..Default::default()
        },
    );
    ui::end_frame();
    assert_eq!(grown.element().children.len(), 3);
    // the heading keeps its key (and therefore its retained nodes)
    assert_eq!(
        grown.element().children[0].element().key.as_deref(),
        Some("md-e1")
    );
}

#[test]
fn markdown_widget_keeps_a_cache_per_instance_id() {
    use zant::components::widgets::{MarkdownOptions, markdown};

    ui::begin_frame();
    let first = markdown(
        0x4d44_0007,
        MarkdownOptions {
            source: "# A".into(),
            ..Default::default()
        },
    );
    ui::end_frame();
    // the default key prefix is derived from the instance id, and the facade
    // is stretchable (Streamdown's root is 100% wide)
    assert_eq!(first.element().key.as_deref(), Some("md-4d440007"));
    assert!(first.element().layout.width_percent.is_none());

    // a second build reuses the cached blocks of the same instance
    ui::begin_frame();
    let second = markdown(
        0x4d44_0007,
        MarkdownOptions {
            source: "# B".into(),
            key: Some("custom".into()),
            ..Default::default()
        },
    );
    ui::end_frame();
    assert_eq!(second.element().key.as_deref(), Some("custom"));
}
