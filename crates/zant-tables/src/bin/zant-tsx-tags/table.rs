//! Build the reflected tag table.
//!
//! The options structs come from `#[derive(TsxProps)]`
//! (`zant::ui::options::TSX_REGISTRY`), the builder list / theme / enum
//! variants from [`source::Index`], and the tags that are not reflected yet
//! from [`pending`]. The result is emitted by [`crate::emit`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use zant::ui::options::TSX_REGISTRY;
use zant::ui::options::tsx_props::{TsxField, TsxStruct};
use zant_tsx::tags::{Content, Kind, Prop, Sides, Skipped, Tag, Value};

use crate::pending;
use crate::source::{Builder, Index, Resolved, last_type_ident, type_arguments, type_segments};

/// Every `ui.*` builder in the order of the generated table. Reflected tags
/// and [`pending`] tags together fill it.
const TAG_ORDER: &[&str] = &[
    "column",
    "row",
    "clickable",
    "space",
    "text",
    "heading",
    "title",
    "paragraph",
    "button",
    "link",
    "input",
    "checkbox",
    "switch",
    "radio_group",
    "tabs",
    "tooltip",
    "image",
    "select",
    "icon",
    "icon_button",
    "dismiss_layer",
    "rich_text",
    "pdf_preview",
    "divider",
    "tag",
    "card",
    "alert",
    "avatar",
    "progress",
    "dropdown_menu",
    "segmented",
    "modal",
    "shortcut_list",
    "message",
    "hotkey_input",
    "view",
    "markdown",
    "pdf_viewer",
];

/// Builders intentionally excluded from the generated table: adapters,
/// runtime-only builders and the composite / viewer builders that take
/// non-option types.
pub(crate) const SKIPPED: &[Skipped] = &[
    Skipped {
        builder: "ui.portal",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.appendChildren",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.styled",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.pointerArea",
        reason: "options have unsupported required fields (see omitted props)",
    },
    Skipped {
        builder: "ui.textFmt",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.withKey",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.vector",
        reason: "options have unsupported required fields (see omitted props)",
    },
    Skipped {
        builder: "ui.memo",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.windowControls",
        reason: "runtime-only window controls",
    },
    Skipped {
        builder: "ui.list",
        reason: "List uses *ListState and a compiler special case",
    },
];

