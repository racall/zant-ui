//! Unit tests.

use super::*;
use crate::Str;
use crate::core::node::{NodeData, RetainedNode, ToggleData, ToggleKind};
use crate::renderer::renderer::Renderer;
use crate::theme::tokens::ant_light;

/// Clear colour: any painted pixel is visibly different from it.
const CLEAR: u32 = 0xff00_ff00;

fn node(kind: ToggleKind, checked: bool) -> RetainedNode {
    let v = if checked { 1.0 } else { 0.0 };
    let data = ToggleData {
        label: Str::from("Label"),
        kind,
        checked,
        check: motion::Animated::at(v),
        mark: motion::Animated::at(v),
        check_t: v,
        mark_t: v,
        ..Default::default()
    };
    let mut node = RetainedNode::new(0, NodeData::Toggle(data));
    node.bounds = Rect::init(8.0, 4.0, 160.0, CONTROL_HEIGHT);
    node
}

fn paint_into(node: &mut RetainedNode, width: usize, height: usize, theme: Theme) -> Renderer {
    let mut renderer =
        Renderer::create_raster(width as u32, height as u32).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("frame canvas");
        canvas.clear(CLEAR);
        paint(node, canvas, theme);
    }
    renderer.end();
    renderer
}

fn changed_pixels(a: &Renderer, b: &Renderer, width: usize, height: usize) -> usize {
    let mut changed = 0;
    for y in 0..height {
        for x in 0..width {
            if a.pixel_at(x, y) != b.pixel_at(x, y) {
                changed += 1;
            }
        }
    }
    changed
}

#[test]
fn checkbox_paints_its_fill_and_tick() {
    let theme = ant_light();
    let mut unchecked = node(ToggleKind::Checkbox, false);
    let plain = paint_into(&mut unchecked, 160, 32, theme);
    // 16px box at (8, 4 + (24 - 16) / 2 = 8); (13, 13) is inside the fill
    assert_eq!(plain.pixel_at(13, 13), Some(theme.color_bg_container));

    let mut checked = node(ToggleKind::Checkbox, true);
    let ticked = paint_into(&mut checked, 160, 32, theme);
    assert_eq!(ticked.pixel_at(13, 13), Some(theme.color_primary));

    // the white tick only shows once the mark animation has started
    let mut no_mark = node(ToggleKind::Checkbox, true);
    no_mark.toggle_mut().expect("toggle").mark_t = 0.0;
    let bare = paint_into(&mut no_mark, 160, 32, theme);
    assert!(changed_pixels(&ticked, &bare, 160, 32) > 0);
    assert_eq!(bare.pixel_at(13, 13), Some(theme.color_primary));
}

#[test]
fn switch_paints_its_track_and_handle() {
    let theme = ant_light();
    // track is 40×22 at (8, 5); the handle is 18 wide at the left / right
    let mut unchecked = node(ToggleKind::SwitchControl, false);
    let off = paint_into(&mut unchecked, 160, 32, theme);
    // knob sits at the left when off: the right end of the track is the
    // (disabled-text coloured) off track
    let off_pixel = off.pixel_at(8 + 35, 4 + 12).expect("pixel");
    assert_ne!(off_pixel, CLEAR);
    assert_ne!(off_pixel, 0xffff_ffff);

    let mut checked = node(ToggleKind::SwitchControl, true);
    let on = paint_into(&mut checked, 160, 32, theme);
    // checked: handle moved right, track left of it is primary
    assert_eq!(on.pixel_at(8 + 4, 4 + 12), Some(theme.color_primary));
    assert_eq!(on.pixel_at(8 + 35, 4 + 12), Some(0xffff_ffff));
    assert_ne!(off_pixel, on.pixel_at(8 + 35, 4 + 12).expect("pixel"));
}

#[test]
fn radio_paints_its_dot() {
    let theme = ant_light();
    let mut unchecked = node(ToggleKind::Radio, false);
    let empty = paint_into(&mut unchecked, 160, 32, theme);
    assert_eq!(empty.pixel_at(16, 16), Some(theme.color_bg_container));

    let mut checked = node(ToggleKind::Radio, true);
    let dotted = paint_into(&mut checked, 160, 32, theme);
    // dotSize = 8 of a 16px box, so the centre 4px are primary
    assert_eq!(dotted.pixel_at(16, 16), Some(theme.color_primary));
    assert!(changed_pixels(&empty, &dotted, 160, 32) > 0);
}

#[test]
fn hover_and_focus_change_pixels() {
    let theme = ant_light();
    let mut plain = node(ToggleKind::Checkbox, false);
    let base = paint_into(&mut plain, 160, 32, theme);

    let mut hovered = node(ToggleKind::Checkbox, false);
    hovered.toggle_mut().expect("toggle").hover_t = 1.0;
    let hover = paint_into(&mut hovered, 160, 32, theme);
    assert!(
        changed_pixels(&base, &hover, 160, 32) > 0,
        "hover must brighten the border"
    );

    let mut focused = node(ToggleKind::Checkbox, false);
    focused.toggle_mut().expect("toggle").focus_visible = true;
    let focus = paint_into(&mut focused, 160, 32, theme);
    assert!(
        changed_pixels(&base, &focus, 160, 32) > 0,
        "keyboard focus draws a ring outside the box"
    );
}

