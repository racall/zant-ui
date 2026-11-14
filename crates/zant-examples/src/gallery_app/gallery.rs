//! The imperative gallery of `zant-gallery`.
//!
//! The gallery is the framework's retained-mode demo: it builds the tree by
//! hand (`Tree::add_container` / `add_text` / `add_button` / `add_input`) and
//! mutates nodes on interaction instead of rebuilding a declarative tree.
//! The callbacks and the event loop live in
//! [`super::app`]; this module only builds the tree and returns the node ids
//! the app keeps.
#![forbid(unsafe_code)]

use zant::core::geometry::Edges;
use zant::core::node::{ButtonSize, ButtonType, FlexStyle, NodeData, NodeId};
use zant::core::tree::Tree;
use zant::layout::flex;
use zant::renderer::text::{Style as TextStyle, Weight};
use zant::theme::tokens::Theme;

/// Node ids `GalleryState` keeps (`counter_text`, `value_text`,
/// `increment_button` and the input fields).
pub struct GalleryIds {
    pub counter_text: NodeId,
    pub value_text: NodeId,
    pub increment_button: NodeId,
    /// The four input fields of the Inputs card (editable and disabled).
    pub inputs: Vec<NodeId>,
}

/// The gallery's own state (the counter).
pub struct GalleryState {
    pub theme: Theme,
    pub counter: u32,
}

/// The default `FlexStyle` with `f` applied.
fn flex_style(f: impl FnOnce(&mut FlexStyle)) -> FlexStyle {
    let mut style = FlexStyle::default();
    f(&mut style);
    style
}

/// A titled card; returns the card node the content goes into.
fn add_card(tree: &mut Tree, parent: NodeId, title: &str) -> NodeId {
    let theme = tree.theme;
    let card = tree.add_container(
        Some(parent),
        flex_style(|style| {
            style.direction = flex::Direction::Column;
            style.align_items = flex::Align::Stretch;
            style.gap = 10.0;
            style.padding = Edges::symmetric(16.0, 14.0);
        }),
    );
    {
        let node = tree.get_mut(card);
        node.visual.background = Some(theme.color_bg_container);
        node.visual.border_color = Some(theme.color_border_secondary);
        node.visual.border_width = 1.0;
        node.visual.radius = 10.0;
    }
    tree.add_text(
        Some(card),
        title,
        TextStyle {
            font_size: 16.0,
            weight: Weight::Semibold,
            color: theme.color_text,
            wrap: false,
            ..Default::default()
        },
    );
    card
}

/// A secondary-colored description line.
fn add_description(tree: &mut Tree, parent: NodeId, value: &str, theme: &Theme) {
    tree.add_text(
        Some(parent),
        value,
        TextStyle {
            font_size: 12.0,
            color: theme.color_text_secondary,
            wrap: true,
            ..Default::default()
        },
    );
}

impl GalleryState {
    pub fn new(theme: Theme) -> GalleryState {
        GalleryState { theme, counter: 0 }
    }

    /// Builds the whole window tree.
    pub fn build(&mut self, tree: &mut Tree) -> GalleryIds {
        let root = tree.root;
        {
            let node = tree.get_mut(root);
            node.visual.background = Some(self.theme.color_bg_base);
            node.style.direction = flex::Direction::Column;
            node.style.align_items = flex::Align::Stretch;
            node.style.gap = 0.0;
        }

        let header = tree.add_container(
            Some(root),
            flex_style(|style| {
                style.direction = flex::Direction::Column;
                style.justify = flex::Justify::Center;
                style.gap = 4.0;
                style.height = Some(90.0);
                style.padding = Edges {
                    left: 34.0,
                    right: 34.0,
                    top: 18.0,
                    bottom: 14.0,
                };
            }),
        );
        tree.get_mut(header).visual.background = Some(self.theme.color_bg_container);

        tree.add_text(
            Some(header),
            "Zant UI — Component Gallery",
            TextStyle {
                font_size: 26.0,
                weight: Weight::Semibold,
                color: self.theme.color_text,
                wrap: false,
                ..Default::default()
            },
        );
        tree.add_text(
            Some(header),
            "Native Desktop UI powered by Rust + Skia",
            TextStyle {
                font_size: 14.0,
                color: self.theme.color_text_secondary,
                wrap: false,
                ..Default::default()
            },
        );

        let divider = tree.add_container(
            Some(root),
            flex_style(|style| {
                style.height = Some(1.0);
            }),
        );
        tree.get_mut(divider).visual.background = Some(self.theme.color_border_secondary);

        let body = tree.add_container(
            Some(root),
            flex_style(|style| {
                style.direction = flex::Direction::Row;
                style.align_items = flex::Align::Start;
                style.gap = 20.0;
                style.grow = 1.0;
                style.padding = Edges {
                    left: 32.0,
                    right: 32.0,
                    top: 18.0,
                    bottom: 18.0,
                };
            }),
        );

        let left = tree.add_container(
            Some(body),
            flex_style(|style| {
                style.direction = flex::Direction::Column;
                style.align_items = flex::Align::Stretch;
                style.gap = 14.0;
                style.basis = Some(0.0);
                style.grow = 1.0;
            }),
        );
        let right = tree.add_container(
            Some(body),
            flex_style(|style| {
                style.direction = flex::Direction::Column;
                style.align_items = flex::Align::Stretch;
                style.gap = 14.0;
                style.width = Some(322.0);
            }),
        );

        self.build_flex_card(tree, left);
        self.build_buttons_card(tree, left);
        let (increment_button, counter_text) = self.build_interactive_card(tree, left);
        self.build_typography_card(tree, right);
        let (value_text, inputs) = self.build_inputs_card(tree, right);

        tree.mark_layout_dirty();
        GalleryIds {
            counter_text,
            value_text,
            increment_button,
            inputs,
        }
    }