/// One classified field type.
enum KindData {
    Plain {
        kind: Kind,
        values: Vec<Value>,
        int_type: &'static str,
    },
    /// `Vec<T>` of a reflected struct (`RadioOption`, `TabItem`).
    StructList(&'static TsxStruct),
    /// A non-optional nested options struct, flattened into its parent
    /// (`ClickableOptions.box`).
    NestedOptions(&'static TsxStruct),
}

#[derive(Default)]
struct PropsResult {
    props: Vec<Prop>,
    /// An unsupported required leaf: the tag / array is unusable (it is skipped
    /// with "options have unsupported required fields").
    invalid: bool,
    children: bool,
    child_count: usize,
    child_field: &'static str,
}

/// The generated table before formatting.
pub struct Table {
    pub tags: Vec<Tag>,
    pub theme: Vec<Prop>,
}

/// Locate the `zant` crate: `$ZANT_HOME` (zant-ui root or the crate itself)
/// first, then this checkout (`crates/zant-tsx/../zant`).
pub fn locate_zant() -> PathBuf {
    if let Ok(home) = std::env::var("ZANT_HOME")
        && !home.is_empty()
    {
        let candidates = [
            PathBuf::from(&home),
            Path::new(&home).join("crates/zant"),
            Path::new(&home).join("zant"),
        ];
        for candidate in candidates {
            if candidate.join("src/ui/options.rs").is_file() {
                return candidate;
            }
        }
        fail(format!(
            "ZANT_HOME={home} does not contain the zant crate (no src/ui/options.rs)"
        ));
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../zant")
}

pub fn generate() -> Table {
    let zant_dir = locate_zant();
    let index = Index::load(&zant_dir.join("src"));
    if index.builders.is_empty() || index.theme.is_empty() {
        fail(format!(
            "could not read the zant sources under {}",
            zant_dir.join("src").display()
        ));
    }
    let registry = registry();
    let mut discovered: HashMap<String, Builder> = HashMap::new();
    for builder in &index.builders {
        if skipped()
            .iter()
            .any(|skipped| skipped.builder == builder_name(builder))
        {
            continue;
        }
        discovered.insert(builder.name.clone(), builder.clone());
    }
    let icons = icon_values(&index);
    let pending = pending_tags(&icons);
    let mut tags = Vec::with_capacity(TAG_ORDER.len());
    for name in TAG_ORDER {
        if let Some(tag) = pending.get(*name) {
            if discovered.contains_key(*name) {
                fail(format!(
                    "ui::{name} is now a builder in crates/zant/src/ui/builders/; \
                     delete its entry from crates/zant-tables/src/bin/zant-tsx-tags/pending.rs and regenerate"
                ));
            }
            tags.push(tag.clone());
            continue;
        }
        let Some(builder) = discovered.get(*name) else {
            fail(format!(
                "ui::{name} of the generated tag table is missing from crates/zant/src/ui/builders/"
            ));
        };
        tags.push(build_tag(&index, &registry, builder));
    }
    for name in discovered.keys() {
        if TAG_ORDER.contains(&name.as_str()) {
            continue;
        }
        fail(format!(
            "crates/zant/src/ui/builders/ has a `pub fn {name} -> Node` builder that is \
             neither in TAG_ORDER nor in SKIPPED; port it in the tag table first"
        ));
    }
    Table {
        tags,
        theme: build_theme(&index),
    }
}

/// `TSX_REGISTRY` keyed by type name; the derive also reports the module path.
fn registry() -> HashMap<&'static str, &'static TsxStruct> {
    let mut registry = HashMap::with_capacity(TSX_REGISTRY.len());
    for tsx in TSX_REGISTRY {
        registry.insert(tsx.name, *tsx);
    }
    registry
}

/// The hand-written tags, with every `icon` enumeration filled from the
/// `Icon` enum (as for reflected props), so a new icon reaches them too.
fn pending_tags(icons: &[Value]) -> HashMap<String, Tag> {
    pending::tags()
        .into_iter()
        .map(|mut tag| {
            fill_icons(&mut tag.props, icons);
            (rust_name(tag.builder), tag)
        })
        .collect()
}

fn fill_icons(props: &mut [Prop], icons: &[Value]) {
    for prop in props {
        if prop.name == "icon" && prop.kind == Kind::Enumeration {
            if !prop.values.is_empty() {
                fail(format!(
                    "pending.rs lists the spellings of an `icon` prop by hand; \
                     leave `values` empty, they come from the Icon enum ({})",
                    prop.field
                ));
            }
            prop.values = icons.to_vec();
        }
        fill_icons(&mut prop.item_props, icons);
    }
}

/// The TSX spellings of `zant::core::node::Icon`.
fn icon_values(index: &Index) -> Vec<Value> {
    let module = ["crate".to_owned(), "ui".to_owned(), "options".to_owned()];
    match index.resolve(&module, &["Icon".to_owned()]) {
        Some(Resolved::Enum(variants)) => enum_values(&variants),
        _ => fail("could not resolve the Icon enum from crates/zant/src/ui/options.rs".to_owned()),
    }
}

fn skipped() -> &'static [Skipped] {
    SKIPPED
}

fn build_tag(
    index: &Index,
    registry: &HashMap<&'static str, &'static TsxStruct>,
    builder: &Builder,
) -> Tag {
    let Some(&tsx) = registry.get(builder.options.as_str()) else {
        fail(format!(
            "ui::{} takes `{}`, which does not derive `TsxProps` or is missing from \
             zant::ui::options::TSX_REGISTRY",
            builder.name, builder.options
        ));
    };
    let props = build_props(index, registry, tsx, "", tsx);
    let content = if builder.text_arg {
        Content::TextArg
    } else if props.children {
        Content::Children
    } else if props.child_count == 1 {
        Content::Child
    } else {
        Content::None
    };
    let content_field = if content == Content::Child {
        props.child_field
    } else {
        ""
    };
    Tag {
        name: intern(zant_tsx::tags::pascal(&builder.name)),
        builder: intern(builder_name(builder)),
        content,
        content_field,
        props: props.props,
        ..Default::default()
    }
}

/// `ui.radioGroup` / `ui.@"switch"` — the builder spelling stored in the table.
fn builder_name(builder: &Builder) -> String {
    let name = zant_tsx::tags::camel(&builder.name);
    if builder.raw_ident {
        format!("ui.@\"{name}\"")
    } else {
        format!("ui.{name}")
    }
}

/// `ui.dropdownMenu` → `dropdown_menu` (`pending.rs` keys).
fn rust_name(builder: &str) -> String {
    let name = builder.strip_prefix("ui.").unwrap_or(builder);
    let name = name
        .strip_prefix("@\"")
        .and_then(|name| name.strip_suffix('"'))
        .unwrap_or(name);
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

fn build_props(
    index: &Index,
    registry: &HashMap<&'static str, &'static TsxStruct>,
    tsx: &'static TsxStruct,
    parent: &str,
    top: &'static TsxStruct,
) -> PropsResult {
    let module = module_path(tsx.module_path);
    let mut result = PropsResult::default();
    for field in tsx.fields {
        let prop_name = zant_tsx::tags::camel(field.name);
        if !parent.is_empty()
            && top
                .fields
                .iter()
                .any(|top_field| zant_tsx::tags::camel(top_field.name) == prop_name)
        {
            // A flattened name that collides with a top-level prop loses.
            continue;
        }
        let required = field.required;
        match classify(index, registry, &module, field) {
            Some(KindData::Plain {
                kind,
                values,
                int_type,
            }) => {
                if kind == Kind::Children && field.name == "children" {
                    result.children = true;
                }
                if kind == Kind::Node && required {
                    result.child_count += 1;
                    result.child_field = intern(field.name.to_string());
                }
                result.props.push(Prop {
                    name: intern(prop_name),
                    field: intern(field.name.to_string()),
                    kind,
                    values,
                    sides: if kind == Kind::Edges {
                        Sides::ALL
                    } else {
                        Sides::NONE
                    },
                    int_type,
                    required,
                    parent: if parent.is_empty() {
                        ""
                    } else {
                        intern(parent.to_string())
                    },
                    ..Default::default()
                });
            }
            Some(KindData::StructList(item)) => {
                let item_props = build_props(index, registry, item, "", item);
                if item_props.invalid {
                    result.invalid |= required;
                    continue;
                }
                result.props.push(Prop {
                    name: intern(prop_name),
                    field: intern(field.name.to_string()),
                    kind: Kind::StructList,
                    required,
                    item_type: intern(format!("ui.{}", item.name)),
                    item_props: item_props.props,
                    parent: if parent.is_empty() {
                        ""
                    } else {
                        intern(parent.to_string())
                    },
                    ..Default::default()
                });
            }
            Some(KindData::NestedOptions(nested)) if parent.is_empty() => {
                let nested = build_props(index, registry, nested, field.name, tsx);
                result.invalid |= nested.invalid;
                result.children |= nested.children;
                result.child_count += nested.child_count;
                if nested.child_count > 0 {
                    result.child_field = nested.child_field;
                }
                result.props.extend(nested.props);
            }
            _ => result.invalid |= required,
        }
    }
    result
}

fn build_theme(index: &Index) -> Vec<Prop> {
    index
        .theme
        .iter()
        .map(|field| {
            let (kind, int_type) = if field.ty == "u32" && field.name.starts_with("color") {
                (Kind::Color, "")
            } else if field.ty == "u32" {
                (Kind::Integer, "u32")
            } else if field.ty == "f32" {
                (Kind::Number, "")
            } else {
                fail(format!(
                    "theme field {} has an unsupported type {}",
                    field.name, field.ty
                ));
            };
            Prop {
                name: intern(zant_tsx::tags::camel(&field.name)),
                field: intern(field.name.clone()),
                kind,
                int_type,
                ..Default::default()
            }
        })
        .collect()
}

fn classify(
    index: &Index,
    registry: &HashMap<&'static str, &'static TsxStruct>,
    module: &[String],
    field: &TsxField,
) -> Option<KindData> {
    let ty = syn::parse_str::<syn::Type>(field.ty).ok()?;
    classify_type(index, registry, module, &ty, false)
}

fn classify_type(
    index: &Index,
    registry: &HashMap<&'static str, &'static TsxStruct>,
    module: &[String],
    ty: &syn::Type,
    optional: bool,
) -> Option<KindData> {
    let name = last_type_ident(ty)?;
    match name.as_str() {
        "Option" => {
            let inner = type_arguments(ty).first().copied()?;
            classify_type(index, registry, module, inner, true)
        }
        // `Option<Rc<Image>>` is an optional `Image`: look through the `Rc`.
        "Rc" => {
            let inner = type_arguments(ty).first().copied()?;
            classify_type(index, registry, module, inner, optional)
        }
        "u32" => Some(plain(Kind::Color)),
        "f32" => Some(plain(Kind::Number)),
        "bool" => Some(plain(Kind::Boolean)),
        "u8" | "u16" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize" => {
            Some(KindData::Plain {
                kind: Kind::Integer,
                values: Vec::new(),
                int_type: intern(name),
            })
        }
        "Str" => Some(plain(Kind::String)),
        "Node" => Some(plain(if optional {
            Kind::OptionalNode
        } else {
            Kind::Node
        })),
        "Edges" => Some(plain(Kind::Edges)),
        "Action" => Some(plain(Kind::Action)),
        "BoolAction" => Some(plain(Kind::BoolAction)),
        "TextAction" => Some(plain(Kind::TextAction)),
        "IndexAction" => Some(plain(Kind::IndexAction)),
        "Image" => Some(plain(Kind::Image)),
        "Shortcut" => Some(plain(Kind::Shortcut)),
        "HotkeyAction" => Some(plain(Kind::ShortcutAction)),
        "Vec" => classify_vec(index, registry, module, ty),
        _ => {
            // Rust enums used by the options (`flex::Wrap`, `text::Weight`,
            // `Cursor`, `Icon`, ...) and nested options structs: the path is
            // resolved in the source index.
            let segments = type_segments(ty)?;
            match index.resolve(module, &segments)? {
                Resolved::Enum(variants) => Some(KindData::Plain {
                    kind: Kind::Enumeration,
                    values: enum_values(&variants),
                    int_type: "",
                }),
                Resolved::Struct if !optional => registry
                    .get(name.as_str())
                    .copied()
                    .map(KindData::NestedOptions),
                _ => None,
            }
        }
    }
}

fn classify_vec(
    index: &Index,
    registry: &HashMap<&'static str, &'static TsxStruct>,
    module: &[String],
    ty: &syn::Type,
) -> Option<KindData> {
    let item = type_arguments(ty).first().copied()?;
    let item_name = last_type_ident(item)?;
    match item_name.as_str() {
        "Node" => Some(plain(Kind::Children)),
        "Str" => Some(plain(Kind::StringList)),
        _ => {
            let segments = type_segments(item)?;
            match index.resolve(module, &segments)? {
                Resolved::Struct => registry
                    .get(item_name.as_str())
                    .copied()
                    .map(KindData::StructList),
                _ => None,
            }
        }
    }
}

fn plain(kind: Kind) -> KindData {
    KindData::Plain {
        kind,
        values: Vec::new(),
        int_type: "",
    }
}

/// One `tags.Value` per TSX spelling of every enum variant, in declaration
/// order: the Rust `WrapReverse` becomes `wrap_reverse` / `wrap-reverse` /
/// `wrapReverse` (table value `wrap_reverse`).
fn enum_values(variants: &[String]) -> Vec<Value> {
    let mut values = Vec::new();
    for variant in variants {
        let rust = intern(snake_case(variant));
        for spelling in zant_tsx::tags::spellings(rust) {
            values.push(Value {
                ts: intern(spelling),
                rust,
            });
        }
    }
    values
}

/// `WrapReverse` → `wrap_reverse`: the variant spelling stored in the table.
pub fn snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    for (index, char) in name.chars().enumerate() {
        if char.is_ascii_uppercase() {
            if index != 0 {
                result.push('_');
            }
            result.push(char.to_ascii_lowercase());
        } else {
            result.push(char);
        }
    }
    result
}

/// `zant::ui::options` → `["crate", "ui", "options"]`.
fn module_path(module_path: &str) -> Vec<String> {
    let mut segments: Vec<String> = module_path.split("::").map(str::to_string).collect();
    if segments.first().is_some_and(|first| first == "zant") {
        segments.remove(0);
    }
    segments.insert(0, "crate".to_string());
    segments
}

/// `&'static str` for generated data; the generator is a short-lived process.
fn intern(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn fail(message: String) -> ! {
    eprintln!("zant-tsx-tags: {message}");
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
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
}