#[test]
fn disabled_checkbox_keeps_a_dimmed_primary_fill() {
    let theme = ant_light();
    let mut checked = node(ToggleKind::Checkbox, true);
    let enabled = paint_into(&mut checked, 160, 32, theme);
    let mut disabled = node(ToggleKind::Checkbox, true);
    disabled.enabled = false;
    let dimmed = paint_into(&mut disabled, 160, 32, theme);
    assert_ne!(dimmed.pixel_at(13, 13), enabled.pixel_at(13, 13));
    // the fill is the primary colour at 40 % over the background
    let mut reference =
        Renderer::create_raster(160, 32).expect("raster renderer for the reference fill");
    {
        let canvas = reference.begin(1.0).expect("reference canvas");
        canvas.clear(CLEAR);
        canvas.fill_rect(
            Rect::init(0.0, 0.0, 160.0, 32.0),
            tokens::with_alpha(theme.color_primary, 0x66),
        );
    }
    reference.end();
    assert_eq!(dimmed.pixel_at(13, 13), reference.pixel_at(13, 13));
}

#[test]
fn checkbox_checked_animation_interpolates() {
    let theme = ant_light();
    let mut data = ToggleData {
        checked: true,
        ..Default::default()
    };

    assert!(
        animate(&mut data, 1_000, theme),
        "a transition just started"
    );
    assert_eq!(data.check_t, 0.0);
    assert_eq!(data.mark_t, 0.0);

    // halfway through the 300ms fill
    animate(&mut data, 1_150, theme);
    assert!(data.check_t > 0.0 && data.check_t < 1.0);

    // well past every transition: settled at the checked state
    assert!(!animate(&mut data, 10_000, theme));
    assert_eq!(data.check_t, 1.0);
    assert_eq!(data.mark_t, 1.0);
    assert_eq!(data.hover_t, 0.0);
}

#[test]
fn switch_animate_retargets_check_on_flip() {
    let theme = ant_light();
    let mut data = ToggleData {
        kind: ToggleKind::SwitchControl,
        ..Default::default()
    };
    animate(&mut data, 1_000, theme);
    assert_eq!(data.check_t, 0.0);
    data.checked = true;
    assert!(animate(&mut data, 1_000, theme));
    animate(&mut data, 10_000, theme);
    assert_eq!(data.check_t, 1.0);
}

#[test]
fn tree_paint_dispatches_to_the_toggle_module() {
    use crate::core::geometry::Size;
    use crate::core::tree::Tree;
    let theme = ant_light();
    let mut tree = Tree::init(theme);
    let root = tree.root;
    tree.add_toggle(Some(root), "A", ToggleKind::Checkbox, true);
    tree.layout(Size {
        width: 200.0,
        height: 60.0,
    });
    let mut renderer = Renderer::create_raster(200, 60).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("frame canvas");
        canvas.clear(CLEAR);
        tree.paint(canvas);
    }
    renderer.end();
    // the 16px box at (0, 4); (5, 5) is its primary fill (no tick there)
    assert_eq!(renderer.pixel_at(5, 5), Some(theme.color_primary));
}

#[test]
fn measure_width_and_height_follow_the_label_and_style() {
    let theme = ant_light();
    let style = FlexStyle::default();
    // no label: just the indicator (the empty paragraph is special-cased)
    let mut bare = ToggleData {
        kind: ToggleKind::Checkbox,
        ..Default::default()
    };
    assert_eq!(
        measure_width(&mut bare, &style, true, theme, 1000.0),
        CHECKBOX_SIZE
    );
    let mut switch = ToggleData {
        kind: ToggleKind::SwitchControl,
        ..Default::default()
    };
    assert_eq!(
        measure_width(&mut switch, &style, true, theme, 1000.0),
        SWITCH_WIDTH
    );
    // a label adds its longest line plus the 8px gap
    let mut labelled = ToggleData {
        label: Str::from("Save"),
        ..Default::default()
    };
    let width = measure_width(&mut labelled, &style, true, theme, 1000.0);
    assert!(width > CHECKBOX_SIZE + LABEL_GAP);
    // explicit style sizes win
    let sized = FlexStyle {
        width: Some(200.0),
        height: Some(40.0),
        ..Default::default()
    };
    assert_eq!(
        measure_width(&mut labelled, &sized, true, theme, 0.0),
        200.0
    );
    assert_eq!(measure_height(&sized), 40.0);
    assert_eq!(measure_height(&style), CONTROL_HEIGHT);
}
