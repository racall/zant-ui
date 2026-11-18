//! The selection menu and synchronous callbacks.

use super::*;

/// An app that counts the viewer's callbacks in its own state.
pub(super) struct CallbackApp {
    pub(super) viewer: Viewer,
    pub(super) changes: u32,
    pub(super) annotations: u32,
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

pub(super) fn click_key(runtime: &mut Runtime, key: &str) {
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
