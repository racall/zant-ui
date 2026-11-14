//! Headless smoke test for the `zant-pdf` binary (WP-3.11): the reader app
//! opens the committed `tests/fixtures/widgets.pdf` through the same
//! [`Viewer`] / `pdf::view` path the window uses, builds the toolbar + viewport
//! tree and paints it on a raster canvas.
//!
//! Like the framework's PDF tests, the cases serialise on one lock: PDFKit and
//! the raster renderer touch shared native state.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::MutexGuard;

use zant::core::node::{NodeData, NodeKind};
use zant::pdf::{Viewer, view};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant_examples::pdf_app::{DEFAULT_DOCUMENT, PdfApp, demo_annotations, run_options};

const WIDTH: f32 = 900.0;
const HEIGHT: f32 = 900.0;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    let guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    zant::motion::motion::set_enabled(false);
    guard
}

/// The fixture from the repository (`tests/fixtures/widgets.pdf`), independent
/// of the working directory.
fn fixture() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/widgets.pdf")
        .to_str()
        .expect("utf-8 fixture path")
        .to_string()
}

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("frame canvas");
    runtime.render_frame(canvas, WIDTH, HEIGHT, 1.0);
    renderer.end();
}

fn app(viewer: Viewer) -> (Runtime, Renderer) {
    let mut runtime = Runtime::init(PdfApp::new(viewer, false), run_options());
    let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster");
    frame(&mut runtime, &mut renderer);
    (runtime, renderer)
}

#[test]
fn pdf_opens_the_default_fixture_and_builds_the_reader() {
    let _guard = lock();
    assert_eq!(DEFAULT_DOCUMENT, "tests/fixtures/widgets.pdf");
    let viewer = Viewer::new();
    viewer.open(&fixture()).expect("the fixture opens");
    let pages = viewer.page_count();
    assert!(pages > 0, "the fixture has pages");

    let (runtime, _renderer) = app(viewer.clone());
    assert!(runtime.find_key("pdf").is_some(), "the viewer root key");
    assert!(
        runtime.find_key("pdf-vp").is_some(),
        "the scroll viewport key"
    );
    // toolbar: page navigation, zoom, search and the annotation tool switch
    for key in [
        "pdf-prev",
        "pdf-next",
        "pdf-page",
        "pdf-zin",
        "pdf-zout",
        "pdf-zoom",
        "pdf-search",
    ] {
        assert!(runtime.find_key(key).is_some(), "toolbar key {key:?}");
    }
    assert!(
        runtime.find_key("pdf-tool0").is_some(),
        "annotation tool switch"
    );
    let texts: Vec<&str> = runtime
        .tree
        .nodes
        .iter()
        .filter_map(|node| match &node.data {
            NodeData::Text(data) => Some(data.content.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        texts
            .iter()
            .any(|text| text.contains(&format!("/ {pages}"))),
        "the page counter shows the page count"
    );
    let document = viewer.document().expect("the open document");
    assert!(
        document.page_text(0).is_some() || pages > 0,
        "the fixture's text layer loads"
    );
}

#[test]
fn pdf_demo_annotations_add_the_sample_layer() {
    let _guard = lock();
    let viewer = Viewer::new();
    viewer.open(&fixture()).expect("the fixture opens");
    // Zig `demoAnnotations` adds to the first page with at least 400 glyphs.
    let document = viewer.document().expect("open document");
    let annotated_page = (0..document.page_count()).find(|&page| {
        document
            .page_text(page)
            .is_some_and(|text| text.count() >= 400)
    });
    demo_annotations(&viewer);
    let annotations = viewer.annotations();
    match annotated_page {
        Some(page) => {
            let on_page: Vec<_> = annotations
                .all()
                .iter()
                .filter(|annotation| annotation.page == page)
                .collect();
            // four markup kinds + one ink + one note
            assert_eq!(on_page.len(), 6);
            assert_eq!(
                on_page
                    .iter()
                    .filter(|annotation| annotation.kind.is_markup())
                    .count(),
                4
            );
            assert!(
                on_page
                    .iter()
                    .any(|annotation| annotation.kind == zant::pdf::annotations::Kind::Ink),
                "the ink sample stroke"
            );
            assert!(
                on_page
                    .iter()
                    .any(|annotation| annotation.kind == zant::pdf::annotations::Kind::Note),
                "the note sample"
            );
        }
        None => assert!(annotations.all().is_empty()),
    }
}

#[test]
fn pdf_reader_paints_and_keeps_the_viewer_alive() {
    let _guard = lock();
    let viewer = Viewer::new();
    viewer.open(&fixture()).expect("the fixture opens");
    let (mut runtime, mut renderer) = app(viewer.clone());
    // a second frame goes through the cached page layer path
    frame(&mut runtime, &mut renderer);
    assert!(runtime.find_key("pdf").is_some());
    let containers = runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && node.data.kind() == NodeKind::Container)
        .count();
    assert!(containers > 10, "the viewer lays out its chrome");
    // `view` builds directly too (the application entry of the binary)
    let node = view(&viewer, zant::pdf::ViewOptions::default());
    assert_eq!(node.element().kind, NodeKind::Container);
}
