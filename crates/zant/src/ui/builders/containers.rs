//! Container builders: `space`, `divider`, `tag`, `card`.
//!
//! These compose the basic container / text / button builders and only add
//! antd defaults (gaps, paddings, token colours, radii). `alert` / `avatar` /
//! `progress` live in the sibling `feedback` module.
#![forbid(unsafe_code)]

use crate::core::node::{FlexStyle, NodeKind, Visual};
use crate::layout::flex;
use crate::renderer::text as text_mod;

use super::super::context::current_theme;
use super::super::element::{Element, Node};
use super::super::options::{
    BoxOptions, CardOptions, DividerOptions, SpaceOptions, TagOptions, TextOptions,
};
use super::super::styled::inset_xy;
use super::{column, row, text};

/// `ui::space`: a flex row / column with an 8 px default gap.
pub fn space(options: SpaceOptions) -> Node {
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout: FlexStyle {
            direction: options.direction,
            justify: options.justify,
            align_items: options.align_items,
            gap: options.gap,
            width: options.width,
            height: options.height,
            ..FlexStyle::default()
        },
        children: options.children,
        ..Element::default()
    })
}

/// `ui::divider`: a 1 px horizontal rule in the split colour.
pub fn divider(options: DividerOptions) -> Node {
    let theme = current_theme();
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout: FlexStyle {
            height: Some(1.0),
            margin: options.margin,
            shrink: 0.0,
            ..FlexStyle::default()
        },
        visual: Visual {
            background: Some(options.color.unwrap_or(theme.color_border_secondary)),
            ..Visual::default()
        },
        ..Element::default()
    })
}

/// `ui::tag`: an antd tag: a 12 px label on the secondary fill, radius 4.
pub fn tag(options: TagOptions) -> Node {
    let theme = current_theme();
    let children = vec![text(
        options.text,
        TextOptions {
            size: 12.0,
            color: Some(options.color.unwrap_or(theme.color_text)),
            wrap: false,
            ..Default::default()
        },
    )];
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout: FlexStyle {
            direction: flex::Direction::Row,
            align_items: flex::Align::Center,
            padding: inset_xy(8.0, 2.0),
            shrink: 0.0,
            ..FlexStyle::default()
        },
        visual: Visual {
            background: Some(options.background.unwrap_or(theme.color_fill_secondary)),
            radius: 4.0,
            ..Visual::default()
        },
        children,
        ..Element::default()
    })
}

