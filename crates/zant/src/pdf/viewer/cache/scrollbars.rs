//! The reader's scrollbars.

use super::*;

pub(super) fn build_scrollbars(
    inner: &Inner,
    viewer: &Viewer,
    out: &mut Vec<Node>,
    theme: Theme,
    options: &ViewOptions,
) {
    let thumb_color = tokens::with_alpha(
        theme.color_text,
        if inner.bar_drag.is_some() { 0x70 } else { 0x40 },
    );
    if inner.content_h > inner.view_h + 0.5 {
        let track = inner.view_h - 8.0;
        let thumb = (track * inner.view_h / inner.content_h).max(28.0);
        let top = 4.0 + (track - thumb) * (inner.scroll_y / inner.max_scroll_y());
        let pointer_viewer = viewer.clone();
        out.push(pointer_area(PointerAreaOptions {
            r#box: BoxOptions {
                key: Some(Str::from(format!("{}-vbar", options.key))),
                absolute: true,
                right: Some(0.0),
                top: Some(top),
                width: Some(12.0),
                height: Some(thumb),
                align_items: flex::Align::Center,
                children: vec![styled(
                    column(BoxOptions::default()),
                    Style {
                        width: Some(6.0),
                        height: Some(thumb),
                        radius: Some(3.0),
                        background: Some(thumb_color),
                        ..Default::default()
                    },
                )],
                ..Default::default()
            },
            on_pointer: PointerAction::new(move |cx, info| {
                let changed = pointer_viewer.borrow_mut().on_vbar(info);
                pointer_viewer.flush_callbacks(cx);
                changed
            }),
            ..Default::default()
        }));
    }
    if inner.content_w > inner.view_w + 0.5 {
        let track = inner.view_w - 8.0;
        let thumb = (track * inner.view_w / inner.content_w).max(28.0);
        let left = 4.0 + (track - thumb) * (inner.scroll_x / inner.max_scroll_x());
        let pointer_viewer = viewer.clone();
        out.push(pointer_area(PointerAreaOptions {
            r#box: BoxOptions {
                key: Some(Str::from(format!("{}-hbar", options.key))),
                absolute: true,
                bottom: Some(0.0),
                left: Some(left),
                width: Some(thumb),
                height: Some(12.0),
                justify: flex::Justify::Center,
                flex_direction: Some(flex::Direction::Column),
                children: vec![styled(
                    column(BoxOptions::default()),
                    Style {
                        width: Some(thumb),
                        height: Some(6.0),
                        radius: Some(3.0),
                        background: Some(thumb_color),
                        ..Default::default()
                    },
                )],
                ..Default::default()
            },
            on_pointer: PointerAction::new(move |cx, info| {
                let changed = pointer_viewer.borrow_mut().on_hbar(info);
                pointer_viewer.flush_callbacks(cx);
                changed
            }),
            ..Default::default()
        }));
    }
}
