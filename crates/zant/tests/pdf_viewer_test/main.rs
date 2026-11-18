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

mod callbacks;
mod fixtures;
mod synthetic;

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
