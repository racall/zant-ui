//! The viewer on the fixture PDF: layout, zoom, selection, annotations, headless paint.

use super::*;

/// Opens `tests/fixtures/widgets.pdf` in a 500 × 500 window and returns the
/// viewer (interface hidden by the caller as needed).
pub(super) fn open_fixture(toolbar: bool, width: u32, height: u32) -> (Viewer, Runtime, Renderer) {
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
