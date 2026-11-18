//! Unit tests.

use super::*;

#[test]
fn rust_enum_variants_become_snake_case_spellings() {
    assert_eq!(snake_case("WrapReverse"), "wrap_reverse");
    assert_eq!(snake_case("ExtraLight"), "extra_light");
    assert_eq!(snake_case("SerifCjk"), "serif_cjk");
    assert_eq!(snake_case("Ibeam"), "ibeam");
    assert_eq!(snake_case("None"), "none");
    assert_eq!(snake_case("SpaceBetween"), "space_between");
}

#[test]
fn builder_names_are_camel_case_with_raw_identifiers_quoted() {
    let builder = |name: &str, raw_ident: bool| Builder {
        name: name.to_string(),
        raw_ident,
        options: "ToggleOptions".to_string(),
        text_arg: false,
    };
    assert_eq!(
        builder_name(&builder("radio_group", false)),
        "ui.radioGroup"
    );
    assert_eq!(
        builder_name(&builder("dismiss_layer", false)),
        "ui.dismissLayer"
    );
    assert_eq!(builder_name(&builder("switch", true)), "ui.@\"switch\"");
    assert_eq!(rust_name("ui.@\"switch\""), "switch");
    assert_eq!(rust_name("ui.dropdownMenu"), "dropdown_menu");
    assert_eq!(rust_name("ui.iconButton"), "icon_button");
}

#[test]
fn the_table_order_lists_every_native_tag_once() {
    let mut seen = std::collections::HashSet::new();
    for name in TAG_ORDER {
        assert!(seen.insert(*name), "duplicate builder {name}");
    }
    assert_eq!(TAG_ORDER.len(), 38);
    assert_eq!(SKIPPED.len(), 10);
    for skipped in SKIPPED {
        assert!(
            !TAG_ORDER.contains(&rust_name(skipped.builder).as_str()),
            "{} is both skipped and tagged",
            skipped.builder
        );
    }
}
