//! The `<List>` tag: its props as a tag-table entry.

use super::*;

/// `ui::ListOptions` fields settable from TSX (camelCase); `data`,
/// `renderItem` and `keyExtractor` are handled here.
pub(super) fn option_props() -> Vec<Prop> {
    vec![
        Prop {
            name: "key",
            field: "key",
            kind: Kind::String,
            ..Prop::default()
        },
        Prop {
            name: "estimatedItemHeight",
            field: "estimated_height",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "itemHeight",
            field: "item_height",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "followTail",
            field: "follow_tail",
            kind: Kind::Boolean,
            ..Prop::default()
        },
        Prop {
            name: "overdraw",
            field: "overdraw",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "gap",
            field: "gap",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "padding",
            field: "padding",
            kind: Kind::Edges,
            sides: Sides::ALL,
            ..Prop::default()
        },
        Prop {
            name: "background",
            field: "background",
            kind: Kind::Color,
            ..Prop::default()
        },
        Prop {
            name: "width",
            field: "width",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "height",
            field: "height",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "grow",
            field: "grow",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "selectable",
            field: "selectable",
            kind: Kind::Boolean,
            ..Prop::default()
        },
        Prop {
            name: "focusable",
            field: "focusable",
            kind: Kind::Boolean,
            ..Prop::default()
        },
        Prop {
            name: "scrollbar",
            field: "scrollbar",
            kind: Kind::Boolean,
            ..Prop::default()
        },
        Prop {
            name: "alignment",
            field: "alignment",
            kind: Kind::Enumeration,
            values: vec![
                tags::Value {
                    ts: "top",
                    rust: "top",
                },
                tags::Value {
                    ts: "bottom",
                    rust: "bottom",
                },
            ],
            ..Prop::default()
        },
    ]
}

pub(super) fn list_tag() -> Tag {
    Tag {
        name: "List",
        builder: "ui.list",
        instance_id: false,
        content: tags::Content::None,
        content_field: "",
        props: option_props(),
    }
}
