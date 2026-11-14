//! Continuous scrolling and page layout of the PDF viewer — the layout /
//! navigation half of `src/pdf/viewer.zig` plus the toolbar chrome (WP-3.9).
//!
//! Page tops are computed in content pixels at `layout_zoom`; the viewport
//! scrolls vertically through the stacked pages, and horizontally only when a
//! page is wider than the viewport (then the pages are centred on the widest
//! one, like EmbedPDF's scroll plugin). The toolbar (`toolbar`), its icon
//! buttons and its page / zoom / search actions live here as well: the viewer
//! draws its own chrome with the ordinary `ui::*` builders.

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{ButtonSize, ButtonType, Icon};
use crate::layout::flex;
use crate::theme::tokens::Theme;
use crate::ui::actions::Action;
use crate::ui::builders::{button, icon_button, input, row, space, text, tooltip};
use crate::ui::element::Node;
use crate::ui::options::{
    BoxOptions, ButtonOptions, IconButtonOptions, InputOptions, SpaceOptions, TextOptions,
    TooltipOptions,
};
use crate::ui::{Style, styled};

use super::{BarDrag, Fit, Inner, ViewOptions, Viewer};

impl Inner {
    /// Recomputes the page tops and the content box at `zoom` (Zig
    /// `Viewer.relayout`).
    pub(crate) fn relayout(&mut self) {
        let Some(doc) = self.doc.clone() else {
            return;
        };
        let count = doc.page_count();
        self.tops.clear();
        self.tops.resize(count, 0.0);
        let mut y = self.padding;
        let mut max_w: f32 = 0.0;
        for (i, top) in self.tops.iter_mut().enumerate() {
            let size = doc.page_size(i);
            *top = y;
            y += size.height * self.zoom + self.page_gap;
            max_w = max_w.max(size.width * self.zoom);
        }
        self.max_page_w = max_w;
        self.content_h = if count > 0 {
            y - self.page_gap + self.padding
        } else {
            0.0
        };
        self.content_w = max_w + self.padding * 2.0;
        self.layout_zoom = self.zoom;
    }

    pub(crate) fn max_scroll_x(&self) -> f32 {
        (self.content_w - self.view_w).max(0.0)
    }

    pub(crate) fn max_scroll_y(&self) -> f32 {
        (self.content_h - self.view_h).max(0.0)
    }

    pub(crate) fn clamp_scroll(&mut self) {
        self.scroll_x = self.scroll_x.clamp(0.0, self.max_scroll_x());
        self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll_y());
    }

    pub(crate) fn page_size_px(&self, page: usize) -> [f32; 2] {
        let size = self.doc.as_ref().expect("open document").page_size(page);
        [size.width * self.zoom, size.height * self.zoom]
    }

    /// Top-left of a page in viewport coordinates.
    pub(crate) fn page_origin(&self, page: usize) -> [f32; 2] {
        let size = self.page_size_px(page);
        let x = if self.content_w <= self.view_w {
            (self.view_w - size[0]) / 2.0
        } else {
            self.padding + (self.max_page_w - size[0]) / 2.0 - self.scroll_x
        };
        [x, self.tops[page] - self.scroll_y]
    }

    /// Page whose vertical span (with half the gap) contains viewport y.
    pub(crate) fn page_near_y(&self, vy: f32) -> usize {
        let y = vy + self.scroll_y;
        let n = self.tops.len();
        if n == 0 {
            return 0;
        }
        let (mut lo, mut hi) = (0usize, n);
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if self.tops[mid] - self.page_gap / 2.0 <= y {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// The page near the top third of the viewport becomes `current_page`.
    pub(crate) fn update_current_page(&mut self) {
        if self.tops.is_empty() {
            return;
        }
        let page = self.page_near_y(self.view_h / 3.0);
        if page != self.current_page {
            self.current_page = page;
            self.page_input_page = None;
        }
    }

    /// Scrolls to a page (0-based); `y` is a point offset from its top.
    pub(crate) fn go_to_page(&mut self, page: usize, y: f32) {
        let Some(doc) = self.doc.clone() else {
            return;
        };
        if doc.page_count() == 0 {
            return;
        }
        let p = page.min(doc.page_count() - 1);
        if self.tops.len() != doc.page_count() || self.view_h <= 0.0 {
            self.pending_goto = Some((p, y));
            return;
        }
        self.scroll_y = self.tops[p] + y * self.zoom - self.padding;
        self.clamp_scroll();
        self.current_page = p;
        self.page_input_page = None;
    }

    pub(crate) fn next_page(&mut self) {
        if self.current_page + 1 < self.page_count() {
            self.go_to_page(self.current_page + 1, 0.0);
        }
    }

    pub(crate) fn previous_page(&mut self) {
        if self.current_page > 0 {
            self.go_to_page(self.current_page - 1, 0.0);
        }
    }

    pub(crate) fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.scroll_x += dx;
        self.scroll_y += dy;
        self.clamp_scroll();
        self.update_current_page();
    }

    pub(crate) fn page_count(&self) -> usize {
        self.doc.as_ref().map_or(0, |doc| doc.page_count())
    }

    /// Scrollbar drag (Zig `Viewer.onScrollbar`); `true` while dragging.
    pub(crate) fn on_scrollbar(
        &mut self,
        info: crate::ui::actions::PointerInfo,
        horizontal: bool,
    ) -> bool {
        let track = if horizontal { self.view_w } else { self.view_h };
        let content = if horizontal {
            self.content_w
        } else {
            self.content_h
        };
        if content <= track {
            return false;
        }
        let pos = if horizontal {
            info.window_x
        } else {
            info.window_y
        };
        match info.phase {
            crate::ui::actions::PointerPhase::Down => {
                self.bar_drag = Some(BarDrag {
                    start: pos,
                    scroll: if horizontal {
                        self.scroll_x
                    } else {
                        self.scroll_y
                    },
                    horizontal,
                })
            }
            crate::ui::actions::PointerPhase::Drag => {
                let Some(drag) = self.bar_drag else {
                    return false;
                };
                let ratio = content / track;
                if horizontal {
                    self.scroll_x = drag.scroll + (pos - drag.start) * ratio;
                } else {
                    self.scroll_y = drag.scroll + (pos - drag.start) * ratio;
                }
                self.clamp_scroll();
                self.update_current_page();
            }
            crate::ui::actions::PointerPhase::Up => self.bar_drag = None,
            _ => return false,
        }
        true
    }

    /// Vertical scrollbar drag.
    pub(crate) fn on_vbar(&mut self, info: crate::ui::actions::PointerInfo) -> bool {
        self.on_scrollbar(info, false)
    }

    /// Horizontal scrollbar drag.
    pub(crate) fn on_hbar(&mut self, info: crate::ui::actions::PointerInfo) -> bool {
        self.on_scrollbar(info, true)
    }
}

