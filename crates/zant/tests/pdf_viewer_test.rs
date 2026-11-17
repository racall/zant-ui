//! PDF viewer tests.
//!
//! The first two tests generate a two-page PDF at runtime, drive the real
//! runtime with pointer / key events and paint with a raster renderer. The
//! others use the committed `tests/fixtures/widgets.pdf`: layout / scroll
//! clamping, the fit
//! modes and zoom steps, selection text / rects, incremental search, annotation
//! undo / redo and annotated export, and a headless paint + render-worker
//! smoke test (channel wake, bounded wait).
//!
//! The tests serialise on one lock: PDFKit and the raster renderer are global
//! state and the `ui` build context is per thread, so running them one at a
//! time keeps the assertions deterministic.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use zant::core::events::{Event, Key, KeyEvent, PointerEvent};
use zant::pdf::annotations::Kind;
use zant::pdf::document::Document;
use zant::pdf::renderer::{self, Key as RenderKey, Renderer as PageRenderer};
use zant::pdf::{Fit, Viewer};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::Node;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    zant_sys::init();
    // no animation frames
    zant::motion::motion::set_enabled(false);
    guard
}

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
        .to_str()
        .expect("utf-8 fixture path")
        .to_string()
}

fn tmp_path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).expect("test tmp dir");
    dir.join(name)
}

/// A minimal two-page PDF: a
/// 300 × 200 pt page with "Hello Zant PDF" and a second one with
/// "Second page" in Helvetica 24.
fn minimal_pdf() -> Vec<u8> {
    let objects: [&str; 7] = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        "<< /Length 46 >>\nstream\nBT /F1 24 Tf 20 120 Td (Hello Zant PDF) Tj ET\nendstream",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 7 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        "<< /Length 43 >>\nstream\nBT /F1 24 Tf 20 120 Td (Second page) Tj ET\nendstream",
    ];
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n");
    let mut offsets: Vec<usize> = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, body).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:0>10} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref
        )
        .as_bytes(),
    );
    out
}

fn write_minimal_pdf(name: &str) -> String {
    let path = tmp_path(name);
    std::fs::write(&path, minimal_pdf()).expect("write generated PDF");
    path.to_str().expect("utf-8 pdf path").to_string()
}

/// The app of the viewer tests: one column around the viewer.
struct ViewerApp {
    viewer: Viewer,
    toolbar: bool,
}

impl zant::ui::App for ViewerApp {
    fn render(&mut self) -> Node {
        zant::ui::column(zant::ui::BoxOptions {
            grow: 1.0,
            align_items: zant::flex::Align::Stretch,
            children: vec![zant::pdf::view(
                &self.viewer,
                zant::pdf::ViewOptions {
                    key: "pdf".into(),
                    toolbar: self.toolbar,
                    ..Default::default()
                },
            )],
            ..Default::default()
        })
    }
}

fn frame(runtime: &mut Runtime, renderer: &mut Renderer, width: f32, height: f32) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, width, height, 1.0);
    renderer.end();
}

fn setup(viewer: &Viewer, toolbar: bool, width: u32, height: u32) -> (Runtime, Renderer) {
    let app = ViewerApp {
        viewer: viewer.clone(),
        toolbar,
    };
    let mut runtime = Runtime::init(app, zant::ui::RunOptions::default());
    let mut renderer = Renderer::create_raster(width, height).expect("raster renderer");
    frame(&mut runtime, &mut renderer, width as f32, height as f32);
    frame(&mut runtime, &mut renderer, width as f32, height as f32);
    (runtime, renderer)
}

fn pointer(x: f32, y: f32) -> PointerEvent {
    PointerEvent {
        x,
        y,
        ..Default::default()
    }
}

