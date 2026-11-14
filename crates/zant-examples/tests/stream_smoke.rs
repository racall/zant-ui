//! Headless smoke test for the `zant-md-stream` binary (WP-3.11): the chat
//! (history messages + streaming reply) is built through [`Runtime`] on a
//! raster canvas and the streaming path is driven by appending to the shared
//! buffer exactly like the worker thread does.

use std::sync::{Arc, Mutex};

use zant::core::node::NodeData;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant_examples::stream_app::{
    HISTORY_SAMPLES, SharedInner, StreamApp, append_delta, finish, handle_sse_line, run_options,
};

const WIDTH: f32 = 900.0;
const HEIGHT: f32 = 900.0;

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("frame canvas");
    runtime.render_frame(canvas, WIDTH, HEIGHT, 1.0);
    renderer.end();
}

fn app() -> (Runtime, Renderer, zant_examples::stream_app::Shared) {
    let history = vec![
        HISTORY_SAMPLES[0].to_string(),
        HISTORY_SAMPLES[1].to_string(),
    ];
    let (app, shared) = StreamApp::headless(history, "");
    let mut runtime = Runtime::init(app, run_options(WIDTH));
    let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster");
    frame(&mut runtime, &mut renderer);
    (runtime, renderer, shared)
}

/// The header status text (`"{bytes} bytes · {deltas} deltas{s}"`).
fn header(runtime: &Runtime) -> String {
    runtime
        .tree
        .nodes
        .iter()
        .find_map(|node| match &node.data {
            NodeData::Text(data) if data.content.contains("deltas") => {
                Some(data.content.to_string())
            }
            _ => None,
        })
        .expect("the streaming header text")
}

fn has_caret(runtime: &Runtime) -> bool {
    runtime.tree.nodes.iter().any(|node| {
        node.alive
            && match &node.data {
                NodeData::Text(data) => data.content == "▋",
                NodeData::RichText(data) => data.spans.iter().any(|span| span.text == "▋"),
                _ => false,
            }
    })
}

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
fn stream_builds_the_chat_with_history_and_the_reply() {
    let (runtime, _renderer, _shared) = app();
    // the virtual list and its history bubbles are in the tree
    assert!(
        runtime.find_key("chat").is_some() || runtime.find_key("chat-viewport").is_some(),
        "the chat list is built"
    );
    assert_eq!(header(&runtime), "0 bytes · 0 deltas");
    assert!(contains_text(&runtime, HISTORY_SAMPLES[0]));
    assert!(contains_text(&runtime, "二次函数"));
    assert!(
        runtime
            .tree
            .nodes
            .iter()
            .any(|node| node.alive && matches!(node.data, NodeData::RichText(_)))
    );
}

#[test]
fn stream_appends_update_the_header_and_the_reply_bubble() {
    let (mut runtime, mut renderer, shared) = app();
    let delta = "## 结论\n\n最小值是 $f(2) = -5$。";
    append_delta(&shared, delta.as_bytes());
    // the worker's `ui::post_wake()` reaches the native host in the window path;
    // headless, mark the runtime for the rebuild it would schedule
    runtime.needs_build = true;
    frame(&mut runtime, &mut renderer);
    assert_eq!(
        header(&runtime),
        format!("{} bytes · 1 deltas", delta.len())
    );
    assert!(
        contains_text(&runtime, "结论"),
        "the streamed heading is in the reply bubble"
    );
    assert!(
        has_caret(&runtime),
        "the streaming caret is drawn while not done"
    );

    finish(&shared);
    runtime.needs_build = true;
    frame(&mut runtime, &mut renderer);
    assert_eq!(
        header(&runtime),
        format!("{} bytes · 1 deltas · done", delta.len())
    );
    assert!(
        !has_caret(&runtime),
        "the caret disappears when the stream is done"
    );
}

#[test]
fn stream_sse_lines_append_text_and_reset_the_turn() {
    let shared = Arc::new(Mutex::new(SharedInner::default()));
    handle_sse_line(
        &shared,
        "data: {\"type\":\"text_delta\",\"content\":\"你好\"}",
    );
    handle_sse_line(
        &shared,
        "data: {\"type\":\"text_delta\",\"content\":\"，Zant\"}",
    );
    {
        let inner = shared.lock().expect("shared buffer");
        assert_eq!(String::from_utf8_lossy(&inner.text), "你好，Zant");
        assert_eq!(inner.deltas, 2);
    }
    handle_sse_line(&shared, "event: ping");
    handle_sse_line(&shared, "data: not json");
    handle_sse_line(
        &shared,
        "data: {\"type\":\"text_delta\",\"content\":\"ignored\"}",
    );
    assert_eq!(shared.lock().expect("shared buffer").deltas, 3);
    handle_sse_line(&shared, "data: {\"type\":\"turn_reset\"}");
    assert!(shared.lock().expect("shared buffer").text.is_empty());
}
