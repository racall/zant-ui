//! Unit tests.

use super::*;

#[test]
fn find_resolves_every_generated_tag_and_rejects_unknown_names() {
    for tag in all() {
        assert!(std::ptr::eq(find(tag.name).unwrap(), tag));
    }
    assert!(find("").is_none());
    assert!(find("view").is_none());
    assert!(find("Unknown").is_none());
}

#[test]
fn find_prop_resolves_every_prop_and_keeps_aliases_tag_specific() {
    for tag in all() {
        for prop in &tag.props {
            assert!(std::ptr::eq(find_prop(tag, prop.name).unwrap(), prop));
        }
        if let Some(key) = find_prop(tag, "key") {
            assert_eq!(Kind::String, key.kind);
        }
        assert!(find_prop(tag, "unknown").is_none());
    }
    let view = find("View").unwrap();
    assert_eq!(
        "background",
        find_prop(view, "backgroundColor").unwrap().field
    );
    assert_eq!("background", find_prop(view, "background").unwrap().field);
    assert_eq!("direction", find_prop(view, "flexDirection").unwrap().field);
    assert_eq!(
        "flex_direction",
        find_prop(find("Column").unwrap(), "flexDirection")
            .unwrap()
            .field
    );
    assert!(find_prop(view, "BackgroundColor").is_none());
    assert_eq!(Kind::Enumeration, find_prop(view, "wrap").unwrap().kind);
    assert_eq!(
        Kind::Boolean,
        find_prop(find("Text").unwrap(), "wrap").unwrap().kind
    );
}

#[test]
fn find_value_resolves_enum_aliases_and_distinguishes_no_op_flags_from_invalid_values() {
    for tag in all() {
        for prop in &tag.props {
            for value in &prop.values {
                assert_eq!(value.rust, find_value(prop, value.ts).unwrap());
            }
            assert!(find_value(prop, "unknown").is_none());
        }
    }
    let view = find("View").unwrap();
    let justify = find_prop(view, "justifyContent").unwrap();
    assert_eq!(
        "space_between",
        find_value(justify, "space-between").unwrap()
    );
    assert_eq!(
        "space_between",
        find_value(justify, "space_between").unwrap()
    );
    let weight = find_prop(find("Text").unwrap(), "fontWeight").unwrap();
    assert_eq!("semibold", find_value(weight, "600").unwrap());
    assert_eq!("extra_light", find_value(weight, "extraLight").unwrap());
    assert!(find_value(weight, "950").is_none());
    let position = find_prop(view, "position").unwrap();
    assert_eq!("absolute", find_value(position, "absolute").unwrap());
    assert_eq!("", find_value(position, "relative").unwrap());
    assert_eq!(
        "",
        find_value(find_prop(view, "overflow").unwrap(), "visible").unwrap()
    );
    assert!(find_value(position, "Relative").is_none());
    assert!(find_value(find_prop(view, "gap").unwrap(), "0").is_none());
}

// ── generator name helpers ──────────────────────────────────────────────

#[test]
fn snake_case_fields_become_camel_case() {
    for (name, expected) in [
        ("on_change", "onChange"),
        ("width_percent", "widthPercent"),
        ("radioGroup", "radioGroup"),
        ("key", "key"),
        ("", ""),
    ] {
        assert_eq!(expected, camel(name));
    }
}

#[test]
fn builders_become_pascal_case() {
    for (name, expected) in [
        ("radioGroup", "RadioGroup"),
        ("switch", "Switch"),
        ("pdfPreview", "PdfPreview"),
        ("foo_bar", "FooBar"),
        ("", ""),
    ] {
        assert_eq!(expected, pascal(name));
    }
}

#[test]
fn enum_spellings_are_ordered_and_deduplicated() {
    let values = spellings("chevron_down");
    assert_eq!(3, values.len());
    for (actual, expected) in values
        .iter()
        .zip(["chevron_down", "chevron-down", "chevronDown"])
    {
        assert_eq!(expected, actual);
    }
    assert_eq!(1, spellings("none").len());
    assert_eq!(1, spellings("alreadyCamel").len());
}

// ── generated table (`crates/zant-tsx/src/ui_tags/`) ─────────────────

/// The pinned size of the table: 38 tags / 474 props / 854 spellings / 48
/// theme fields / 10 skipped. Whoever moves a composite / markdown / pdf
/// builder out of the generator's `pending.rs` (or adds a tag) also
/// updates these counts.
#[test]
fn the_generated_table_has_the_pinned_counts() {
    let tags = native_all();
    assert_eq!(38, tags.len());
    assert_eq!(474, tags.iter().map(|tag| tag.props.len()).sum::<usize>());
    assert_eq!(
        854,
        tags.iter()
            .flat_map(|tag| &tag.props)
            .map(|prop| prop.values.len())
            .sum::<usize>()
    );
    assert_eq!(48, theme_fields().len());
    assert_eq!(10, skipped().len());
}