#[test]
fn pdf_viewer_open_layout_drag_selection_word_selection_copy_text_search() {
    let _guard = lock();
    let path = write_minimal_pdf("test-viewer.pdf");

    let viewer = Viewer::new();
    viewer.open(&path).expect("open the generated PDF");
    assert_eq!(viewer.page_count(), 2);

    let (mut runtime, mut renderer) = setup(&viewer, false, 400, 600);
    // fit width: 300pt page in a 400px viewport with 16px padding
    assert!(
        (viewer.zoom() - 368.0 / 300.0).abs() < 0.01,
        "zoom {}",
        viewer.zoom()
    );

    let doc = viewer.document().expect("document");
    let text = doc.page_text(0).expect("page text");
    assert!(text.count() >= 14);
    // glyph boxes of "Hello" and "PDF" → viewport points
    let mut first: Option<usize> = None;
    let mut last: Option<usize> = None;
    for (i, g) in text.glyphs.iter().enumerate() {
        if g.codepoint == u32::from(b'H') && first.is_none() {
            first = Some(i);
        }
        if g.codepoint == u32::from(b'F') {
            last = Some(i);
        }
    }
    let first = first.expect("glyph 'H'");
    let last = last.expect("glyph 'F'");
    let z = viewer.zoom();
    let page_x: f32 = (400.0 - 300.0 * z) / 2.0;
    let page_y: f32 = 16.0;
    let a = text.glyphs[first].r#box;
    let b = text.glyphs[last].r#box;
    let ax = page_x + (a.x + 1.0) * z;
    let ay = page_y + (a.y + a.height / 2.0) * z;
    let bx = page_x + (b.x + b.width - 1.0) * z;
    let _ = runtime.dispatch(Event::PointerDown(pointer(ax, ay)));
    let _ = runtime.dispatch(Event::PointerMove(pointer((ax + bx) / 2.0, ay)));
    let _ = runtime.dispatch(Event::PointerMove(pointer(bx, ay)));
    let _ = runtime.dispatch(Event::PointerUp(pointer(bx, ay)));
    assert_eq!(viewer.selected_text(), "Hello Zant PDF");
    frame(&mut runtime, &mut renderer, 400.0, 600.0);

    // a click without a drag clears it; a double click selects a word
    let _ = runtime.dispatch(Event::PointerDown(pointer(ax, ay)));
    let _ = runtime.dispatch(Event::PointerUp(pointer(ax, ay)));
    assert!(viewer.selection().is_none());
    let _ = runtime.dispatch(Event::PointerDown(pointer(ax, ay)));
    let _ = runtime.dispatch(Event::PointerUp(pointer(ax, ay)));
    assert_eq!(viewer.selected_text(), "Hello");

    // search across pages
    viewer.search("second PAGE");
    frame(&mut runtime, &mut renderer, 400.0, 600.0);
    let results = viewer.search_results();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].page, 1);
    assert_eq!(viewer.active_match(), Some(0));
}

#[test]
fn pdf_viewer_annotations_markup_from_selection_ink_undo_redo_annotated_export() {
    let _guard = lock();
    let data = minimal_pdf();
    let path = write_minimal_pdf("test-annot.pdf");

    let viewer = Viewer::new();
    viewer.open(&path).expect("open the generated PDF");
    let (mut runtime, mut renderer) = setup(&viewer, false, 400, 600);

    viewer.select_all();
    let id = viewer
        .annotate_selection(Kind::Highlight, None)
        .expect("annotation");
    // one per page with text, the selection is consumed
    assert_eq!(viewer.annotations().all().len(), 2);
    assert!(viewer.selection().is_none());
    assert_eq!(
        viewer.annotations().get(id).expect("annotation").text,
        "Second page"
    );
    frame(&mut runtime, &mut renderer, 400.0, 600.0);

    // ink: drag with the pen tool on page 1
    viewer.set_tool(zant::pdf::Tool::Ink);
    let _ = runtime.dispatch(Event::PointerDown(pointer(100.0, 60.0)));
    let _ = runtime.dispatch(Event::PointerMove(pointer(140.0, 80.0)));
    let _ = runtime.dispatch(Event::PointerMove(pointer(180.0, 70.0)));
    let _ = runtime.dispatch(Event::PointerUp(pointer(180.0, 70.0)));
    assert_eq!(viewer.annotations().all().len(), 3);
    let ink = viewer.annotations().all()[2].clone();
    assert_eq!(ink.kind, Kind::Ink);
    assert_eq!(ink.strokes[0].len(), 3);
    frame(&mut runtime, &mut renderer, 400.0, 600.0);

    // undo / redo
    assert!(viewer.undo());
    assert_eq!(viewer.annotations().all().len(), 2);
    assert!(viewer.redo());
    assert_eq!(viewer.annotations().all().len(), 3);

    // clicking an annotation with the select tool selects it; Delete removes it
    viewer.set_tool(zant::pdf::Tool::Select);
    frame(&mut runtime, &mut renderer, 400.0, 600.0);
    let _ = runtime.dispatch(Event::PointerDown(pointer(140.0, 80.0)));
    let _ = runtime.dispatch(Event::PointerUp(pointer(140.0, 80.0)));
    assert_eq!(viewer.selected_annotation_id(), Some(ink.id));
    let _ = runtime.dispatch(Event::KeyDown(KeyEvent {
        key: Key::Delete,
        modifiers: Default::default(),
        repeat: false,
    }));
    assert_eq!(viewer.annotations().all().len(), 2);
    assert!(viewer.selected_annotation_id().is_none());

    // annotated copy: a valid PDF that still opens, larger than the source
    viewer.set_annotation_contents(id, "remember this");
    let out_path = tmp_path("test-annot-out.pdf");
    let _ = std::fs::remove_file(&out_path);
    viewer
        .save_annotated(out_path.to_str().expect("utf-8 out path"))
        .expect("annotated export");
    let saved =
        Document::open(out_path.to_str().expect("utf-8 out path"), None).expect("open saved");
    assert_eq!(saved.page_count(), 2);
    let saved_size = std::fs::metadata(&out_path).expect("metadata").len();
    assert!(saved_size > data.len() as u64);
}

