//! Mermaid parsing, layout, and rendering tests.

use super::flowchart::{Flow, Shape};
use super::{Colors, Kind, Options, detect, render};
use crate::core::vector::{self, Command};
use crate::renderer::text::Family;

fn test_colors() -> Colors {
    Colors {
        text: 0xff000000,
        node_fill: 0xffeeeeee,
        node_border: 0xff999999,
        edge: 0xff666666,
        label_bg: 0xffffffff,
        muted: 0xff888888,
    }
}

fn test_options(max_width: f32) -> Options {
    Options {
        max_width,
        ..Options::new(test_colors())
    }
}

#[test]
fn options_new_uses_the_default_size_and_width() {
    let options = Options::new(test_colors());
    assert_eq!(options.font_size, 14.0);
    assert_eq!(options.max_width, 640.0);
    assert_eq!(options.colors.text, 0xff000000);
}

#[test]
fn flowchart_parsing() {
    let mut flow = Flow::default();
    flow.parse_statement("A[Start] --> B{Is it?} -->|Yes| C(Done)");
    flow.parse_statement("B -- No --> D[[Retry]]");
    assert_eq!(4, flow.nodes.len());
    assert_eq!(3, flow.edges.len());
    assert_eq!(flow.edges[1].label, "Yes");
    assert_eq!(flow.edges[2].label, "No");
    assert_eq!(Shape::Diamond, flow.nodes[1].shape);
    assert_eq!(Some(Kind::Flowchart), detect("%% c\ngraph TD\nA-->B"));
}

#[test]
fn wide_flowcharts_shrink_to_max_width() {
    let src = "flowchart LR\n  A[A very long first node] --> B[Another long node]\n  B --> C[Third long node] --> D[Fourth long node] --> E[Fifth long node]";
    let wide = render(src, test_options(0.0)).expect("wide diagram");
    assert!(wide.width > 300.0);
    let fitted = render(src, test_options(300.0)).expect("fitted diagram");
    assert!(
        (fitted.width - 300.0).abs() < 0.5,
        "fitted width = {}",
        fitted.width
    );
}

#[test]
fn flowchart_diagrams_render() {
    let src = "flowchart TD\n  A[Start] --> B{Is it?}\n  B -->|Yes| C(Done)\n  B -->|No| D[[Retry]]\n  D -.-> A";
    let drawing = render(src, test_options(640.0)).expect("flowchart");
    assert_eq!(
        drawing.descent, 0.0,
        "diagrams sit entirely above the baseline"
    );
    assert!(drawing.width > 0.0);
    assert!(drawing.ascent > 0.0);
    assert!(!drawing.commands.is_empty());
    assert_ne!(drawing.hash, 0, "render pins the content hash");
    assert_eq!(drawing.hash, render(src, test_options(640.0)).unwrap().hash);
    assert_eq!(detect(src), Some(Kind::Flowchart));
}

#[test]
fn sequence_diagrams_render() {
    let src = "sequenceDiagram\n  participant A as Alice\n  participant B as Bob\n  A->>B: Hello\n  B-->>A: Hi\n  Note over A,B: shared\n  A->A: retry";
    let drawing = render(src, test_options(640.0)).expect("sequence diagram");
    assert_eq!(drawing.descent, 0.0);
    assert!(drawing.width > 0.0);
    assert!(drawing.ascent > 0.0);
    assert!(!drawing.commands.is_empty());
    assert_eq!(detect(src), Some(Kind::Sequence));
}

/// A single node pins the layout constants: `8` margin, `fs * 2.6`
/// box height, `advance + 28` box width (min `56`), radius `3`, and the
/// diagram is anchored entirely above the baseline.
#[test]
fn flowchart_single_node_layout_constants() {
    let options = test_options(640.0);
    let drawing = render("flowchart TD\n  A[Start]", options).expect("flowchart");
    let fs = options.font_size;
    let node_h = fs * 2.6;
    let advance = vector::measure("Start", fs, Family::Sans, 400, false).advance;
    let node_w = (advance + 28.0).max(56.0);
    assert!(
        (drawing.ascent - (node_h + 16.0)).abs() < 1e-4,
        "ascent = {}",
        drawing.ascent
    );
    assert!(
        (drawing.width - (node_w + 16.0)).abs() < 1e-4,
        "width = {}",
        drawing.width
    );
    // rect + rounded border path + centered label
    assert_eq!(drawing.commands.len(), 3);
    match &drawing.commands[0] {
        Command::Rect(rect) => {
            assert_eq!(rect.x, 8.0);
            assert_eq!(rect.y, 8.0 - drawing.ascent);
            assert_eq!(rect.width, node_w);
            assert_eq!(rect.height, node_h);
            assert_eq!(rect.color, options.colors.node_fill);
            assert_eq!(rect.radius, 3.0);
        }
        other => panic!("expected a node rect, got {other:?}"),
    }
    match &drawing.commands[2] {
        Command::Text(text) => {
            assert_eq!(text.text, "Start");
            assert_eq!(text.size, fs);
            assert_eq!(text.color, options.colors.text);
            assert_eq!(text.family, Family::Sans);
        }
        other => panic!("expected the node label, got {other:?}"),
    }
}

#[test]
fn detection_skips_front_matter_and_comments() {
    assert_eq!(
        detect("---\ntitle: x\n---\ngraph TD\nA-->B"),
        Some(Kind::Flowchart)
    );
    assert_eq!(
        detect("%% comment\nflowchart LR\nA-->B"),
        Some(Kind::Flowchart)
    );
    assert_eq!(
        detect("\n\nsequenceDiagram\nA->>B: hi"),
        Some(Kind::Sequence)
    );
    // Never leaves front matter -> no diagram kind.
    assert_eq!(detect("---\ntitle: x\n---"), None);
    assert_eq!(detect(""), None);
}

#[test]
fn pie_charts_render() {
    let src = "pie title Pets\n  \"Dogs\" : 30\n  \"Cats\" : 20";
    let drawing = render(src, test_options(640.0)).expect("pie chart");
    assert_eq!(drawing.descent, 0.0);
    assert!(drawing.width > 0.0);
    assert!(drawing.ascent > 0.0);
    assert!(!drawing.commands.is_empty());
    assert_eq!(detect(src), Some(Kind::Pie));
}

/// Unsupported or empty diagrams fall back to a code block (`render` → None).
#[test]
fn unsupported_diagrams_fall_back() {
    let options = test_options(640.0);
    assert_eq!(None, detect("classDiagram\n  A --> B"));
    assert_eq!(None, detect("not a diagram"));
    assert!(render("classDiagram\n  A --> B", options).is_none());
    // A recognised header with no drawable content also falls back.
    assert!(render("flowchart TD\n", options).is_none());
    assert!(render("sequenceDiagram\n", options).is_none());
    assert!(render("pie\n", options).is_none());
}
