//! Media builders: `image`, `icon`, `icon_button`, `vector`. `pdf_preview`
//! lives in the sibling `pdf_preview` module.
//!
//! `ui::icon` is a `vector` node whose drawing comes from `core::icons`, and
//! `ui::icon_button` is a small text-variant button carrying the icon name —
//! the retained-tree painter draws both from `core::icons` (no bitmap).
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::icons;
use crate::core::node::{ButtonSize, ButtonType, FlexStyle, NodeKind, Visual};

use super::super::context::current_theme;
use super::super::element::{Element, Node};
use super::super::options::{
    ButtonOptions, IconButtonOptions, IconOptions, ImageOptions, TooltipOptions, VectorOptions,
};
use super::super::styled::{Style, styled};
use super::{button, tooltip};

/// `ui::image`: a file path or an in-memory image in a rounded, clipped box.
pub fn image(options: ImageOptions) -> Node {
    let image_handle = options.image.as_ref().map(Rc::clone);
    let image_id = options.image.as_ref().map_or(0, |img| img.id);
    // An in-memory image wins, and its handle replaces the path.
    let content = if options.image.is_some() {
        Str::default()
    } else {
        options.src
    };
    Node::new(Element {
        kind: NodeKind::Image,
        key: options.key,
        content,
        image_handle,
        image_id,
        image_fit: options.fit,
        layout: FlexStyle {
            width: options.width,
            height: options.height,
            shrink: 0.0,
            ..FlexStyle::default()
        },
        visual: Visual {
            radius: options.radius,
            clip: true,
            ..Visual::default()
        },
        tooltip: options.tooltip,
        ..Element::default()
    })
}

/// `ui::icon`: a line icon from the Zant icon set.
pub fn icon(options: IconOptions) -> Node {
    let theme = current_theme();
    let drawing = icons::drawing(
        options.name,
        options.size,
        options.color.unwrap_or(theme.color_text),
        options.stroke,
    );
    vector(VectorOptions {
        key: options.key,
        drawing: Rc::new(drawing),
        width: None,
        copy: Str::default(),
    })
}

