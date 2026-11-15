//! JSX tags `zant-tsx` knows and how their props map onto `ui::*` options
//! (`docs/tsx.md`, "全部 `ui.*` 组件"). The types below are the contract the code
//! generator relies on; [`all`] is the table it reads.
//!
//! The table has two halves. The native half is data: `zant-tsx-tags`
//! generates `crate::ui_tags` from the `#[derive(TsxProps)]` metadata of the
//! `ui` option structs (re-exported as the `native` module below).
//! `tag_aliases` patches it with React Native spellings; the merge runs once
//! on first use into a [`LazyLock`]. The merge / lookup layer here is
//! hand-written.
//!
//! Rust cannot reflect on the `ui` option types, so the tests check the table
//! against the builder names read from the `ui` sources instead (see
//! `every_tag_builder_exists_in_the_rust_ui`).

use std::sync::LazyLock;

use crate::tag_aliases;

/// How a prop's value is checked and which kind of field it sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// `f32` or `Option<f32>` field: any number expression (ints convert).
    #[default]
    Number,
    /// Integer field of type `int_type` (`numberOfLines` u16, `order` i32):
    /// an integer expression.
    Integer,
    Boolean,
    /// String or optional string field: a string expression.
    String,
    /// Rust enum field: a string literal (or a `?:` of string literals) that
    /// must be one of `values` (its `.ts` spelling); emits `.rust`.
    Enumeration,
    /// `Option<u32>` ARGB field: "#rgb" / "#rrggbb" / "#rrggbbaa" string literals or a
    /// number expression (`0xff1677ff`).
    Color,
    /// One or more sides of an `Edges` field (`padding`, `margin`): a number
    /// expression applied to `sides`. Several props on the same field combine;
    /// later props win per side.
    Edges,
    /// `f32` / `Option<f32>` field that also takes a "50%" string literal, which sets
    /// `percent_field` instead.
    Dimension,
    /// A static string literal that sets a bool field: `values` maps each
    /// accepted `.ts` spelling to the bool field (`.rust`) it sets to true; an
    /// empty `.rust` accepts the spelling and sets nothing.
    Flag,
    /// RN `flex: n`: `grow = n, shrink = 1, basis = 0`.
    Flex,
    /// `actions::Action` (`() => void`).
    Action,
    /// `actions::BoolAction` (`(checked: boolean) => void`).
    BoolAction,
    /// `actions::TextAction` (`(value: string) => void`).
    TextAction,
    /// `actions::IndexAction` (`(index: usize) => void`).
    IndexAction,
    /// `ui::Node` field (required unless optional): an element expression.
    Node,
    /// `?ui::Node` field: an element expression, or `null`.
    OptionalNode,
    /// `Vec<ui::Node>` field: the element's children (also settable as a prop
    /// holding an array of elements).
    Children,
    /// `Vec<String>`-equivalent field: an array of strings.
    StringList,
    /// `&[T]` field with T a struct: an array of object literals whose keys
    /// are `item_props` (emits the item type `item_type`).
    StructList,
    /// `?zant::image::Image`: an in-memory image (TSX type `Image`, from a
    /// Rust module such as a thumbnail cache).
    Image,
    /// `?desktop::Shortcut`: a shortcut string ("ctrl+cmd+v"; "" / null for
    /// none).
    Shortcut,
    /// `?ui::HotkeyAction`: `(shortcut: string) => void`.
    ShortcutAction,
}

/// One or more sides of an `Edges` field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl Sides {
    pub const NONE: Sides = Sides {
        top: false,
        right: false,
        bottom: false,
        left: false,
    };
    pub const ALL: Sides = Sides {
        top: true,
        right: true,
        bottom: true,
        left: true,
    };
    pub const HORIZONTAL: Sides = Sides {
        top: false,
        right: true,
        bottom: false,
        left: true,
    };
    pub const VERTICAL: Sides = Sides {
        top: true,
        right: false,
        bottom: true,
        left: false,
    };

    /// Any side selected.
    pub fn any(self) -> bool {
        self.top || self.right || self.bottom || self.left
    }
}

/// One accepted spelling of an enum value and what it emits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value {
    /// The spelling TSX accepts (`"space-between"`).
    pub ts: &'static str,
    /// The Rust-side (snake_case) spelling of the enum value
    /// (`space_between`).
    pub rust: &'static str,
}

