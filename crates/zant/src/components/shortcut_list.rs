//! Port of `src/components/shortcut_list.zig` — WP-2.5: the keyboard shortcut
//! reference.
//!
//! Grouped "action — keys" rows laid out in balanced columns (macOS / antd
//! Descriptions style). Typically the content of a `ui::modal` opened with
//! mod+/ (⌘/ on macOS, Ctrl+/ on Windows).
//!
//! ```ignore
//! ui::shortcut_list(ShortcutListOptions { groups: vec![
//!     ShortcutGroup { title: "File".into(), items: vec![Shortcut {
//!         label: "Save".into(),
//!         keys: zant::core::keys::label("mod+s").into(),
//!     }] },
//! ], ..Default::default() })
//! ```
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Edges;
use crate::layout::flex;
use crate::ui::builders::{column, row, text};
use crate::ui::context::current_theme;
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, TextOptions};
use crate::ui::styled::{Style, styled};

/// One "action — keys" row (`Shortcut`).
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct Shortcut {
    pub label: Str,
    /// Key text as shown (e.g. "⇧⌘S" / "Ctrl+Shift+S", "Esc"); build it with
    /// `keys::label("mod+shift+s")` so it matches the platform.
    pub keys: Str,
}

/// One group of shortcuts (`ShortcutGroup`).
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct ShortcutGroup {
    pub title: Str,
    pub items: Vec<Shortcut>,
}

/// Options of [`shortcut_list`] (`ShortcutListOptions`).
#[derive(Clone, zant_macros::TsxProps)]
pub struct ShortcutListOptions {
    pub key: Str,
    pub groups: Vec<ShortcutGroup>,
    /// Groups are spread over this many columns, keeping heights balanced.
    pub columns: usize,
    /// Windows key names ("Ctrl+Shift+S") are wider than macOS glyphs ("⇧⌘S").
    pub column_width: f32,
    pub column_gap: f32,
    pub row_height: f32,
    pub font_size: f32,
}

impl Default for ShortcutListOptions {
    fn default() -> Self {
        ShortcutListOptions {
            key: Str::Static("shortcuts"),
            groups: Vec::new(),
            columns: 2,
            // Zig: `if (@import("builtin").os.tag == .macos) 250 else 310`
            column_width: if crate::core::keys::IS_MACOS {
                250.0
            } else {
                310.0
            },
            column_gap: 40.0,
            row_height: 28.0,
            font_size: 13.0,
        }
    }
}