// ---- fixture-based coverage -------------------------------------------

/// Opens `tests/fixtures/widgets.pdf` in a 500 × 500 window and returns the
/// viewer (interface hidden by the caller as needed).
fn open_fixture(toolbar: bool, width: u32, height: u32) -> (Viewer, Runtime, Renderer) {
    let viewer = Viewer::new();
    viewer.open(&fixture("widgets.pdf")).expect("open fixture");
    assert_eq!(viewer.page_count(), 1);
    let (runtime, renderer) = setup(&viewer, toolbar, width, height);
    (viewer, runtime, renderer)
}

#[test]
fn pdf_viewer_fixture_open_layout_scroll_and_clamp() {
    let _guard = lock();
    let (viewer, mut runtime, mut renderer) = open_fixture(false, 500, 500);
    // fit width: 360pt page in a 500px viewport with 16px padding
    assert!(
        (viewer.zoom() - 468.0 / 360.0).abs() < 0.01,
        "zoom {}",
        viewer.zoom()
    );

    // Zoom in: the content is now taller and wider than the viewport.
    viewer.set_zoom(2.0);
    assert_eq!(viewer.fit(), Fit::None);
    assert!((viewer.zoom() - 2.0).abs() < 0.001);
    let [_, content_h] = viewer.content_size();
    assert!(content_h > 500.0);
    // goToPage scrolls by the page offset and clamps to the content
    viewer.go_to_page(0, 100.0);
    let scroll = viewer.scroll();
    assert!((scroll[1] - 200.0).abs() < 0.01, "scroll {:?}", scroll);
    viewer.go_to_page(0, 10_000.0);
    let [_, content_h] = viewer.content_size();
    // the viewport height is the window (toolbar off)
    let max_scroll_y = (content_h - 500.0).max(0.0);
    assert!(
        (viewer.scroll()[1] - max_scroll_y).abs() < 0.01,
        "clamped {} vs {}",
        viewer.scroll()[1],
        max_scroll_y
    );
    assert_eq!(viewer.current_page(), 0);

    // a frame after scrolling keeps the page visible and still paints
    frame(&mut runtime, &mut renderer, 500.0, 500.0);
    assert_eq!(viewer.current_page(), 0);
}

#[test]
fn pdf_viewer_fixture_zoom_fit_width_page_and_steps() {
    let _guard = lock();
    let (viewer, mut runtime, mut renderer) = open_fixture(false, 500, 500);
    // fit width: 360pt page in a 500px viewport with 16px padding
    assert!((viewer.zoom() - 468.0 / 360.0).abs() < 0.01);
    assert_eq!(viewer.fit(), Fit::Width);
    // fit page: the whole 360 × 480 page fits the 500 × 500 viewport
    viewer.set_fit(Fit::Page);
    assert!(
        (viewer.zoom() - 468.0 / 480.0).abs() < 0.01,
        "{}",
        viewer.zoom()
    );
    // back to fit width
    viewer.set_fit(Fit::Width);
    assert!((viewer.zoom() - 468.0 / 360.0).abs() < 0.01);

    // manual zoom steps of EmbedPDF's presets, clamped to 25% … 500%
    viewer.set_zoom(2.0);
    viewer.zoom_in();
    assert!((viewer.zoom() - 2.5).abs() < 0.001);
    viewer.zoom_out();
    assert!((viewer.zoom() - 2.0).abs() < 0.001);
    viewer.set_zoom(0.1);
    assert!((viewer.zoom() - zant::pdf::viewer::MIN_ZOOM).abs() < 0.001);
    viewer.set_zoom(10.0);
    assert!((viewer.zoom() - zant::pdf::viewer::MAX_ZOOM).abs() < 0.001);
    // ⌘/Ctrl + wheel zooms around the pointer; a wheel notch is about 1.27×
    viewer.set_zoom(1.0);
    frame(&mut runtime, &mut renderer, 500.0, 500.0);
}