/// One TSX prop of a tag. Fields without a value use their [`Default`]; the
/// native table only spells out the ones it sets.
#[derive(Clone, Debug)]
pub struct Prop {
    /// TSX prop name (`backgroundColor`).
    pub name: &'static str,
    /// Rust options field (`background`).
    pub field: &'static str,
    pub kind: Kind,
    /// `.enumeration` / `.flag` spellings.
    pub values: Vec<Value>,
    /// `.edges` sides.
    pub sides: Sides,
    /// `.dimension` percent field.
    pub percent_field: &'static str,
    /// `.integer`: the Rust integer type (`u16`, `i32`).
    pub int_type: &'static str,
    /// The field has no default value: the prop must be given.
    pub required: bool,
    /// `.struct_list`: the element type (`ui::TabItem`) and its fields as
    /// props (camelCase names, same kinds; nesting allowed).
    pub item_type: &'static str,
    pub item_props: Vec<Prop>,
    /// The field lives in a nested options struct that TSX flattens
    /// (`ClickableOptions.box`): emits `.parent = .{ .field = … }`. Empty for
    /// top-level fields.
    pub parent: &'static str,
}

impl Default for Prop {
    fn default() -> Self {
        Prop {
            name: "",
            field: "",
            kind: Kind::Number,
            values: Vec::new(),
            sides: Sides::NONE,
            percent_field: "",
            int_type: "",
            required: false,
            item_type: "",
            item_props: Vec::new(),
            parent: "",
        }
    }
}

/// Where the children of a tag go.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Content {
    /// Child elements go to `.children` (text children are an error).
    Children,
    /// Text children form the first argument: `ui::text(value, .{...})`.
    TextArg,
    /// Text children set the string field `content_field` (Button `text`,
    /// Checkbox `label`), same as the prop.
    TextField,
    /// The only child element sets the `Node` field `content_field` (Tooltip
    /// `child`), same as the prop.
    Child,
    /// No children allowed.
    #[default]
    None,
}

/// One supported `ui.*` builder.
#[derive(Clone, Debug, Default)]
pub struct Tag {
    /// JSX name (`View`).
    pub name: &'static str,
    /// Rust builder (`ui::view`).
    pub builder: &'static str,
    /// Builder takes a stable instance id before its options.
    pub instance_id: bool,
    pub content: Content,
    pub content_field: &'static str,
    pub props: Vec<Prop>,
}

/// A builder intentionally excluded from the generated table.
#[derive(Clone, Copy, Debug)]
pub struct Skipped {
    pub builder: &'static str,
    pub reason: &'static str,
}

/// The native table of [`native`], built once.
static NATIVE: LazyLock<Vec<Tag>> = LazyLock::new(native::tags);

/// The merged table of [`all`], built once.
static ALL: LazyLock<Vec<Tag>> = LazyLock::new(build_all);

/// Fields of the theme (`useTheme()`), built once.
static THEME: LazyLock<Vec<Prop>> = LazyLock::new(native::theme);

/// Builders intentionally excluded from the generated table, emitted alongside
/// the table for coverage checks.
pub fn skipped() -> &'static [Skipped] {
    native::SKIPPED
}

/// The generated (`crate::ui_tags`) table before the alias layer patches it.
pub fn native_all() -> &'static [Tag] {
    &NATIVE
}

/// Fields of `zant.tokens.Theme` for `useTheme()` (`colorPrimary` →
/// `color_primary`), generated with the tag table.
pub fn theme_fields() -> &'static [Prop] {
    &THEME
}

/// Replace matching names in declaration order, then append extra RN aliases.
/// Alias replacement is exact, including content overrides and enum spellings.
fn merge_props(native_props: &[Prop], extra: &[Prop]) -> Vec<Prop> {
    let mut result = Vec::with_capacity(native_props.len() + extra.len());
    for prop in native_props {
        let mut replacement = prop.clone();
        for alias in extra {
            if alias.name == prop.name {
                replacement = alias.clone();
            }
        }
        result.push(replacement);
    }
    for alias in extra {
        if !native_props.iter().any(|prop| prop.name == alias.name) {
            result.push(alias.clone());
        }
    }
    result
}

/// Every supported `ui.*` builder, in the generated table's order: the
/// generated table with [`tag_aliases::all`] applied.
fn build_all() -> Vec<Tag> {
    let aliases = tag_aliases::all();
    let mut result = Vec::with_capacity(native_all().len());
    for native in native_all() {
        let mut tag = native.clone();
        for alias in aliases {
            if alias.name == tag.name {
                tag.props = merge_props(&tag.props, &alias.props);
                if let Some(content) = alias.content {
                    tag.content = content;
                    tag.content_field = alias.content_field;
                }
            }
        }
        result.push(tag);
    }
    result
}

