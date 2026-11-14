//! PDF document layer tests: the preview helpers, page text geometry, the
//! `Document` cache API, the render worker, the annotation store and the
//! annotated export, on the committed `tests/fixtures/widgets.pdf` fixture.
//! The viewer-level tests are in `pdf_viewer_test.rs`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use zant::pdf::annotations::{self, Annotation, Kind, Store};
use zant::pdf::document::Document;
use zant::pdf::native::{self, Glyph};
use zant::pdf::renderer::{self, Key, Renderer};
use zant::pdf::text::PageText;

/// Path of `tests/fixtures/<name>`.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn fixture_str(name: &str) -> String {
    fixture(name)
        .to_str()
        .expect("utf-8 fixture path")
        .to_string()
}

/// Path of `<CARGO_TARGET_TMPDIR>/<name>` (created).
fn tmp_path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).expect("test tmp dir");
    dir.join(name)
}

// ---- preview helpers ---------------------------------------------------

/// One page, a rendered
/// PNG temp file that exists, and the page text.
#[test]
fn pdf_page_count_rendered_page_and_page_text() {
    let path = fixture_str("widgets.pdf");

    assert_eq!(zant::platform::pdf::page_count(&path), 1);

    let mut out = [0u8; 1024];
    let png = zant::platform::pdf::render_page(&path, 0, 400, 200, &mut out)
        .expect("rendered page returns a PNG path");
    assert!(png.ends_with(".png"));
    assert!(std::path::Path::new(png).exists());

    let mut text_buf = [0u8; 256];
    let text =
        zant::platform::pdf::page_text(&path, 0, &mut text_buf).expect("page text of the fixture");
    assert!(text.contains("Zant TSX Reader"));
}

// ---- page text ---------------------------------------------------------

fn blank_glyph() -> Glyph {
    Glyph {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        tight_x: 0.0,
        tight_y: 0.0,
        tight_width: 0.0,
        tight_height: 0.0,
        codepoint: 0,
        font_size: 0.0,
        run: 0,
        flags: 0,
    }
}

fn glyph(x: f32, y: f32, w: f32, cp: u32) -> Glyph {
    Glyph {
        x,
        y,
        width: w,
        height: 10.0,
        tight_x: x,
        tight_y: y + 1.0,
        tight_width: w,
        tight_height: 8.0,
        codepoint: cp,
        font_size: 10.0,
        run: -1,
        flags: if cp == u32::from(b' ') {
            1
        } else if cp == u32::from(b'\n') {
            2
        } else {
            0
        },
    }
}

/// Writes `s` as glyphs starting at `start`; returns the next free index.
fn line(buf: &mut [Glyph], start: usize, s: &str, y: f32) -> usize {
    let mut x: f32 = 10.0;
    for (i, c) in s.bytes().enumerate() {
        buf[start + i] = glyph(x, y, 6.0, u32::from(c));
        if c == b'\n' {
            buf[start + i].width = 0.0;
        }
        x += 6.0;
    }
    start + s.len()
}

#[test]
fn pdf_text_runs_hit_testing_rects_words_lines_search() {
    let mut raw = vec![blank_glyph(); 64];
    let mut n = line(&mut raw, 0, "Hello world\n", 20.0);
    n = line(&mut raw, n, "second line", 40.0);
    let page = PageText::build(&raw[0..n]);
    // two visual lines → two runs (geometry grouping)
    assert_eq!(page.runs.len(), 2);
    // 'w' of "world" is glyph 6 at x = 46..52
    assert_eq!(page.glyph_at(48.0, 25.0, 1.5), Some(6));
    // tolerance: a little above the line still hits
    assert!(page.glyph_at(48.0, 19.5, 1.5).is_some());
    assert_eq!(page.glyph_at(300.0, 300.0, 1.5), None);
    // word / line expansion
    let w = page.word_at(7).expect("word");
    assert_eq!(w.from, 6);
    assert_eq!(w.to, 10);
    let l = page.line_at(14).expect("line");
    assert_eq!(l.from, 12);
    assert_eq!(l.to, 22);
    // a range across both lines gives one rect per line
    let rects = page.rects(6, 17);
    assert_eq!(rects.len(), 2);
    assert!((rects[0].x - 46.0).abs() < 0.01);
    // text and search (case-insensitive, line breaks match spaces)
    let mut out = String::new();
    page.append_text(&mut out, 6, 17);
    assert_eq!(out, "world\nsecond");
    let mut found = Vec::new();
    page.search("WORLD  second", &mut found);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].from, 6);
    assert_eq!(found[0].to, 17);
}

