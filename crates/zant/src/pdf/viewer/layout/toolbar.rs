//! The reader's toolbar: page navigation, zoom, fit, search and the page number input.

use super::*;

pub(crate) fn toolbar(inner: &Inner, viewer: &Viewer, options: &ViewOptions, theme: Theme) -> Node {
    let labels = &options.labels;
    let count = inner.page_count();
    let page_text = if inner.page_input_page == Some(inner.current_page) {
        inner.page_input.clone()
    } else {
        format!("{}", inner.current_page + 1)
    };
    let zoom_text = format!("{}%", (inner.zoom * 100.0).round() as u32);
    let match_text = if inner.query.is_empty() {
        String::new()
    } else if inner.matches.is_empty() {
        if inner.search_done {
            labels.no_results.to_string()
        } else {
            String::from("…")
        }
    } else {
        format!(
            "{}/{}",
            inner.active_match.unwrap_or(0) + 1,
            inner.matches.len()
        )
    };
    let mut children: Vec<Node> = Vec::new();
    children.push(toolbar_button(
        &format!("{}-prev", options.key),
        labels.previous_page.clone(),
        Icon::ChevronUp,
        inner.current_page == 0,
        viewer,
        tb_prev,
    ));
    children.push(toolbar_button(
        &format!("{}-next", options.key),
        labels.next_page.clone(),
        Icon::ChevronDown,
        inner.current_page + 1 >= count,
        viewer,
        tb_next,
    ));
    let page_input_viewer = viewer.clone();
    let page_submit_viewer = viewer.clone();
    children.push(input(InputOptions {
        key: Some(Str::from(format!("{}-page", options.key))),
        value: Some(Str::from(page_text)),
        width: Some(44.0),
        height: 24.0,
        font_size: 12.0,
        on_change: Some(crate::ui::actions::TextAction::new(move |cx, value| {
            tb_page_input(&page_input_viewer, value);
            page_input_viewer.flush_callbacks(cx);
        })),
        on_submit: Some(Action::new(move |cx| {
            tb_page_submit(&page_submit_viewer);
            page_submit_viewer.flush_callbacks(cx);
        })),
        ..Default::default()
    }));
    children.push(text(
        format!("/ {count}"),
        TextOptions {
            size: 12.0,
            color: Some(theme.color_text_secondary),
            ..Default::default()
        },
    ));
    children.push(space(SpaceOptions {
        width: Some(8.0),
        ..Default::default()
    }));
    children.push(toolbar_button(
        &format!("{}-zout", options.key),
        labels.zoom_out.clone(),
        Icon::Minus,
        inner.zoom <= crate::pdf::viewer::MIN_ZOOM + 0.001,
        viewer,
        tb_zoom_out,
    ));
    let zoom_viewer = viewer.clone();
    let zoom_button = button(ButtonOptions {
        key: Some(Str::from(format!("{}-zoom", options.key))),
        text: Str::from(zoom_text),
        variant: ButtonType::Text,
        size: ButtonSize::Small,
        width: Some(56.0),
        font_size: Some(12.0),
        focusable: false,
        on_click: Some(Action::new(move |cx| {
            tb_fit(&zoom_viewer);
            zoom_viewer.flush_callbacks(cx);
        })),
        ..Default::default()
    });
    children.push(tooltip(TooltipOptions {
        text: if inner.fit == Fit::Width {
            labels.fit_page.clone()
        } else {
            labels.fit_width.clone()
        },
        child: zoom_button,
    }));
    children.push(toolbar_button(
        &format!("{}-zin", options.key),
        labels.zoom_in.clone(),
        Icon::Plus,
        inner.zoom >= crate::pdf::viewer::MAX_ZOOM - 0.001,
        viewer,
        tb_zoom_in,
    ));
    if options.annotation_tools {
        children.push(crate::pdf::viewer::annotate::tool_switch(
            inner, viewer, options,
        ));
    } else {
        children.push(styled(
            space(SpaceOptions::default()),
            Style {
                width: Some(0.0),
                ..Default::default()
            },
        ));
    }
    children.push(styled(
        space(SpaceOptions::default()),
        Style {
            grow: Some(1.0),
            ..Default::default()
        },
    ));
    let search_change_viewer = viewer.clone();
    let search_key_viewer = viewer.clone();
    children.push(input(InputOptions {
        key: Some(Str::from(format!("{}-search", options.key))),
        value: Some(Str::from(inner.query.clone())),
        placeholder: labels.search.clone(),
        width: Some(160.0),
        height: 24.0,
        font_size: 12.0,
        on_change: Some(crate::ui::actions::TextAction::new(move |cx, value| {
            tb_search(&search_change_viewer, value);
            search_change_viewer.flush_callbacks(cx);
        })),
        on_key: Some(crate::ui::actions::InputKeyAction::new(
            move |cx, event, _text, _anchor, _cursor| {
                let consumed = tb_search_key(&search_key_viewer, event);
                search_key_viewer.flush_callbacks(cx);
                consumed
            },
        )),
        ..Default::default()
    }));
    children.push(styled(
        text(
            match_text,
            TextOptions {
                size: 12.0,
                color: Some(theme.color_text_secondary),
                max_lines: 1,
                ..Default::default()
            },
        ),
        Style {
            min_width: Some(40.0),
            ..Default::default()
        },
    ));
    children.push(toolbar_button(
        &format!("{}-mprev", options.key),
        labels.previous_match.clone(),
        Icon::ChevronUp,
        inner.matches.is_empty(),
        viewer,
        tb_search_prev,
    ));
    children.push(toolbar_button(
        &format!("{}-mnext", options.key),
        labels.next_match.clone(),
        Icon::ChevronDown,
        inner.matches.is_empty(),
        viewer,
        tb_search_next,
    ));
    row(BoxOptions {
        key: Some(Str::from(format!("{}-toolbar", options.key))),
        width_percent: Some(100.0),
        height: Some(40.0),
        gap: 4.0,
        align_items: flex::Align::Center,
        padding: Edges {
            left: 8.0,
            right: 8.0,
            ..Default::default()
        },
        background: Some(theme.color_bg_container),
        children,
        ..Default::default()
    })
}