#[test]
fn pdf_viewer_fixture_selection_rects_and_search() {
    let _guard = lock();
    let (viewer, mut runtime, mut renderer) = open_fixture(false, 400, 600);
    let doc = viewer.document().expect("document");
    let text = doc.page_text(0).expect("text");
    assert!(text.count() > 0);

    // select all: the viewer's text equals the page text of the fixture
    viewer.select_all();
    let selection = viewer.selection().expect("selection");
    assert_eq!(selection.start.page, 0);
    assert_eq!(selection.end.index, text.count() - 1);
    assert_eq!(viewer.selected_text(), doc.page_text_string(0));

    // selection rectangles come from `pdf::text` (one or more per line)
    let rects = text.rects(selection.start.index, selection.end.index);
    assert!(!rects.is_empty());
    assert!(rects.iter().all(|r| r.width > 0.0 && r.height > 0.0));

    // clearSelection / setSelection round-trip
    viewer.clear_selection();
    assert!(viewer.selection().is_none());
    viewer.set_selection(
        zant::pdf::Pos { page: 0, index: 0 },
        zant::pdf::Pos { page: 0, index: 3 },
    );
    let partial = viewer.selected_text();
    assert!(!partial.is_empty());
    assert!(doc.page_text_string(0).starts_with(&partial));

    // incremental search: the fixture text "Zant TSX Reader"
    viewer.search("zant");
    for _ in 0..5 {
        runtime.invalidate();
        frame(&mut runtime, &mut renderer, 400.0, 600.0);
        if viewer.search_done() {
            break;
        }
    }
    let results = viewer.search_results();
    assert!(!results.is_empty(), "no match for 'zant'");
    assert_eq!(results[0].page, 0);
    assert_eq!(viewer.active_match(), Some(0));
    let hit = text.rects(results[0].from, results[0].to);
    assert!(!hit.is_empty(), "a search hit highlights rects");

    // several hits: nextMatch / previousMatch move the active one
    viewer.search("e");
    for _ in 0..5 {
        runtime.invalidate();
        frame(&mut runtime, &mut renderer, 400.0, 600.0);
        if viewer.search_done() {
            break;
        }
    }
    assert!(viewer.search_results().len() >= 2);
    let first = viewer.active_match();
    viewer.next_match();
    assert_ne!(
        viewer.active_match(),
        first,
        "nextMatch moves the highlight"
    );
    viewer.previous_match();
    assert_eq!(viewer.active_match(), first);

    // no results
    viewer.search("zzz-not-in-the-document");
    for _ in 0..5 {
        runtime.invalidate();
        frame(&mut runtime, &mut renderer, 400.0, 600.0);
        if viewer.search_done() {
            break;
        }
    }
    assert!(viewer.search_results().is_empty());
    assert!(viewer.active_match().is_none());
}

#[test]
fn pdf_viewer_fixture_annotation_add_undo_redo_and_export() {
    let _guard = lock();
    let (viewer, mut runtime, mut renderer) = open_fixture(false, 400, 600);
    let source_len = std::fs::metadata(fixture("widgets.pdf"))
        .expect("fixture metadata")
        .len();

    // a highlight from the whole page selection, then undo / redo
    viewer.select_all();
    let id = viewer
        .annotate_selection(Kind::Highlight, None)
        .expect("highlight");
    assert_eq!(viewer.annotations().all().len(), 1);
    let highlight = viewer.annotations().get(id).cloned().expect("annotation");
    assert_eq!(highlight.kind, Kind::Highlight);
    assert!(!highlight.rects.is_empty());
    assert_eq!(highlight.color, Viewer::default_color(Kind::Highlight));
    frame(&mut runtime, &mut renderer, 400.0, 600.0);

    assert!(viewer.undo());
    assert!(viewer.annotations().all().is_empty());
    assert!(!viewer.undo());
    assert!(viewer.redo());
    assert_eq!(viewer.annotations().all().len(), 1);

    // colours / contents / removal are annotation steps too
    viewer.set_annotation_color(id, 0x8057_d975);
    assert_eq!(
        viewer.annotations().get(id).expect("annotation").color,
        0x8057_d975
    );
    viewer.set_annotation_contents(id, "remember this");
    assert_eq!(
        viewer.annotations().get(id).expect("annotation").contents,
        "remember this"
    );
    assert_eq!(viewer.selected_annotation_id(), None);
    viewer.select_annotation(Some(id));
    assert_eq!(viewer.selected_annotation_id(), Some(id));
    let key = Viewer::annotation_key("pdf", id);
    assert_eq!(key, format!("pdf-a{id}"));
    frame(&mut runtime, &mut renderer, 400.0, 600.0);

    // annotated export: a valid PDF, larger than the source
    let out_path = tmp_path("test-viewer-fixture-annotated.pdf");
    let _ = std::fs::remove_file(&out_path);
    viewer
        .save_annotated(out_path.to_str().expect("utf-8 out path"))
        .expect("annotated export");
    let saved =
        Document::open(out_path.to_str().expect("utf-8 out path"), None).expect("open saved");
    assert_eq!(saved.page_count(), 1);
    let saved_len = std::fs::metadata(&out_path).expect("metadata").len();
    assert!(saved_len > source_len);

    // removing it is undoable as well
    viewer.remove_annotation(id);
    assert!(viewer.annotations().all().is_empty());
    assert!(viewer.selected_annotation_id().is_none());
}