// ---- document ----------------------------------------------------------

#[test]
fn document_opens_fixture_with_sizes_text_links_outline_and_tiles() {
    // The render worker hands `Arc<Document>` to a thread, so Document must be
    // Send + Sync (the native handle is immutable and the bridge serialises).
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Document>();

    let path = fixture_str("widgets.pdf");
    let doc = Document::open(&path, None).expect("open fixture");
    assert_eq!(doc.page_count(), 1);
    let size = doc.page_size(0);
    assert!((size.width - 360.0).abs() < 0.01, "width {}", size.width);
    assert!((size.height - 480.0).abs() < 0.01, "height {}", size.height);

    // Lazy text geometry, cached: the second call returns the same snapshot.
    let text = doc.page_text(0).expect("text of page 0");
    assert!(text.count() > 0);
    assert!(Arc::ptr_eq(&text, &doc.page_text(0).expect("cached text")));
    assert!(doc.page_text_string(0).contains("Zant TSX Reader"));
    assert!(doc.page_text(1).is_none(), "past the last page");

    assert!(doc.page_links(0).is_empty(), "fixture has no links");
    assert!(doc.outline().is_empty(), "fixture has no bookmarks");

    zant_sys::init();
    let image = doc
        .render_tile(0, 360, 480, 0, 0, 360, 480)
        .expect("whole page tile");
    assert_eq!((image.width, image.height), (360, 480));
    // Tiles of a region keep the requested pixel size.
    let tile = doc
        .render_tile(0, 360, 480, 100, 100, 64, 64)
        .expect("region tile");
    assert_eq!((tile.width, tile.height), (64, 64));
    assert!(doc.render_tile(0, 360, 480, 0, 0, 0, 64).is_none());
}

// ---- render worker -----------------------------------------------------

#[test]
fn render_worker_renders_the_base_tile_and_page_height() {
    zant_sys::init();
    let doc = Arc::new(Document::open(&fixture_str("widgets.pdf"), None).expect("open"));
    // `create` takes the UI-thread wake callback instead of calling the
    // not-yet-ported `ui.post_wake` (see the module docs).
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let renderer = Renderer::create(doc, move || {
        let _ = tx.send(());
    });
    // The whole fixture page at the base width is one 360 × 480 tile.
    assert_eq!(renderer.page_height_for(0, renderer::BASE_WIDTH), 480);
    let key = Key {
        page: 0,
        page_w: renderer::BASE_WIDTH,
        tx: 0,
        ty: 0,
    };
    // The first ask only queues the tile.
    assert!(renderer.tile(key).is_none());
    rx.recv_timeout(Duration::from_secs(20))
        .expect("worker wakes the UI thread after rendering");
    let image = renderer.tile(key).expect("rendered tile");
    assert_eq!((image.width, image.height), (360, 480));

    // A ready whole-page image is the fallback while sharp tiles render.
    renderer.begin_frame();
    let base = renderer
        .any_page_image(0, 720)
        .expect("base image of the page");
    assert_eq!((base.width, base.height), (360, 480));
    assert!(renderer.any_page_image(1, 720).is_none());
}

// ---- annotation store --------------------------------------------------

