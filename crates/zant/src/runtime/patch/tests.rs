//! Unit tests.

use super::*;
use crate::ui::app;
use crate::ui::builders::{column, image, text};
use crate::ui::options::{BoxOptions, ImageOptions, TextOptions};
use crate::ui::{Node, RunOptions};
use std::rc::Rc;

struct Doc {
    title: &'static str,
}

impl app::App for Doc {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            children: crate::nodes![text(
                self.title,
                TextOptions {
                    key: Some("title".into()),
                    ..Default::default()
                }
            )],
            ..Default::default()
        })
    }
}

fn title_node(runtime: &Runtime) -> &crate::core::node::RetainedNode {
    let id = runtime.tree.id_of_key("title").expect("the title node");
    runtime.tree.get(id)
}

#[test]
fn patch_updates_text_content_in_place() {
    let mut runtime = Runtime::init(Doc { title: "one" }, RunOptions::default());
    runtime.rebuild_for_test();
    let id = runtime.tree.id_of_key("title").expect("title");
    runtime.app_state_mut::<Doc>().title = "two";
    runtime.rebuild_for_test();
    assert_eq!(runtime.tree.id_of_key("title"), Some(id), "reused node");
    match &title_node(&runtime).data {
        NodeData::Text(data) => assert_eq!(data.content, "two"),
        _ => panic!("expected a text node"),
    }
    // unchanged content on the next build does not dirty the layout again
    runtime.tree.clear_paint();
    runtime.tree.get_mut(runtime.tree.root).dirty.layout = false;
    runtime.rebuild_for_test();
    assert!(!runtime.tree.needs_layout(), "content did not change");
}

#[test]
fn apply_selection_clamps_to_char_boundaries_and_skips_unchanged_values() {
    let mut data = InputData {
        text: String::from("héllo"),
        ..Default::default()
    };
    assert!(apply_selection(&mut data, Some([0, 6])));
    assert_eq!(data.cursor, 6, "len is 6 bytes");
    // "é" is two bytes: a selection in the middle of it moves back
    assert!(!apply_selection(&mut data, Some([0, 6])), "unchanged");
    assert!(apply_selection(&mut data, Some([2, 3])));
    // "héllo": é occupies bytes 1-2, so byte 2 clamps back to 1 while
    // byte 3 (the "l") is a boundary
    assert_eq!(data.anchor, 1, "byte 2 is inside é");
    assert_eq!(data.cursor, 3);
    assert!(data.reveal_caret);
    assert!(data.blink_on);
    assert!(!apply_selection(&mut data, None));
}

#[test]
fn rich_spans_compare_field_by_field() {
    let base = RichSpan {
        text: Str::from("hello"),
        ..Default::default()
    };
    assert!(rich_spans_equal(
        std::slice::from_ref(&base),
        std::slice::from_ref(&base)
    ));
    let mut changed = base.clone();
    changed.font_size = 99.0;
    assert!(!rich_spans_equal(
        std::slice::from_ref(&base),
        std::slice::from_ref(&changed)
    ));
    let mut linked = base.clone();
    linked.link = Str::from("https://example.com");
    assert!(!rich_spans_equal(
        std::slice::from_ref(&base),
        std::slice::from_ref(&linked)
    ));
    assert!(!rich_spans_equal(std::slice::from_ref(&base), &[]));
}

// ── image loading in the reconcile / patch arms ─────────────────────────

struct Picture {
    path: String,
    image: Option<Rc<crate::image::Image>>,
}

impl app::App for Picture {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            children: crate::nodes![image(ImageOptions {
                key: Some("pic".into()),
                src: Str::copy(&self.path),
                image: self.image.clone(),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

fn image_state(runtime: &Runtime) -> &crate::core::node::ImageData {
    let id = runtime.find_key("pic").expect("image node");
    match &runtime.tree.get(id).data {
        NodeData::Image(data) => data,
        _ => panic!("expected an image node"),
    }
}

#[test]
fn the_image_arms_load_paths_and_in_memory_sources() {
    // a 40x20 PNG on disk
    let pixels = vec![255u8; (40 * 20 * 4) as usize];
    let source = crate::image::Image::from_pixels(&pixels, 40, 20, 40 * 4).expect("image pixels");
    let png = source.encode_png().expect("png");
    let path =
        std::env::temp_dir().join(format!("zant_wp21_patch_image_{}.png", std::process::id()));
    std::fs::write(&path, &png).expect("write png");
    let real_path = path.to_str().expect("utf-8 path").to_string();
    let mut runtime = Runtime::init(
        Picture {
            path: String::from("this/image/does/not/exist.png"),
            image: None,
        },
        RunOptions::default(),
    );
    // a created image node with an unreadable path stays a placeholder
    runtime.rebuild_for_test();
    {
        let data = image_state(&runtime);
        assert!(data.handle.is_none());
        assert_eq!((data.intrinsic_width, data.intrinsic_height), (0, 0));
    }
    // the path changes: the patch arm loads it and
    // its intrinsic size re-lays out the box
    runtime.app_state_mut::<Picture>().path = real_path.clone();
    runtime.rebuild_for_test();
    {
        let data = image_state(&runtime);
        assert!(data.handle.is_some(), "the png was decoded");
        assert_eq!((data.intrinsic_width, data.intrinsic_height), (40, 20));
    }
    assert!(
        runtime.tree.needs_layout(),
        "a new intrinsic size dirties the layout"
    );
    // an in-memory source goes through `set_source`
    runtime.app_state_mut::<Picture>().image = Some(Rc::new(source));
    runtime.rebuild_for_test();
    {
        let data = image_state(&runtime);
        assert!(data.handle.is_some());
        assert_eq!(
            data.source_id,
            data.handle.as_ref().map_or(0, |image| image.id)
        );
        assert_eq!((data.intrinsic_width, data.intrinsic_height), (40, 20));
    }
    // switching back to the path loads the file again
    runtime.app_state_mut::<Picture>().image = None;
    runtime.rebuild_for_test();
    {
        let data = image_state(&runtime);
        assert_eq!(data.source_id, 0);
        assert!(data.handle.is_some());
        assert_eq!((data.intrinsic_width, data.intrinsic_height), (40, 20));
    }
    std::fs::remove_file(&path).expect("remove png");
}
