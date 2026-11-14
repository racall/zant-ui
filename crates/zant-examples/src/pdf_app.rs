//! `zant-pdf`: port of `examples/pdf.zig` (WP-3.11).
//!
//! `pdf::view` with continuous scrolling, zoom (toolbar, ⌘/Ctrl + wheel,
//! pinch), text selection and copy, search and links. `ZANT_PDF_PAGE` picks
//! the first page shown (0-based; "last" for the last page), `ZANT_PDF_ZOOM` a
//! manual zoom (e.g. 1.5), `ZANT_PDF_SEARCH` an initial search,
//! `ZANT_PDF_DEMO=1` adds sample annotations, and `ZANT_PDF_PROBE=1` prints
//! document facts without a window. ⌘S / Ctrl+S saves an annotated copy.
//! `ZANT_DARK=1` for dark.
//!
//! Deviation from the Zig source (WP-3.11 report): without an argv path the
//! Zig example prints `usage: zant-pdf <file.pdf>` and exits; so that the
//! binary can be captured statically by `scripts/parity/compare.sh` (which
//! passes no arguments), the port opens `tests/fixtures/widgets.pdf` instead
//! (a path relative to the working directory, like the other example assets).
#![forbid(unsafe_code)]

use zant::core::events::{KeyEvent, character_equals};
use zant::pdf::annotations::{Annotation, Kind, Rect};
use zant::pdf::{self, Document, OpenError, Pos, Size, ViewOptions, Viewer};
use zant::platform::dialog;
use zant::platform::platform::WindowOptions;
use zant::theme::tokens::{self, Theme};
use zant::ui::{self, BoxOptions, RunOptions};

/// Document shown when no argv path is given (not committed: the fixture is
/// tracked and reused; see `.gitignore`).
pub const DEFAULT_DOCUMENT: &str = "tests/fixtures/widgets.pdf";

/// Zig `State`.
pub struct PdfApp {
    viewer: Viewer,
    dark: bool,
}

impl PdfApp {
    pub fn new(viewer: Viewer, dark: bool) -> PdfApp {
        PdfApp { viewer, dark }
    }

    /// Zig `State.render`.
    fn build(&mut self) -> ui::Node {
        ui::column(BoxOptions {
            grow: 1.0,
            align_items: zant::layout::flex::Align::Stretch,
            children: vec![pdf::view(
                &self.viewer,
                ViewOptions {
                    key: "pdf".into(),
                    labels: pdf::Labels::ZH,
                    annotation_tools: true,
                    ..ViewOptions::default()
                },
            )],
            ..BoxOptions::default()
        })
    }
}

impl ui::App for PdfApp {
    fn render(&mut self) -> ui::Node {
        self.build()
    }

    /// Zig `State.onKey`: ⌘S / Ctrl+S saves an annotated copy.
    fn on_key(&mut self, key: &KeyEvent) -> bool {
        if !key.modifiers.primary() || !character_equals(key.key, b's') {
            return false;
        }
        let Some(out) = dialog::save_file_as("annotated.pdf", "pdf") else {
            return true;
        };
        if let Err(error) = self.viewer.save_annotated(&out) {
            eprintln!("save failed: {error}");
        }
        true
    }

    /// Zig `State.theme`.
    fn theme(&self) -> Option<Theme> {
        Some(if self.dark {
            tokens::ant_dark()
        } else {
            tokens::ant_light()
        })
    }
}

