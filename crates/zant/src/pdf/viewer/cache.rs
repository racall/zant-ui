//! Building the PDF viewer's retained tree: [`view`], the page layers, their
//! tiles and the scrollbars.
//!
//! The page tile cache itself is `pdf::renderer`: this module asks it
//! for the low-resolution base image of each page plus the sharp tiles of the
//! visible part at the current zoom, and paints whatever is ready; missing
//! tiles are queued by the renderer's worker and wake the UI thread. Nothing
//! here renders a PDF synchronously.

use std::rc::Rc;

use crate::Str;
use crate::core::node::ImageFit;
use crate::layout::flex;
use crate::pdf::annotations::Kind;
use crate::pdf::document::Document;
use crate::pdf::renderer::{self as renderer_mod, Key as RenderKey, Renderer};
use crate::pdf::text::PageText;
use crate::theme::tokens::{self, Theme};
use crate::ui::actions::{Action, KeyAction, PointerAction};
use crate::ui::builders::{column, image, pointer_area, text};
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, ImageOptions, PointerAreaOptions, TextOptions};
use crate::ui::{self, Style, styled};

use super::annotate;
use super::{Fit, Inner, Tool, ViewOptions, Viewer};

/// Logical px snapped to the display grid.
fn snap(value: f32, scale: f32) -> f32 {
    (value * scale).round() / scale
}

pub fn view(viewer: &Viewer, options: ViewOptions) -> Node {
    let vp_key = format!("{}-vp", options.key);
    let bounds = ui::key_bounds(&vp_key);
    let theme = ui::current_theme();
    let mut children: Vec<Node> = Vec::new();
    let mut inner = viewer.borrow_mut();
    inner.options = options.clone();
    inner.padding = options.padding;
    inner.page_gap = options.page_gap;

    if options.toolbar && inner.doc.is_some() {
        children.push(super::layout::toolbar(&inner, viewer, &options, theme));
        children.push(styled(
            column(BoxOptions::default()),
            Style {
                height: Some(1.0),
                width_percent: Some(100.0),
                background: Some(theme.color_split),
                ..Default::default()
            },
        ));
    }

    // viewport size of the last layout (on_resize rebuilds when it changes)
    if let Some(b) = bounds {
        let resized = (b.width - inner.view_w).abs() > 0.5 || (b.height - inner.view_h).abs() > 0.5;
        let anchor = if resized {
            inner.anchor_at(0.0, 0.0)
        } else {
            None
        };
        inner.view_w = b.width;
        inner.view_h = b.height;
        if resized {
            if inner.fit != Fit::None {
                inner.apply_fit(anchor);
            } else {
                inner.clamp_scroll();
            }
        }
    }

    if inner.doc.is_some() {
        if inner.layout_zoom == 0.0 {
            if inner.fit != Fit::None
                && let Some(zoom) = inner.fit_zoom(inner.fit)
            {
                inner.zoom = zoom;
            }
            inner.relayout();
        } else if inner.layout_zoom != inner.zoom {
            inner.relayout();
        }
        if let Some((page, y)) = inner.pending_goto
            && inner.view_h > 0.0
        {
            inner.pending_goto = None;
            inner.go_to_page(page, y);
        }
        inner.clamp_scroll();
        // search once the layout is known (results reveal relative to it)
        if inner.view_h > 0.0 && inner.continue_search() {
            ui::wake_after(1);
        }
    }

    // Events run `on_change` / `on_annotation` themselves
    // (`Viewer::flush_callbacks`). What is still due here came from a call
    // without an event (the app driving the viewer API, or this build's own
    // layout): run it as an effect and build again, so the callbacks' state
    // changes show in this frame.
    if inner.pending_change || inner.pending_annotation {
        let (change, annotation) = inner.take_callbacks();
        if change.is_some() || annotation.is_some() {
            ui::effect(Action::new(move |cx| {
                if let Some(callback) = &change {
                    callback.call(cx);
                }
                if let Some(callback) = &annotation {
                    callback.call(cx);
                }
                ui::request_rebuild();
            }));
        }
    }

    let mut vp_children: Vec<Node> = Vec::new();
    match inner.doc.clone() {
        Some(doc) => {
            if let Some(renderer) = inner.renderer.as_ref() {
                renderer.begin_frame();
            }
            if inner.view_h > 0.0 && inner.tops.len() == doc.page_count() {
                build_pages(&inner, &doc, &mut vp_children, theme, &options);
                build_scrollbars(&inner, viewer, &mut vp_children, theme, &options);
                if options.selection_menu && inner.tool == Tool::Select && inner.drag.is_none() {
                    super::select::selection_menu(
                        &inner,
                        viewer,
                        &mut vp_children,
                        theme,
                        &options,
                    );
                }
            }
        }
        None => {
            let message = match inner.open_error {
                Some(super::OpenErrorKind::PasswordRequired) => options.labels.password.clone(),
                Some(super::OpenErrorKind::FileNotFound) => options.labels.cannot_open.clone(),
                None => Str::default(),
            };
            if !message.is_empty() {
                vp_children.push(column(BoxOptions {
                    width_percent: Some(100.0),
                    height_percent: Some(100.0),
                    justify: flex::Justify::Center,
                    align_items: flex::Align::Center,
                    children: vec![text(
                        message,
                        TextOptions {
                            size: 14.0,
                            color: Some(theme.color_text_secondary),
                            ..Default::default()
                        },
                    )],
                    ..Default::default()
                }));
            }
        }
    }

    let cursor = if inner.tool == Tool::Ink || inner.tool == Tool::Note {
        crate::platform::platform::Cursor::Crosshair
    } else {
        match inner.hover {
            super::Hover::Link => crate::platform::platform::Cursor::PointingHand,
            super::Hover::Text => crate::platform::platform::Cursor::Ibeam,
            super::Hover::None => crate::platform::platform::Cursor::Arrow,
        }
    };

    let pointer_viewer = viewer.clone();
    let on_pointer = PointerAction::new(move |cx, info| pointer_viewer.pointer(cx, info));
    let resize_viewer = viewer.clone();
    let key_viewer = viewer.clone();
    let on_key = KeyAction::new(move |cx, event| key_viewer.key(cx, event));

    children.push(pointer_area(PointerAreaOptions {
        r#box: BoxOptions {
            key: Some(Str::from(vp_key)),
            grow: 1.0,
            shrink: 1.0,
            basis: Some(0.0),
            width_percent: Some(100.0),
            clip: true,
            background: Some(options.background.unwrap_or(theme.color_bg_layout)),
            on_resize: Some(Action::new(move |_| resize_viewer.borrow_mut().on_resize())),
            children: vp_children,
            ..Default::default()
        },
        on_pointer,
        capture_scroll: true,
        cursor: Some(cursor),
        focusable: true,
        on_key: Some(on_key),
        ..Default::default()
    }));

    column(BoxOptions {
        key: Some(options.key),
        grow: 1.0,
        width_percent: Some(100.0),
        height_percent: Some(100.0),
        align_items: flex::Align::Stretch,
        children,
        ..Default::default()
    })
}

