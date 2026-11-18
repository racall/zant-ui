//! Unit tests.

use super::*;

#[test]
fn every_tag_maps_to_a_rust_builder_and_options_struct() {
    for tag in tags::all() {
        let builder = builder_path(tag).expect("builder path");
        assert!(
            builder.starts_with("ui::"),
            "tag {} builder {builder}",
            tag.name
        );
        assert!(
            !builder.contains('.') && !builder.contains('@'),
            "tag {} builder {builder} is not a Rust builder path",
            tag.name
        );
        let options = options_path(tag).expect("options path");
        assert!(
            options.starts_with("ui::") && options.ends_with("Options"),
            "tag {} options {options}",
            tag.name
        );
    }
}

#[test]
fn option_structs_with_different_names_are_mapped() {
    let cases = [
        ("Column", "ui::BoxOptions"),
        ("Row", "ui::BoxOptions"),
        ("Switch", "ui::ToggleOptions"),
        ("Checkbox", "ui::ToggleOptions"),
        ("Title", "ui::HeadingOptions"),
        ("PdfPreview", "ui::PdfOptions"),
        ("DropdownMenu", "ui::MenuOptions"),
        ("RichText", "ui::RichTextOptions"),
        ("IconButton", "ui::IconButtonOptions"),
        ("View", "ui::ViewOptions"),
        ("Markdown", "ui::MarkdownOptions"),
    ];
    for (name, expected) in cases {
        let tag = tags::find(name).expect("tag");
        assert_eq!(expected, options_path(tag).expect("options"), "tag {name}");
    }
    assert_eq!(
        "ui::switch",
        builder_path(tags::find("Switch").unwrap()).unwrap()
    );
    assert_eq!(
        "ui::rich_text",
        builder_path(tags::find("RichText").unwrap()).unwrap()
    );
}

#[test]
fn enum_paths_are_known_for_every_enumeration_prop() {
    for tag in tags::all() {
        for prop in &tag.props {
            if prop.kind != Kind::Enumeration {
                continue;
            }
            assert!(
                enum_type_path(tag.name, prop.field).is_some(),
                "tag {} prop {} has no Rust enum path",
                tag.name,
                prop.name
            );
        }
    }
}

#[test]
fn optional_fields_match_the_frozen_option_struct_rules() {
    let button = tags::find("Button").unwrap();
    let key = tags::find_prop(button, "key").unwrap();
    assert!(optional_field("Button", key));
    let variant = tags::find_prop(button, "variant").unwrap();
    assert!(!optional_field("Button", variant));
    // key is a plain `Str` on these option structs
    for name in ["Modal", "Message", "ShortcutList", "HotkeyInput"] {
        let tag = tags::find(name).unwrap();
        let prop = tags::find_prop(tag, "key").unwrap();
        assert!(!optional_field(name, prop), "tag {name}");
    }
    let text = tags::find("Text").unwrap();
    assert!(!optional_field(
        "Text",
        tags::find_prop(text, "size").unwrap()
    ));
    let button_size = tags::find_prop(button, "size").unwrap();
    assert!(!optional_field("Button", button_size));
    let column = tags::find("Column").unwrap();
    for field in ["width", "background", "rowGap", "justifyContent"] {
        let prop = tags::find_prop(column, field);
        let Some(prop) = prop else { continue };
        assert!(
            optional_field("Column", prop),
            "Column {field} kind {:?} field {}",
            prop.kind,
            prop.field
        );
    }
}