#[test]
fn annotation_store_undo_redo_bounds_and_hit_testing() {
    let mut store = Store::new();
    assert!(!store.can_undo() && !store.can_redo());
    let revision = store.revision;

    let highlight = Annotation {
        page: 0,
        kind: Kind::Highlight,
        color: annotations::colors::YELLOW,
        rects: vec![
            zant::pdf::text::Rect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 12.0,
            },
            zant::pdf::text::Rect {
                x: 10.0,
                y: 40.0,
                width: 40.0,
                height: 12.0,
            },
        ],
        text: "Second page".to_string(),
        ..Default::default()
    };
    let id = store.add(highlight);
    assert_eq!(store.all().len(), 1);
    assert!(store.revision > revision);
    assert_eq!(store.get(id).expect("annotation").text, "Second page");
    assert!(store.can_undo());
    // bounds unite the markup boxes; hit tests use them
    let bounds = store.get(id).expect("annotation").bounds();
    assert_eq!(
        (bounds.x, bounds.y, bounds.width, bounds.height),
        (10.0, 20.0, 100.0, 32.0)
    );
    let annotation = store.get(id).expect("annotation");
    assert!(annotation.hit(50.0, 25.0));
    assert!(!annotation.hit(200.0, 25.0));

    // A second step: ink strokes hit along the segment, undo / redo restore.
    let ink = Annotation {
        page: 0,
        kind: Kind::Ink,
        color: annotations::colors::RED,
        strokes: vec![vec![[100.0, 60.0], [140.0, 80.0], [180.0, 70.0]]],
        width: 2.0,
        ..Default::default()
    };
    let ink_id = store.add(ink);
    assert_eq!(store.all().len(), 2);
    assert_eq!(store.all()[1].kind, Kind::Ink);
    assert_eq!(store.all()[1].strokes[0].len(), 3);
    assert!(store.get(ink_id).expect("ink").hit(120.0, 70.0));
    assert!(!store.get(ink_id).expect("ink").hit(120.0, 90.0));

    assert!(store.undo());
    assert_eq!(store.all().len(), 1);
    assert!(store.can_redo());
    assert!(store.redo());
    assert_eq!(store.all().len(), 2);

    // Editing contents records a step; unchanged values are no-ops (the
    // revision is bumped on every change).
    store.set_contents(id, "remember this");
    assert_eq!(store.get(id).expect("annotation").contents, "remember this");
    let after_edit = store.revision;
    store.set_contents(id, "remember this");
    assert_eq!(store.revision, after_edit);

    assert!(store.remove(ink_id));
    assert_eq!(store.all().len(), 1);
    assert!(!store.remove(ink_id));

    // Loading a saved list replaces everything and clears history.
    store.load(&[]);
    assert!(store.all().is_empty());
    assert!(!store.can_undo() && !store.can_redo());
}

// ---- annotated export --------------------------------------------------

#[test]
fn annotated_copy_writes_a_valid_pdf() {
    let source = fixture("widgets.pdf");
    let out = tmp_path("test-annotated.pdf");
    let _ = std::fs::remove_file(&out);
    let source_len = std::fs::metadata(&source).expect("fixture metadata").len();

    let annotation = native::Annotation {
        page: 0,
        kind: native::annot_kind::HIGHLIGHT,
        color: annotations::colors::YELLOW,
        width: 2.0,
        // One (x, y, width, height) box per line, in displayed-page points.
        rects: vec![40.0, 380.0, 200.0, 20.0],
        contents: Some("remember this".to_string()),
        ..Default::default()
    };
    let out_str = out.to_str().expect("utf-8 out path");
    assert!(native::save_annotated(
        source.to_str().expect("utf-8 source path"),
        None,
        out_str,
        &[annotation],
    ));

    // A valid PDF that still opens, larger than the source.
    let saved = Document::open(out_str, None).expect("saved PDF opens");
    assert_eq!(saved.page_count(), 1);
    let saved_len = std::fs::metadata(&out).expect("saved metadata").len();
    assert!(saved_len > source_len);
}
