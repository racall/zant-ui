//! Headless smoke test for the `zant-markdown` binary: the same
//! state / tree the window paints is built through [`Runtime`] on a raster
//! canvas, twice (the second frame exercises the cached block path).

use zant::core::node::{NodeData, NodeKind};
use zant::markdown;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant_examples::markdown_app::{MarkdownApp, SAMPLE, run_options};

const WIDTH: f32 = 860.0;
const HEIGHT: f32 = 1500.0;

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("frame canvas");
    runtime.render_frame(canvas, WIDTH, HEIGHT, 1.0);
    renderer.end();
}

fn app(dark: bool) -> (Runtime, Renderer) {
    let mut runtime = Runtime::init(MarkdownApp::new(dark), run_options(dark));
    let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster");
    frame(&mut runtime, &mut renderer);
    (runtime, renderer)
}

fn count(runtime: &Runtime, kind: NodeKind) -> usize {
    runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && node.data.kind() == kind)
        .count()
}

/// True when a text / rich-text node of the tree contains `needle` (headings
/// render as text, paragraphs as rich text).
fn contains_text(runtime: &Runtime, needle: &str) -> bool {
    runtime.tree.nodes.iter().any(|node| {
        node.alive
            && match &node.data {
                NodeData::Text(data) => data.content.contains(needle),
                NodeData::RichText(data) => {
                    data.spans.iter().any(|span| span.text.contains(needle))
                }
                _ => false,
            }
    })
}

#[test]
fn markdown_builds_the_sample_tree_and_paints() {
    assert!(SAMPLE.starts_with("# Streamdown for Zant\n"));
    assert!(SAMPLE.contains("```mermaid"));
    assert!(SAMPLE.contains("| remend | ✅ |"));

    let (runtime, _renderer) = app(false);
    // Streamdown's root column and one keyed node per parsed block.
    assert!(runtime.find_key("md").is_some(), "the streamdown root key");
    let block_keys = runtime
        .tree
        .nodes
        .iter()
        .filter(|node| {
            node.alive
                && node
                    .key
                    .as_deref()
                    .is_some_and(|key| key.starts_with("md-e"))
        })
        .count();
    assert!(block_keys >= 8, "one UI key per block, got {block_keys}");
    assert!(count(&runtime, NodeKind::Text) > 3);
    // paragraphs / list items / table cells of the sample
    assert!(count(&runtime, NodeKind::RichText) > 5);
    // code, table and mermaid controls: copy + download each
    assert!(
        count(&runtime, NodeKind::Button) >= 6,
        "Streamdown block controls are buttons"
    );
    assert!(contains_text(&runtime, "Streamdown for Zant"));
    assert!(contains_text(&runtime, "Lists"));
    // `head(0)` heading style: the markdown theme's foreground is used
    assert!(contains_text(
        &runtime,
        "Footnotes render at the end of the document."
    ));
}

#[test]
fn markdown_second_frame_reuses_the_block_cache() {
    let (mut runtime, mut renderer) = app(false);
    let before = runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && matches!(node.data, NodeData::RichText(_)))
        .count();
    frame(&mut runtime, &mut renderer);
    let after = runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && matches!(node.data, NodeData::RichText(_)))
        .count();
    assert_eq!(before, after, "the second frame renders the same blocks");
}

#[test]
fn markdown_dark_and_light_use_the_streamdown_themes() {
    let (light, _light_renderer) = app(false);
    let light_background = markdown::theme::light().background;
    assert!(
        light
            .tree
            .nodes
            .iter()
            .any(|node| node.alive && node.visual.background == Some(light_background)),
        "the page uses the light Streamdown background"
    );

    let (dark, _dark_renderer) = app(true);
    let dark_background = markdown::theme::dark().background;
    assert!(
        dark.tree
            .nodes
            .iter()
            .any(|node| node.alive && node.visual.background == Some(dark_background)),
        "ZANT_DARK uses the dark Streamdown background"
    );
    assert_ne!(light_background, dark_background);
}