    fn build_flex_card(&mut self, tree: &mut Tree, parent: NodeId) {
        let card = add_card(tree, parent, "Flex Layout");
        add_description(
            tree,
            card,
            "Flex rows and columns with gaps, padding, grow and wrap.",
            &self.theme,
        );

        let row = tree.add_container(
            Some(card),
            flex_style(|style| {
                style.direction = flex::Direction::Row;
                style.align_items = flex::Align::Center;
                style.gap = 8.0;
                style.height = Some(44.0);
            }),
        );
        self.add_box(tree, row, self.theme.color_primary, None, 1.0, 30.0);
        self.add_box(tree, row, 0xff91_caff, Some(60.0), 0.0, 30.0);
        self.add_box(tree, row, 0xffba_e0ff, None, 1.0, 30.0);

        let nested = tree.add_container(
            Some(card),
            flex_style(|style| {
                style.direction = flex::Direction::Row;
                style.align_items = flex::Align::Center;
                style.gap = 6.0;
                style.height = Some(34.0);
                style.padding = Edges::symmetric(6.0, 4.0);
            }),
        );
        {
            let node = tree.get_mut(nested);
            node.visual.background = Some(self.theme.color_fill_secondary);
            node.visual.radius = 6.0;
        }
        self.add_box(tree, nested, 0xffff_d591, Some(44.0), 0.0, 18.0);
        self.add_box(tree, nested, self.theme.color_primary, None, 1.0, 10.0);
        self.add_box(tree, nested, 0xffff_f1b8, Some(30.0), 0.0, 18.0);
    }

    fn build_typography_card(&mut self, tree: &mut Tree, parent: NodeId) {
        let card = add_card(tree, parent, "Typography");
        tree.add_text(
            Some(card),
            "Heading",
            TextStyle {
                font_size: 22.0,
                weight: Weight::Semibold,
                color: self.theme.color_text,
                wrap: false,
                ..Default::default()
            },
        );
        tree.add_text(
            Some(card),
            "Body text is rendered with Skia paragraph shaping and system font fallback.",
            TextStyle {
                font_size: 14.0,
                color: self.theme.color_text,
                wrap: true,
                ..Default::default()
            },
        );
        tree.add_text(
            Some(card),
            "Secondary text · 12 px",
            TextStyle {
                font_size: 12.0,
                color: self.theme.color_text_secondary,
                wrap: false,
                ..Default::default()
            },
        );
        tree.add_text(
            Some(card),
            "中文排版与英文混排，自动字体回退。",
            TextStyle {
                font_size: 14.0,
                color: self.theme.color_text,
                wrap: true,
                ..Default::default()
            },
        );
        tree.add_text(
            Some(card),
            "This paragraph wraps onto multiple lines and its measured height participates in flex layout.",
            TextStyle {
                font_size: 13.0,
                color: self.theme.color_text_secondary,
                wrap: true,
                line_height: 1.5,
                ..Default::default()
            },
        );
    }

    fn build_buttons_card(&mut self, tree: &mut Tree, parent: NodeId) {
        let card = add_card(tree, parent, "Buttons");
        let row1 = tree.add_container(
            Some(card),
            flex_style(|style| {
                style.direction = flex::Direction::Row;
                style.align_items = flex::Align::Center;
                style.gap = 8.0;
            }),
        );
        tree.add_button(
            Some(row1),
            "Primary",
            ButtonType::Primary,
            ButtonSize::Middle,
            None,
        );
        tree.add_button(
            Some(row1),
            "Default",
            ButtonType::Default,
            ButtonSize::Middle,
            None,
        );
        let disabled = tree.add_button(
            Some(row1),
            "Disabled",
            ButtonType::Default,
            ButtonSize::Middle,
            None,
        );
        tree.set_node_enabled(disabled, false);

        let row2 = tree.add_container(
            Some(card),
            flex_style(|style| {
                style.direction = flex::Direction::Row;
                style.align_items = flex::Align::Center;
                style.gap = 8.0;
            }),
        );
        tree.add_button(
            Some(row2),
            "Text",
            ButtonType::Text,
            ButtonSize::Middle,
            None,
        );
        tree.add_button(
            Some(row2),
            "Link",
            ButtonType::Link,
            ButtonSize::Middle,
            None,
        );
        tree.add_button(
            Some(row2),
            "Small",
            ButtonType::Primary,
            ButtonSize::Small,
            None,
        );
        tree.add_button(
            Some(row2),
            "Middle",
            ButtonType::Default,
            ButtonSize::Middle,
            None,
        );
        tree.add_button(
            Some(row2),
            "Large",
            ButtonType::Link,
            ButtonSize::Large,
            None,
        );

        tree.add_text(
            Some(card),
            "Tab to focus · Space / Enter to activate a button",
            TextStyle {
                font_size: 12.0,
                color: self.theme.color_text_secondary,
                wrap: true,
                ..Default::default()
            },
        );
    }

