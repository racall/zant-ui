//! Stateful document widgets (`ui::markdown` / `ui::pdf_viewer`) in a real
//! runtime: the block cache and the open document survive rebuilds, a changed
//! source / password reopens the PDF, and unmounting releases the slots.
//!
//! The tests serialise on one lock: PDFKit and the raster renderer are global
//! state and the `ui` build context is per thread.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use zant::Str;
use zant::components::widgets::{MarkdownState, PdfViewerState};
use zant::core::events::{Event, PointerEvent};
use zant::markdown::render::options::Caret;
use zant::pdf::Pos;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::theme::tokens;
use zant::tsx;
use zant::ui::{self, Node};

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    zant_sys::init();
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

fn frame(runtime: &mut Runtime, renderer: &mut Renderer, width: f32, height: f32) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, width, height, 1.0);
    renderer.end();
}

/// The live slot of a mounted widget. Only call it for an id that is mounted:
/// asking creates the slot otherwise.
fn state<T: 'static>(runtime: &Runtime, id: u64) -> Rc<RefCell<T>> {
    let slot = runtime
        .local_states
        .borrow_mut()
        .slot::<T>(id, || panic!("no local state slot {id:x}"));
    assert!(!slot.fresh);
    slot.value
}

fn slot_count(runtime: &Runtime) -> usize {
    runtime.local_states.borrow().count()
}

fn click_key(runtime: &mut Runtime, key: &str) {
    let id = runtime
        .find_key(key)
        .unwrap_or_else(|| panic!("node {key}"));
    let bounds = runtime.tree.get(id).bounds;
    let point = PointerEvent {
        x: bounds.x + bounds.width * 0.5,
        y: bounds.y + bounds.height * 0.5,
        ..Default::default()
    };
    let _ = runtime.dispatch(Event::PointerDown(point));
    let _ = runtime.dispatch(Event::PointerUp(point));
}

// ── The app the TSX compiler generates for this source ────────────────────
//
//   function Reply({ source }) {
//     return <Markdown key="reply" streaming caret="block" fontSize={15}>{source}</Markdown>;
//   }
//   function Document({ src, onChange }) {
//     return <PdfViewer key="document" src={src} toolbar annotationTools
//                       onChange={onChange} height={240} />;
//   }
//   export default function App() {
//     const [source, setSource] = useState("# Title\n\nFirst paragraph.\n\nSecond **bo");
//     const [path, setPath] = useState("tests/fixtures/widgets.pdf");
//     const [visible, setVisible] = useState(true);
//     const [changes, setChanges] = useState(0);
//     return <View gap={8}>
//       <Markdown key={7} source="Independent **cache**" />
//       <PdfViewer src="" height={80} onChange={() => setChanges(changes + 1)} />
//       {visible && <Reply key="answer" source={source} />}
//       {visible && <Document key="file" src={path} onChange={() => setChanges(changes + 1)} />}
//     </View>;
//   }

#[derive(Default)]
struct Reply {
    source: Str,
    id: u64,
}

impl Reply {
    fn build(source: Str, id: u64) -> Node {
        let state = ui::local_state::<Reply>(id, Reply::default);
        let mut instance = state.borrow_mut();
        instance.source = source;
        instance.id = id;
        instance.render()
    }

    fn render(&mut self) -> Node {
        ui::markdown(
            tsx::child_id(self.id, "Markdown", "reply"),
            ui::MarkdownOptions {
                key: Some(Str::from("reply")),
                streaming: true,
                caret: Caret::Block,
                font_size: 15.0,
                source: self.source.clone(),
                ..Default::default()
            },
        )
    }
}

#[derive(Default)]
struct Document {
    src: Str,
    on_change: Option<ui::Action>,
    id: u64,
}

impl Document {
    fn build(src: Str, on_change: ui::Action, id: u64) -> Node {
        let state = ui::local_state::<Document>(id, Document::default);
        let mut instance = state.borrow_mut();
        instance.src = src;
        instance.on_change = Some(on_change);
        instance.id = id;
        instance.render()
    }

    fn render(&mut self) -> Node {
        ui::pdf_viewer(
            tsx::child_id(self.id, "PdfViewer", "document"),
            ui::PdfViewerOptions {
                key: Some(Str::from("document")),
                src: self.src.clone(),
                toolbar: true,
                annotation_tools: true,
                on_change: self.on_change.clone(),
                height: Some(240.0),
                ..Default::default()
            },
        )
    }
}

struct WidgetsApp {
    source: Str,
    path: Str,
    visible: bool,
    changes: f64,
}

