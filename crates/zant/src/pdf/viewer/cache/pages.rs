//! Page nodes: placed pages, tiles of the rendered bitmap and search / selection highlights.

use super::*;

pub(super) fn build_pages(
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

pub(super) fn fill(node: Node, x: f32, y: f32, w: f32, h: f32) -> Node {
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
pub(super) fn tiles(
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

pub(super) fn highlight(
    t: &PageText,
    from: usize,
    to: usize,
    zoom: f32,
    color: u32,
    out: &mut Vec<Node>,
) {
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