/// The declaration order of the generated table; the TSX compiler and
/// `zant.d.ts` render in this order.
#[test]
fn the_generated_table_keeps_the_ui_declaration_order() {
    let names: Vec<&str> = native_all().iter().map(|tag| tag.name).collect();
    assert_eq!(
        names,
        [
            "Column",
            "Row",
            "Clickable",
            "Space",
            "Text",
            "Heading",
            "Title",
            "Paragraph",
            "Button",
            "Link",
            "Input",
            "Checkbox",
            "Switch",
            "RadioGroup",
            "Tabs",
            "Tooltip",
            "Image",
            "Select",
            "Icon",
            "IconButton",
            "DismissLayer",
            "RichText",
            "PdfPreview",
            "Divider",
            "Tag",
            "Card",
            "Alert",
            "Avatar",
            "Progress",
            "DropdownMenu",
            "Segmented",
            "Modal",
            "ShortcutList",
            "Message",
            "HotkeyInput",
            "View",
            "Markdown",
            "PdfViewer",
        ]
    );
}

// ── builder coverage ───────────────────────────────────────────────────

/// Tag builders that are not functions of `crates/zant/src/ui/builders/`
/// (they live in `crates/zant/src/components/`). The test below reports a
/// newly missing one instead of silently dropping it.
const KNOWN_MISSING_BUILDERS: &[&str] = &[
    // Composite components.
    "dropdown_menu",
    "segmented",
    "modal",
    "shortcut_list",
    "message",
    "hotkey_input",
    "view",
    // Markdown / PDF viewer builders.
    "markdown",
    "pdf_viewer",
];

/// Builders of `crates/zant/src/ui/builders/` that are not tags (skipped
/// entries that are ordinary builders).
const UNTAGGED_BUILDERS: &[&str] = &["pointer_area", "vector"];

/// `pub fn` names of `crates/zant/src/ui/builders/`, read from the sources
/// at test time because Rust cannot reflect on them.
fn rust_builder_names() -> Vec<String> {
    const SOURCES: &[&str] = &[
        include_str!("../../../zant/src/ui/builders/mod.rs"),
        include_str!("../../../zant/src/ui/builders/choice.rs"),
        include_str!("../../../zant/src/ui/builders/containers.rs"),
        include_str!("../../../zant/src/ui/builders/feedback.rs"),
        include_str!("../../../zant/src/ui/builders/form.rs"),
        include_str!("../../../zant/src/ui/builders/interaction.rs"),
        include_str!("../../../zant/src/ui/builders/media.rs"),
        include_str!("../../../zant/src/ui/builders/pdf_preview.rs"),
        include_str!("../../../zant/src/ui/builders/typography.rs"),
    ];
    let mut names = Vec::new();
    for source in SOURCES {
        for line in source.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("pub fn ") else {
                continue;
            };
            let name = rest
                .split(['(', '<'])
                .next()
                .expect("split always yields one item");
            names.push(name.strip_prefix("r#").unwrap_or(name).to_string());
        }
    }
    names
}

/// `ui.someBuilder` / `ui.r#some_builder` → the Rust function name.
fn rust_builder_name(builder: &str) -> String {
    let name = builder
        .strip_prefix("ui.")
        .expect("tag builders are ui.* functions");
    // Keyword builders are spelled `ui.@"switch"` in the table.
    let name = name
        .strip_prefix("@\"")
        .and_then(|name| name.strip_suffix('"'))
        .unwrap_or(name);
    let name = name.strip_prefix("r#").unwrap_or(name);
    let mut result = String::with_capacity(name.len() + 4);
    for char in name.chars() {
        if char.is_ascii_uppercase() {
            result.push('_');
            result.push(char.to_ascii_lowercase());
        } else {
            result.push(char);
        }
    }
    result
}

#[test]
fn every_tag_builder_exists_in_the_rust_ui() {
    let builders = rust_builder_names();
    let mut matched = 0;
    let mut missing: Vec<String> = Vec::new();
    for tag in all() {
        let name = rust_builder_name(tag.builder);
        if builders.contains(&name) {
            matched += 1;
        } else {
            missing.push(name);
        }
    }
    missing.sort();
    missing.dedup();
    for name in &missing {
        assert!(
            KNOWN_MISSING_BUILDERS.contains(&name.as_str()),
            "tags.rs uses ui::{name}, but crates/zant/src/ui/builders/ has no such function: \
                 add the builder instead of dropping the tag"
        );
    }
    assert_eq!(matched + missing.len(), all().len());
    eprintln!(
        "tsx tags: {} tags, {matched} ui builders matched, {} missing {missing:?}",
        all().len(),
        missing.len()
    );
}

#[test]
fn every_rust_ui_builder_is_a_tag_or_skipped() {
    let tags: Vec<String> = all()
        .iter()
        .map(|tag| rust_builder_name(tag.builder))
        .collect();
    for name in rust_builder_names() {
        assert!(
            tags.contains(&name) || UNTAGGED_BUILDERS.contains(&name.as_str()),
            "ui::{name} is neither a TSX tag nor a skipped builder"
        );
    }
}
