//! JSX tags `zant-tsx` knows and how their props map onto `ui::*` options
//! (Zig `tools/tsx/tags.zig`, `docs/tsx.md`, "全部 `ui.*` 组件"). The types
//! below are the contract the code generator relies on; [`all`] is the table it
//! reads.
//!
//! The Zig version is computed at comptime: `ui_tags.zig` is generated from
//! `src/ui.zig`, `tag_aliases.zig` patches it with React Native spellings, and
//! `tags.zig` merges the two. Rust has no comptime reflection, so the native
//! table is data (the `native` module below) and the merge runs once on first
//! use into a [`LazyLock`]. WP-5.2's `zant-tsx-tags` will generate the native
//! half from `#[derive(TsxProps)]` into `crate::ui_tags`; it then replaces the
//! `native` module, while the merge / lookup layer here stays hand-written.
//!
//! Every Zig `test` of `tags.zig` and of the `manifest/names.zig` test block it
//! pulls in is ported below. `tests/tsx_tags_test.zig` validates the table
//! against the reflected `ui` option types at comptime, which Rust cannot do;
//! its Rust counterpart checks the builder names instead (see
//! [`every_tag_builder_exists_in_the_rust_ui`]).

use std::sync::LazyLock;

use crate::tag_aliases;

/// f32 or ?f32 field: any number expression (ints convert).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// f32 or ?f32 field: any number expression (ints convert).
    #[default]
    Number,
    /// Integer field of type `int_type` (`numberOfLines` u16, `order` i32):
    /// an integer expression.
    Integer,
    Boolean,
    /// `[]const u8` / `?[]const u8`-equivalent field: a string expression.
    String,
    /// Rust enum field: a string literal (or a `?:` of string literals) that
    /// must be one of `values` (its `.ts` spelling); emits `.zig`.
    Enumeration,
    /// `?u32` ARGB field: "#rgb" / "#rrggbb" / "#rrggbbaa" string literals or a
    /// number expression (`0xff1677ff`).
    Color,
    /// One or more sides of an `Edges` field (`padding`, `margin`): a number
    /// expression applied to `sides`. Several props on the same field combine;
    /// later props win per side.
    Edges,
    /// f32/?f32 field that also takes a "50%" string literal, which sets
    /// `percent_field` instead.
    Dimension,
    /// A static string literal that sets a bool field: `values` maps each
    /// accepted `.ts` spelling to the bool field (`.zig`) it sets to true; an
    /// empty `.zig` accepts the spelling and sets nothing.
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

/// One or more sides of an `Edges` field (Zig `tags.Sides`). The Zig version is
/// a `packed struct(u4)`; the Rust port keeps the named bool fields and the
/// same predicates.
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

    /// Any side selected (Zig `@as(u4, @bitCast(sides)) != 0`).
    pub fn any(self) -> bool {
        self.top || self.right || self.bottom || self.left
    }
}

/// A `.ts` spelling and the `.zig` spelling it emits (Zig `tags.Value`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value {
    pub ts: &'static str,
    pub zig: &'static str,
}

/// One TSX prop of a tag (Zig `tags.Prop`). Fields without a value use the Zig
/// defaults via [`Default`]; the native table only spells out the ones it sets.
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

/// Where the children of a tag go (Zig `tags.Content`).
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

/// One supported `ui.*` builder (Zig `tags.Tag`).
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

/// A builder intentionally excluded from the generated table (Zig
/// `ui_tags.skipped`).
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

/// The generated (`ui_tags.zig`) table before the alias layer patches it. WP-5.2
/// replaces the `native` module with `crate::ui_tags`; this accessor is what the
/// rest of the compiler should use.
pub fn native_all() -> &'static [Tag] {
    &NATIVE
}

/// Fields of `zant.tokens.Theme` for `useTheme()` (`colorPrimary` →
/// `color_primary`), reflected with the tag table (Zig `tags.theme_fields`).
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

