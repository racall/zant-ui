//! Unit tests.

use super::*;
use crate::core::events::PointerButton;
use crate::core::node::NodeKind;
use crate::ui::actions::{ActionCx, PointerInfo, PointerPhase, action, index_action};
use crate::ui::builders::text;
use crate::ui::options::TextOptions;

fn full_options() -> ViewOptions {
    ViewOptions {
        key: Some("box".into()),
        direction: flex::Direction::Column,
        flex_direction: Some(flex::Direction::Column),
        gap: 1.0,
        row_gap: Some(2.0),
        column_gap: Some(3.0),
        wrap: flex::Wrap::Wrap,
        align_content: flex::AlignContent::Center,
        align_self: flex::AlignSelf::End,
        order: 5,
        padding: Edges {
            left: 4.0,
            right: 5.0,
            top: 6.0,
            bottom: 7.0,
        },
        margin: Edges::all(8.0),
        justify: flex::Justify::Center,
        justify_content: Some(flex::Justify::End),
        align_items: flex::Align::Center,
        width: Some(100.0),
        height: Some(120.0),
        width_percent: Some(50.0),
        height_percent: Some(25.0),
        absolute: true,
        top: Some(9.0),
        right: Some(10.0),
        bottom: Some(11.0),
        left: Some(12.0),
        min_width: Some(13.0),
        min_height: Some(14.0),
        max_width: Some(15.0),
        max_height: Some(16.0),
        grow: 2.0,
        shrink: 3.0,
        basis: Some(17.0),
        background: Some(0xff11_1111),
        border_color: Some(0xff22_2222),
        border_width: 4.0,
        radius: 5.0,
        clip: true,
        scroll: true,
        block_pointer: true,
        selectable: true,
        unselectable: true,
        hover_background: Some(0xff33_3333),
        hover_border_color: Some(0xff44_4444),
        focus_scope: true,
        show_on_hover: true,
        on_resize: Some(action(|s: &mut u32| *s += 1)),
        motion: motion::Presence::Fade,
        motion_origin: motion::Origin::Bottom,
        layout_transition: true,
        shadow_color: Some(0xff55_5555),
        shadow_blur: 18.0,
        shadow_offset_y: 19.0,
        on_click: Some(action(|s: &mut u32| *s += 1)),
        index_action: Some(index_action(|s: &mut u32, i| *s = i as u32)),
        index: 6,
        cursor: Cursor::Arrow,
        style: Some(Style::default()),
        children: vec![text("child", TextOptions::default())],
    }
}

#[test]
fn view_defaults() {
    let defaults = ViewOptions::default();
    assert!(defaults.key.is_none());
    assert_eq!(defaults.direction, flex::Direction::Column);
    assert!(defaults.flex_direction.is_none());
    assert_eq!(defaults.align_items, flex::Align::Stretch);
    assert_eq!(defaults.shrink, 0.0);
    assert_eq!(defaults.grow, 0.0);
    assert_eq!(defaults.cursor, Cursor::PointingHand);
    assert!(defaults.style.is_none());
    assert!(defaults.on_click.is_none());
    assert!(defaults.index_action.is_none());
    assert!(defaults.shadow_color.is_none());
    assert!(defaults.children.is_empty());
}

#[test]
fn box_options_copies_every_field_of_box_options() {
    let options = full_options();
    let box_options = box_options(&options);
    assert_eq!(box_options.key.as_deref(), Some("box"));
    assert_eq!(box_options.gap, 1.0);
    assert_eq!(box_options.row_gap, Some(2.0));
    assert_eq!(box_options.column_gap, Some(3.0));
    assert_eq!(box_options.wrap, flex::Wrap::Wrap);
    assert_eq!(box_options.align_content, flex::AlignContent::Center);
    assert_eq!(box_options.align_self, flex::AlignSelf::End);
    assert_eq!(box_options.order, 5);
    assert_eq!(box_options.padding, options.padding);
    assert_eq!(box_options.margin, Edges::all(8.0));
    assert_eq!(box_options.justify, flex::Justify::Center);
    assert_eq!(box_options.justify_content, Some(flex::Justify::End));
    assert_eq!(box_options.align_items, flex::Align::Center);
    assert_eq!(box_options.flex_direction, Some(flex::Direction::Column));
    assert_eq!(box_options.width, Some(100.0));
    assert_eq!(box_options.height, Some(120.0));
    assert_eq!(box_options.width_percent, Some(50.0));
    assert_eq!(box_options.height_percent, Some(25.0));
    assert!(box_options.absolute);
    assert_eq!(box_options.top, Some(9.0));
    assert_eq!(box_options.right, Some(10.0));
    assert_eq!(box_options.bottom, Some(11.0));
    assert_eq!(box_options.left, Some(12.0));
    assert_eq!(box_options.min_width, Some(13.0));
    assert_eq!(box_options.min_height, Some(14.0));
    assert_eq!(box_options.max_width, Some(15.0));
    assert_eq!(box_options.max_height, Some(16.0));
    assert_eq!(box_options.grow, 2.0);
    assert_eq!(box_options.shrink, 3.0);
    assert_eq!(box_options.basis, Some(17.0));
    assert_eq!(box_options.background, Some(0xff11_1111));
    assert_eq!(box_options.border_color, Some(0xff22_2222));
    assert_eq!(box_options.border_width, 4.0);
    assert_eq!(box_options.radius, 5.0);
    assert!(box_options.clip);
    assert!(box_options.scroll);
    assert!(box_options.block_pointer);
    assert!(box_options.selectable);
    assert!(box_options.unselectable);
    assert_eq!(box_options.hover_background, Some(0xff33_3333));
    assert_eq!(box_options.hover_border_color, Some(0xff44_4444));
    assert!(box_options.focus_scope);
    assert!(box_options.show_on_hover);
    assert!(box_options.on_resize.is_some());
    assert_eq!(box_options.motion, motion::Presence::Fade);
    assert_eq!(box_options.motion_origin, motion::Origin::Bottom);
    assert!(box_options.layout_transition);
    assert_eq!(box_options.children.len(), 1);
}