#[test]
fn pdf_viewer_fixture_headless_paint_and_render_worker_wake() {
    let _guard = lock();
    let (viewer, mut runtime, mut renderer) = open_fixture(false, 500, 500);
    // The page is painted as an opaque white box even before any tile is ready.
    let pixel = renderer.pixel_at(30, 30).expect("pixel");
    assert_eq!(pixel >> 24, 0xff, "opaque page background: {pixel:#010x}");
    let red = (pixel >> 16) & 0xff;
    let green = (pixel >> 8) & 0xff;
    let blue = pixel & 0xff;
    assert!(
        red > 200 && green > 200 && blue > 200,
        "white page: {pixel:#010x}"
    );

    // A few more frames keep painting (tiles wake the UI thread through
    // `ui::post_wake`, a no-op without a window in this headless test).
    frame(&mut runtime, &mut renderer, 500.0, 500.0);
    frame(&mut runtime, &mut renderer, 500.0, 500.0);
    assert!(viewer.zoom() > 0.0);

    // The render worker itself: a channel wake instead of `ui::post_wake`, and
    // one bounded wait for the tile (no sleeping / polling).
    let doc = Arc::new(Document::open(&fixture("widgets.pdf"), None).expect("open fixture"));
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let page_renderer = PageRenderer::create(doc, move || {
        let _ = tx.send(());
    });
    page_renderer.begin_frame();
    let key = RenderKey {
        page: 0,
        page_w: renderer::BASE_WIDTH,
        tx: 0,
        ty: 0,
    };
    assert!(
        page_renderer.tile(key).is_none(),
        "first ask only queues it"
    );
    rx.recv_timeout(Duration::from_secs(20))
        .expect("the worker wakes the UI thread when the tile is ready");
    let image = page_renderer.tile(key).expect("rendered base tile");
    assert_eq!((image.width, image.height), (360, 480));
    drop(page_renderer);

    // The painted page rect of the fixture (360 × 480 pt at ~1.3×) is inside
    // the viewport; annotations / selection / search layers draw on top of it.
    viewer.select_all();
    let _ = viewer.annotate_selection(Kind::Underline, None);
    viewer.search("Zant");
    for _ in 0..5 {
        runtime.invalidate();
        frame(&mut runtime, &mut renderer, 500.0, 500.0);
        if viewer.search_done() {
            break;
        }
    }
    assert_eq!(viewer.annotations().all().len(), 1, "underline annotation");
    frame(&mut runtime, &mut renderer, 500.0, 500.0);
}

/// An app that counts the viewer's callbacks in its own state.
struct CallbackApp {
    viewer: Viewer,
    changes: u32,
    annotations: u32,
}

impl zant::ui::App for CallbackApp {
    fn render(&mut self) -> Node {
        zant::ui::column(zant::ui::BoxOptions {
            grow: 1.0,
            align_items: zant::flex::Align::Stretch,
            children: vec![zant::pdf::view(
                &self.viewer,
                zant::pdf::ViewOptions {
                    key: "pdf".into(),
                    toolbar: false,
                    on_change: Some(zant::ui::action(|app: &mut CallbackApp| app.changes += 1)),
                    on_annotation: Some(zant::ui::action(|app: &mut CallbackApp| {
                        app.annotations += 1
                    })),
                    ..Default::default()
                },
            )],
            ..Default::default()
        })
    }
}

