//! Hand-written React Native compatibility layer (Zig
//! `tools/tsx/tag_aliases.zig`). Names here override the generated props;
//! native fields and new enum spellings continue to come from `ui_tags`.
//!
//! `tags::all` merges this list into the generated table: alias props replace
//! same-named native props and extra aliases are appended, while
//! [`Alias::content`] overrides where the tag's children go.

use std::sync::LazyLock;

use crate::tags::{self, Content, Kind, Prop, Sides, Value};

/// A React Native / CSS compatibility patch for one native tag (Zig
/// `tag_aliases.Alias`).
#[derive(Clone, Debug, Default)]
pub struct Alias {
    pub name: &'static str,
    pub props: Vec<Prop>,
    /// Overrides `Tag::content` when set.
    pub content: Option<Content>,
    pub content_field: &'static str,
}

/// Alias enums inherit every reflected spelling before adding CSS / RN forms.
/// Keeping this lookup in the alias layer makes prop replacement exact without
/// maintaining a second list of native enum members.
fn enum_values(tag_name: &str, field: &str, extra: &[Value]) -> Vec<Value> {
    let tag = tags::native_all()
        .iter()
        .find(|tag| tag.name == tag_name)
        .unwrap_or_else(|| panic!("missing reflected tag {tag_name}"));
    let prop = tag
        .props
        .iter()
        .find(|prop| prop.field == field)
        .unwrap_or_else(|| panic!("missing reflected enum {tag_name}.{field}"));
    let mut result = prop.values.clone();
    for value in extra {
        if !result.iter().any(|native| native.ts == value.ts) {
            result.push(*value);
        }
    }
    result
}

/// A `.ts` / `.zig` value list (Zig array literal of `tags.Value`).
fn values(items: &[(&'static str, &'static str)]) -> Vec<Value> {
    items.iter().map(|(ts, zig)| Value { ts, zig }).collect()
}

fn prop(name: &'static str, field: &'static str, kind: Kind) -> Prop {
    Prop {
        name,
        field,
        kind,
        ..Default::default()
    }
}

fn edges(name: &'static str, field: &'static str, sides: Sides) -> Prop {
    Prop {
        name,
        field,
        kind: Kind::Edges,
        sides,
        ..Default::default()
    }
}

fn enumeration(name: &'static str, field: &'static str, values: Vec<Value>) -> Prop {
    Prop {
        name,
        field,
        kind: Kind::Enumeration,
        values,
        ..Default::default()
    }
}

fn flag(name: &'static str, values: Vec<Value>) -> Prop {
    Prop {
        name,
        kind: Kind::Flag,
        values,
        ..Default::default()
    }
}

static ALL: LazyLock<Vec<Alias>> = LazyLock::new(build_all);

/// Phase-1 names, CSS spellings, and text-child conventions.
pub fn all() -> &'static [Alias] {
    &ALL
}