/// Every supported `ui.*` builder, in `src/ui.zig` declaration order: the
/// generated table with [`tag_aliases::all`] applied (Zig `tags.all`).
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
        .map(|value| value.zig)
}

// ── generator name helpers (`tools/tsx/manifest/names.zig`) ────────────────
//
// `tags.zig` pulls in `manifest/names.zig` through its last test block; WP-5.2's
// table generator needs the same deterministic spellings, so they are ported
// here next to the table they describe.

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

// ── generated table (`tools/tsx/ui_tags.zig`) ──────────────────────────────
//
// The Zig table was generated by `zig build tsx-tags` from `src/ui.zig`;
// `crate::ui_tags` is generated by `zant-tsx-tags` from the
// `#[derive(TsxProps)]` metadata of the Rust option structs (WP-5.2). It keeps
// the shape the WP-4.4 placeholder module had (`tags`, `theme`, `SKIPPED`), so
// the merge / lookup layer above is unchanged.
/// The generated table, re-exported under the name WP-4.4 used.
pub(crate) mod native {
    pub(crate) use crate::ui_tags::*;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every Zig `test` of `tags.zig` (the generated-table tests of
    // `tests/tsx_tags_test.zig` are covered by the builder cross-check below;
    // they reflect on `ui` type information, which Rust does not expose).

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
                    assert_eq!(value.zig, find_value(prop, value.ts).unwrap());
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

    // ── `manifest/names.zig` tests (pulled in by `tags.zig`'s last test) ────

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

    // ── generated table (`crates/zant-tsx/src/ui_tags.rs`, WP-5.2) ─────────

    /// The WP-4.4 reference table: 38 tags / 453 props / 718 spellings / 46
    /// theme fields / 10 skipped. The Rust generator must reproduce it while
    /// the composite / markdown / pdf builders are pending; whoever ports one
    /// of them (and deletes its `pending.rs` entry) also updates this count.
    #[test]
    fn the_generated_table_matches_the_wp_4_4_reference_counts() {
        let tags = native_all();
        assert_eq!(38, tags.len());
        assert_eq!(453, tags.iter().map(|tag| tag.props.len()).sum::<usize>());
        assert_eq!(
            718,
            tags.iter()
                .flat_map(|tag| &tag.props)
                .map(|prop| prop.values.len())
                .sum::<usize>()
        );
        assert_eq!(46, theme_fields().len());
        assert_eq!(10, skipped().len());
    }

    /// `src/ui.zig` declaration order (`ui_tags.zig` order); the TSX compiler
    /// and `zant.d.ts` render in this order.
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

    // ── builder coverage (`tests/tsx_tags_test.zig` in Rust) ───────────────

    /// Tag builders the Rust `ui::*` module does not expose yet. The tag table
    /// keeps the Zig entries so WP-5.2 can wire them as the builders land; the
    /// test below reports a newly missing one instead of silently dropping it.
    const KNOWN_MISSING_BUILDERS: &[&str] = &[
        // WP-2.5 composite components.
        "dropdown_menu",
        "segmented",
        "modal",
        "shortcut_list",
        "message",
        "hotkey_input",
        "view",
        // Markdown / PDF viewer builders of WP-3.3 / 3.9.
        "markdown",
        "pdf_viewer",
    ];

    /// Builders of `crates/zant/src/ui/builders/` that are not tags (Zig
    /// `ui_tags.skipped` entries that are ordinary builders).
    const UNTAGGED_BUILDERS: &[&str] = &["pointer_area", "vector"];

    /// `pub fn` names of `crates/zant/src/ui/builders/`, read from the sources
    /// at test time because Rust cannot reflect on them (the Zig test uses
    /// comptime `@typeInfo(ui)`).
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
        // Zig keywords are spelled `ui.@"switch"`.
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
                 port the builder instead of dropping the tag"
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
