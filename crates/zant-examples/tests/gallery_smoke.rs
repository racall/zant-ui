//! Headless smoke test for the `zant-gallery` binary: the imperative app builds
//! its retained tree, paints it on a raster canvas and reacts to a real click.
//!
//! The gallery does not use the declarative `Runtime` (see
//! `zant_examples::gallery_app`), so the frame is driven through
//! [`GalleryApp::draw`], the same entry the native host calls.

use zant::core::events::{Event, PointerEvent};
use zant::core::node::NodeData;
use zant::renderer::renderer::Renderer;
use zant_examples::gallery_app::GalleryApp;

const WIDTH: f32 = 900.0;
const HEIGHT: f32 = 640.0;

fn renderer() -> Renderer {
    Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster renderer")
}

fn frame(app: &mut GalleryApp, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("frame canvas");
    app.draw(canvas, WIDTH, HEIGHT, 1.0);
    renderer.end();
}

fn find_button(app: &GalleryApp, label: &str) -> zant::core::node::NodeId {
    for node in &app.tree.nodes {
        if node.alive
            && let NodeData::Button(data) = &node.data
            && data.label == label
        {
            return node.id;
        }
    }
    panic!("no button {label:?}");
}

fn text_content<'a>(app: &'a GalleryApp, needle: &str) -> Option<&'a str> {
    for node in &app.tree.nodes {
        if node.alive
            && let NodeData::Text(data) = &node.data
            && data.content.contains(needle)
        {
            return Some(&data.content);
        }
    }
    None
}

fn click(app: &mut GalleryApp, id: zant::core::node::NodeId) {
    let bounds = app.tree.get(id).bounds;
    let point = PointerEvent {
        x: bounds.x + bounds.width * 0.5,
        y: bounds.y + bounds.height * 0.5,
        ..Default::default()
    };
    app.handle(Event::PointerDown(point));
    app.handle(Event::PointerUp(point));
}

#[test]
fn gallery_builds_the_same_sections_and_paints() {
    let mut app = GalleryApp::new();
    let mut renderer = renderer();
    frame(&mut app, &mut renderer);

    // five cards: Flex Layout, Buttons, Interactive Demo, Typography, Inputs
    let cards = app
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && matches!(node.data, NodeData::Container(_)))
        .count();
    assert!(cards >= 20, "the gallery containers are in the tree");
    for title in [
        "Flex Layout",
        "Buttons",
        "Interactive Demo",
        "Typography",
        "Inputs",
    ] {
        assert!(text_content(&app, title).is_some(), "card {title:?}");
    }
    // nine buttons, two of them inside the interactive card / rows
    let buttons = app
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && matches!(node.data, NodeData::Button(_)))
        .count();
    assert_eq!(buttons, 9, "eight card buttons plus Increment");
    let inputs = app
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && matches!(node.data, NodeData::Input(_)))
        .count();
    assert_eq!(inputs, 4, "Default, Placeholder, Disabled, Initial content");
    assert_eq!(app.counter(), 0);
}

#[test]
fn gallery_increment_button_updates_the_counter_text() {
    let mut app = GalleryApp::new();
    let mut renderer = renderer();
    frame(&mut app, &mut renderer);
    let increment = find_button(&app, "Increment");
    click(&mut app, increment);
    assert_eq!(app.counter(), 1);
    frame(&mut app, &mut renderer);
    assert!(
        text_content(&app, "Clicked: 1").is_some(),
        "the retained label was updated in place"
    );
    click(&mut app, increment);
    frame(&mut app, &mut renderer);
    assert_eq!(app.counter(), 2);
    assert!(text_content(&app, "Clicked: 2").is_some());
}

#[test]
fn gallery_typing_updates_the_value_read_out() {
    let mut app = GalleryApp::new();
    let mut renderer = renderer();
    frame(&mut app, &mut renderer);
    // the placeholder input is empty; clicking focuses it
    let placeholder = app
        .tree
        .nodes
        .iter()
        .find(|node| {
            let NodeData::Input(data) = &node.data else {
                return false;
            };
            node.alive && &*data.placeholder == "Type something…"
        })
        .map(|node| node.id)
        .expect("the placeholder input");
    let bounds = app.tree.get(placeholder).bounds;
    let point = PointerEvent {
        x: bounds.x + bounds.width - 4.0,
        y: bounds.y + 12.0,
        ..Default::default()
    };
    app.handle(Event::PointerDown(point));
    app.handle(Event::PointerUp(point));
    assert_eq!(app.tree.focused, Some(placeholder));
    app.handle(Event::TextInput(String::from("hi")));
    let text = text_content(&app, "Value: hi");
    assert!(text.is_some(), "the read-out follows the input");
    assert_eq!(text, Some("Value: hi"));
}