fn build_all() -> Vec<Alias> {
    let css_align = values(&[("flex-start", "start"), ("flex-end", "end")]);
    let justify_values = enum_values("Column", "justify", &css_align);
    let align_values = enum_values("Column", "align_items", &css_align);
    let align_self_values = enum_values("Column", "align_self", &css_align);
    let align_content_values = enum_values("Column", "align_content", &css_align);
    let wrap_values = enum_values("Column", "wrap", &[]);
    let direction_values = enum_values("View", "direction", &[]);

    let position_values = values(&[("absolute", "absolute"), ("relative", "")]);

    let overflow_values = values(&[("hidden", "clip"), ("scroll", "scroll"), ("visible", "")]);

    let weight_values = enum_values(
        "Text",
        "weight",
        &values(&[
            ("normal", "regular"),
            ("semi-bold", "semibold"),
            ("100", "thin"),
            ("200", "extra_light"),
            ("300", "light"),
            ("400", "regular"),
            ("500", "medium"),
            ("600", "semibold"),
            ("700", "bold"),
            ("800", "extra_bold"),
            ("900", "black"),
        ]),
    );
    let family_values = enum_values("Text", "family", &[]);

    let box_props = vec![
        edges("paddingHorizontal", "padding", Sides::HORIZONTAL),
        edges("paddingVertical", "padding", Sides::VERTICAL),
        edges(
            "paddingTop",
            "padding",
            Sides {
                top: true,
                ..Sides::NONE
            },
        ),
        edges(
            "paddingRight",
            "padding",
            Sides {
                right: true,
                ..Sides::NONE
            },
        ),
        edges(
            "paddingBottom",
            "padding",
            Sides {
                bottom: true,
                ..Sides::NONE
            },
        ),
        edges(
            "paddingLeft",
            "padding",
            Sides {
                left: true,
                ..Sides::NONE
            },
        ),
        edges("marginHorizontal", "margin", Sides::HORIZONTAL),
        edges("marginVertical", "margin", Sides::VERTICAL),
        edges(
            "marginTop",
            "margin",
            Sides {
                top: true,
                ..Sides::NONE
            },
        ),
        edges(
            "marginRight",
            "margin",
            Sides {
                right: true,
                ..Sides::NONE
            },
        ),
        edges(
            "marginBottom",
            "margin",
            Sides {
                bottom: true,
                ..Sides::NONE
            },
        ),
        edges(
            "marginLeft",
            "margin",
            Sides {
                left: true,
                ..Sides::NONE
            },
        ),
        Prop {
            name: "width",
            field: "width",
            kind: Kind::Dimension,
            percent_field: "width_percent",
            ..Default::default()
        },
        Prop {
            name: "height",
            field: "height",
            kind: Kind::Dimension,
            percent_field: "height_percent",
            ..Default::default()
        },
        Prop {
            name: "flex",
            kind: Kind::Flex,
            ..Default::default()
        },
        prop("flexGrow", "grow", Kind::Number),
        prop("flexShrink", "shrink", Kind::Number),
        prop("flexBasis", "basis", Kind::Number),
        enumeration("justifyContent", "justify_content", justify_values.clone()),
        enumeration("justify", "justify", justify_values),
        enumeration("alignItems", "align_items", align_values),
        enumeration("alignSelf", "align_self", align_self_values),
        enumeration("alignContent", "align_content", align_content_values),
        enumeration("flexWrap", "wrap", wrap_values.clone()),
        enumeration("wrap", "wrap", wrap_values),
        flag("position", position_values),
        flag("overflow", overflow_values),
        prop("backgroundColor", "background", Kind::Color),
        prop("borderRadius", "radius", Kind::Number),
    ];

    let mut view_props = box_props.clone();
    view_props.extend([
        enumeration("flexDirection", "direction", direction_values),
        prop("shadowRadius", "shadow_blur", Kind::Number),
        prop("onPress", "on_click", Kind::Action), // React Native
    ]);

    let text_props = vec![
        prop("fontSize", "size", Kind::Number),
        enumeration("fontWeight", "weight", weight_values.clone()),
        enumeration("weight", "weight", weight_values),
        Prop {
            name: "numberOfLines",
            field: "max_lines",
            kind: Kind::Integer,
            int_type: "u16",
            ..Default::default()
        },
        enumeration("fontFamily", "family", family_values),
    ];

    let button_props = vec![prop("onPress", "on_click", Kind::Action)]; // React Native

    vec![
        Alias {
            name: "Markdown",
            props: Vec::new(),
            content: Some(Content::TextField),
            content_field: "source",
        },
        Alias {
            name: "View",
            props: view_props,
            ..Default::default()
        },
        Alias {
            name: "Column",
            props: box_props.clone(),
            ..Default::default()
        },
        Alias {
            name: "Row",
            props: box_props,
            ..Default::default()
        },
        Alias {
            name: "Text",
            props: text_props,
            ..Default::default()
        },
        Alias {
            name: "Button",
            props: button_props,
            content: Some(Content::TextField),
            content_field: "text",
        },
        Alias {
            name: "Checkbox",
            props: Vec::new(),
            content: Some(Content::TextField),
            content_field: "label",
        },
        Alias {
            name: "Switch",
            props: Vec::new(),
            content: Some(Content::TextField),
            content_field: "label",
        },
        Alias {
            name: "Link",
            props: Vec::new(),
            content: Some(Content::TextField),
            content_field: "text",
        },
        Alias {
            name: "Tag",
            props: Vec::new(),
            content: Some(Content::TextField),
            content_field: "text",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The alias layer only patches tags of the same name; a rename in the
    /// generated table would silently drop a compatibility spelling.
    #[test]
    fn every_alias_patches_a_native_tag() {
        for alias in all() {
            assert!(
                tags::native_all().iter().any(|tag| tag.name == alias.name),
                "alias {} has no generated tag",
                alias.name
            );
        }
    }
}