/// `ui::icon_button`: a square icon-only button with hover feedback.
pub fn icon_button(options: IconButtonOptions) -> Node {
    let theme = current_theme();
    let node = styled(
        button(ButtonOptions {
            key: options.key,
            text: Str::default(),
            variant: ButtonType::Text,
            size: ButtonSize::Small,
            icon: options.icon,
            icon_size: options.size,
            text_color: options.color,
            disabled: options.disabled,
            focusable: options.focusable,
            on_click: options.on_click,
            index_action: options.index_action,
            index: options.index,
            ..Default::default()
        }),
        Style {
            width: Some(options.r#box),
            height: Some(options.r#box),
            padding: Some(Edges::default()),
            radius: Some(options.r#box * 0.28),
            background: if options.active {
                Some(
                    options
                        .active_background
                        .unwrap_or(theme.color_fill_secondary),
                )
            } else {
                None
            },
            ..Default::default()
        },
    );
    match options.tooltip {
        Some(value) => tooltip(TooltipOptions {
            text: value,
            child: node,
        }),
        None => node,
    }
}

/// `ui::vector`: a retained vector drawing (math formulas, diagrams).
pub fn vector(options: VectorOptions) -> Node {
    let width = options.width.unwrap_or(options.drawing.width);
    let height = options.drawing.height();
    Node::new(Element {
        kind: NodeKind::Vector,
        key: options.key,
        content: options.copy,
        drawing: Some(options.drawing),
        layout: FlexStyle {
            width: Some(width),
            height: Some(height),
            shrink: 0.0,
            ..FlexStyle::default()
        },
        ..Element::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::ImageFit;
    use crate::core::vector::{Command, Drawing};
    use crate::image::Image;
    use crate::theme::tokens;
    use crate::ui::actions::Action;
    use crate::ui::context::set_build_theme;

    fn theme() -> tokens::Theme {
        current_theme()
    }

    fn drawing_colour(drawing: &Drawing) -> Option<u32> {
        drawing.commands.first().map(|command| match command {
            Command::Path(path) => path.color,
            Command::Rect(rect) => rect.color,
            Command::Line(line) => line.color,
            Command::Text(text) => text.color,
        })
    }

    #[test]
    fn image_from_a_path_sets_the_box_and_link() {
        let node = image(ImageOptions {
            key: Some("avatar".into()),
            src: "photo.png".into(),
            width: Some(48.0),
            height: Some(48.0),
            fit: ImageFit::Fill,
            radius: 24.0,
            tooltip: Some("Me".into()),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Image);
        assert_eq!(element.key.as_deref(), Some("avatar"));
        assert_eq!(element.content, "photo.png");
        assert!(element.image_handle.is_none());
        assert_eq!(element.image_id, 0);
        assert_eq!(element.image_fit, ImageFit::Fill);
        assert_eq!(element.layout.width, Some(48.0));
        assert_eq!(element.layout.height, Some(48.0));
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(element.visual.radius, 24.0);
        assert!(element.visual.clip, "images are clipped to their radius");
        assert_eq!(element.tooltip.as_deref(), Some("Me"));
    }

    #[test]
    fn image_from_memory_wins_over_the_path() {
        let pixels = vec![255u8; 2 * 2 * 4];
        let source = Image::from_pixels(&pixels, 2, 2, 8).expect("2x2 image");
        let id = source.id;
        let node = image(ImageOptions {
            src: "ignored.png".into(),
            image: Some(Rc::new(source)),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.content, "", "the path is dropped");
        assert_eq!(element.image_id, id);
        assert!(element.image_handle.is_some());
        assert_eq!(element.visual.radius, 8.0, "the default radius");
        assert_eq!(element.image_fit, ImageFit::Cover);
    }

    #[test]
    fn icon_is_a_vector_drawing_sized_to_the_icon_box() {
        let node = icon(IconOptions {
            key: Some("check".into()),
            name: crate::core::node::Icon::Check,
            size: 24.0,
            color: Some(0xff112233),
            stroke: Some(2.0),
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Vector);
        assert_eq!(element.key.as_deref(), Some("check"));
        assert_eq!(element.content, "");
        assert_eq!(element.layout.width, Some(24.0));
        assert_eq!(element.layout.height, Some(24.0));
        assert_eq!(element.layout.shrink, 0.0);
        let drawing = element.drawing.as_ref().expect("retained drawing");
        assert_eq!(drawing.width, 24.0);
        assert_eq!(drawing.ascent, 24.0);
        assert!(!drawing.commands.is_empty());
        assert_eq!(drawing_colour(drawing), Some(0xff11_2233));
    }

    #[test]
    fn icon_uses_the_theme_text_colour_by_default() {
        let node = icon(IconOptions {
            name: crate::core::node::Icon::Search,
            ..Default::default()
        });
        let element = node.element();
        let drawing = element.drawing.as_ref().expect("retained drawing");
        assert_eq!(drawing_colour(drawing), Some(theme().color_text));
        assert_eq!(element.layout.width, Some(18.0), "default icon size");

        set_build_theme(tokens::ant_dark());
        let dark = icon(IconOptions {
            name: crate::core::node::Icon::Search,
            ..Default::default()
        });
        assert_eq!(
            drawing_colour(dark.element().drawing.as_ref().expect("drawing")),
            Some(tokens::ant_dark().color_text)
        );
        set_build_theme(tokens::ant_light());
    }

    #[test]
    fn icon_button_is_a_square_small_text_button() {
        let node = icon_button(IconButtonOptions {
            key: Some("search".into()),
            icon: crate::core::node::Icon::Search,
            size: 16.0,
            r#box: 28.0,
            color: Some(0xff00_00ff),
            active: true,
            disabled: false,
            on_click: Some(Action::new(|_| {})),
            index_action: Some(crate::ui::actions::index_action(|s: &mut u32, index| {
                *s = index as u32
            })),
            index: 2,
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Button);
        assert_eq!(element.key.as_deref(), Some("search"));
        assert_eq!(element.button_type, ButtonType::Text);
        assert_eq!(element.button_size, ButtonSize::Small);
        assert_eq!(element.content, "");
        assert_eq!(element.icon, crate::core::node::Icon::Search);
        assert_eq!(element.icon_size, 16.0);
        assert_eq!(element.text_color, Some(0xff00_00ff));
        assert_eq!(element.layout.width, Some(28.0));
        assert_eq!(element.layout.height, Some(28.0));
        assert_eq!(element.visual.radius, 28.0 * 0.28);
        assert_eq!(element.layout.padding, Edges::default());
        assert_eq!(
            element.visual.background,
            Some(theme().color_fill_secondary),
            "an active icon button gets the secondary fill"
        );
        assert!(!element.focusable, "toolbar buttons do not steal focus");
        assert!(element.on_click.is_some());
        assert!(element.index_action.is_some());
        assert_eq!(element.index, 2);

        let custom = icon_button(IconButtonOptions {
            active: true,
            active_background: Some(0xff11_2233),
            ..Default::default()
        });
        assert_eq!(
            custom.element().visual.background,
            Some(0xff11_2233),
            "an explicit active background wins"
        );
        let inactive = icon_button(IconButtonOptions::default());
        assert_eq!(inactive.element().visual.background, None);
    }

    #[test]
    fn icon_button_wraps_the_tooltip() {
        let node = icon_button(IconButtonOptions {
            icon: crate::core::node::Icon::Close,
            tooltip: Some("Close".into()),
            ..Default::default()
        });
        assert_eq!(node.element().tooltip.as_deref(), Some("Close"));
        assert_eq!(node.element().kind, NodeKind::Button);
    }

    #[test]
    fn vector_uses_the_drawing_extent_and_copy_text() {
        let drawing = Drawing {
            width: 30.0,
            ascent: 12.0,
            descent: 3.0,
            commands: Vec::new(),
            hash: 0,
        };
        let node = vector(VectorOptions {
            key: Some("formula".into()),
            drawing: Rc::new(drawing.clone()),
            width: Some(40.0),
            copy: "x^2".into(),
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Vector);
        assert_eq!(element.key.as_deref(), Some("formula"));
        assert_eq!(element.content, "x^2");
        assert_eq!(element.layout.width, Some(40.0));
        assert_eq!(element.layout.height, Some(15.0), "ascent + descent");
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(
            element.drawing.as_ref().map(|d| d.width),
            Some(30.0),
            "the node keeps the drawing"
        );

        let natural = vector(VectorOptions {
            drawing: Rc::new(drawing),
            ..Default::default()
        });
        assert_eq!(natural.element().layout.width, Some(30.0));
        assert_eq!(natural.element().layout.height, Some(15.0));
        assert_eq!(natural.element().content, "");
    }
}