/// Zig `shortcutList`: grouped shortcuts in balanced columns.
pub fn shortcut_list(o: ShortcutListOptions) -> Node {
    let theme = current_theme();
    let n_cols = o.columns.clamp(1, 8);
    let mut cols: [Vec<Node>; 8] = std::array::from_fn(|_| Vec::new());
    let mut heights = [0usize; 8];
    for (gi, g) in o.groups.iter().enumerate() {
        // the shortest column takes the next group (order is kept per column)
        let mut target: usize = 0;
        for c in 1..n_cols {
            if heights[c] < heights[target] {
                target = c;
            }
        }
        let first = cols[target].is_empty();
        cols[target].push(column(BoxOptions {
            key: Some(Str::from(format!("{}-g{}", o.key.as_str(), gi))),
            padding: Edges {
                top: if first { 0.0 } else { 14.0 },
                bottom: 4.0,
                ..Edges::default()
            },
            children: vec![text(
                g.title.clone(),
                TextOptions {
                    size: o.font_size - 1.0,
                    color: Some(theme.color_text_secondary),
                    ..Default::default()
                },
            )],
            ..Default::default()
        }));
        for item in &g.items {
            cols[target].push(row(BoxOptions {
                height: Some(o.row_height),
                gap: 16.0,
                justify: flex::Justify::SpaceBetween,
                align_items: flex::Align::Center,
                children: vec![
                    styled(
                        text(
                            item.label.clone(),
                            TextOptions {
                                size: o.font_size,
                                color: Some(theme.color_text),
                                max_lines: 1,
                                ..Default::default()
                            },
                        ),
                        Style {
                            shrink: Some(1.0),
                            min_width: Some(0.0),
                            ..Default::default()
                        },
                    ),
                    text(
                        item.keys.clone(),
                        TextOptions {
                            size: o.font_size,
                            color: Some(theme.color_text_secondary),
                            ..Default::default()
                        },
                    ),
                ],
                ..Default::default()
            }));
        }
        heights[target] += g.items.len() + 2;
    }
    let columns: Vec<Node> = cols
        .into_iter()
        .take(n_cols)
        .map(|children| {
            column(BoxOptions {
                width: Some(o.column_width),
                align_items: flex::Align::Stretch,
                children,
                ..Default::default()
            })
        })
        .collect();
    row(BoxOptions {
        key: Some(o.key),
        gap: o.column_gap,
        align_items: flex::Align::Start,
        children: columns,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::NodeKind;
    use crate::theme::tokens::ant_light;

    fn groups() -> Vec<ShortcutGroup> {
        vec![
            ShortcutGroup {
                title: "File".into(),
                items: vec![
                    Shortcut {
                        label: "Save".into(),
                        keys: "⌘S".into(),
                    },
                    Shortcut {
                        label: "Open".into(),
                        keys: "⌘O".into(),
                    },
                ],
            },
            ShortcutGroup {
                title: "Edit".into(),
                items: vec![Shortcut {
                    label: "Undo".into(),
                    keys: "⌘Z".into(),
                }],
            },
            ShortcutGroup {
                title: "View".into(),
                items: vec![Shortcut {
                    label: "Search".into(),
                    keys: "⌘F".into(),
                }],
            },
        ]
    }

    #[test]
    fn shortcut_list_defaults_match_the_zig_struct() {
        let defaults = ShortcutListOptions::default();
        assert_eq!(defaults.key.as_str(), "shortcuts");
        assert!(defaults.groups.is_empty());
        assert_eq!(defaults.columns, 2);
        let expected = if crate::core::keys::IS_MACOS {
            250.0
        } else {
            310.0
        };
        assert_eq!(defaults.column_width, expected);
        assert_eq!(defaults.column_gap, 40.0);
        assert_eq!(defaults.row_height, 28.0);
        assert_eq!(defaults.font_size, 13.0);
        let shortcut = Shortcut::default();
        assert!(shortcut.label.is_empty());
        assert!(shortcut.keys.is_empty());
        assert!(ShortcutGroup::default().items.is_empty());
    }

    #[test]
    fn shortcut_list_balances_groups_across_columns() {
        let theme = ant_light();
        let node = shortcut_list(ShortcutListOptions {
            groups: groups(),
            ..Default::default()
        });
        let list = node.element();
        assert_eq!(list.key.as_deref(), Some("shortcuts"));
        assert_eq!(list.kind, NodeKind::Container);
        assert_eq!(list.layout.direction, flex::Direction::Row);
        assert_eq!(list.layout.gap, 40.0);
        assert_eq!(list.layout.align_items, flex::Align::Start);
        assert_eq!(list.children.len(), 2);

        // File (2 rows) goes to the first (both empty) column, then Edit fits
        // the second column and View also goes there: 0 vs 1+2 groups
        let first = list.children[0].element();
        // Zig: `if (@import("builtin").os.tag == .macos) 250 else 310`
        let column_width = if crate::core::keys::IS_MACOS {
            250.0
        } else {
            310.0
        };
        assert_eq!(first.layout.width, Some(column_width));
        assert_eq!(first.layout.align_items, flex::Align::Stretch);
        assert_eq!(first.children.len(), 3, "title group + two rows");
        let group = first.children[0].element();
        assert_eq!(group.key.as_deref(), Some("shortcuts-g0"));
        assert_eq!(group.layout.padding.top, 0.0, "the first group has no gap");
        assert_eq!(group.layout.padding.bottom, 4.0);
        let title = group.children[0].element();
        assert_eq!(title.content, "File");
        assert_eq!(title.text_style.font_size, 12.0);
        assert_eq!(title.text_style.color, theme.color_text_secondary);
        let save = first.children[1].element();
        assert_eq!(save.layout.height, Some(28.0));
        assert_eq!(save.layout.gap, 16.0);
        assert_eq!(save.layout.justify, flex::Justify::SpaceBetween);
        assert_eq!(save.children.len(), 2);
        let label = save.children[0].element();
        assert_eq!(label.content, "Save");
        assert_eq!(label.text_style.font_size, 13.0);
        assert_eq!(label.text_style.color, theme.color_text);
        assert_eq!(label.text_style.max_lines, 1);
        assert_eq!(label.layout.shrink, 1.0);
        assert_eq!(label.layout.min_width, Some(0.0));
        let keys = save.children[1].element();
        assert_eq!(keys.content, "⌘S");
        assert_eq!(keys.text_style.color, theme.color_text_secondary);

        let second = list.children[1].element();
        assert_eq!(second.children.len(), 4, "two title groups + one row each");
        assert_eq!(
            second.children[0].element().key.as_deref(),
            Some("shortcuts-g1")
        );
        assert_eq!(
            second.children[0].element().layout.padding.top,
            0.0,
            "the first group in a column has no gap"
        );
        assert_eq!(
            second.children[2].element().key.as_deref(),
            Some("shortcuts-g2")
        );
        assert_eq!(second.children[2].element().layout.padding.top, 14.0);

        assert_eq!(
            second.children[0].element().children[0].element().content,
            "Edit"
        );
        assert_eq!(
            second.children[2].element().children[0].element().content,
            "View"
        );
    }

    #[test]
    fn shortcut_list_clamps_the_column_count() {
        let one = shortcut_list(ShortcutListOptions {
            groups: groups(),
            columns: 0,
            ..Default::default()
        });
        assert_eq!(one.element().children.len(), 1, "at least one column");
        let eight = shortcut_list(ShortcutListOptions {
            groups: groups(),
            columns: 99,
            ..Default::default()
        });
        assert_eq!(eight.element().children.len(), 8, "at most eight columns");
        // empty groups still get the requested columns
        let empty = shortcut_list(ShortcutListOptions {
            columns: 3,
            ..Default::default()
        });
        assert_eq!(empty.element().children.len(), 3);
        for column in &empty.element().children {
            assert!(column.element().children.is_empty());
        }
    }

    #[test]
    fn shortcut_list_uses_the_font_size_for_titles_and_rows() {
        let node = shortcut_list(ShortcutListOptions {
            groups: groups(),
            font_size: 15.0,
            row_height: 34.0,
            column_width: 200.0,
            column_gap: 12.0,
            ..Default::default()
        });
        let list = node.element();
        assert_eq!(list.layout.gap, 12.0);
        assert_eq!(list.children[0].element().layout.width, Some(200.0));
        let group = list.children[0].element().children[0].element();
        assert_eq!(group.children[0].element().text_style.font_size, 14.0);
        let save = list.children[0].element().children[1].element();
        assert_eq!(save.layout.height, Some(34.0));
        assert_eq!(save.children[0].element().text_style.font_size, 15.0);
        assert_eq!(save.children[1].element().text_style.font_size, 15.0);
    }
}