/// One toolbar icon button with a tooltip.
pub(super) fn toolbar_button(
    key: &str,
    tooltip_text: Str,
    icon: Icon,
    disabled: bool,
    viewer: &Viewer,
    callback: fn(&Viewer),
) -> Node {
    let v = viewer.clone();
    icon_button(IconButtonOptions {
        key: Some(Str::copy(key)),
        icon,
        size: 16.0,
        r#box: 28.0,
        disabled,
        tooltip: Some(tooltip_text),
        on_click: Some(Action::new(move |cx| {
            callback(&v);
            v.flush_callbacks(cx);
        })),
        ..Default::default()
    })
}

// ---- toolbar buttons and their actions ----------------------------------------------------

pub(super) fn tb_prev(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.previous_page();
    inner.changed();
}

pub(super) fn tb_next(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.next_page();
    inner.changed();
}

pub(super) fn tb_zoom_in(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.zoom_in();
    inner.changed();
}

pub(super) fn tb_zoom_out(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.zoom_out();
    inner.changed();
}

pub(super) fn tb_fit(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    let fit = if inner.fit == Fit::Width {
        Fit::Page
    } else {
        Fit::Width
    };
    inner.set_fit_mode(fit);
    inner.changed();
}

pub(super) fn tb_search(viewer: &Viewer, value: &str) {
    let mut inner = viewer.borrow_mut();
    inner.search(value);
    inner.changed();
}

pub(super) fn tb_search_next(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.next_match();
    inner.changed();
}

pub(super) fn tb_search_prev(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.previous_match();
    inner.changed();
}

pub(super) fn tb_search_key(viewer: &Viewer, event: crate::core::events::KeyEvent) -> bool {
    use crate::core::events::Key;
    let mut inner = viewer.borrow_mut();
    match event.key {
        Key::Enter => {
            if event.modifiers.shift {
                inner.previous_match();
            } else {
                inner.next_match();
            }
            inner.changed();
            true
        }
        Key::Escape => {
            inner.search("");
            inner.changed();
            false
        }
        _ => false,
    }
}

pub(super) fn tb_page_input(viewer: &Viewer, value: &str) {
    let mut inner = viewer.borrow_mut();
    inner.page_input.clear();
    inner.page_input.push_str(value);
    inner.page_input_page = Some(inner.current_page);
}

pub(super) fn tb_page_submit(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    let trimmed = inner.page_input.trim_matches(' ');
    let n = trimmed.parse::<usize>().unwrap_or(0);
    if n >= 1 {
        inner.go_to_page(n - 1, 0.0);
    }
    inner.page_input_page = None;
    inner.changed();
}