/// `ui::card`: an antd card panel with an optional title / extra header row.
pub fn card(options: CardOptions) -> Node {
    let theme = current_theme();
    let has_header = options.title.is_some() || options.extra.is_some();
    let mut children = options.children;
    if has_header {
        let title = options.title.clone().unwrap_or_default();
        let extra = options.extra.clone().unwrap_or_default();
        children.insert(
            0,
            row(BoxOptions {
                align_items: flex::Align::Center,
                gap: 8.0,
                children: vec![
                    text(
                        title,
                        TextOptions {
                            size: 16.0,
                            weight: text_mod::Weight::Semibold,
                            color: Some(theme.color_text),
                            wrap: false,
                            ..Default::default()
                        },
                    ),
                    // spacer: pushes `extra` to the right edge
                    column(BoxOptions {
                        grow: 1.0,
                        basis: Some(0.0),
                        ..Default::default()
                    }),
                    text(
                        extra,
                        TextOptions {
                            size: 12.0,
                            color: Some(theme.color_text_secondary),
                            wrap: false,
                            ..Default::default()
                        },
                    ),
                ],
                ..Default::default()
            }),
        );
    }
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout: FlexStyle {
            direction: flex::Direction::Column,
            align_items: flex::Align::Stretch,
            gap: options.gap,
            padding: options.padding,
            width: options.width,
            height: options.height,
            shrink: 0.0,
            ..FlexStyle::default()
        },
        visual: Visual {
            background: Some(theme.color_bg_container),
            border_color: if options.bordered {
                Some(theme.color_border_secondary)
            } else {
                None
            },
            border_width: if options.bordered { 1.0 } else { 0.0 },
            radius: 10.0,
            ..Visual::default()
        },
        children,
        ..Element::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Edges;
    use crate::theme::tokens;
    use crate::ui::styled::{Style, inset, styled};

    fn theme() -> tokens::Theme {
        current_theme()
    }

    #[test]
    fn space_sets_direction_gap_and_alignment() {
        let node = space(SpaceOptions {
            key: Some("s".into()),
            children: vec![text("a", TextOptions::default())],
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("s"));
        assert_eq!(element.layout.direction, flex::Direction::Row);
        assert_eq!(element.layout.gap, 8.0);
        assert_eq!(element.layout.align_items, flex::Align::Center);
        assert_eq!(element.layout.justify, flex::Justify::Start);
        assert_eq!(element.layout.shrink, 1.0, "space keeps the flex default");
        assert_eq!(element.children.len(), 1);

        let custom = space(SpaceOptions {
            direction: flex::Direction::Column,
            gap: 3.0,
            align_items: flex::Align::End,
            justify: flex::Justify::SpaceBetween,
            width: Some(120.0),
            height: Some(40.0),
            ..Default::default()
        });
        let element = custom.element();
        assert_eq!(element.layout.direction, flex::Direction::Column);
        assert_eq!(element.layout.gap, 3.0);
        assert_eq!(element.layout.align_items, flex::Align::End);
        assert_eq!(element.layout.justify, flex::Justify::SpaceBetween);
        assert_eq!(element.layout.width, Some(120.0));
        assert_eq!(element.layout.height, Some(40.0));
    }

    #[test]
    fn divider_is_a_one_px_rule() {
        let node = divider(DividerOptions::default());
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.layout.height, Some(1.0));
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(
            element.visual.background,
            Some(theme().color_border_secondary)
        );

        let custom = divider(DividerOptions {
            key: Some("rule".into()),
            margin: Edges {
                top: 4.0,
                bottom: 4.0,
                ..Edges::default()
            },
            color: Some(0xff11_2233),
        });
        let element = custom.element();
        assert_eq!(element.key.as_deref(), Some("rule"));
        assert_eq!(element.layout.margin.top, 4.0);
        assert_eq!(element.layout.margin.bottom, 4.0);
        assert_eq!(element.visual.background, Some(0xff11_2233));
    }

    #[test]
    fn tag_is_a_12px_label_on_the_secondary_fill() {
        let node = tag(TagOptions {
            key: Some("tag".into()),
            text: "new".into(),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("tag"));
        assert_eq!(element.layout.direction, flex::Direction::Row);
        assert_eq!(element.layout.align_items, flex::Align::Center);
        assert_eq!(element.layout.padding, inset_xy(8.0, 2.0));
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(
            element.visual.background,
            Some(theme().color_fill_secondary)
        );
        assert_eq!(element.visual.radius, 4.0);
        assert_eq!(element.children.len(), 1);
        let label = element.children[0].element();
        assert_eq!(label.kind, NodeKind::Text);
        assert_eq!(label.content, "new");
        assert_eq!(label.text_style.font_size, 12.0);
        assert_eq!(label.text_style.color, theme().color_text);
        assert!(!label.text_style.wrap);

        let custom = tag(TagOptions {
            text: "hot".into(),
            color: Some(0xff00_ff00),
            background: Some(0xff00_0000),
            ..Default::default()
        });
        let element = custom.element();
        assert_eq!(element.visual.background, Some(0xff00_0000));
        assert_eq!(element.children[0].element().text_style.color, 0xff00_ff00);
    }

    #[test]
    fn card_sets_the_antd_panel_defaults() {
        let node = card(CardOptions {
            key: Some("card".into()),
            children: vec![text("body", TextOptions::default())],
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("card"));
        assert_eq!(element.layout.direction, flex::Direction::Column);
        assert_eq!(element.layout.align_items, flex::Align::Stretch);
        assert_eq!(element.layout.gap, 12.0);
        assert_eq!(element.layout.padding, Edges::all(20.0));
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(element.visual.background, Some(theme().color_bg_container));
        assert_eq!(element.visual.border_width, 1.0);
        assert_eq!(
            element.visual.border_color,
            Some(theme().color_border_secondary)
        );
        assert_eq!(element.visual.radius, 10.0);
        assert_eq!(element.children.len(), 1, "no header without a title");

        let unbordered = card(CardOptions {
            bordered: false,
            width: Some(300.0),
            height: Some(200.0),
            ..Default::default()
        });
        let element = unbordered.element();
        assert_eq!(element.visual.border_width, 0.0);
        assert_eq!(element.visual.border_color, None);
        assert_eq!(element.layout.width, Some(300.0));
        assert_eq!(element.layout.height, Some(200.0));
    }

    #[test]
    fn card_builds_the_header_row_and_keeps_children() {
        let node = card(CardOptions {
            title: Some("Title".into()),
            extra: Some("More".into()),
            children: vec![
                text("one", TextOptions::default()),
                text("two", TextOptions::default()),
            ],
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.children.len(), 3, "header + two children");
        let header = element.children[0].element();
        assert_eq!(header.kind, NodeKind::Container);
        assert_eq!(header.layout.direction, flex::Direction::Row);
        assert_eq!(header.layout.gap, 8.0);
        assert_eq!(header.layout.align_items, flex::Align::Center);
        assert_eq!(header.children.len(), 3);
        let title = header.children[0].element();
        assert_eq!(title.content, "Title");
        assert_eq!(title.text_style.font_size, 16.0);
        assert_eq!(title.text_style.weight, text_mod::Weight::Semibold);
        let spacer = header.children[1].element();
        assert_eq!(spacer.layout.grow, 1.0);
        assert_eq!(spacer.layout.basis, Some(0.0));
        let extra = header.children[2].element();
        assert_eq!(extra.content, "More");
        assert_eq!(extra.text_style.font_size, 12.0);
        assert_eq!(extra.text_style.color, theme().color_text_secondary);
        assert_eq!(element.children[1].element().content, "one");
        assert_eq!(element.children[2].element().content, "two");
    }

    #[test]
    fn styled_overrides_surface_fields_of_these_builders() {
        let node = styled(
            tag(TagOptions {
                text: "t".into(),
                ..Default::default()
            }),
            Style {
                background: Some(0xff12_3456),
                border_color: Some(0xff65_4321),
                border_width: Some(2.0),
                radius: Some(9.0),
                padding: Some(inset(6.0)),
                height: Some(30.0),
                ..Default::default()
            },
        );
        let element = node.element();
        assert_eq!(
            element.kind,
            NodeKind::Container,
            "the component stays a tag"
        );
        assert_eq!(element.visual.background, Some(0xff12_3456));
        assert_eq!(element.visual.border_color, Some(0xff65_4321));
        assert_eq!(element.visual.border_width, 2.0);
        assert_eq!(element.visual.radius, 9.0);
        assert_eq!(element.layout.padding, inset(6.0));
        assert_eq!(element.layout.height, Some(30.0));
        assert_eq!(element.children.len(), 1, "children survive the override");
    }
}