/// Zig `demoAnnotations`: sample annotations on the first page with text
/// (visual checks).
pub fn demo_annotations(viewer: &Viewer) {
    let Some(document) = viewer.document() else {
        return;
    };
    for page in 0..document.page_count() {
        let Some(text) = document.page_text(page) else {
            continue;
        };
        if text.count() < 400 {
            continue;
        }
        let kinds = [
            Kind::Highlight,
            Kind::Underline,
            Kind::Strikeout,
            Kind::Squiggly,
        ];
        for (i, kind) in kinds.into_iter().enumerate() {
            viewer.set_selection(
                Pos {
                    page,
                    index: 60 + i * 80,
                },
                Pos {
                    page,
                    index: 100 + i * 80,
                },
            );
            let _ = viewer.annotate_selection(kind, None);
        }
        let size = document.page_size(page);
        let mut stroke: Vec<[f32; 2]> = Vec::with_capacity(40);
        for i in 0..40 {
            let f = i as f32;
            stroke.push([
                size.width * 0.15 + f * 6.0,
                size.height * 0.7 + (f / 3.0).sin() * 20.0,
            ]);
        }
        let _ = viewer.annotations_mut().add(Annotation {
            id: 0,
            page,
            kind: Kind::Ink,
            color: 0xffe5484d,
            strokes: vec![stroke],
            width: 2.0,
            ..Annotation::default()
        });
        let note = vec![Rect {
            x: size.width * 0.85,
            y: size.height * 0.2,
            width: 18.0,
            height: 18.0,
        }];
        let _ = viewer.annotations_mut().add(Annotation {
            id: 0,
            page,
            kind: Kind::Note,
            color: 0xffffc53d,
            rects: note,
            contents: String::from("note"),
            ..Annotation::default()
        });
        viewer.go_to_page(page, 0.0);
        viewer.set_selection(Pos { page, index: 420 }, Pos { page, index: 470 });
        return;
    }
}

/// Zig `ZANT_PDF_PROBE=1`: prints document facts without a window.
fn probe(path: &str) -> zant::Result<()> {
    let document = match Document::open(path, None) {
        Ok(document) => document,
        Err(error) => {
            eprintln!("open failed: {error:?}");
            return Ok(());
        }
    };
    let count = document.page_count();
    let size = if count > 0 {
        document.page_size(0)
    } else {
        Size {
            width: 0.0,
            height: 0.0,
        }
    };
    let text = if count > 0 {
        document.page_text(0)
    } else {
        None
    };
    let glyphs = text.as_ref().map(|text| text.count()).unwrap_or(0);
    let runs = text.as_ref().map(|text| text.runs.len()).unwrap_or(0);
    let first = document.page_text_string(0);
    let mut links: usize = 0;
    for i in 0..count {
        links += document.page_links(i).len();
    }
    eprintln!(
        "pages={} size0={:.1}x{:.1} glyphs0={} runs0={} text0={} links={} outline={}",
        count,
        size.width,
        size.height,
        glyphs,
        runs,
        first.len(),
        links,
        document.outline().len()
    );
    Ok(())
}

/// Zig `launch.run(.., .{ .title = "Zant PDF", .window = .{ .width = 900,
/// .height = 900, .min_width = 480, .min_height = 480 } })`.
pub fn run_options() -> RunOptions {
    RunOptions {
        title: String::from("Zant PDF"),
        window: WindowOptions {
            width: 900.0,
            height: 900.0,
            min_width: 480.0,
            min_height: 480.0,
            ..WindowOptions::default()
        },
        ..RunOptions::default()
    }
}

/// Zig `main`.
pub fn run() -> zant::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.get(1) {
        Some(path) => path.clone(),
        None => String::from(DEFAULT_DOCUMENT),
    };
    if std::env::var_os("ZANT_PDF_PROBE").is_some() {
        return probe(&path);
    }
    let viewer = Viewer::new();
    let _ = viewer.open(&path);
    if let Ok(zoom) = std::env::var("ZANT_PDF_ZOOM") {
        viewer.set_zoom(zoom.parse::<f32>().unwrap_or(1.0));
    }
    if let Ok(page) = std::env::var("ZANT_PDF_PAGE") {
        let count = viewer.page_count();
        let page = if page == "last" {
            count.saturating_sub(1)
        } else {
            page.parse::<usize>().unwrap_or(0)
        };
        viewer.go_to_page(page, 0.0);
    }
    if let Ok(query) = std::env::var("ZANT_PDF_SEARCH") {
        viewer.search(&query);
    }
    let dark = std::env::var_os("ZANT_DARK").is_some();
    if std::env::var_os("ZANT_PDF_DEMO").is_some() {
        demo_annotations(&viewer);
    }
    ui::run(PdfApp::new(viewer, dark), run_options())
}

/// Re-exported so the binary and the smoke test use the same error names.
pub type PdfOpenError = OpenError;