    fn build_inputs_card(&mut self, tree: &mut Tree, parent: NodeId) -> (NodeId, Vec<NodeId>) {
        let card = add_card(tree, parent, "Inputs");

        let row1 = tree.add_container(
            Some(card),
            flex_style(|style| {
                style.direction = flex::Direction::Row;
                style.align_items = flex::Align::Start;
                style.gap = 12.0;
            }),
        );
        let default_input = self.add_input_column(
            tree,
            row1,
            "Default",
            "Zant UI",
            "Type here",
            false,
            true,
            false,
        );
        let placeholder_input = self.add_input_column(
            tree,
            row1,
            "Placeholder",
            "",
            "Type something…",
            false,
            true,
            false,
        );

        let row2 = tree.add_container(
            Some(card),
            flex_style(|style| {
                style.direction = flex::Direction::Row;
                style.align_items = flex::Align::Start;
                style.gap = 12.0;
            }),
        );
        let disabled_input = self.add_input_column(
            tree,
            row2,
            "Disabled",
            "Disabled value",
            "",
            true,
            false,
            false,
        );
        let initial_input = self.add_input_column(
            tree,
            row2,
            "Initial content",
            "中文 English mix",
            "",
            false,
            true,
            false,
        );

        let value_text = tree.add_text(
            Some(card),
            "Value: —",
            TextStyle {
                font_size: 12.0,
                color: self.theme.color_primary,
                wrap: true,
                max_lines: 1,
                ..Default::default()
            },
        );
        tree.get_mut(value_text).style.height = Some(18.0);
        (
            value_text,
            vec![
                default_input,
                placeholder_input,
                disabled_input,
                initial_input,
            ],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn add_input_column(
        &mut self,
        tree: &mut Tree,
        parent: NodeId,
        label: &str,
        value: &str,
        placeholder: &str,
        disabled: bool,
        editable: bool,
        read_only: bool,
    ) -> NodeId {
        let column = tree.add_container(
            Some(parent),
            flex_style(|style| {
                style.direction = flex::Direction::Column;
                style.align_items = flex::Align::Stretch;
                style.gap = 5.0;
                style.basis = Some(0.0);
                style.grow = 1.0;
            }),
        );
        tree.add_text(
            Some(column),
            label,
            TextStyle {
                font_size: 12.0,
                color: self.theme.color_text_secondary,
                wrap: false,
                ..Default::default()
            },
        );
        let input_id = tree.add_input(Some(column), value, placeholder, None);
        {
            let node = tree.get_mut(input_id);
            node.style.height = Some(32.0);
            node.style.width = None;
            if let NodeData::Input(data) = &mut node.data {
                data.read_only = read_only;
                if !editable {
                    data.read_only = true;
                }
            }
        }
        if disabled {
            tree.set_node_enabled(input_id, false);
        }
        input_id
    }

    fn build_interactive_card(&mut self, tree: &mut Tree, parent: NodeId) -> (NodeId, NodeId) {
        let card = add_card(tree, parent, "Interactive Demo");
        let row = tree.add_container(
            Some(card),
            flex_style(|style| {
                style.direction = flex::Direction::Row;
                style.align_items = flex::Align::Center;
                style.gap = 14.0;
            }),
        );
        let increment_button = tree.add_button(
            Some(row),
            "Increment",
            ButtonType::Primary,
            ButtonSize::Middle,
            None,
        );
        let counter_text = tree.add_text(
            Some(row),
            "Clicked: 0",
            TextStyle {
                font_size: 14.0,
                weight: Weight::Medium,
                color: self.theme.color_text,
                wrap: false,
                ..Default::default()
            },
        );
        (increment_button, counter_text)
    }

    fn add_box(
        &mut self,
        tree: &mut Tree,
        parent: NodeId,
        color: u32,
        width: Option<f32>,
        grow: f32,
        height: f32,
    ) -> NodeId {
        let id = tree.add_container(
            Some(parent),
            flex_style(|style| {
                style.width = width;
                style.height = Some(height);
                style.grow = grow;
            }),
        );
        let node = tree.get_mut(id);
        node.visual.background = Some(color);
        node.visual.radius = 4.0;
        id
    }
}
