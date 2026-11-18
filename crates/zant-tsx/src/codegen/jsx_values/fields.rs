//! `Fields`: the option struct fields a tag's props fill in, with their Rust names and parent options.

use super::*;

/// One option field: a plain value, an `Edges` pattern, or a nested options
/// struct.
#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub field: String,
    pub code: Option<String>,
    pub edges: [Option<Edge>; 4],
    pub nested: Option<Box<Fields>>,
}

/// Ordered option fields. Later writes replace earlier ones; edges merge per
/// side in attribute order.
#[derive(Clone, Debug, Default)]
pub struct Fields {
    pub list: Vec<Entry>,
}

impl Fields {
    pub fn new() -> Fields {
        Fields::default()
    }

    /// The nested options literal of a flattened prop family (`box`).
    pub fn group<'f>(&'f mut self, parent: &str) -> &'f mut Fields {
        if parent.is_empty() {
            return self;
        }
        let index = match self.list.iter().position(|entry| entry.field == parent) {
            Some(index) => index,
            None => {
                self.list.push(Entry {
                    field: parent.to_owned(),
                    ..Entry::default()
                });
                self.list.len() - 1
            }
        };
        let entry = &mut self.list[index];
        if entry.nested.is_none() {
            entry.nested = Some(Box::new(Fields::default()));
        }
        entry.nested.as_mut().expect("nested fields")
    }

    /// Required props are checked by their actual field, including aliases.
    pub fn has_prop(&self, prop: &Prop) -> bool {
        if prop.parent.is_empty() {
            return self.has(prop.field);
        }
        for entry in &self.list {
            if entry.field != prop.parent {
                continue;
            }
            return entry
                .nested
                .as_ref()
                .is_some_and(|nested| nested.has(prop.field));
        }
        false
    }

    pub fn entry(&mut self, field: &str) -> &mut Entry {
        let index = match self.list.iter().position(|entry| entry.field == field) {
            Some(index) => index,
            None => {
                self.list.push(Entry {
                    field: field.to_owned(),
                    ..Entry::default()
                });
                self.list.len() - 1
            }
        };
        &mut self.list[index]
    }

    pub fn remove(&mut self, field: &str) {
        if let Some(index) = self.list.iter().position(|entry| entry.field == field) {
            self.list.remove(index);
        }
    }

    pub fn has(&self, field: &str) -> bool {
        self.list.iter().any(|entry| entry.field == field)
    }

    pub fn put(&mut self, field: &str, value: String) {
        self.entry(field).code = Some(value);
    }

    /// Content follows key, with all other option fields retaining their order.
    pub fn content_first(&mut self, field: &str) {
        let Some(content) = self
            .list
            .iter()
            .position(|entry| entry.field == field)
            .map(|index| self.list.remove(index))
        else {
            return;
        };
        let mut position = 0usize;
        if let Some(index) = self.list.iter().position(|entry| entry.field == "key") {
            let key = self.list.remove(index);
            self.list.insert(0, key);
            position = 1;
        }
        self.list.insert(position, content);
    }

    /// `ui::XOptions { field: value, ..Default::default() }`.
    pub fn emit(&self, options: &str) -> String {
        self.emit_with_default(options, true)
    }

    /// `emit`, with or without the trailing `..Default::default()` (item
    /// structs whose every field is required have no `Default`).
    pub fn emit_with_default(&self, options: &str, default: bool) -> String {
        let spread = if default {
            "..Default::default() }"
        } else {
            "}"
        };
        if self.list.is_empty() {
            return format!("{options} {{ {spread}");
        }
        let mut out = format!("{options} {{ ");
        for entry in &self.list {
            let code = match &entry.code {
                Some(code) => code.clone(),
                None => match entry.nested.as_ref() {
                    Some(nested) => match parent_options(&entry.field) {
                        Some(path) => nested.emit(path),
                        None => continue,
                    },
                    None => edges_value(&entry.edges),
                },
            };
            out.push_str(&format!("{}: {code}, ", rust_field(&entry.field)));
        }
        out.push_str(spread);
        out
    }
}

/// Rust spelling of an option field name: Rust keywords need a raw
/// identifier (`ClickableOptions.box` is `r#box`).
pub(super) fn rust_field(name: &str) -> String {
    match name {
        "box" | "type" => format!("r#{name}"),
        _ => name.to_owned(),
    }
}

/// The Rust options struct of a flattened prop family (`ClickableOptions.box`).
pub(super) fn parent_options(parent: &str) -> Option<&'static str> {
    match parent {
        "box" => Some("ui::BoxOptions"),
        _ => None,
    }
}
