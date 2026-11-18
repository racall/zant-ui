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

mod order;
mod props;
#[cfg(test)]
mod tests;

pub(crate) use self::order::*;
pub use self::props::*;

/// The generated table before formatting.
pub struct Table {
    pub tags: Vec<Tag>,
    pub theme: Vec<Prop>,
    /// The TSX spellings of the `Icon` enum (every `icon` prop lists them).
    pub icons: Vec<Value>,
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
        icons,
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
