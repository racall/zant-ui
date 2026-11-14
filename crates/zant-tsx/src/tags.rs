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

// ── ported generated table (`tools/tsx/ui_tags.zig`) ─────────────────────
//
// The Zig table is generated by `zig build tsx-tags` from `src/ui.zig`. The
// Rust table is generated by WP-5.2's `zant-tsx-tags` from the Rust option
// structs into `crate::ui_tags`. Until that lands, this module carries the
// same data 1:1 so the merge / lookup layer below is testable; WP-5.2 only
// needs to replace this module with the generated one (`native::tags`,
// `native::theme`, `native::SKIPPED`).
//
// The table is generated data: one entry per line keeps it reviewable, so
// rustfmt skips this module (it would expand the table to thousands of lines).
#[rustfmt::skip]
pub(crate) mod native {
    use super::{Content, Kind, Prop, Sides, Skipped, Tag, Value};

    /// Every supported `ui.*` builder, in `ui.zig` declaration order (Zig
    /// `ui_tags.all`).
    pub(crate) fn tags() -> Vec<Tag> {
        vec![
                        Tag { name: "Column", builder: "ui.column", content: Content::Children, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "gap", field: "gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "rowGap", field: "row_gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "columnGap", field: "column_gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "wrap", field: "wrap", kind: Kind::Enumeration, values: vec![Value { ts: "nowrap", zig: "nowrap" }, Value { ts: "wrap", zig: "wrap" }, Value { ts: "wrap_reverse", zig: "wrap_reverse" }, Value { ts: "wrap-reverse", zig: "wrap_reverse" }, Value { ts: "wrapReverse", zig: "wrap_reverse" }], ..Default::default() },
                Prop { name: "alignContent", field: "align_content", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "alignSelf", field: "align_self", kind: Kind::Enumeration, values: vec![Value { ts: "auto", zig: "auto" }, Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "order", field: "order", kind: Kind::Integer, int_type: "i32", ..Default::default() },
                Prop { name: "padding", field: "padding", kind: Kind::Edges, sides: Sides::ALL, ..Default::default() },
                Prop { name: "margin", field: "margin", kind: Kind::Edges, sides: Sides::ALL, ..Default::default() },
                Prop { name: "justify", field: "justify", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "space_evenly", zig: "space_evenly" }, Value { ts: "space-evenly", zig: "space_evenly" }, Value { ts: "spaceEvenly", zig: "space_evenly" }], ..Default::default() },
                Prop { name: "justifyContent", field: "justify_content", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "space_evenly", zig: "space_evenly" }, Value { ts: "space-evenly", zig: "space_evenly" }, Value { ts: "spaceEvenly", zig: "space_evenly" }], ..Default::default() },
                Prop { name: "alignItems", field: "align_items", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "flexDirection", field: "flex_direction", kind: Kind::Enumeration, values: vec![Value { ts: "row", zig: "row" }, Value { ts: "column", zig: "column" }], ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "widthPercent", field: "width_percent", kind: Kind::Number, ..Default::default() },
                Prop { name: "heightPercent", field: "height_percent", kind: Kind::Number, ..Default::default() },
                Prop { name: "absolute", field: "absolute", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "top", field: "top", kind: Kind::Number, ..Default::default() },
                Prop { name: "right", field: "right", kind: Kind::Number, ..Default::default() },
                Prop { name: "bottom", field: "bottom", kind: Kind::Number, ..Default::default() },
                Prop { name: "left", field: "left", kind: Kind::Number, ..Default::default() },
                Prop { name: "minWidth", field: "min_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "minHeight", field: "min_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "maxWidth", field: "max_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "maxHeight", field: "max_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "grow", field: "grow", kind: Kind::Number, ..Default::default() },
                Prop { name: "shrink", field: "shrink", kind: Kind::Number, ..Default::default() },
                Prop { name: "basis", field: "basis", kind: Kind::Number, ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, ..Default::default() },
                Prop { name: "borderColor", field: "border_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "borderWidth", field: "border_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "radius", field: "radius", kind: Kind::Number, ..Default::default() },
                Prop { name: "clip", field: "clip", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "scroll", field: "scroll", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "blockPointer", field: "block_pointer", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "selectable", field: "selectable", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "unselectable", field: "unselectable", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "hoverBackground", field: "hover_background", kind: Kind::Color, ..Default::default() },
                Prop { name: "hoverBorderColor", field: "hover_border_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "focusScope", field: "focus_scope", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "showOnHover", field: "show_on_hover", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "onResize", field: "on_resize", kind: Kind::Action, ..Default::default() },
                Prop { name: "children", field: "children", kind: Kind::Children, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Row", builder: "ui.row", content: Content::Children, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "gap", field: "gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "rowGap", field: "row_gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "columnGap", field: "column_gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "wrap", field: "wrap", kind: Kind::Enumeration, values: vec![Value { ts: "nowrap", zig: "nowrap" }, Value { ts: "wrap", zig: "wrap" }, Value { ts: "wrap_reverse", zig: "wrap_reverse" }, Value { ts: "wrap-reverse", zig: "wrap_reverse" }, Value { ts: "wrapReverse", zig: "wrap_reverse" }], ..Default::default() },
                Prop { name: "alignContent", field: "align_content", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "alignSelf", field: "align_self", kind: Kind::Enumeration, values: vec![Value { ts: "auto", zig: "auto" }, Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "order", field: "order", kind: Kind::Integer, int_type: "i32", ..Default::default() },
                Prop { name: "padding", field: "padding", kind: Kind::Edges, sides: Sides::ALL, ..Default::default() },
                Prop { name: "margin", field: "margin", kind: Kind::Edges, sides: Sides::ALL, ..Default::default() },
                Prop { name: "justify", field: "justify", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "space_evenly", zig: "space_evenly" }, Value { ts: "space-evenly", zig: "space_evenly" }, Value { ts: "spaceEvenly", zig: "space_evenly" }], ..Default::default() },
                Prop { name: "justifyContent", field: "justify_content", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "space_evenly", zig: "space_evenly" }, Value { ts: "space-evenly", zig: "space_evenly" }, Value { ts: "spaceEvenly", zig: "space_evenly" }], ..Default::default() },
                Prop { name: "alignItems", field: "align_items", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "flexDirection", field: "flex_direction", kind: Kind::Enumeration, values: vec![Value { ts: "row", zig: "row" }, Value { ts: "column", zig: "column" }], ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "widthPercent", field: "width_percent", kind: Kind::Number, ..Default::default() },
                Prop { name: "heightPercent", field: "height_percent", kind: Kind::Number, ..Default::default() },
                Prop { name: "absolute", field: "absolute", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "top", field: "top", kind: Kind::Number, ..Default::default() },
                Prop { name: "right", field: "right", kind: Kind::Number, ..Default::default() },
                Prop { name: "bottom", field: "bottom", kind: Kind::Number, ..Default::default() },
                Prop { name: "left", field: "left", kind: Kind::Number, ..Default::default() },
                Prop { name: "minWidth", field: "min_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "minHeight", field: "min_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "maxWidth", field: "max_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "maxHeight", field: "max_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "grow", field: "grow", kind: Kind::Number, ..Default::default() },
                Prop { name: "shrink", field: "shrink", kind: Kind::Number, ..Default::default() },
                Prop { name: "basis", field: "basis", kind: Kind::Number, ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, ..Default::default() },
                Prop { name: "borderColor", field: "border_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "borderWidth", field: "border_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "radius", field: "radius", kind: Kind::Number, ..Default::default() },
                Prop { name: "clip", field: "clip", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "scroll", field: "scroll", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "blockPointer", field: "block_pointer", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "selectable", field: "selectable", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "unselectable", field: "unselectable", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "hoverBackground", field: "hover_background", kind: Kind::Color, ..Default::default() },
                Prop { name: "hoverBorderColor", field: "hover_border_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "focusScope", field: "focus_scope", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "showOnHover", field: "show_on_hover", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "onResize", field: "on_resize", kind: Kind::Action, ..Default::default() },
                Prop { name: "children", field: "children", kind: Kind::Children, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Clickable", builder: "ui.clickable", content: Content::Children, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, parent: "box", ..Default::default() },
                Prop { name: "gap", field: "gap", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "rowGap", field: "row_gap", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "columnGap", field: "column_gap", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "wrap", field: "wrap", kind: Kind::Enumeration, values: vec![Value { ts: "nowrap", zig: "nowrap" }, Value { ts: "wrap", zig: "wrap" }, Value { ts: "wrap_reverse", zig: "wrap_reverse" }, Value { ts: "wrap-reverse", zig: "wrap_reverse" }, Value { ts: "wrapReverse", zig: "wrap_reverse" }], parent: "box", ..Default::default() },
                Prop { name: "alignContent", field: "align_content", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "stretch", zig: "stretch" }], parent: "box", ..Default::default() },
                Prop { name: "alignSelf", field: "align_self", kind: Kind::Enumeration, values: vec![Value { ts: "auto", zig: "auto" }, Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "stretch", zig: "stretch" }], parent: "box", ..Default::default() },
                Prop { name: "order", field: "order", kind: Kind::Integer, int_type: "i32", parent: "box", ..Default::default() },
                Prop { name: "padding", field: "padding", kind: Kind::Edges, sides: Sides::ALL, parent: "box", ..Default::default() },
                Prop { name: "margin", field: "margin", kind: Kind::Edges, sides: Sides::ALL, parent: "box", ..Default::default() },
                Prop { name: "justify", field: "justify", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "space_evenly", zig: "space_evenly" }, Value { ts: "space-evenly", zig: "space_evenly" }, Value { ts: "spaceEvenly", zig: "space_evenly" }], parent: "box", ..Default::default() },
                Prop { name: "justifyContent", field: "justify_content", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "space_evenly", zig: "space_evenly" }, Value { ts: "space-evenly", zig: "space_evenly" }, Value { ts: "spaceEvenly", zig: "space_evenly" }], parent: "box", ..Default::default() },
                Prop { name: "alignItems", field: "align_items", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "stretch", zig: "stretch" }], parent: "box", ..Default::default() },
                Prop { name: "flexDirection", field: "flex_direction", kind: Kind::Enumeration, values: vec![Value { ts: "row", zig: "row" }, Value { ts: "column", zig: "column" }], parent: "box", ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "widthPercent", field: "width_percent", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "heightPercent", field: "height_percent", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "absolute", field: "absolute", kind: Kind::Boolean, parent: "box", ..Default::default() },
                Prop { name: "top", field: "top", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "right", field: "right", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "bottom", field: "bottom", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "left", field: "left", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "minWidth", field: "min_width", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "minHeight", field: "min_height", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "maxWidth", field: "max_width", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "maxHeight", field: "max_height", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "grow", field: "grow", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "shrink", field: "shrink", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "basis", field: "basis", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, parent: "box", ..Default::default() },
                Prop { name: "borderColor", field: "border_color", kind: Kind::Color, parent: "box", ..Default::default() },
                Prop { name: "borderWidth", field: "border_width", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "radius", field: "radius", kind: Kind::Number, parent: "box", ..Default::default() },
                Prop { name: "clip", field: "clip", kind: Kind::Boolean, parent: "box", ..Default::default() },
                Prop { name: "scroll", field: "scroll", kind: Kind::Boolean, parent: "box", ..Default::default() },
                Prop { name: "blockPointer", field: "block_pointer", kind: Kind::Boolean, parent: "box", ..Default::default() },
                Prop { name: "selectable", field: "selectable", kind: Kind::Boolean, parent: "box", ..Default::default() },
                Prop { name: "unselectable", field: "unselectable", kind: Kind::Boolean, parent: "box", ..Default::default() },
                Prop { name: "hoverBackground", field: "hover_background", kind: Kind::Color, parent: "box", ..Default::default() },
                Prop { name: "hoverBorderColor", field: "hover_border_color", kind: Kind::Color, parent: "box", ..Default::default() },
                Prop { name: "focusScope", field: "focus_scope", kind: Kind::Boolean, parent: "box", ..Default::default() },
                Prop { name: "showOnHover", field: "show_on_hover", kind: Kind::Boolean, parent: "box", ..Default::default() },
                Prop { name: "onResize", field: "on_resize", kind: Kind::Action, parent: "box", ..Default::default() },
                Prop { name: "children", field: "children", kind: Kind::Children, parent: "box", ..Default::default() },
                Prop { name: "onClick", field: "on_click", kind: Kind::Action, ..Default::default() },
                Prop { name: "indexAction", field: "index_action", kind: Kind::IndexAction, ..Default::default() },
                Prop { name: "index", field: "index", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                Prop { name: "cursor", field: "cursor", kind: Kind::Enumeration, values: vec![Value { ts: "arrow", zig: "arrow" }, Value { ts: "pointing_hand", zig: "pointing_hand" }, Value { ts: "pointing-hand", zig: "pointing_hand" }, Value { ts: "pointingHand", zig: "pointing_hand" }, Value { ts: "ibeam", zig: "ibeam" }, Value { ts: "crosshair", zig: "crosshair" }, Value { ts: "resize_horizontal", zig: "resize_horizontal" }, Value { ts: "resize-horizontal", zig: "resize_horizontal" }, Value { ts: "resizeHorizontal", zig: "resize_horizontal" }, Value { ts: "resize_vertical", zig: "resize_vertical" }, Value { ts: "resize-vertical", zig: "resize_vertical" }, Value { ts: "resizeVertical", zig: "resize_vertical" }, Value { ts: "grab", zig: "grab" }, Value { ts: "grabbing", zig: "grabbing" }, Value { ts: "not_allowed", zig: "not_allowed" }, Value { ts: "not-allowed", zig: "not_allowed" }, Value { ts: "notAllowed", zig: "not_allowed" }], ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Space", builder: "ui.space", content: Content::Children, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "direction", field: "direction", kind: Kind::Enumeration, values: vec![Value { ts: "row", zig: "row" }, Value { ts: "column", zig: "column" }], ..Default::default() },
                Prop { name: "gap", field: "gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "alignItems", field: "align_items", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "justify", field: "justify", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "space_evenly", zig: "space_evenly" }, Value { ts: "space-evenly", zig: "space_evenly" }, Value { ts: "spaceEvenly", zig: "space_evenly" }], ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "children", field: "children", kind: Kind::Children, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Text", builder: "ui.text", content: Content::TextArg, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "size", field: "size", kind: Kind::Number, ..Default::default() },
                Prop { name: "weight", field: "weight", kind: Kind::Enumeration, values: vec![Value { ts: "thin", zig: "thin" }, Value { ts: "extra_light", zig: "extra_light" }, Value { ts: "extra-light", zig: "extra_light" }, Value { ts: "extraLight", zig: "extra_light" }, Value { ts: "light", zig: "light" }, Value { ts: "regular", zig: "regular" }, Value { ts: "medium", zig: "medium" }, Value { ts: "semibold", zig: "semibold" }, Value { ts: "bold", zig: "bold" }, Value { ts: "extra_bold", zig: "extra_bold" }, Value { ts: "extra-bold", zig: "extra_bold" }, Value { ts: "extraBold", zig: "extra_bold" }, Value { ts: "black", zig: "black" }], ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
                Prop { name: "textAlign", field: "text_align", kind: Kind::Enumeration, values: vec![Value { ts: "left", zig: "left" }, Value { ts: "center", zig: "center" }, Value { ts: "right", zig: "right" }], ..Default::default() },
                Prop { name: "wrap", field: "wrap", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "lineHeight", field: "line_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "maxLines", field: "max_lines", kind: Kind::Integer, int_type: "u16", ..Default::default() },
                Prop { name: "family", field: "family", kind: Kind::Enumeration, values: vec![Value { ts: "sans", zig: "sans" }, Value { ts: "serif", zig: "serif" }, Value { ts: "mono", zig: "mono" }, Value { ts: "serif_cjk", zig: "serif_cjk" }, Value { ts: "serif-cjk", zig: "serif_cjk" }, Value { ts: "serifCjk", zig: "serif_cjk" }, Value { ts: "math", zig: "math" }], ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Heading", builder: "ui.heading", content: Content::TextArg, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "level", field: "level", kind: Kind::Integer, int_type: "u8", ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
                Prop { name: "weight", field: "weight", kind: Kind::Enumeration, values: vec![Value { ts: "thin", zig: "thin" }, Value { ts: "extra_light", zig: "extra_light" }, Value { ts: "extra-light", zig: "extra_light" }, Value { ts: "extraLight", zig: "extra_light" }, Value { ts: "light", zig: "light" }, Value { ts: "regular", zig: "regular" }, Value { ts: "medium", zig: "medium" }, Value { ts: "semibold", zig: "semibold" }, Value { ts: "bold", zig: "bold" }, Value { ts: "extra_bold", zig: "extra_bold" }, Value { ts: "extra-bold", zig: "extra_bold" }, Value { ts: "extraBold", zig: "extra_bold" }, Value { ts: "black", zig: "black" }], ..Default::default() },
                Prop { name: "textAlign", field: "text_align", kind: Kind::Enumeration, values: vec![Value { ts: "left", zig: "left" }, Value { ts: "center", zig: "center" }, Value { ts: "right", zig: "right" }], ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Title", builder: "ui.title", content: Content::TextArg, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "level", field: "level", kind: Kind::Integer, int_type: "u8", ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
                Prop { name: "weight", field: "weight", kind: Kind::Enumeration, values: vec![Value { ts: "thin", zig: "thin" }, Value { ts: "extra_light", zig: "extra_light" }, Value { ts: "extra-light", zig: "extra_light" }, Value { ts: "extraLight", zig: "extra_light" }, Value { ts: "light", zig: "light" }, Value { ts: "regular", zig: "regular" }, Value { ts: "medium", zig: "medium" }, Value { ts: "semibold", zig: "semibold" }, Value { ts: "bold", zig: "bold" }, Value { ts: "extra_bold", zig: "extra_bold" }, Value { ts: "extra-bold", zig: "extra_bold" }, Value { ts: "extraBold", zig: "extra_bold" }, Value { ts: "black", zig: "black" }], ..Default::default() },
                Prop { name: "textAlign", field: "text_align", kind: Kind::Enumeration, values: vec![Value { ts: "left", zig: "left" }, Value { ts: "center", zig: "center" }, Value { ts: "right", zig: "right" }], ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Paragraph", builder: "ui.paragraph", content: Content::TextArg, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "size", field: "size", kind: Kind::Number, ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
                Prop { name: "textAlign", field: "text_align", kind: Kind::Enumeration, values: vec![Value { ts: "left", zig: "left" }, Value { ts: "center", zig: "center" }, Value { ts: "right", zig: "right" }], ..Default::default() },
                Prop { name: "lineHeight", field: "line_height", kind: Kind::Number, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Button", builder: "ui.button", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "text", field: "text", kind: Kind::String, ..Default::default() },
                Prop { name: "variant", field: "variant", kind: Kind::Enumeration, values: vec![Value { ts: "primary", zig: "primary" }, Value { ts: "default", zig: "default" }, Value { ts: "text", zig: "text" }, Value { ts: "link", zig: "link" }, Value { ts: "plain", zig: "plain" }, Value { ts: "caption", zig: "caption" }, Value { ts: "caption_close", zig: "caption_close" }, Value { ts: "caption-close", zig: "caption_close" }, Value { ts: "captionClose", zig: "caption_close" }], ..Default::default() },
                Prop { name: "size", field: "size", kind: Kind::Enumeration, values: vec![Value { ts: "small", zig: "small" }, Value { ts: "middle", zig: "middle" }, Value { ts: "large", zig: "large" }], ..Default::default() },
                Prop { name: "disabled", field: "disabled", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "fontSize", field: "font_size", kind: Kind::Number, ..Default::default() },
                Prop { name: "textAlign", field: "text_align", kind: Kind::Enumeration, values: vec![Value { ts: "left", zig: "left" }, Value { ts: "center", zig: "center" }, Value { ts: "right", zig: "right" }], ..Default::default() },
                Prop { name: "labelPadding", field: "label_padding", kind: Kind::Number, ..Default::default() },
                Prop { name: "textColor", field: "text_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "icon", field: "icon", kind: Kind::Enumeration, values: vec![Value { ts: "none", zig: "none" }, Value { ts: "menu", zig: "menu" }, Value { ts: "list", zig: "list" }, Value { ts: "search", zig: "search" }, Value { ts: "eye", zig: "eye" }, Value { ts: "eye_off", zig: "eye_off" }, Value { ts: "eye-off", zig: "eye_off" }, Value { ts: "eyeOff", zig: "eye_off" }, Value { ts: "more", zig: "more" }, Value { ts: "more_vertical", zig: "more_vertical" }, Value { ts: "more-vertical", zig: "more_vertical" }, Value { ts: "moreVertical", zig: "more_vertical" }, Value { ts: "close", zig: "close" }, Value { ts: "plus", zig: "plus" }, Value { ts: "minus", zig: "minus" }, Value { ts: "check", zig: "check" }, Value { ts: "chevron_down", zig: "chevron_down" }, Value { ts: "chevron-down", zig: "chevron_down" }, Value { ts: "chevronDown", zig: "chevron_down" }, Value { ts: "chevron_up", zig: "chevron_up" }, Value { ts: "chevron-up", zig: "chevron_up" }, Value { ts: "chevronUp", zig: "chevron_up" }, Value { ts: "chevron_left", zig: "chevron_left" }, Value { ts: "chevron-left", zig: "chevron_left" }, Value { ts: "chevronLeft", zig: "chevron_left" }, Value { ts: "chevron_right", zig: "chevron_right" }, Value { ts: "chevron-right", zig: "chevron_right" }, Value { ts: "chevronRight", zig: "chevron_right" }, Value { ts: "arrow_down", zig: "arrow_down" }, Value { ts: "arrow-down", zig: "arrow_down" }, Value { ts: "arrowDown", zig: "arrow_down" }, Value { ts: "arrow_up", zig: "arrow_up" }, Value { ts: "arrow-up", zig: "arrow_up" }, Value { ts: "arrowUp", zig: "arrow_up" }, Value { ts: "file", zig: "file" }, Value { ts: "file_text", zig: "file_text" }, Value { ts: "file-text", zig: "file_text" }, Value { ts: "fileText", zig: "file_text" }, Value { ts: "folder", zig: "folder" }, Value { ts: "save", zig: "save" }, Value { ts: "copy", zig: "copy" }, Value { ts: "download", zig: "download" }, Value { ts: "upload", zig: "upload" }, Value { ts: "link", zig: "link" }, Value { ts: "external_link", zig: "external_link" }, Value { ts: "external-link", zig: "external_link" }, Value { ts: "externalLink", zig: "external_link" }, Value { ts: "pencil", zig: "pencil" }, Value { ts: "trash", zig: "trash" }, Value { ts: "undo", zig: "undo" }, Value { ts: "redo", zig: "redo" }, Value { ts: "refresh", zig: "refresh" }, Value { ts: "settings", zig: "settings" }, Value { ts: "sun", zig: "sun" }, Value { ts: "moon", zig: "moon" }, Value { ts: "columns", zig: "columns" }, Value { ts: "keyboard", zig: "keyboard" }, Value { ts: "book_open", zig: "book_open" }, Value { ts: "book-open", zig: "book_open" }, Value { ts: "bookOpen", zig: "book_open" }, Value { ts: "maximize", zig: "maximize" }, Value { ts: "minimize", zig: "minimize" }, Value { ts: "focus", zig: "focus" }, Value { ts: "type_size", zig: "type_size" }, Value { ts: "type-size", zig: "type_size" }, Value { ts: "typeSize", zig: "type_size" }, Value { ts: "heading", zig: "heading" }, Value { ts: "bold", zig: "bold" }, Value { ts: "italic", zig: "italic" }, Value { ts: "strikethrough", zig: "strikethrough" }, Value { ts: "code", zig: "code" }, Value { ts: "code_block", zig: "code_block" }, Value { ts: "code-block", zig: "code_block" }, Value { ts: "codeBlock", zig: "code_block" }, Value { ts: "quote", zig: "quote" }, Value { ts: "list_bullet", zig: "list_bullet" }, Value { ts: "list-bullet", zig: "list_bullet" }, Value { ts: "listBullet", zig: "list_bullet" }, Value { ts: "list_ordered", zig: "list_ordered" }, Value { ts: "list-ordered", zig: "list_ordered" }, Value { ts: "listOrdered", zig: "list_ordered" }, Value { ts: "list_checks", zig: "list_checks" }, Value { ts: "list-checks", zig: "list_checks" }, Value { ts: "listChecks", zig: "list_checks" }, Value { ts: "table", zig: "table" }, Value { ts: "sigma", zig: "sigma" }, Value { ts: "image", zig: "image" }, Value { ts: "minus_rule", zig: "minus_rule" }, Value { ts: "minus-rule", zig: "minus_rule" }, Value { ts: "minusRule", zig: "minus_rule" }, Value { ts: "window_minimize", zig: "window_minimize" }, Value { ts: "window-minimize", zig: "window_minimize" }, Value { ts: "windowMinimize", zig: "window_minimize" }, Value { ts: "window_maximize", zig: "window_maximize" }, Value { ts: "window-maximize", zig: "window_maximize" }, Value { ts: "windowMaximize", zig: "window_maximize" }, Value { ts: "window_restore", zig: "window_restore" }, Value { ts: "window-restore", zig: "window_restore" }, Value { ts: "windowRestore", zig: "window_restore" }, Value { ts: "window_close", zig: "window_close" }, Value { ts: "window-close", zig: "window_close" }, Value { ts: "windowClose", zig: "window_close" }, Value { ts: "info", zig: "info" }, Value { ts: "alert", zig: "alert" }, Value { ts: "lightbulb", zig: "lightbulb" }, Value { ts: "clipboard", zig: "clipboard" }, Value { ts: "monitor", zig: "monitor" }, Value { ts: "wifi", zig: "wifi" }, Value { ts: "wifi_off", zig: "wifi_off" }, Value { ts: "wifi-off", zig: "wifi_off" }, Value { ts: "wifiOff", zig: "wifi_off" }, Value { ts: "clock", zig: "clock" }, Value { ts: "power", zig: "power" }], ..Default::default() },
                Prop { name: "iconSize", field: "icon_size", kind: Kind::Number, ..Default::default() },
                Prop { name: "onClick", field: "on_click", kind: Kind::Action, ..Default::default() },
                Prop { name: "indexAction", field: "index_action", kind: Kind::IndexAction, ..Default::default() },
                Prop { name: "index", field: "index", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                Prop { name: "focusable", field: "focusable", kind: Kind::Boolean, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Link", builder: "ui.link", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "text", field: "text", kind: Kind::String, ..Default::default() },
                Prop { name: "onClick", field: "on_click", kind: Kind::Action, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Input", builder: "ui.input", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "value", field: "value", kind: Kind::String, ..Default::default() },
                Prop { name: "placeholder", field: "placeholder", kind: Kind::String, ..Default::default() },
                Prop { name: "disabled", field: "disabled", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "readOnly", field: "read_only", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "password", field: "password", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "multiline", field: "multiline", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "focusRing", field: "focus_ring", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "fontSize", field: "font_size", kind: Kind::Number, ..Default::default() },
                Prop { name: "lineHeight", field: "line_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "onChange", field: "on_change", kind: Kind::TextAction, ..Default::default() },
                Prop { name: "onSubmit", field: "on_submit", kind: Kind::Action, ..Default::default() },
                Prop { name: "onUndo", field: "on_undo", kind: Kind::Action, ..Default::default() },
                Prop { name: "onRedo", field: "on_redo", kind: Kind::Action, ..Default::default() },
                Prop { name: "contentMaxWidth", field: "content_max_width", kind: Kind::Number, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Checkbox", builder: "ui.checkbox", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "label", field: "label", kind: Kind::String, ..Default::default() },
                Prop { name: "checked", field: "checked", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "disabled", field: "disabled", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "fontSize", field: "font_size", kind: Kind::Number, ..Default::default() },
                Prop { name: "onChange", field: "on_change", kind: Kind::BoolAction, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Switch", builder: "ui.@\"switch\"", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "label", field: "label", kind: Kind::String, ..Default::default() },
                Prop { name: "checked", field: "checked", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "disabled", field: "disabled", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "fontSize", field: "font_size", kind: Kind::Number, ..Default::default() },
                Prop { name: "onChange", field: "on_change", kind: Kind::BoolAction, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "RadioGroup", builder: "ui.radioGroup", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "options", field: "options", kind: Kind::StructList, item_type: "ui.RadioOption", item_props: vec![
                    Prop { name: "label", field: "label", kind: Kind::String, required: true, ..Default::default() },
                    Prop { name: "disabled", field: "disabled", kind: Kind::Boolean, ..Default::default() },
                ], ..Default::default() },
                Prop { name: "selected", field: "selected", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                Prop { name: "direction", field: "direction", kind: Kind::Enumeration, values: vec![Value { ts: "row", zig: "row" }, Value { ts: "column", zig: "column" }], ..Default::default() },
                Prop { name: "gap", field: "gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "onChange", field: "on_change", kind: Kind::IndexAction, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Tabs", builder: "ui.tabs", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "items", field: "items", kind: Kind::StructList, item_type: "ui.TabItem", item_props: vec![
                    Prop { name: "label", field: "label", kind: Kind::String, required: true, ..Default::default() },
                    Prop { name: "content", field: "content", kind: Kind::Node, required: true, ..Default::default() },
                ], ..Default::default() },
                Prop { name: "active", field: "active", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                Prop { name: "onChange", field: "on_change", kind: Kind::IndexAction, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Tooltip", builder: "ui.tooltip", content: Content::Child, content_field: "child", props: vec![
                Prop { name: "text", field: "text", kind: Kind::String, required: true, ..Default::default() },
                Prop { name: "child", field: "child", kind: Kind::Node, required: true, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Image", builder: "ui.image", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "src", field: "src", kind: Kind::String, ..Default::default() },
                Prop { name: "image", field: "image", kind: Kind::Image, ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "fit", field: "fit", kind: Kind::Enumeration, values: vec![Value { ts: "cover", zig: "cover" }, Value { ts: "contain", zig: "contain" }, Value { ts: "fill", zig: "fill" }], ..Default::default() },
                Prop { name: "radius", field: "radius", kind: Kind::Number, ..Default::default() },
                Prop { name: "tooltip", field: "tooltip", kind: Kind::String, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Select", builder: "ui.select", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "options", field: "options", kind: Kind::StringList, ..Default::default() },
                Prop { name: "selected", field: "selected", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "disabled", field: "disabled", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "tooltip", field: "tooltip", kind: Kind::String, ..Default::default() },
                Prop { name: "onChange", field: "on_change", kind: Kind::IndexAction, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Icon", builder: "ui.icon", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "name", field: "name", kind: Kind::Enumeration, values: vec![Value { ts: "none", zig: "none" }, Value { ts: "menu", zig: "menu" }, Value { ts: "list", zig: "list" }, Value { ts: "search", zig: "search" }, Value { ts: "eye", zig: "eye" }, Value { ts: "eye_off", zig: "eye_off" }, Value { ts: "eye-off", zig: "eye_off" }, Value { ts: "eyeOff", zig: "eye_off" }, Value { ts: "more", zig: "more" }, Value { ts: "more_vertical", zig: "more_vertical" }, Value { ts: "more-vertical", zig: "more_vertical" }, Value { ts: "moreVertical", zig: "more_vertical" }, Value { ts: "close", zig: "close" }, Value { ts: "plus", zig: "plus" }, Value { ts: "minus", zig: "minus" }, Value { ts: "check", zig: "check" }, Value { ts: "chevron_down", zig: "chevron_down" }, Value { ts: "chevron-down", zig: "chevron_down" }, Value { ts: "chevronDown", zig: "chevron_down" }, Value { ts: "chevron_up", zig: "chevron_up" }, Value { ts: "chevron-up", zig: "chevron_up" }, Value { ts: "chevronUp", zig: "chevron_up" }, Value { ts: "chevron_left", zig: "chevron_left" }, Value { ts: "chevron-left", zig: "chevron_left" }, Value { ts: "chevronLeft", zig: "chevron_left" }, Value { ts: "chevron_right", zig: "chevron_right" }, Value { ts: "chevron-right", zig: "chevron_right" }, Value { ts: "chevronRight", zig: "chevron_right" }, Value { ts: "arrow_down", zig: "arrow_down" }, Value { ts: "arrow-down", zig: "arrow_down" }, Value { ts: "arrowDown", zig: "arrow_down" }, Value { ts: "arrow_up", zig: "arrow_up" }, Value { ts: "arrow-up", zig: "arrow_up" }, Value { ts: "arrowUp", zig: "arrow_up" }, Value { ts: "file", zig: "file" }, Value { ts: "file_text", zig: "file_text" }, Value { ts: "file-text", zig: "file_text" }, Value { ts: "fileText", zig: "file_text" }, Value { ts: "folder", zig: "folder" }, Value { ts: "save", zig: "save" }, Value { ts: "copy", zig: "copy" }, Value { ts: "download", zig: "download" }, Value { ts: "upload", zig: "upload" }, Value { ts: "link", zig: "link" }, Value { ts: "external_link", zig: "external_link" }, Value { ts: "external-link", zig: "external_link" }, Value { ts: "externalLink", zig: "external_link" }, Value { ts: "pencil", zig: "pencil" }, Value { ts: "trash", zig: "trash" }, Value { ts: "undo", zig: "undo" }, Value { ts: "redo", zig: "redo" }, Value { ts: "refresh", zig: "refresh" }, Value { ts: "settings", zig: "settings" }, Value { ts: "sun", zig: "sun" }, Value { ts: "moon", zig: "moon" }, Value { ts: "columns", zig: "columns" }, Value { ts: "keyboard", zig: "keyboard" }, Value { ts: "book_open", zig: "book_open" }, Value { ts: "book-open", zig: "book_open" }, Value { ts: "bookOpen", zig: "book_open" }, Value { ts: "maximize", zig: "maximize" }, Value { ts: "minimize", zig: "minimize" }, Value { ts: "focus", zig: "focus" }, Value { ts: "type_size", zig: "type_size" }, Value { ts: "type-size", zig: "type_size" }, Value { ts: "typeSize", zig: "type_size" }, Value { ts: "heading", zig: "heading" }, Value { ts: "bold", zig: "bold" }, Value { ts: "italic", zig: "italic" }, Value { ts: "strikethrough", zig: "strikethrough" }, Value { ts: "code", zig: "code" }, Value { ts: "code_block", zig: "code_block" }, Value { ts: "code-block", zig: "code_block" }, Value { ts: "codeBlock", zig: "code_block" }, Value { ts: "quote", zig: "quote" }, Value { ts: "list_bullet", zig: "list_bullet" }, Value { ts: "list-bullet", zig: "list_bullet" }, Value { ts: "listBullet", zig: "list_bullet" }, Value { ts: "list_ordered", zig: "list_ordered" }, Value { ts: "list-ordered", zig: "list_ordered" }, Value { ts: "listOrdered", zig: "list_ordered" }, Value { ts: "list_checks", zig: "list_checks" }, Value { ts: "list-checks", zig: "list_checks" }, Value { ts: "listChecks", zig: "list_checks" }, Value { ts: "table", zig: "table" }, Value { ts: "sigma", zig: "sigma" }, Value { ts: "image", zig: "image" }, Value { ts: "minus_rule", zig: "minus_rule" }, Value { ts: "minus-rule", zig: "minus_rule" }, Value { ts: "minusRule", zig: "minus_rule" }, Value { ts: "window_minimize", zig: "window_minimize" }, Value { ts: "window-minimize", zig: "window_minimize" }, Value { ts: "windowMinimize", zig: "window_minimize" }, Value { ts: "window_maximize", zig: "window_maximize" }, Value { ts: "window-maximize", zig: "window_maximize" }, Value { ts: "windowMaximize", zig: "window_maximize" }, Value { ts: "window_restore", zig: "window_restore" }, Value { ts: "window-restore", zig: "window_restore" }, Value { ts: "windowRestore", zig: "window_restore" }, Value { ts: "window_close", zig: "window_close" }, Value { ts: "window-close", zig: "window_close" }, Value { ts: "windowClose", zig: "window_close" }, Value { ts: "info", zig: "info" }, Value { ts: "alert", zig: "alert" }, Value { ts: "lightbulb", zig: "lightbulb" }, Value { ts: "clipboard", zig: "clipboard" }, Value { ts: "monitor", zig: "monitor" }, Value { ts: "wifi", zig: "wifi" }, Value { ts: "wifi_off", zig: "wifi_off" }, Value { ts: "wifi-off", zig: "wifi_off" }, Value { ts: "wifiOff", zig: "wifi_off" }, Value { ts: "clock", zig: "clock" }, Value { ts: "power", zig: "power" }], required: true, ..Default::default() },
                Prop { name: "size", field: "size", kind: Kind::Number, ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
                Prop { name: "stroke", field: "stroke", kind: Kind::Number, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "IconButton", builder: "ui.iconButton", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "icon", field: "icon", kind: Kind::Enumeration, values: vec![Value { ts: "none", zig: "none" }, Value { ts: "menu", zig: "menu" }, Value { ts: "list", zig: "list" }, Value { ts: "search", zig: "search" }, Value { ts: "eye", zig: "eye" }, Value { ts: "eye_off", zig: "eye_off" }, Value { ts: "eye-off", zig: "eye_off" }, Value { ts: "eyeOff", zig: "eye_off" }, Value { ts: "more", zig: "more" }, Value { ts: "more_vertical", zig: "more_vertical" }, Value { ts: "more-vertical", zig: "more_vertical" }, Value { ts: "moreVertical", zig: "more_vertical" }, Value { ts: "close", zig: "close" }, Value { ts: "plus", zig: "plus" }, Value { ts: "minus", zig: "minus" }, Value { ts: "check", zig: "check" }, Value { ts: "chevron_down", zig: "chevron_down" }, Value { ts: "chevron-down", zig: "chevron_down" }, Value { ts: "chevronDown", zig: "chevron_down" }, Value { ts: "chevron_up", zig: "chevron_up" }, Value { ts: "chevron-up", zig: "chevron_up" }, Value { ts: "chevronUp", zig: "chevron_up" }, Value { ts: "chevron_left", zig: "chevron_left" }, Value { ts: "chevron-left", zig: "chevron_left" }, Value { ts: "chevronLeft", zig: "chevron_left" }, Value { ts: "chevron_right", zig: "chevron_right" }, Value { ts: "chevron-right", zig: "chevron_right" }, Value { ts: "chevronRight", zig: "chevron_right" }, Value { ts: "arrow_down", zig: "arrow_down" }, Value { ts: "arrow-down", zig: "arrow_down" }, Value { ts: "arrowDown", zig: "arrow_down" }, Value { ts: "arrow_up", zig: "arrow_up" }, Value { ts: "arrow-up", zig: "arrow_up" }, Value { ts: "arrowUp", zig: "arrow_up" }, Value { ts: "file", zig: "file" }, Value { ts: "file_text", zig: "file_text" }, Value { ts: "file-text", zig: "file_text" }, Value { ts: "fileText", zig: "file_text" }, Value { ts: "folder", zig: "folder" }, Value { ts: "save", zig: "save" }, Value { ts: "copy", zig: "copy" }, Value { ts: "download", zig: "download" }, Value { ts: "upload", zig: "upload" }, Value { ts: "link", zig: "link" }, Value { ts: "external_link", zig: "external_link" }, Value { ts: "external-link", zig: "external_link" }, Value { ts: "externalLink", zig: "external_link" }, Value { ts: "pencil", zig: "pencil" }, Value { ts: "trash", zig: "trash" }, Value { ts: "undo", zig: "undo" }, Value { ts: "redo", zig: "redo" }, Value { ts: "refresh", zig: "refresh" }, Value { ts: "settings", zig: "settings" }, Value { ts: "sun", zig: "sun" }, Value { ts: "moon", zig: "moon" }, Value { ts: "columns", zig: "columns" }, Value { ts: "keyboard", zig: "keyboard" }, Value { ts: "book_open", zig: "book_open" }, Value { ts: "book-open", zig: "book_open" }, Value { ts: "bookOpen", zig: "book_open" }, Value { ts: "maximize", zig: "maximize" }, Value { ts: "minimize", zig: "minimize" }, Value { ts: "focus", zig: "focus" }, Value { ts: "type_size", zig: "type_size" }, Value { ts: "type-size", zig: "type_size" }, Value { ts: "typeSize", zig: "type_size" }, Value { ts: "heading", zig: "heading" }, Value { ts: "bold", zig: "bold" }, Value { ts: "italic", zig: "italic" }, Value { ts: "strikethrough", zig: "strikethrough" }, Value { ts: "code", zig: "code" }, Value { ts: "code_block", zig: "code_block" }, Value { ts: "code-block", zig: "code_block" }, Value { ts: "codeBlock", zig: "code_block" }, Value { ts: "quote", zig: "quote" }, Value { ts: "list_bullet", zig: "list_bullet" }, Value { ts: "list-bullet", zig: "list_bullet" }, Value { ts: "listBullet", zig: "list_bullet" }, Value { ts: "list_ordered", zig: "list_ordered" }, Value { ts: "list-ordered", zig: "list_ordered" }, Value { ts: "listOrdered", zig: "list_ordered" }, Value { ts: "list_checks", zig: "list_checks" }, Value { ts: "list-checks", zig: "list_checks" }, Value { ts: "listChecks", zig: "list_checks" }, Value { ts: "table", zig: "table" }, Value { ts: "sigma", zig: "sigma" }, Value { ts: "image", zig: "image" }, Value { ts: "minus_rule", zig: "minus_rule" }, Value { ts: "minus-rule", zig: "minus_rule" }, Value { ts: "minusRule", zig: "minus_rule" }, Value { ts: "window_minimize", zig: "window_minimize" }, Value { ts: "window-minimize", zig: "window_minimize" }, Value { ts: "windowMinimize", zig: "window_minimize" }, Value { ts: "window_maximize", zig: "window_maximize" }, Value { ts: "window-maximize", zig: "window_maximize" }, Value { ts: "windowMaximize", zig: "window_maximize" }, Value { ts: "window_restore", zig: "window_restore" }, Value { ts: "window-restore", zig: "window_restore" }, Value { ts: "windowRestore", zig: "window_restore" }, Value { ts: "window_close", zig: "window_close" }, Value { ts: "window-close", zig: "window_close" }, Value { ts: "windowClose", zig: "window_close" }, Value { ts: "info", zig: "info" }, Value { ts: "alert", zig: "alert" }, Value { ts: "lightbulb", zig: "lightbulb" }, Value { ts: "clipboard", zig: "clipboard" }, Value { ts: "monitor", zig: "monitor" }, Value { ts: "wifi", zig: "wifi" }, Value { ts: "wifi_off", zig: "wifi_off" }, Value { ts: "wifi-off", zig: "wifi_off" }, Value { ts: "wifiOff", zig: "wifi_off" }, Value { ts: "clock", zig: "clock" }, Value { ts: "power", zig: "power" }], required: true, ..Default::default() },
                Prop { name: "size", field: "size", kind: Kind::Number, ..Default::default() },
                Prop { name: "box", field: "box", kind: Kind::Number, ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
                Prop { name: "active", field: "active", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "activeBackground", field: "active_background", kind: Kind::Color, ..Default::default() },
                Prop { name: "disabled", field: "disabled", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "focusable", field: "focusable", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "tooltip", field: "tooltip", kind: Kind::String, ..Default::default() },
                Prop { name: "onClick", field: "on_click", kind: Kind::Action, ..Default::default() },
                Prop { name: "indexAction", field: "index_action", kind: Kind::IndexAction, ..Default::default() },
                Prop { name: "index", field: "index", kind: Kind::Integer, int_type: "usize", ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "DismissLayer", builder: "ui.dismissLayer", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "onDismiss", field: "on_dismiss", kind: Kind::Action, ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "RichText", builder: "ui.richText", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "lineHeight", field: "line_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "onLink", field: "on_link", kind: Kind::TextAction, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "PdfPreview", builder: "ui.pdfPreview", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "src", field: "src", kind: Kind::String, required: true, ..Default::default() },
                Prop { name: "page", field: "page", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "showToolbar", field: "show_toolbar", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, ..Default::default() },
                Prop { name: "onPageChange", field: "on_page_change", kind: Kind::IndexAction, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Divider", builder: "ui.divider", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "margin", field: "margin", kind: Kind::Edges, sides: Sides::ALL, ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Tag", builder: "ui.tag", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "text", field: "text", kind: Kind::String, ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Card", builder: "ui.card", content: Content::Children, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "title", field: "title", kind: Kind::String, ..Default::default() },
                Prop { name: "extra", field: "extra", kind: Kind::String, ..Default::default() },
                Prop { name: "children", field: "children", kind: Kind::Children, ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "gap", field: "gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "padding", field: "padding", kind: Kind::Edges, sides: Sides::ALL, ..Default::default() },
                Prop { name: "bordered", field: "bordered", kind: Kind::Boolean, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Alert", builder: "ui.alert", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "type", field: "type", kind: Kind::Enumeration, values: vec![Value { ts: "info", zig: "info" }, Value { ts: "success", zig: "success" }, Value { ts: "warning", zig: "warning" }, Value { ts: "error_state", zig: "error_state" }, Value { ts: "error-state", zig: "error_state" }, Value { ts: "errorState", zig: "error_state" }], ..Default::default() },
                Prop { name: "title", field: "title", kind: Kind::String, ..Default::default() },
                Prop { name: "description", field: "description", kind: Kind::String, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Avatar", builder: "ui.avatar", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "text", field: "text", kind: Kind::String, ..Default::default() },
                Prop { name: "size", field: "size", kind: Kind::Number, ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Progress", builder: "ui.progress", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "percent", field: "percent", kind: Kind::Number, ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "color", field: "color", kind: Kind::Color, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "DropdownMenu", builder: "ui.dropdownMenu", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "items", field: "items", kind: Kind::StructList, required: true, item_type: "ui.MenuItem", item_props: vec![
                    Prop { name: "kind", field: "kind", kind: Kind::Enumeration, values: vec![Value { ts: "item", zig: "item" }, Value { ts: "divider", zig: "divider" }, Value { ts: "group", zig: "group" }], ..Default::default() },
                    Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                    Prop { name: "label", field: "label", kind: Kind::String, ..Default::default() },
                    Prop { name: "icon", field: "icon", kind: Kind::Enumeration, values: vec![Value { ts: "none", zig: "none" }, Value { ts: "menu", zig: "menu" }, Value { ts: "list", zig: "list" }, Value { ts: "search", zig: "search" }, Value { ts: "eye", zig: "eye" }, Value { ts: "eye_off", zig: "eye_off" }, Value { ts: "eye-off", zig: "eye_off" }, Value { ts: "eyeOff", zig: "eye_off" }, Value { ts: "more", zig: "more" }, Value { ts: "more_vertical", zig: "more_vertical" }, Value { ts: "more-vertical", zig: "more_vertical" }, Value { ts: "moreVertical", zig: "more_vertical" }, Value { ts: "close", zig: "close" }, Value { ts: "plus", zig: "plus" }, Value { ts: "minus", zig: "minus" }, Value { ts: "check", zig: "check" }, Value { ts: "chevron_down", zig: "chevron_down" }, Value { ts: "chevron-down", zig: "chevron_down" }, Value { ts: "chevronDown", zig: "chevron_down" }, Value { ts: "chevron_up", zig: "chevron_up" }, Value { ts: "chevron-up", zig: "chevron_up" }, Value { ts: "chevronUp", zig: "chevron_up" }, Value { ts: "chevron_left", zig: "chevron_left" }, Value { ts: "chevron-left", zig: "chevron_left" }, Value { ts: "chevronLeft", zig: "chevron_left" }, Value { ts: "chevron_right", zig: "chevron_right" }, Value { ts: "chevron-right", zig: "chevron_right" }, Value { ts: "chevronRight", zig: "chevron_right" }, Value { ts: "arrow_down", zig: "arrow_down" }, Value { ts: "arrow-down", zig: "arrow_down" }, Value { ts: "arrowDown", zig: "arrow_down" }, Value { ts: "arrow_up", zig: "arrow_up" }, Value { ts: "arrow-up", zig: "arrow_up" }, Value { ts: "arrowUp", zig: "arrow_up" }, Value { ts: "file", zig: "file" }, Value { ts: "file_text", zig: "file_text" }, Value { ts: "file-text", zig: "file_text" }, Value { ts: "fileText", zig: "file_text" }, Value { ts: "folder", zig: "folder" }, Value { ts: "save", zig: "save" }, Value { ts: "copy", zig: "copy" }, Value { ts: "download", zig: "download" }, Value { ts: "upload", zig: "upload" }, Value { ts: "link", zig: "link" }, Value { ts: "external_link", zig: "external_link" }, Value { ts: "external-link", zig: "external_link" }, Value { ts: "externalLink", zig: "external_link" }, Value { ts: "pencil", zig: "pencil" }, Value { ts: "trash", zig: "trash" }, Value { ts: "undo", zig: "undo" }, Value { ts: "redo", zig: "redo" }, Value { ts: "refresh", zig: "refresh" }, Value { ts: "settings", zig: "settings" }, Value { ts: "sun", zig: "sun" }, Value { ts: "moon", zig: "moon" }, Value { ts: "columns", zig: "columns" }, Value { ts: "keyboard", zig: "keyboard" }, Value { ts: "book_open", zig: "book_open" }, Value { ts: "book-open", zig: "book_open" }, Value { ts: "bookOpen", zig: "book_open" }, Value { ts: "maximize", zig: "maximize" }, Value { ts: "minimize", zig: "minimize" }, Value { ts: "focus", zig: "focus" }, Value { ts: "type_size", zig: "type_size" }, Value { ts: "type-size", zig: "type_size" }, Value { ts: "typeSize", zig: "type_size" }, Value { ts: "heading", zig: "heading" }, Value { ts: "bold", zig: "bold" }, Value { ts: "italic", zig: "italic" }, Value { ts: "strikethrough", zig: "strikethrough" }, Value { ts: "code", zig: "code" }, Value { ts: "code_block", zig: "code_block" }, Value { ts: "code-block", zig: "code_block" }, Value { ts: "codeBlock", zig: "code_block" }, Value { ts: "quote", zig: "quote" }, Value { ts: "list_bullet", zig: "list_bullet" }, Value { ts: "list-bullet", zig: "list_bullet" }, Value { ts: "listBullet", zig: "list_bullet" }, Value { ts: "list_ordered", zig: "list_ordered" }, Value { ts: "list-ordered", zig: "list_ordered" }, Value { ts: "listOrdered", zig: "list_ordered" }, Value { ts: "list_checks", zig: "list_checks" }, Value { ts: "list-checks", zig: "list_checks" }, Value { ts: "listChecks", zig: "list_checks" }, Value { ts: "table", zig: "table" }, Value { ts: "sigma", zig: "sigma" }, Value { ts: "image", zig: "image" }, Value { ts: "minus_rule", zig: "minus_rule" }, Value { ts: "minus-rule", zig: "minus_rule" }, Value { ts: "minusRule", zig: "minus_rule" }, Value { ts: "window_minimize", zig: "window_minimize" }, Value { ts: "window-minimize", zig: "window_minimize" }, Value { ts: "windowMinimize", zig: "window_minimize" }, Value { ts: "window_maximize", zig: "window_maximize" }, Value { ts: "window-maximize", zig: "window_maximize" }, Value { ts: "windowMaximize", zig: "window_maximize" }, Value { ts: "window_restore", zig: "window_restore" }, Value { ts: "window-restore", zig: "window_restore" }, Value { ts: "windowRestore", zig: "window_restore" }, Value { ts: "window_close", zig: "window_close" }, Value { ts: "window-close", zig: "window_close" }, Value { ts: "windowClose", zig: "window_close" }, Value { ts: "info", zig: "info" }, Value { ts: "alert", zig: "alert" }, Value { ts: "lightbulb", zig: "lightbulb" }, Value { ts: "clipboard", zig: "clipboard" }, Value { ts: "monitor", zig: "monitor" }, Value { ts: "wifi", zig: "wifi" }, Value { ts: "wifi_off", zig: "wifi_off" }, Value { ts: "wifi-off", zig: "wifi_off" }, Value { ts: "wifiOff", zig: "wifi_off" }, Value { ts: "clock", zig: "clock" }, Value { ts: "power", zig: "power" }], ..Default::default() },
                    Prop { name: "extra", field: "extra", kind: Kind::String, ..Default::default() },
                    Prop { name: "disabled", field: "disabled", kind: Kind::Boolean, ..Default::default() },
                    Prop { name: "danger", field: "danger", kind: Kind::Boolean, ..Default::default() },
                    Prop { name: "selected", field: "selected", kind: Kind::Boolean, ..Default::default() },
                    Prop { name: "indent", field: "indent", kind: Kind::Integer, int_type: "u8", ..Default::default() },
                    Prop { name: "onClick", field: "on_click", kind: Kind::Action, ..Default::default() },
                    Prop { name: "indexAction", field: "index_action", kind: Kind::IndexAction, ..Default::default() },
                    Prop { name: "index", field: "index", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                    Prop { name: "content", field: "content", kind: Kind::OptionalNode, ..Default::default() },
                ], ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "maxHeight", field: "max_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "fontSize", field: "font_size", kind: Kind::Number, ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, ..Default::default() },
                Prop { name: "textColor", field: "text_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "extraColor", field: "extra_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "borderColor", field: "border_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "motion", field: "motion", kind: Kind::Enumeration, values: vec![Value { ts: "none", zig: "none" }, Value { ts: "fade", zig: "fade" }, Value { ts: "zoom_big", zig: "zoom_big" }, Value { ts: "zoom-big", zig: "zoom_big" }, Value { ts: "zoomBig", zig: "zoom_big" }, Value { ts: "zoom_big_fast", zig: "zoom_big_fast" }, Value { ts: "zoom-big-fast", zig: "zoom_big_fast" }, Value { ts: "zoomBigFast", zig: "zoom_big_fast" }, Value { ts: "slide_up", zig: "slide_up" }, Value { ts: "slide-up", zig: "slide_up" }, Value { ts: "slideUp", zig: "slide_up" }, Value { ts: "slide_down", zig: "slide_down" }, Value { ts: "slide-down", zig: "slide_down" }, Value { ts: "slideDown", zig: "slide_down" }, Value { ts: "move_up", zig: "move_up" }, Value { ts: "move-up", zig: "move_up" }, Value { ts: "moveUp", zig: "move_up" }], ..Default::default() },
                Prop { name: "motionOrigin", field: "motion_origin", kind: Kind::Enumeration, values: vec![Value { ts: "top_left", zig: "top_left" }, Value { ts: "top-left", zig: "top_left" }, Value { ts: "topLeft", zig: "top_left" }, Value { ts: "top", zig: "top" }, Value { ts: "top_right", zig: "top_right" }, Value { ts: "top-right", zig: "top_right" }, Value { ts: "topRight", zig: "top_right" }, Value { ts: "left", zig: "left" }, Value { ts: "center", zig: "center" }, Value { ts: "right", zig: "right" }, Value { ts: "bottom_left", zig: "bottom_left" }, Value { ts: "bottom-left", zig: "bottom_left" }, Value { ts: "bottomLeft", zig: "bottom_left" }, Value { ts: "bottom", zig: "bottom" }, Value { ts: "bottom_right", zig: "bottom_right" }, Value { ts: "bottom-right", zig: "bottom_right" }, Value { ts: "bottomRight", zig: "bottom_right" }], ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Segmented", builder: "ui.segmented", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "options", field: "options", kind: Kind::StructList, required: true, item_type: "ui.SegmentedOption", item_props: vec![
                    Prop { name: "label", field: "label", kind: Kind::String, ..Default::default() },
                    Prop { name: "icon", field: "icon", kind: Kind::Enumeration, values: vec![Value { ts: "none", zig: "none" }, Value { ts: "menu", zig: "menu" }, Value { ts: "list", zig: "list" }, Value { ts: "search", zig: "search" }, Value { ts: "eye", zig: "eye" }, Value { ts: "eye_off", zig: "eye_off" }, Value { ts: "eye-off", zig: "eye_off" }, Value { ts: "eyeOff", zig: "eye_off" }, Value { ts: "more", zig: "more" }, Value { ts: "more_vertical", zig: "more_vertical" }, Value { ts: "more-vertical", zig: "more_vertical" }, Value { ts: "moreVertical", zig: "more_vertical" }, Value { ts: "close", zig: "close" }, Value { ts: "plus", zig: "plus" }, Value { ts: "minus", zig: "minus" }, Value { ts: "check", zig: "check" }, Value { ts: "chevron_down", zig: "chevron_down" }, Value { ts: "chevron-down", zig: "chevron_down" }, Value { ts: "chevronDown", zig: "chevron_down" }, Value { ts: "chevron_up", zig: "chevron_up" }, Value { ts: "chevron-up", zig: "chevron_up" }, Value { ts: "chevronUp", zig: "chevron_up" }, Value { ts: "chevron_left", zig: "chevron_left" }, Value { ts: "chevron-left", zig: "chevron_left" }, Value { ts: "chevronLeft", zig: "chevron_left" }, Value { ts: "chevron_right", zig: "chevron_right" }, Value { ts: "chevron-right", zig: "chevron_right" }, Value { ts: "chevronRight", zig: "chevron_right" }, Value { ts: "arrow_down", zig: "arrow_down" }, Value { ts: "arrow-down", zig: "arrow_down" }, Value { ts: "arrowDown", zig: "arrow_down" }, Value { ts: "arrow_up", zig: "arrow_up" }, Value { ts: "arrow-up", zig: "arrow_up" }, Value { ts: "arrowUp", zig: "arrow_up" }, Value { ts: "file", zig: "file" }, Value { ts: "file_text", zig: "file_text" }, Value { ts: "file-text", zig: "file_text" }, Value { ts: "fileText", zig: "file_text" }, Value { ts: "folder", zig: "folder" }, Value { ts: "save", zig: "save" }, Value { ts: "copy", zig: "copy" }, Value { ts: "download", zig: "download" }, Value { ts: "upload", zig: "upload" }, Value { ts: "link", zig: "link" }, Value { ts: "external_link", zig: "external_link" }, Value { ts: "external-link", zig: "external_link" }, Value { ts: "externalLink", zig: "external_link" }, Value { ts: "pencil", zig: "pencil" }, Value { ts: "trash", zig: "trash" }, Value { ts: "undo", zig: "undo" }, Value { ts: "redo", zig: "redo" }, Value { ts: "refresh", zig: "refresh" }, Value { ts: "settings", zig: "settings" }, Value { ts: "sun", zig: "sun" }, Value { ts: "moon", zig: "moon" }, Value { ts: "columns", zig: "columns" }, Value { ts: "keyboard", zig: "keyboard" }, Value { ts: "book_open", zig: "book_open" }, Value { ts: "book-open", zig: "book_open" }, Value { ts: "bookOpen", zig: "book_open" }, Value { ts: "maximize", zig: "maximize" }, Value { ts: "minimize", zig: "minimize" }, Value { ts: "focus", zig: "focus" }, Value { ts: "type_size", zig: "type_size" }, Value { ts: "type-size", zig: "type_size" }, Value { ts: "typeSize", zig: "type_size" }, Value { ts: "heading", zig: "heading" }, Value { ts: "bold", zig: "bold" }, Value { ts: "italic", zig: "italic" }, Value { ts: "strikethrough", zig: "strikethrough" }, Value { ts: "code", zig: "code" }, Value { ts: "code_block", zig: "code_block" }, Value { ts: "code-block", zig: "code_block" }, Value { ts: "codeBlock", zig: "code_block" }, Value { ts: "quote", zig: "quote" }, Value { ts: "list_bullet", zig: "list_bullet" }, Value { ts: "list-bullet", zig: "list_bullet" }, Value { ts: "listBullet", zig: "list_bullet" }, Value { ts: "list_ordered", zig: "list_ordered" }, Value { ts: "list-ordered", zig: "list_ordered" }, Value { ts: "listOrdered", zig: "list_ordered" }, Value { ts: "list_checks", zig: "list_checks" }, Value { ts: "list-checks", zig: "list_checks" }, Value { ts: "listChecks", zig: "list_checks" }, Value { ts: "table", zig: "table" }, Value { ts: "sigma", zig: "sigma" }, Value { ts: "image", zig: "image" }, Value { ts: "minus_rule", zig: "minus_rule" }, Value { ts: "minus-rule", zig: "minus_rule" }, Value { ts: "minusRule", zig: "minus_rule" }, Value { ts: "window_minimize", zig: "window_minimize" }, Value { ts: "window-minimize", zig: "window_minimize" }, Value { ts: "windowMinimize", zig: "window_minimize" }, Value { ts: "window_maximize", zig: "window_maximize" }, Value { ts: "window-maximize", zig: "window_maximize" }, Value { ts: "windowMaximize", zig: "window_maximize" }, Value { ts: "window_restore", zig: "window_restore" }, Value { ts: "window-restore", zig: "window_restore" }, Value { ts: "windowRestore", zig: "window_restore" }, Value { ts: "window_close", zig: "window_close" }, Value { ts: "window-close", zig: "window_close" }, Value { ts: "windowClose", zig: "window_close" }, Value { ts: "info", zig: "info" }, Value { ts: "alert", zig: "alert" }, Value { ts: "lightbulb", zig: "lightbulb" }, Value { ts: "clipboard", zig: "clipboard" }, Value { ts: "monitor", zig: "monitor" }, Value { ts: "wifi", zig: "wifi" }, Value { ts: "wifi_off", zig: "wifi_off" }, Value { ts: "wifi-off", zig: "wifi_off" }, Value { ts: "wifiOff", zig: "wifi_off" }, Value { ts: "clock", zig: "clock" }, Value { ts: "power", zig: "power" }], ..Default::default() },
                    Prop { name: "tooltip", field: "tooltip", kind: Kind::String, ..Default::default() },
                ], ..Default::default() },
                Prop { name: "selected", field: "selected", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                Prop { name: "onChange", field: "on_change", kind: Kind::IndexAction, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "fontSize", field: "font_size", kind: Kind::Number, ..Default::default() },
                Prop { name: "itemWidth", field: "item_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "block", field: "block", kind: Kind::Boolean, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Modal", builder: "ui.modal", content: Content::Child, content_field: "content", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "title", field: "title", kind: Kind::String, ..Default::default() },
                Prop { name: "content", field: "content", kind: Kind::Node, required: true, ..Default::default() },
                Prop { name: "footer", field: "footer", kind: Kind::OptionalNode, ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "onClose", field: "on_close", kind: Kind::Action, ..Default::default() },
                Prop { name: "closable", field: "closable", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "maskClosable", field: "mask_closable", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "mask", field: "mask", kind: Kind::Color, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "ShortcutList", builder: "ui.shortcutList", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "groups", field: "groups", kind: Kind::StructList, required: true, item_type: "ui.ShortcutGroup", item_props: vec![
                    Prop { name: "title", field: "title", kind: Kind::String, required: true, ..Default::default() },
                    Prop { name: "items", field: "items", kind: Kind::StructList, required: true, item_type: "ui.Shortcut", item_props: vec![
                        Prop { name: "label", field: "label", kind: Kind::String, required: true, ..Default::default() },
                        Prop { name: "keys", field: "keys", kind: Kind::String, required: true, ..Default::default() },
                    ], ..Default::default() },
                ], ..Default::default() },
                Prop { name: "columns", field: "columns", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                Prop { name: "columnWidth", field: "column_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "columnGap", field: "column_gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "rowHeight", field: "row_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "fontSize", field: "font_size", kind: Kind::Number, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Message", builder: "ui.message", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "text", field: "text", kind: Kind::String, required: true, ..Default::default() },
                Prop { name: "kind", field: "kind", kind: Kind::Enumeration, values: vec![Value { ts: "success", zig: "success" }, Value { ts: "info", zig: "info" }, Value { ts: "warning", zig: "warning" }, Value { ts: "err", zig: "err" }], ..Default::default() },
                Prop { name: "top", field: "top", kind: Kind::Number, ..Default::default() },
                Prop { name: "iconColor", field: "icon_color", kind: Kind::Color, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "HotkeyInput", builder: "ui.hotkeyInput", props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "value", field: "value", kind: Kind::Shortcut, ..Default::default() },
                Prop { name: "recording", field: "recording", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "placeholder", field: "placeholder", kind: Kind::String, ..Default::default() },
                Prop { name: "recordingText", field: "recording_text", kind: Kind::String, ..Default::default() },
                Prop { name: "onStart", field: "on_start", kind: Kind::Action, ..Default::default() },
                Prop { name: "onRecord", field: "on_record", kind: Kind::ShortcutAction, ..Default::default() },
                Prop { name: "onCancel", field: "on_cancel", kind: Kind::Action, ..Default::default() },
                Prop { name: "onClear", field: "on_clear", kind: Kind::Action, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "View", builder: "ui.view", content: Content::Children, props: vec![
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "direction", field: "direction", kind: Kind::Enumeration, values: vec![Value { ts: "row", zig: "row" }, Value { ts: "column", zig: "column" }], ..Default::default() },
                Prop { name: "flexDirection", field: "flex_direction", kind: Kind::Enumeration, values: vec![Value { ts: "row", zig: "row" }, Value { ts: "column", zig: "column" }], ..Default::default() },
                Prop { name: "gap", field: "gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "rowGap", field: "row_gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "columnGap", field: "column_gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "wrap", field: "wrap", kind: Kind::Enumeration, values: vec![Value { ts: "nowrap", zig: "nowrap" }, Value { ts: "wrap", zig: "wrap" }, Value { ts: "wrap_reverse", zig: "wrap_reverse" }, Value { ts: "wrap-reverse", zig: "wrap_reverse" }, Value { ts: "wrapReverse", zig: "wrap_reverse" }], ..Default::default() },
                Prop { name: "alignContent", field: "align_content", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "alignSelf", field: "align_self", kind: Kind::Enumeration, values: vec![Value { ts: "auto", zig: "auto" }, Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "order", field: "order", kind: Kind::Integer, int_type: "i32", ..Default::default() },
                Prop { name: "padding", field: "padding", kind: Kind::Edges, sides: Sides::ALL, ..Default::default() },
                Prop { name: "margin", field: "margin", kind: Kind::Edges, sides: Sides::ALL, ..Default::default() },
                Prop { name: "justify", field: "justify", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "space_evenly", zig: "space_evenly" }, Value { ts: "space-evenly", zig: "space_evenly" }, Value { ts: "spaceEvenly", zig: "space_evenly" }], ..Default::default() },
                Prop { name: "justifyContent", field: "justify_content", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "space_between", zig: "space_between" }, Value { ts: "space-between", zig: "space_between" }, Value { ts: "spaceBetween", zig: "space_between" }, Value { ts: "space_around", zig: "space_around" }, Value { ts: "space-around", zig: "space_around" }, Value { ts: "spaceAround", zig: "space_around" }, Value { ts: "space_evenly", zig: "space_evenly" }, Value { ts: "space-evenly", zig: "space_evenly" }, Value { ts: "spaceEvenly", zig: "space_evenly" }], ..Default::default() },
                Prop { name: "alignItems", field: "align_items", kind: Kind::Enumeration, values: vec![Value { ts: "start", zig: "start" }, Value { ts: "center", zig: "center" }, Value { ts: "end", zig: "end" }, Value { ts: "stretch", zig: "stretch" }], ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "widthPercent", field: "width_percent", kind: Kind::Number, ..Default::default() },
                Prop { name: "heightPercent", field: "height_percent", kind: Kind::Number, ..Default::default() },
                Prop { name: "absolute", field: "absolute", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "top", field: "top", kind: Kind::Number, ..Default::default() },
                Prop { name: "right", field: "right", kind: Kind::Number, ..Default::default() },
                Prop { name: "bottom", field: "bottom", kind: Kind::Number, ..Default::default() },
                Prop { name: "left", field: "left", kind: Kind::Number, ..Default::default() },
                Prop { name: "minWidth", field: "min_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "minHeight", field: "min_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "maxWidth", field: "max_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "maxHeight", field: "max_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "grow", field: "grow", kind: Kind::Number, ..Default::default() },
                Prop { name: "shrink", field: "shrink", kind: Kind::Number, ..Default::default() },
                Prop { name: "basis", field: "basis", kind: Kind::Number, ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, ..Default::default() },
                Prop { name: "borderColor", field: "border_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "borderWidth", field: "border_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "radius", field: "radius", kind: Kind::Number, ..Default::default() },
                Prop { name: "clip", field: "clip", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "scroll", field: "scroll", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "blockPointer", field: "block_pointer", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "selectable", field: "selectable", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "unselectable", field: "unselectable", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "hoverBackground", field: "hover_background", kind: Kind::Color, ..Default::default() },
                Prop { name: "hoverBorderColor", field: "hover_border_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "focusScope", field: "focus_scope", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "showOnHover", field: "show_on_hover", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "onResize", field: "on_resize", kind: Kind::Action, ..Default::default() },
                Prop { name: "shadowColor", field: "shadow_color", kind: Kind::Color, ..Default::default() },
                Prop { name: "shadowBlur", field: "shadow_blur", kind: Kind::Number, ..Default::default() },
                Prop { name: "shadowOffsetY", field: "shadow_offset_y", kind: Kind::Number, ..Default::default() },
                Prop { name: "onClick", field: "on_click", kind: Kind::Action, ..Default::default() },
                Prop { name: "indexAction", field: "index_action", kind: Kind::IndexAction, ..Default::default() },
                Prop { name: "index", field: "index", kind: Kind::Integer, int_type: "usize", ..Default::default() },
                Prop { name: "cursor", field: "cursor", kind: Kind::Enumeration, values: vec![Value { ts: "arrow", zig: "arrow" }, Value { ts: "pointing_hand", zig: "pointing_hand" }, Value { ts: "pointing-hand", zig: "pointing_hand" }, Value { ts: "pointingHand", zig: "pointing_hand" }, Value { ts: "ibeam", zig: "ibeam" }, Value { ts: "crosshair", zig: "crosshair" }, Value { ts: "resize_horizontal", zig: "resize_horizontal" }, Value { ts: "resize-horizontal", zig: "resize_horizontal" }, Value { ts: "resizeHorizontal", zig: "resize_horizontal" }, Value { ts: "resize_vertical", zig: "resize_vertical" }, Value { ts: "resize-vertical", zig: "resize_vertical" }, Value { ts: "resizeVertical", zig: "resize_vertical" }, Value { ts: "grab", zig: "grab" }, Value { ts: "grabbing", zig: "grabbing" }, Value { ts: "not_allowed", zig: "not_allowed" }, Value { ts: "not-allowed", zig: "not_allowed" }, Value { ts: "notAllowed", zig: "not_allowed" }], ..Default::default() },
                Prop { name: "children", field: "children", kind: Kind::Children, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "Markdown", builder: "ui.markdown", instance_id: true, props: vec![
                Prop { name: "source", field: "source", kind: Kind::String, required: true, ..Default::default() },
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "streaming", field: "streaming", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "parseIncomplete", field: "parse_incomplete", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "singleDollarMath", field: "single_dollar_math", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "latexDelimiters", field: "latex_delimiters", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "fontSize", field: "font_size", kind: Kind::Number, ..Default::default() },
                Prop { name: "contentWidth", field: "content_width", kind: Kind::Number, ..Default::default() },
                Prop { name: "caret", field: "caret", kind: Kind::Enumeration, values: vec![Value { ts: "none", zig: "none" }, Value { ts: "block", zig: "block" }, Value { ts: "circle", zig: "circle" }], ..Default::default() },
                Prop { name: "lineNumbers", field: "line_numbers", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "codeMaxHeight", field: "code_max_height", kind: Kind::Number, ..Default::default() },
                Prop { name: "codeCopy", field: "code_copy", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "codeDownload", field: "code_download", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "tableCopy", field: "table_copy", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "tableDownload", field: "table_download", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "mermaidCopy", field: "mermaid_copy", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "mermaidDownload", field: "mermaid_download", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "onLink", field: "on_link", kind: Kind::TextAction, ..Default::default() },
                Prop { name: "baseDir", field: "base_dir", kind: Kind::String, ..Default::default() },
                Prop { name: "selectable", field: "selectable", kind: Kind::Boolean, ..Default::default() },
            ], ..Default::default() },
                        Tag { name: "PdfViewer", builder: "ui.pdfViewer", instance_id: true, props: vec![
                Prop { name: "src", field: "src", kind: Kind::String, required: true, ..Default::default() },
                Prop { name: "password", field: "password", kind: Kind::String, ..Default::default() },
                Prop { name: "key", field: "key", kind: Kind::String, ..Default::default() },
                Prop { name: "toolbar", field: "toolbar", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "pageGap", field: "page_gap", kind: Kind::Number, ..Default::default() },
                Prop { name: "padding", field: "padding", kind: Kind::Number, ..Default::default() },
                Prop { name: "background", field: "background", kind: Kind::Color, ..Default::default() },
                Prop { name: "onLink", field: "on_link", kind: Kind::TextAction, ..Default::default() },
                Prop { name: "onChange", field: "on_change", kind: Kind::Action, ..Default::default() },
                Prop { name: "annotationTools", field: "annotation_tools", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "selectionMenu", field: "selection_menu", kind: Kind::Boolean, ..Default::default() },
                Prop { name: "onAnnotation", field: "on_annotation", kind: Kind::Action, ..Default::default() },
                Prop { name: "width", field: "width", kind: Kind::Number, ..Default::default() },
                Prop { name: "height", field: "height", kind: Kind::Number, ..Default::default() },
                Prop { name: "grow", field: "grow", kind: Kind::Number, ..Default::default() },
                Prop { name: "emptyText", field: "empty_text", kind: Kind::String, ..Default::default() },
                Prop { name: "loadingText", field: "loading_text", kind: Kind::String, ..Default::default() },
                Prop { name: "errorText", field: "error_text", kind: Kind::String, ..Default::default() },
            ], ..Default::default() },
        ]
    }

    /// Builders intentionally excluded from the generated table.
    pub(crate) const SKIPPED: &[Skipped] = &[
        Skipped { builder: "ui.portal", reason: "adapter / generic helper, not an options builder" },
        Skipped { builder: "ui.appendChildren", reason: "adapter / generic helper, not an options builder" },
        Skipped { builder: "ui.styled", reason: "adapter / generic helper, not an options builder" },
        Skipped { builder: "ui.pointerArea", reason: "options have unsupported required fields (see omitted props)" },
        Skipped { builder: "ui.textFmt", reason: "adapter / generic helper, not an options builder" },
        Skipped { builder: "ui.withKey", reason: "adapter / generic helper, not an options builder" },
        Skipped { builder: "ui.vector", reason: "options have unsupported required fields (see omitted props)" },
        Skipped { builder: "ui.memo", reason: "adapter / generic helper, not an options builder" },
        Skipped { builder: "ui.windowControls", reason: "runtime-only window controls" },
        Skipped { builder: "ui.list", reason: "List uses *ListState and a compiler special case" },
    ];

    /// Fields of `zant::ui::theme::Theme`, read through `useTheme()`.
    pub(crate) fn theme() -> Vec<Prop> {
        vec![
                        Prop { name: "colorPrimary", field: "color_primary", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorPrimaryHover", field: "color_primary_hover", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorPrimaryActive", field: "color_primary_active", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorPrimaryBg", field: "color_primary_bg", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorText", field: "color_text", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorTextSecondary", field: "color_text_secondary", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorTextDisabled", field: "color_text_disabled", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorTextLight", field: "color_text_light", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorBgBase", field: "color_bg_base", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorBgContainer", field: "color_bg_container", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorBgElevated", field: "color_bg_elevated", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorBgLayout", field: "color_bg_layout", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorFill", field: "color_fill", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorFillSecondary", field: "color_fill_secondary", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorFillTertiary", field: "color_fill_tertiary", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorFillQuaternary", field: "color_fill_quaternary", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorBorder", field: "color_border", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorBorderSecondary", field: "color_border_secondary", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorSplit", field: "color_split", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorSuccess", field: "color_success", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorWarning", field: "color_warning", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorError", field: "color_error", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorSuccessBg", field: "color_success_bg", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorSuccessText", field: "color_success_text", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorWarningBg", field: "color_warning_bg", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorWarningText", field: "color_warning_text", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorErrorBg", field: "color_error_bg", kind: Kind::Color, ..Default::default() },
                        Prop { name: "colorErrorText", field: "color_error_text", kind: Kind::Color, ..Default::default() },
                        Prop { name: "borderRadius", field: "border_radius", kind: Kind::Number, ..Default::default() },
                        Prop { name: "borderRadiusSm", field: "border_radius_sm", kind: Kind::Number, ..Default::default() },
                        Prop { name: "borderRadiusLg", field: "border_radius_lg", kind: Kind::Number, ..Default::default() },
                        Prop { name: "fontSize", field: "font_size", kind: Kind::Number, ..Default::default() },
                        Prop { name: "fontSizeSm", field: "font_size_sm", kind: Kind::Number, ..Default::default() },
                        Prop { name: "fontSizeLg", field: "font_size_lg", kind: Kind::Number, ..Default::default() },
                        Prop { name: "controlHeight", field: "control_height", kind: Kind::Number, ..Default::default() },
                        Prop { name: "controlHeightSm", field: "control_height_sm", kind: Kind::Number, ..Default::default() },
                        Prop { name: "controlHeightLg", field: "control_height_lg", kind: Kind::Number, ..Default::default() },
                        Prop { name: "padding", field: "padding", kind: Kind::Number, ..Default::default() },
                        Prop { name: "paddingSm", field: "padding_sm", kind: Kind::Number, ..Default::default() },
                        Prop { name: "paddingLg", field: "padding_lg", kind: Kind::Number, ..Default::default() },
                        Prop { name: "motionDurationFast", field: "motion_duration_fast", kind: Kind::Integer, int_type: "u32", ..Default::default() },
                        Prop { name: "motionDurationMid", field: "motion_duration_mid", kind: Kind::Integer, int_type: "u32", ..Default::default() },
                        Prop { name: "motionDurationSlow", field: "motion_duration_slow", kind: Kind::Integer, int_type: "u32", ..Default::default() },
                        Prop { name: "controlPaddingHorizontal", field: "control_padding_horizontal", kind: Kind::Number, ..Default::default() },
                        Prop { name: "controlPaddingHorizontalSm", field: "control_padding_horizontal_sm", kind: Kind::Number, ..Default::default() },
                        Prop { name: "controlPaddingHorizontalLg", field: "control_padding_horizontal_lg", kind: Kind::Number, ..Default::default() },
        ]
    }
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