/// Every supported `ui.*` builder, with the alias layer applied.
pub fn all() -> &'static [Tag] {
    &ALL
}

pub fn find(name: &str) -> Option<&'static Tag> {
    all().iter().find(|tag| tag.name == name)
}

pub fn find_prop<'a>(tag: &'a Tag, name: &str) -> Option<&'a Prop> {
    tag.props.iter().find(|prop| prop.name == name)
}

/// The Rust spelling for an `.enumeration` / `.flag` value, or `None` when the
/// value is not accepted.
pub fn find_value<'a>(prop: &'a Prop, ts: &str) -> Option<&'a str> {
    prop.values
        .iter()
        .find(|value| value.ts == ts)
        .map(|value| value.rust)
}

// ── generator name helpers ─────────────────────────────────────────────────
//
// The table generator (`zant-tsx-tags`) needs the same deterministic
// spellings as the lookups above, so they live next to the table they
// describe.

/// Snake case becomes camelCase; existing camelCase stays intact.
pub fn camel(name: &str) -> String {
    let mut result = Vec::with_capacity(name.len());
    let mut upper = false;
    for byte in name.bytes() {
        if byte == b'_' {
            upper = true;
        } else {
            result.push(if upper {
                byte.to_ascii_uppercase()
            } else {
                byte
            });
            upper = false;
        }
    }
    String::from_utf8(result).expect("camel case of a valid UTF-8 name")
}

/// Builder names become PascalCase TSX tags (including Rust keywords).
pub fn pascal(name: &str) -> String {
    let mut result = camel(name).into_bytes();
    if result.is_empty() {
        return String::new();
    }
    result[0] = result[0].to_ascii_uppercase();
    String::from_utf8(result).expect("pascal case of a valid UTF-8 name")
}

/// Native, kebab-case, then camelCase; retain only the first of duplicates.
pub fn spellings(name: &str) -> Vec<String> {
    let kebab: String = name
        .chars()
        .map(|c| if c == '_' { '-' } else { c })
        .collect();
    let mut result: Vec<String> = Vec::with_capacity(3);
    for candidate in [name.to_string(), kebab, camel(name)] {
        if !result.contains(&candidate) {
            result.push(candidate);
        }
    }
    result
}

// ── generated table ────────────────────────────────────────────────────────
//
// `crate::ui_tags` is generated by `zant-tsx-tags` from the
// `#[derive(TsxProps)]` metadata of the Rust option structs. It exposes
// `tags`, `theme` and `SKIPPED`, which the merge / lookup layer above reads.
/// The generated table, under the name the merge layer uses.
pub(crate) mod native {
    pub(crate) use crate::ui_tags::*;
}

#[cfg(test)]
mod tests {
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

    // ── generated table (`crates/zant-tsx/src/ui_tags.rs`) ─────────────────

    /// The pinned size of the table: 38 tags / 466 props / 854 spellings / 46
    /// theme fields / 10 skipped. Whoever moves a composite / markdown / pdf
    /// builder out of the generator's `pending.rs` (or adds a tag) also
    /// updates these counts.
    #[test]
    fn the_generated_table_has_the_pinned_counts() {
        let tags = native_all();
        assert_eq!(38, tags.len());
        assert_eq!(466, tags.iter().map(|tag| tag.props.len()).sum::<usize>());
        assert_eq!(
            854,
            tags.iter()
                .flat_map(|tag| &tag.props)
                .map(|prop| prop.values.len())
                .sum::<usize>()
        );
        assert_eq!(46, theme_fields().len());
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
            include_str!("../../zant/src/ui/builders/mod.rs"),
            include_str!("../../zant/src/ui/builders/choice.rs"),
            include_str!("../../zant/src/ui/builders/containers.rs"),
            include_str!("../../zant/src/ui/builders/feedback.rs"),
            include_str!("../../zant/src/ui/builders/form.rs"),
            include_str!("../../zant/src/ui/builders/interaction.rs"),
            include_str!("../../zant/src/ui/builders/media.rs"),
            include_str!("../../zant/src/ui/builders/pdf_preview.rs"),
            include_str!("../../zant/src/ui/builders/typography.rs"),
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
}
