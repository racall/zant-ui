//! The viewer on a generated minimal PDF: selection, search, annotations.

use super::*;

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