#[test]
fn flex_direction_alias_takes_precedence_over_direction() {
    let options = ViewOptions {
        direction: flex::Direction::Column,
        flex_direction: Some(flex::Direction::Row),
        ..Default::default()
    };
    assert_eq!(
        box_options(&options).flex_direction,
        Some(flex::Direction::Row)
    );
    let plain = ViewOptions {
        direction: flex::Direction::Row,
        ..Default::default()
    };
    assert_eq!(
        box_options(&plain).flex_direction,
        Some(flex::Direction::Row)
    );
}

#[test]
fn view_builds_a_column_or_a_clickable_area() {
    let plain = view(ViewOptions {
        background: Some(0xff11_2222),
        children: vec![text("a", TextOptions::default())],
        ..Default::default()
    });
    let element = plain.element();
    assert_eq!(element.kind, NodeKind::Container);
    assert_eq!(element.layout.direction, flex::Direction::Column);
    assert_eq!(element.layout.align_items, flex::Align::Stretch);
    assert_eq!(element.layout.shrink, 0.0);
    assert_eq!(element.visual.background, Some(0xff11_2222));
    assert!(element.pointer.is_none(), "no click handler: plain column");
    assert_eq!(element.children.len(), 1);

    let clickable = view(ViewOptions {
        on_click: Some(action(|s: &mut u32| *s += 1)),
        ..Default::default()
    });
    let element = clickable.element();
    assert!(
        element.pointer.is_some(),
        "views with clicks use pointerArea"
    );
    assert_eq!(element.cursor, Some(Cursor::PointingHand));
}

#[test]
fn view_click_fires_through_the_clickable_pointer_area() {
    let mut state = 0u32;
    let node = view(ViewOptions {
        on_click: Some(action(|s: &mut u32| *s += 1)),
        index_action: Some(index_action(|s: &mut u32, i| *s = i as u32)),
        index: 5,
        width: Some(80.0),
        height: Some(40.0),
        ..Default::default()
    });
    let element = node.element();
    let pointer = element.pointer.clone().expect("pointer area");
    let info = PointerInfo {
        phase: PointerPhase::Up,
        x: 10.0,
        y: 10.0,
        width: 80.0,
        height: 40.0,
        button: PointerButton::Left,
        ..Default::default()
    };
    let mut cx = ActionCx::new(&mut state);
    assert!(pointer.action.call(&mut cx, info));
    assert_eq!(state, 5, "index_action wins over on_click");
}

#[test]
fn view_applies_the_shadow_only_when_it_or_the_style_needs_one() {
    let plain = view(ViewOptions {
        children: vec![text("a", TextOptions::default())],
        ..Default::default()
    });
    assert!(plain.element().visual.shadow_color.is_none());

    let shadowed = view(ViewOptions {
        shadow_color: Some(0x6612_3456),
        shadow_blur: 12.0,
        shadow_offset_y: 3.0,
        ..Default::default()
    });
    let element = shadowed.element();
    assert_eq!(element.visual.shadow_color, Some(0x6612_3456));
    assert_eq!(element.visual.shadow_blur, 12.0);
    assert_eq!(element.visual.shadow_offset_y, 3.0);

    // the inline style wins field by field; only the unset shadow fields
    // are filled from the options
    let styled = view(ViewOptions {
        shadow_color: Some(0xff00_0000),
        shadow_blur: 12.0,
        shadow_offset_y: 3.0,
        style: Some(Style {
            shadow_color: Some(0x1100_0000),
            ..Default::default()
        }),
        ..Default::default()
    });
    let element = styled.element();
    assert_eq!(element.visual.shadow_color, Some(0x1100_0000));
    assert_eq!(element.visual.shadow_blur, 12.0);
    assert_eq!(element.visual.shadow_offset_y, 3.0);

    // a style alone is applied last
    let inline = view(ViewOptions {
        background: Some(0xff11_1111),
        style: Some(Style {
            background: Some(0xff22_2222),
            radius: Some(9.0),
            ..Default::default()
        }),
        ..Default::default()
    });
    let element = inline.element();
    assert_eq!(element.visual.background, Some(0xff22_2222));
    assert_eq!(element.visual.radius, 9.0);
}