fn build_pages(
    inner: &Inner,
    doc: &Document,
    out: &mut Vec<Node>,
    theme: Theme,
    options: &ViewOptions,
) {
    let scale = ui::display_scale();
    let renderer = inner.renderer.as_ref();
    let selection = inner.selection();
    let sel_color = tokens::with_alpha(theme.color_primary, 0x55);
    const MATCH_COLOR: u32 = 0x66ff_d400;
    const ACTIVE_COLOR: u32 = 0x99ff_9100;
    // pages intersecting the viewport, plus one screen of prefetch
    let first = inner.page_near_y(-inner.view_h * 0.5);
    let mut page = first;
    while page < doc.page_count() {
        let origin = inner.page_origin(page);
        if origin[1] > inner.view_h * 1.5 {
            break;
        }
        let size = inner.page_size_px(page);
        if origin[1] + size[1] < -inner.view_h * 0.5 {
            page += 1;
            continue;
        }
        let visible = origin[1] < inner.view_h && origin[1] + size[1] > 0.0;
        let x = snap(origin[0], scale);
        let y = snap(origin[1], scale);
        let mut layers: Vec<Node> = Vec::new();

        // base layer: a small whole-page image (or the closest ready one)
        let page_px_w = ((size[0] * scale).round() as u32).max(1);
        if let Some(r) = renderer {
            let base_w = renderer_mod::BASE_WIDTH.min(page_px_w);
            let base = r.tile(RenderKey {
                page: page as u32,
                page_w: base_w,
                tx: 0,
                ty: 0,
            });
            let under = r.any_page_image(page as u32, page_px_w).or(base);
            if let Some(img) = under {
                layers.push(fill(
                    image(ImageOptions {
                        key: Some(Str::from(format!("{}-p{}-b", options.key, page))),
                        image: Some(Rc::new(img)),
                        fit: ImageFit::Fill,
                        radius: 0.0,
                        ..Default::default()
                    }),
                    0.0,
                    0.0,
                    size[0],
                    size[1],
                ));
            }
            // sharp tiles for the visible part
            if visible {
                tiles(inner, r, page, page_px_w, size, x, y, &mut layers, options);
            }
        }
        // annotations, search matches and selection
        for a in inner.annotations.all() {
            if a.page == page {
                inner.annotation_layer(a, theme, options, &mut layers);
            }
        }
        if inner.tool == Tool::Ink && !inner.ink_points.is_empty() && inner.ink_page == page {
            let strokes = vec![inner.ink_points.clone()];
            layers.push(annotate::ink_node(
                &strokes,
                inner
                    .tool_color
                    .unwrap_or_else(|| Viewer::default_color(Kind::Ink)),
                2.0,
                inner.zoom,
                size,
                None,
            ));
        }
        if let Some(t) = doc.page_text(page)
            && t.count() > 0
        {
            for (i, m) in inner.matches.iter().enumerate() {
                if m.page != page {
                    continue;
                }
                let color = if inner.active_match == Some(i) {
                    ACTIVE_COLOR
                } else {
                    MATCH_COLOR
                };
                highlight(&t, m.from, m.to, inner.zoom, color, &mut layers);
            }
            if let Some(sel) = selection
                && page >= sel.start.page
                && page <= sel.end.page
            {
                let from = if page == sel.start.page {
                    sel.start.index
                } else {
                    0
                };
                let to = if page == sel.end.page {
                    sel.end.index
                } else {
                    t.count() - 1
                };
                if from <= to {
                    highlight(
                        &t,
                        from,
                        to.min(t.count() - 1),
                        inner.zoom,
                        sel_color,
                        &mut layers,
                    );
                }
            }
        }
        out.push(styled(
            column(BoxOptions {
                key: Some(Str::from(format!("{}-p{}", options.key, page))),
                children: layers,
                ..Default::default()
            }),
            Style {
                absolute: Some(true),
                left: Some(x),
                top: Some(y),
                width: Some(size[0]),
                height: Some(size[1]),
                background: Some(0xffff_ffff),
                shadow_color: Some(0x1f00_0000),
                shadow_blur: Some(6.0),
                shadow_offset_y: Some(1.0),
                ..Default::default()
            },
        ));
        page += 1;
    }
}