impl Viewer {
    /// Number of pages of the open document.
    pub fn page_count(&self) -> usize {
        self.borrow().page_count()
    }

    /// Current page (0-based), the page near the top third of the viewport.
    pub fn current_page(&self) -> usize {
        self.borrow().current_page
    }

    /// Scrolls to a page (0-based); `y` is a point offset from its top.
    pub fn go_to_page(&self, page: usize, y: f32) {
        self.borrow_mut().go_to_page(page, y);
    }

    pub fn next_page(&self) {
        self.borrow_mut().next_page();
    }

    pub fn previous_page(&self) {
        self.borrow_mut().previous_page();
    }

    /// Scroll offset of the viewport (content px).
    pub fn scroll(&self) -> [f32; 2] {
        let inner = self.borrow();
        [inner.scroll_x, inner.scroll_y]
    }

    /// Content size of the stacked pages (px at the current zoom).
    pub fn content_size(&self) -> [f32; 2] {
        let inner = self.borrow();
        [inner.content_w, inner.content_h]
    }

    /// The zoom mode (fit width / fit page / manual).
    pub fn fit(&self) -> Fit {
        self.borrow().fit
    }
}

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
        on_change: Some(crate::ui::actions::TextAction::new(move |_cx, value| {
            tb_page_input(&page_input_viewer, value)
        })),
        on_submit: Some(Action::new(move |_cx| tb_page_submit(&page_submit_viewer))),
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
        inner.zoom <= super::MIN_ZOOM + 0.001,
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
        on_click: Some(Action::new(move |_cx| tb_fit(&zoom_viewer))),
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
        inner.zoom >= super::MAX_ZOOM - 0.001,
        viewer,
        tb_zoom_in,
    ));
    if options.annotation_tools {
        children.push(super::annotate::tool_switch(inner, viewer, options));
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
        on_change: Some(crate::ui::actions::TextAction::new(move |_cx, value| {
            tb_search(&search_change_viewer, value)
        })),
        on_key: Some(crate::ui::actions::InputKeyAction::new(
            move |_cx, event, _text, _anchor, _cursor| tb_search_key(&search_key_viewer, event),
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

/// One toolbar icon button with a tooltip (`ui.iconButton` in Zig).
fn toolbar_button(
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
        on_click: Some(Action::new(move |_| callback(&v))),
        ..Default::default()
    })
}

// ---- toolbar buttons and their actions ----------------------------------------------------

fn tb_prev(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.previous_page();
    inner.changed();
}

fn tb_next(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.next_page();
    inner.changed();
}

fn tb_zoom_in(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.zoom_in();
    inner.changed();
}

fn tb_zoom_out(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.zoom_out();
    inner.changed();
}

fn tb_fit(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    let fit = if inner.fit == Fit::Width {
        Fit::Page
    } else {
        Fit::Width
    };
    inner.set_fit_mode(fit);
    inner.changed();
}

fn tb_search(viewer: &Viewer, value: &str) {
    let mut inner = viewer.borrow_mut();
    inner.search(value);
    inner.changed();
}

fn tb_search_next(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.next_match();
    inner.changed();
}

fn tb_search_prev(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    inner.previous_match();
    inner.changed();
}

fn tb_search_key(viewer: &Viewer, event: crate::core::events::KeyEvent) -> bool {
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

fn tb_page_input(viewer: &Viewer, value: &str) {
    let mut inner = viewer.borrow_mut();
    inner.page_input.clear();
    inner.page_input.push_str(value);
    inner.page_input_page = Some(inner.current_page);
}

fn tb_page_submit(viewer: &Viewer) {
    let mut inner = viewer.borrow_mut();
    let trimmed = inner.page_input.trim_matches(' ');
    let n = trimmed.parse::<usize>().unwrap_or(0);
    if n >= 1 {
        inner.go_to_page(n - 1, 0.0);
    }
    inner.page_input_page = None;
    inner.changed();
}
