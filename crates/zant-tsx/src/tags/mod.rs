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

mod model;
#[cfg(test)]
mod tests;

pub use self::model::*;

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