fn fill(node: Node, x: f32, y: f32, w: f32, h: f32) -> Node {
    styled(
        node,
        Style {
            absolute: Some(true),
            left: Some(x),
            top: Some(y),
            width: Some(w),
            height: Some(h),
            ..Default::default()
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn tiles(
    inner: &Inner,
    r: &Renderer,
    page: usize,
    page_px_w: u32,
    size: [f32; 2],
    x: f32,
    y: f32,
    out: &mut Vec<Node>,
    options: &ViewOptions,
) {
    let page_px_h = r.page_height_for(page as u32, page_px_w);
    // device px per logical px of this page image
    let k = page_px_w as f32 / size[0];
    let ts = renderer_mod::TILE_SIZE as f32;
    let cols = page_px_w.div_ceil(renderer_mod::TILE_SIZE);
    let rows = page_px_h.div_ceil(renderer_mod::TILE_SIZE);
    let mut ty: u32 = 0;
    while ty < rows {
        let top = ty as f32 * ts / k;
        let h = renderer_mod::TILE_SIZE.min(page_px_h - ty * renderer_mod::TILE_SIZE) as f32 / k;
        if y + top > inner.view_h || y + top + h < 0.0 {
            ty += 1;
            continue;
        }
        let mut tx: u32 = 0;
        while tx < cols {
            let left = tx as f32 * ts / k;
            let w =
                renderer_mod::TILE_SIZE.min(page_px_w - tx * renderer_mod::TILE_SIZE) as f32 / k;
            if x + left > inner.view_w || x + left + w < 0.0 {
                tx += 1;
                continue;
            }
            let Some(img) = r.tile(RenderKey {
                page: page as u32,
                page_w: page_px_w,
                tx: tx as u16,
                ty: ty as u16,
            }) else {
                tx += 1;
                continue;
            };
            out.push(fill(
                image(ImageOptions {
                    key: Some(Str::from(format!(
                        "{}-p{}-t{}-{}-{}",
                        options.key, page, page_px_w, tx, ty
                    ))),
                    image: Some(Rc::new(img)),
                    fit: ImageFit::Fill,
                    radius: 0.0,
                    ..Default::default()
                }),
                left,
                top,
                w,
                h,
            ));
            tx += 1;
        }
        ty += 1;
    }
}

fn highlight(t: &PageText, from: usize, to: usize, zoom: f32, color: u32, out: &mut Vec<Node>) {
    for rc in t.rects(from, to) {
        out.push(styled(
            column(BoxOptions::default()),
            Style {
                absolute: Some(true),
                left: Some(rc.x * zoom),
                top: Some(rc.y * zoom),
                width: Some(rc.width * zoom),
                height: Some(rc.height * zoom),
                background: Some(color),
                radius: Some(1.0),
                ..Default::default()
            },
        ));
    }
}

fn build_scrollbars(
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
