//! Unit tests.

use super::*;
use crate::core::node::NodeData;
use crate::renderer::renderer::Renderer;
use crate::theme::tokens::ant_light;

const CLEAR: u32 = 0xff00_ff00;
const WIDTH: usize = 240;
const HEIGHT: usize = 64;

fn input_node() -> RetainedNode {
    let mut node = RetainedNode::new(0, NodeData::Input(InputData::default()));
    node.bounds = Rect::init(10.0, 10.0, 200.0, 32.0);
    node.visual.radius = 6.0;
    node.focusable = true;
    node
}

fn paint_into(node: &mut RetainedNode) -> Renderer {
    let mut renderer =
        Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("frame canvas");
        canvas.clear(CLEAR);
        paint(node, canvas, ant_light());
    }
    renderer.end();
    renderer
}

fn data(node: &mut RetainedNode) -> &mut InputData {
    match &mut node.data {
        NodeData::Input(data) => data,
        _ => panic!("not an input node"),
    }
}

#[test]
fn measure_width_and_height_use_the_default_control_size() {
    assert_eq!(measure_width(&FlexStyle::default()), 220.0);
    assert_eq!(measure_height(&FlexStyle::default()), 32.0);
    assert_eq!(control_height(), 32.0);
    let style = FlexStyle {
        width: Some(240.0),
        height: Some(40.0),
        ..Default::default()
    };
    assert_eq!(measure_width(&style), 240.0);
    assert_eq!(measure_height(&style), 40.0);
}

#[test]
fn text_column_honours_padding_and_content_max_width() {
    let bounds = Rect::init(10.0, 10.0, 200.0, 32.0);
    let data = InputData::default();
    assert_eq!(text_left(&data, bounds), 21.0);
    assert_eq!(text_width(&data, bounds), 178.0);
    let centred = InputData {
        content_max_width: 100.0,
        ..Default::default()
    };
    assert_eq!(text_left(&centred, bounds), 10.0 + 50.0);
    assert_eq!(text_width(&centred, bounds), 100.0);
    // a content width wider than the field keeps the padding
    let wide = InputData {
        content_max_width: 400.0,
        ..Default::default()
    };
    assert_eq!(text_left(&wide, bounds), 21.0);
    assert_eq!(text_width(&wide, bounds), 178.0);
}

#[test]
fn paint_draws_the_shell_value_and_centres_the_text() {
    let theme = ant_light();
    let mut node = input_node();
    data(&mut node).text = String::from("Zant UI");
    data(&mut node).text_color = theme.color_text;
    data(&mut node).rebuild_display();
    let renderer = paint_into(&mut node);
    // inside the shell: the container background
    assert_eq!(renderer.pixel_at(20, 26), Some(theme.color_bg_container));
    // a rounded corner stays clear
    assert_eq!(renderer.pixel_at(10, 10), Some(CLEAR));
    // the value darkens some pixels of the text column
    let ink = (0..32)
        .flat_map(|y| (0..178).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            renderer.pixel_at(21 + x as usize, 11 + y as usize) != Some(theme.color_bg_container)
        })
        .count();
    assert!(ink > 20, "the value is painted ({ink} px)");
}

#[test]
fn paint_shows_the_placeholder_when_empty() {
    let theme = ant_light();
    let mut node = input_node();
    data(&mut node).placeholder = crate::Str::Static("Type something…");
    let renderer = paint_into(&mut node);
    let ink = (0..32)
        .flat_map(|y| (0..178).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            renderer.pixel_at(21 + x as usize, 11 + y as usize) != Some(theme.color_bg_container)
        })
        .count();
    assert!(ink > 20, "the placeholder is painted ({ink} px)");
}

#[test]
fn paint_draws_the_caret_only_when_focused_and_blinking() {
    let mut blink_off = input_node();
    data(&mut blink_off).text = String::from("ab");
    data(&mut blink_off).text_color = ant_light().color_text;
    data(&mut blink_off).rebuild_display();
    let unfocused = paint_into(&mut blink_off);

    let mut blink_on = input_node();
    {
        let data = data(&mut blink_on);
        data.text = String::from("ab");
        data.text_color = ant_light().color_text;
        data.focused = true;
        data.blink_on = true;
        data.rebuild_display();
    }
    let focused = paint_into(&mut blink_on);

    let mut differing = 0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if focused.pixel_at(x, y) != unfocused.pixel_at(x, y) {
                differing += 1;
            }
        }
    }
    assert!(differing > 3, "the caret is painted ({differing} px)");
}

#[test]
fn update_blink_toggles_every_blink_interval() {
    let mut data = InputData {
        focused: true,
        ..Default::default()
    };
    assert!(!update_blink(&mut data, true, 1_000));
    assert!(data.blink_on);
    assert_eq!(data.next_blink_ms, 1_000 + BLINK_INTERVAL_MS);
    assert!(!update_blink(&mut data, true, 1_500));
    assert!(update_blink(&mut data, true, 1_000 + BLINK_INTERVAL_MS));
    assert!(!data.blink_on);
    // losing focus resets the blink
    assert!(!update_blink(&mut data, false, 9_999));
    assert!(data.blink_on);
    assert_eq!(data.next_blink_ms, 0);
}

#[test]
fn editing_helpers_keep_the_caret_and_selection_consistent() {
    let mut data = InputData::default();
    assert!(insert_text(&mut data, "abc"));
    assert_eq!(data.text, "abc");
    assert_eq!((data.cursor, data.anchor), (3, 3));
    select_all(&mut data);
    assert_eq!((data.cursor, data.anchor), (3, 0));
    assert!(has_selection(&data));
    // typing replaces the selection
    assert!(insert_text(&mut data, "Z"));
    assert_eq!(data.text, "Z");
    assert_eq!((data.cursor, data.anchor), (1, 1));
    // backward / forward deletion
    assert!(insert_text(&mut data, "ant"));
    assert_eq!(data.text, "Zant");
    assert!(delete_backward(&mut data));
    assert_eq!(data.text, "Zan");
    move_home(&mut data, false);
    assert!(delete_forward(&mut data));
    assert_eq!(data.text, "an");
    // read-only inputs reject edits
    data.read_only = true;
    assert!(!insert_text(&mut data, "x"));
    assert!(!delete_backward(&mut data));
}

#[test]
fn animate_retargets_the_shell_transitions() {
    crate::motion::motion::set_enabled(true);
    let theme = ant_light();
    let mut data = InputData {
        hovered: true,
        ..Default::default()
    };
    // the first sample starts the transition; the target is not reached
    assert!(animate(&mut data, 1_000, theme));
    assert!(data.hover_t < 1.0);
    let done = !animate(
        &mut data,
        1_000 + i64::from(theme.motion_duration_mid),
        theme,
    );
    assert!(done, "the transition settles after its duration");
    assert_eq!(data.hover_t, 1.0);
}