fn click_key(runtime: &mut Runtime, key: &str) {
    let id = runtime
        .find_key(key)
        .unwrap_or_else(|| panic!("node {key}"));
    let bounds = runtime.tree.get(id).bounds;
    let point = pointer(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    );
    let _ = runtime.dispatch(Event::PointerDown(point));
    let _ = runtime.dispatch(Event::PointerUp(point));
}

#[test]
fn pdf_viewer_selection_menu_and_synchronous_callbacks() {
    let _guard = lock();
    let path = write_minimal_pdf("test-selection-menu.pdf");
    let viewer = Viewer::new();
    viewer.open(&path).expect("open the generated PDF");

    let app = CallbackApp {
        viewer: viewer.clone(),
        changes: 0,
        annotations: 0,
    };
    let mut runtime = Runtime::init(app, zant::ui::RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 600).expect("raster renderer");
    frame(&mut runtime, &mut renderer, 400.0, 600.0);
    frame(&mut runtime, &mut renderer, 400.0, 600.0);
    assert!(
        runtime.find_key("pdf-sel-menu").is_none(),
        "no menu without a selection"
    );

    // drag over the first line of page 1
    let doc = viewer.document().expect("document");
    let text = doc.page_text(0).expect("page text");
    let z = viewer.zoom();
    let page_x: f32 = (400.0 - 300.0 * z) / 2.0;
    let first = text.glyphs[0].r#box;
    let ax = page_x + (first.x + 1.0) * z;
    let ay = 16.0 + (first.y + first.height / 2.0) * z;
    let bx = ax + 60.0;
    runtime.app_state_mut::<CallbackApp>().changes = 0;
    let _ = runtime.dispatch(Event::PointerDown(pointer(ax, ay)));
    let _ = runtime.dispatch(Event::PointerMove(pointer((ax + bx) / 2.0, ay)));
    frame(&mut runtime, &mut renderer, 400.0, 600.0);
    assert!(
        runtime.find_key("pdf-sel-menu").is_none(),
        "the menu stays hidden while dragging"
    );
    let _ = runtime.dispatch(Event::PointerMove(pointer(bx, ay)));
    let _ = runtime.dispatch(Event::PointerUp(pointer(bx, ay)));
    assert!(viewer.selection().is_some());
    // on_change ran inside the pointer events, before any rebuild
    assert!(
        runtime.app_state_mut::<CallbackApp>().changes > 0,
        "on_change runs in the event that changed the selection"
    );

    // the floating menu appears next to the selection with its four actions
    frame(&mut runtime, &mut renderer, 400.0, 600.0);
    let menu = runtime.find_key("pdf-sel-menu").expect("selection menu");
    let menu_bounds = runtime.tree.get(menu).bounds;
    assert_eq!(menu_bounds.height, 36.0);
    assert!(menu_bounds.x >= 8.0 && menu_bounds.y >= 4.0);
    for key in ["pdf-sel-copy", "pdf-sel-hl", "pdf-sel-ul", "pdf-sel-st"] {
        assert!(runtime.find_key(key).is_some(), "menu button {key}");
    }

    // "Highlight" annotates the selection, clears it and reports it at once
    click_key(&mut runtime, "pdf-sel-hl");
    assert_eq!(viewer.annotations().all().len(), 1);
    assert_eq!(viewer.annotations().all()[0].kind, Kind::Highlight);
    assert!(viewer.selection().is_none());
    assert_eq!(
        runtime.app_state_mut::<CallbackApp>().annotations,
        1,
        "on_annotation runs in the click that created the annotation"
    );
    frame(&mut runtime, &mut renderer, 400.0, 600.0);
    assert!(
        runtime.find_key("pdf-sel-menu").is_none(),
        "the menu closes with the selection"
    );

    // a change made through the API (no event) is reported by the next build
    viewer.set_zoom(3.0);
    runtime.needs_build = true;
    frame(&mut runtime, &mut renderer, 400.0, 600.0);
    let before = runtime.app_state_mut::<CallbackApp>().changes;
    viewer.scroll_to(0.0, 40.0);
    assert_eq!(viewer.scroll()[1], 40.0);
    assert_eq!(viewer.viewport_size()[0], 400.0);
    runtime.needs_build = true;
    frame(&mut runtime, &mut renderer, 400.0, 600.0);
    assert!(runtime.app_state_mut::<CallbackApp>().changes > before);
}
