//! `ui::pdf_preview`: a native single-page PDF
//! preview with an optional toolbar (PDFKit on macOS, PDFium on Windows).
//!
//! Interaction is inspired by EmbedPDF, but this remains a native Skia UI
//! (no WebView).
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::node::{ButtonSize, ImageFit};
use crate::layout::flex;
use crate::platform::pdf;
use crate::platform::platform::set_clipboard;

use super::super::actions::Action;
use super::super::element::Node;
use super::super::options::{BoxOptions, ButtonOptions, ImageOptions, PdfOptions, TextOptions};
use super::super::styled::inset;
use super::{button, column, image, row, text};

/// Buffer length for the temp PNG path of a rendered PDF page.
const PATH_BUF_LEN: usize = 4096;
/// Buffer length for the extracted text of a page.
const PAGE_TEXT_BUF_LEN: usize = 64 * 1024;

/// `ui::pdf_preview`: a native single-page PDF preview with an optional toolbar
/// (PDFKit on macOS, PDFium on Windows).
pub fn pdf_preview(options: PdfOptions) -> Node {
    let count = pdf::page_count(&options.src);
    let page = if count == 0 {
        0
    } else {
        options.page.min(count - 1)
    };
    let mut path_buf = [0u8; PATH_BUF_LEN];
    let rendered = if count > 0 {
        // 2x for Retina, at least one pixel.
        let width = (options.width * 2.0).max(1.0) as u32;
        let height = (options.height * 2.0).max(1.0) as u32;
        pdf::render_page(&options.src, page, width, height, &mut path_buf).map(Str::copy)
    } else {
        None
    };

    let image_node = match rendered {
        Some(path) => image(ImageOptions {
            src: path,
            width: Some(options.width),
            height: Some(options.height),
            fit: ImageFit::Contain,
            radius: 4.0,
            ..Default::default()
        }),
        None => column(BoxOptions {
            width: Some(options.width),
            height: Some(options.height),
            background: Some(0xfff5_f5f5),
            radius: 4.0,
            children: vec![text(
                "Can't preview this PDF",
                TextOptions {
                    size: 13.0,
                    color: Some(0xa600_0000),
                    ..Default::default()
                },
            )],
            ..Default::default()
        }),
    };

    if !options.show_toolbar {
        return image_node;
    }

    let prev_disabled = page == 0 || count == 0;
    let next_disabled = count == 0 || page + 1 >= count;
    let copy_src = options.src.clone();
    let copy_action = Action::new(move |_| {
        let mut buffer = [0u8; PAGE_TEXT_BUF_LEN];
        if let Some(page_text) = pdf::page_text(&copy_src, page, &mut buffer)
            && !page_text.is_empty()
        {
            let _ = set_clipboard(page_text);
        }
    });
    let toolbar = row(BoxOptions {
        gap: 6.0,
        align_items: flex::Align::Center,
        children: vec![
            button(ButtonOptions {
                key: Some("pdf-first".into()),
                text: "|‹".into(),
                size: ButtonSize::Small,
                disabled: prev_disabled,
                index_action: options.on_page_change.clone(),
                index: 0,
                ..Default::default()
            }),
            button(ButtonOptions {
                key: Some("pdf-prev".into()),
                text: "‹".into(),
                size: ButtonSize::Small,
                disabled: prev_disabled,
                index_action: options.on_page_change.clone(),
                index: if page > 0 { page - 1 } else { 0 },
                ..Default::default()
            }),
            text(
                format!("{} / {}", if count == 0 { 0 } else { page + 1 }, count),
                TextOptions {
                    size: 12.0,
                    color: Some(0xa600_0000),
                    ..Default::default()
                },
            ),
            button(ButtonOptions {
                key: Some("pdf-next".into()),
                text: "›".into(),
                size: ButtonSize::Small,
                disabled: next_disabled,
                index_action: options.on_page_change.clone(),
                index: page + 1,
                ..Default::default()
            }),
            button(ButtonOptions {
                key: Some("pdf-last".into()),
                text: "›|".into(),
                size: ButtonSize::Small,
                disabled: next_disabled,
                index_action: options.on_page_change,
                index: if count > 0 { count - 1 } else { 0 },
                ..Default::default()
            }),
            button(ButtonOptions {
                key: Some("pdf-copy".into()),
                text: "Copy text".into(),
                size: ButtonSize::Small,
                disabled: count == 0,
                on_click: Some(copy_action),
                ..Default::default()
            }),
        ],
        ..Default::default()
    });

    column(BoxOptions {
        key: options.key,
        gap: 8.0,
        padding: inset(8.0),
        background: options.background,
        radius: 8.0,
        children: vec![toolbar, image_node],
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::NodeKind;

    #[test]
    fn pdf_preview_without_a_document_shows_the_fallback() {
        let node = pdf_preview(PdfOptions {
            key: Some("preview".into()),
            src: "definitely-not-a-file.pdf".into(),
            width: 320.0,
            height: 420.0,
            background: Some(0xff00_0000),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("preview"));
        assert_eq!(element.layout.gap, 8.0);
        assert_eq!(element.layout.padding, inset(8.0));
        assert_eq!(element.visual.radius, 8.0);
        assert_eq!(element.visual.background, Some(0xff00_0000));
        assert_eq!(element.children.len(), 2, "toolbar + fallback");

        let toolbar = element.children[0].element();
        assert_eq!(toolbar.layout.direction, flex::Direction::Row);
        assert_eq!(toolbar.layout.gap, 6.0);
        assert_eq!(toolbar.layout.align_items, flex::Align::Center);
        let keys: Vec<&str> = toolbar
            .children
            .iter()
            .filter(|child| child.element().kind == NodeKind::Button)
            .map(|child| child.element().key.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(
            keys,
            ["pdf-first", "pdf-prev", "pdf-next", "pdf-last", "pdf-copy"]
        );
        let page_label = toolbar.children[2].element();
        assert_eq!(page_label.kind, NodeKind::Text);
        assert_eq!(page_label.content, "0 / 0");
        assert_eq!(page_label.text_style.font_size, 12.0);
        assert_eq!(page_label.text_style.color, 0xa600_0000);
        let first = toolbar.children[0].element();
        assert!(!first.enabled, "no pages: the navigation is disabled");
        assert_eq!(first.button_size, ButtonSize::Small);
        let last = toolbar.children[5].element();
        assert_eq!(last.key.as_deref(), Some("pdf-copy"));
        assert!(!last.enabled, "no pages: nothing to copy");

        let fallback = element.children[1].element();
        assert_eq!(fallback.kind, NodeKind::Container);
        assert_eq!(fallback.layout.width, Some(320.0));
        assert_eq!(fallback.layout.height, Some(420.0));
        assert_eq!(fallback.visual.background, Some(0xfff5_f5f5));
        assert_eq!(fallback.visual.radius, 4.0);
        let message = fallback.children[0].element();
        assert_eq!(message.kind, NodeKind::Text);
        assert_eq!(message.content, "Can't preview this PDF");
        assert_eq!(message.text_style.font_size, 13.0);
        assert_eq!(message.text_style.color, 0xa600_0000);
    }

    #[test]
    fn pdf_preview_without_a_toolbar_is_just_the_page() {
        let node = pdf_preview(PdfOptions {
            key: Some("plain".into()),
            src: "definitely-not-a-file.pdf".into(),
            show_toolbar: false,
            width: 100.0,
            height: 120.0,
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.children.len(), 1, "only the fallback page");
        assert!(
            element.key.is_none(),
            "the page node is returned, not the keyed wrapper"
        );
    }
}