impl ui::App for WidgetsApp {
    fn render(&mut self) -> Node {
        ui::view(ui::ViewOptions {
            gap: 8.0,
            children: tsx::nodes([
                Some(ui::markdown(
                    tsx::child_id(0, "Markdown", "7"),
                    ui::MarkdownOptions {
                        key: Some(Str::from("7")),
                        source: Str::from("Independent **cache**"),
                        ..Default::default()
                    },
                )),
                Some(ui::pdf_viewer(
                    tsx::site_id(0, "PdfViewer", 1),
                    ui::PdfViewerOptions {
                        src: Str::from(""),
                        height: Some(80.0),
                        on_change: Some(ui::action(|s: &mut WidgetsApp| s.changes += 1.0)),
                        ..Default::default()
                    },
                )),
                if self.visible {
                    Some(ui::with_key(
                        Reply::build(self.source.clone(), tsx::child_id(0, "Reply", "answer")),
                        Str::from("answer"),
                    ))
                } else {
                    None
                },
                if self.visible {
                    Some(ui::with_key(
                        Document::build(
                            self.path.clone(),
                            ui::action(|s: &mut WidgetsApp| s.changes += 1.0),
                            tsx::child_id(0, "Document", "file"),
                        ),
                        Str::from("file"),
                    ))
                } else {
                    None
                },
            ]),
            ..Default::default()
        })
    }
}

#[test]
fn tsx_widgets_retain_caches_reopen_pdfs_and_release_slots_on_unmount() {
    let _guard = lock();
    let markdown_id = tsx::child_id(tsx::child_id(0, "Reply", "answer"), "Markdown", "reply");
    let pdf_id = tsx::child_id(
        tsx::child_id(0, "Document", "file"),
        "PdfViewer",
        "document",
    );

    let app = WidgetsApp {
        source: Str::from("# Title\n\nFirst paragraph.\n\nSecond **bo"),
        path: fixture("widgets.pdf").into(),
        visible: true,
        changes: 0.0,
    };
    let mut runtime = Runtime::init(app, ui::RunOptions::default());
    let mut renderer = Renderer::create_raster(800, 700).expect("raster renderer");
    // Laid-out frames (the zoom button is clicked below); the PDF open effect
    // of the first one schedules the content build.
    frame(&mut runtime, &mut renderer, 800.0, 700.0);
    frame(&mut runtime, &mut renderer, 800.0, 700.0);
    let first = state::<MarkdownState>(&runtime, markdown_id);
    // Four widget slots, the instances of the two components, and one view
    // slot per rendered Markdown block (one in the keyed widget, three here).
    assert_eq!(first.borrow().cache.entries.len(), 3);
    assert_eq!(slot_count(&runtime), 4 + 2 + 1 + 3);
    let viewer_state = state::<PdfViewerState>(&runtime, pdf_id);
    let viewer = viewer_state.borrow().viewer.clone();
    assert_eq!(viewer.page_count(), 1);
    let doc = viewer.document().expect("document");
    let zoom_key = format!("{}-zin", viewer_state.borrow().prefix);
    assert!(runtime.find_key(&zoom_key).is_some());
    runtime.app_state_mut::<WidgetsApp>().changes = 0.0;
    click_key(&mut runtime, &zoom_key);
    assert_eq!(runtime.app_state_mut::<WidgetsApp>().changes, 1.0);
    let zoom = viewer.zoom();
    let (head_id, head_hash, head_raw, head_ptr, tail_id) = {
        let state = first.borrow();
        let entries = &state.cache.entries;
        (
            entries[0].id,
            entries[0].hash,
            entries[0].raw.clone(),
            entries[0].raw.as_ptr(),
            entries[2].id,
        )
    };

    for source in [
        "# Title\n\nFirst paragraph.\n\nSecond **bold**",
        "# Title\n\nFirst paragraph.\n\nSecond **bold** grows",
        "# Title\n\nFirst paragraph.\n\nSecond **bold** grows\n\nThird",
    ] {
        runtime.app_state_mut::<WidgetsApp>().source = Str::from(source);
        runtime.rebuild_for_test();
        assert!(Rc::ptr_eq(
            &first,
            &state::<MarkdownState>(&runtime, markdown_id)
        ));
        {
            let state = first.borrow();
            let entries = &state.cache.entries;
            // the head block is the same entry, not a re-parsed copy
            assert_eq!(entries[0].id, head_id);
            assert_eq!(entries[0].hash, head_hash);
            assert_eq!(entries[0].raw, head_raw);
            assert_eq!(entries[0].raw.as_ptr(), head_ptr);
            assert_eq!(entries[2].id, tail_id);
            assert!(state.cache.parsed_last <= 2);
        }
        assert!(Arc::ptr_eq(&doc, &viewer.document().expect("document")));
        assert_eq!(viewer.zoom(), zoom);
    }
    runtime.rebuild_for_test();
    assert_eq!(first.borrow().cache.parsed_last, 0);

    // Reopening replaces the document and clears selection / annotations.
    viewer.set_zoom(2.0);
    viewer.scroll_to(0.0, 50.0);
    assert_eq!(viewer.scroll()[1], 50.0);
    viewer.set_selection(Pos { page: 0, index: 0 }, Pos { page: 0, index: 3 });
    assert!(viewer.selection().is_some());
    runtime.app_state_mut::<WidgetsApp>().path = fixture("no-such-widget.pdf").into();
    runtime.rebuild_for_test();
    assert!(viewer.document().is_none());
    assert!(viewer.open_error().is_some());
    assert!(viewer.selection().is_none());
    assert_eq!(viewer.scroll()[1], 0.0);
    runtime.rebuild_for_test();
    // Failed path is not opened again.
    assert!(!viewer_state.borrow().pending);
    runtime.app_state_mut::<WidgetsApp>().path = fixture("widgets.pdf").into();
    runtime.rebuild_for_test();
    assert_eq!(viewer.page_count(), 1);
    assert!(Rc::ptr_eq(
        &viewer_state,
        &state::<PdfViewerState>(&runtime, pdf_id)
    ));

    // Unmounting frees both slots: nothing else keeps the state alive.
    let markdown_slot = Rc::downgrade(&first);
    let pdf_slot = Rc::downgrade(&viewer_state);
    drop(first);
    drop(viewer_state);
    runtime.app_state_mut::<WidgetsApp>().visible = false;
    runtime.rebuild_for_test();
    // the two top-level widgets and the one Markdown block
    assert_eq!(slot_count(&runtime), 2 + 1);
    assert!(markdown_slot.upgrade().is_none());
    assert!(pdf_slot.upgrade().is_none());
    runtime.app_state_mut::<WidgetsApp>().visible = true;
    runtime.rebuild_for_test();
    // as at the start, with the four blocks of the grown reply
    assert_eq!(slot_count(&runtime), 4 + 2 + 1 + 4);
    let fresh = state::<PdfViewerState>(&runtime, pdf_id);
    assert_eq!(fresh.borrow().viewer.zoom(), 1.0);
    assert_eq!(fresh.borrow().viewer.page_count(), 1);
}

/// Hand-written widgets with `ui::styled` overrides.
struct Manual {
    visible: bool,
    password: Option<Str>,
    theme: tokens::Theme,
}

impl ui::App for Manual {
    fn render(&mut self) -> Node {
        if !self.visible {
            return ui::view(ui::ViewOptions::default());
        }
        ui::column(ui::BoxOptions {
            children: vec![
                ui::styled(
                    ui::markdown(
                        11,
                        ui::MarkdownOptions {
                            source: Str::from("**Hello**"),
                            ..Default::default()
                        },
                    ),
                    ui::Style {
                        width: Some(210.0),
                        padding: Some(ui::inset(8.0)),
                        ..Default::default()
                    },
                ),
                ui::styled(
                    ui::pdf_viewer(
                        12,
                        ui::PdfViewerOptions {
                            src: fixture("widgets.pdf").into(),
                            password: self.password.clone(),
                            toolbar: false,
                            height: Some(180.0),
                            ..Default::default()
                        },
                    ),
                    ui::Style {
                        width: Some(260.0),
                        height: Some(200.0),
                        background: Some(ui::current_theme().color_fill_secondary),
                        ..Default::default()
                    },
                ),
            ],
            ..Default::default()
        })
    }

    fn theme(&self) -> Option<tokens::Theme> {
        Some(self.theme)
    }
}

#[test]
fn widgets_support_styled_overrides_themes_sizes_and_password_changes() {
    let _guard = lock();
    let app = Manual {
        visible: true,
        password: None,
        theme: tokens::ant_light(),
    };
    let mut runtime = Runtime::init(app, ui::RunOptions::default());
    let mut renderer = Renderer::create_raster(800, 700).expect("raster renderer");
    for theme in [tokens::ant_light(), tokens::ant_dark()] {
        runtime.app_state_mut::<Manual>().theme = theme;
        for size in [[800.0, 700.0], [360.0, 420.0]] {
            runtime.needs_build = true;
            frame(&mut runtime, &mut renderer, size[0], size[1]);
            let md = runtime.tree.bounds_of_key("md-b").expect("markdown root");
            let pdf = runtime.tree.bounds_of_key("pdf-c").expect("pdf root");
            assert_eq!(md.width, 210.0);
            assert_eq!(pdf.width, 260.0);
            assert_eq!(pdf.height, 200.0);
        }
    }
    let pdf = state::<PdfViewerState>(&runtime, 12);
    let viewer = pdf.borrow().viewer.clone();
    viewer.scroll_to(0.0, 60.0);
    assert_eq!(viewer.scroll()[1], 60.0);
    runtime.app_state_mut::<Manual>().password = Some(Str::from("changed"));
    runtime.rebuild_for_test();
    assert_eq!(pdf.borrow().password.as_deref(), Some("changed"));
    assert_eq!(viewer.scroll()[1], 0.0);
    drop(pdf);
    runtime.app_state_mut::<Manual>().visible = false;
    runtime.rebuild_for_test();
    assert_eq!(slot_count(&runtime), 0);
}
