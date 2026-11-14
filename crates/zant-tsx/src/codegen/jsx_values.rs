//! Node and literal-list props, recursive item schemas, and required-field
//! checks. Translation of `tools/tsx/codegen/jsx_values.zig`, together with
//! the ordered option-field merging of `jsx_fields.zig` and the `Edges`
//! lowering of `jsx_edges.zig`.
//!
//! Zig emits an anonymous struct literal (`.{ .field = value, }`); Rust needs
//! the concrete options struct, so [`Fields::emit`] spells
//! `ui::XOptions { field: value, ..Default::default() }`. Children become the
//! `nodes!` macro instead of the Zig slice concatenation helpers, and optional
//! Rust fields are wrapped in `Some(..)` by the caller (the tag table does not
//! carry optionality; see `jsx.rs`).

use crate::ast;
use crate::tags::{self, Kind, Prop, Sides, Tag};

use super::expr;
use super::jsx;
use super::jsx_children::ChildList;
use super::model;
use super::scope;
use super::types::{self, Type};
use super::{Generator, Result, fail};

/// `ui::inset` for the four equal sides, or a struct for mixed sides.
pub fn inset(code: &str) -> String {
    format!("ui::inset({code})")
}

/// One side of an `Edges` field.
#[derive(Clone, Debug)]
pub struct Edge {
    pub code: String,
    /// The original source expression, for the "all sides share one value"
    /// test (`ui.inset`). Compared by address, like the Zig pointer.
    pub source: usize,
}

/// One option field: a plain value, an `Edges` pattern, or a nested options
/// struct (Zig `jsx_fields.Fields.Entry`).
#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub field: String,
    pub code: Option<String>,
    pub edges: [Option<Edge>; 4],
    pub nested: Option<Box<Fields>>,
}

/// Ordered option fields (Zig `jsx_fields.Fields`). Later writes replace
/// earlier ones; edges merge per side in attribute order.
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
fn rust_field(name: &str) -> String {
    match name {
        "box" | "type" => format!("r#{name}"),
        _ => name.to_owned(),
    }
}

/// The Rust options struct of a flattened prop family (`ClickableOptions.box`).
fn parent_options(parent: &str) -> Option<&'static str> {
    match parent {
        "box" => Some("ui::BoxOptions"),
        _ => None,
    }
}

/// Use `ui::inset` only when every side shares the same original expression.
fn edges_value(edges: &[Option<Edge>; 4]) -> String {
    let top = edges[0].as_ref().map_or("0.0", |edge| edge.code.as_str());
    let right = edges[1].as_ref().map_or("0.0", |edge| edge.code.as_str());
    let bottom = edges[2].as_ref().map_or("0.0", |edge| edge.code.as_str());
    let left = edges[3].as_ref().map_or("0.0", |edge| edge.code.as_str());
    let all_sides = edges.iter().all(Option::is_some);
    if all_sides {
        let source = edges[0].as_ref().expect("edge").source;
        let same_source = edges
            .iter()
            .all(|edge| edge.as_ref().is_some_and(|edge| edge.source == source));
        if same_source {
            return inset(top);
        }
    }
    format!(
        "zant::core::geometry::Edges {{ top: {top}, right: {right}, bottom: {bottom}, left: {left} }}"
    )
}

/// Object edge keys share the later-wins behavior of padding aliases.
pub fn edges(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
    root: &mut Fields,
) -> Result<()> {
    let fields = root.group(prop.parent);
    if let ast::ExprData::Object(properties) = &e.data {
        for property in properties {
            if property.spread {
                return fail(g, property.value.loc, "object spread is not supported yet");
            }
            let Some(selected) = edge_sides(&property.key.name) else {
                return fail(
                    g,
                    property.key.loc,
                    "unknown edge; valid keys: top, right, bottom, left, horizontal, vertical",
                );
            };
            let intersection = Sides {
                top: selected.top && prop.sides.top,
                right: selected.right && prop.sides.right,
                bottom: selected.bottom && prop.sides.bottom,
                left: selected.left && prop.sides.left,
            };
            sides(g, c, fields, prop.field, intersection, &property.value)?;
        }
        // An empty object still writes an Edges value whose sides default to zero.
        fields.entry(prop.field);
        return Ok(());
    }
    sides(g, c, fields, prop.field, prop.sides, e)
}

/// One source expression applied to every side can later become ui.inset.
fn sides(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    fields: &mut Fields,
    field: &str,
    selected: Sides,
    e: &ast::Expr,
) -> Result<()> {
    let value = expr::emit(g, c, e, Some(Type::F32), false)?.code;
    let source = expression_address(e);
    let entry = fields.entry(field);
    let mask = [selected.top, selected.right, selected.bottom, selected.left];
    for (index, set) in mask.iter().enumerate() {
        if *set {
            entry.edges[index] = Some(Edge {
                code: value.clone(),
                source,
            });
        }
    }
    Ok(())
}

/// Accept the documented side names rather than arbitrary object fields.
fn edge_sides(name: &[u8]) -> Option<Sides> {
    let sides = match name {
        b"top" => Sides {
            top: true,
            ..Sides::NONE
        },
        b"right" => Sides {
            right: true,
            ..Sides::NONE
        },
        b"bottom" => Sides {
            bottom: true,
            ..Sides::NONE
        },
        b"left" => Sides {
            left: true,
            ..Sides::NONE
        },
        b"horizontal" => Sides::HORIZONTAL,
        b"vertical" => Sides::VERTICAL,
        _ => return None,
    };
    Some(sides)
}

/// The address of an AST expression: Zig compares `*const ast.Expr` pointers
/// when deciding whether all edges share one source value.
pub fn expression_address(e: &ast::Expr) -> usize {
    std::ptr::from_ref(e) as usize
}

/// Optional nodes accept null; required nodes always retain a concrete ui.Node.
pub fn node(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    let value = expr::emit(g, c, e, None, false)?;
    let optional = prop.kind == Kind::OptionalNode;
    if value.ty == Type::Node {
        // the option field is `Option<Node>`: wrap the element
        return Ok(if optional {
            format!("Some({})", value.code)
        } else {
            value.code
        });
    }
    if optional && (value.ty == Type::OptionalNode || value.ty == Type::Null) {
        return Ok(types::coerce(g, value, Type::OptionalNode, e.loc)?.code);
    }
    if model::optional_child(g, value.ty).is_some() {
        return fail(
            g,
            e.loc,
            format!(
                "prop `{}` needs a node; unwrap the optional with ?? or !",
                prop.name
            ),
        );
    }
    fail(
        g,
        e.loc,
        format!(
            "prop `{}` requires {}; wrap text in <Text>",
            prop.name,
            if optional {
                "an element or null"
            } else {
                "an element"
            }
        ),
    )
}

/// Arrays remain restricted to prop positions until general arrays are implemented.
/// (The AST stores array items boxed; cloning them keeps handler identities.)
#[allow(clippy::vec_box)]
fn array(g: &mut Generator<'_>, prop: &Prop, e: &ast::Expr) -> Result<Vec<Box<ast::Expr>>> {
    if let ast::ExprData::Array(items) = &e.data {
        return Ok(items.clone());
    }
    fail(
        g,
        e.loc,
        format!("prop `{}` requires an array literal", prop.name),
    )
}

/// Reuse child flattening and optional filtering for an explicit children array.
pub fn children(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    let mut list = ChildList::default();
    if let ast::ExprData::Array(items) = &e.data {
        for item in items {
            if let ast::ExprData::Spread(inner) = &item.data {
                let value = expr::emit(g, c, inner, None, false)?;
                if model::element(g, value.ty).is_none() {
                    return fail(g, item.loc, "spread needs an array");
                }
                list.node(g, c, value, None, item.loc)?;
            } else {
                list.expression(g, c, item, None)?;
            }
        }
    } else {
        let value = expr::emit(g, c, e, None, true)?;
        if model::element(g, value.ty).is_none() {
            return fail(
                g,
                e.loc,
                format!("prop `{}` requires an array of nodes", prop.name),
            );
        }
        list.node(g, c, value, None, e.loc)?;
    }
    let code = children_code(g, &list);
    list.wrap(g, c, &code)
}

/// `nodes![a, b]`: the macro flattens nodes, optional nodes and node lists.
pub fn children_code(_g: &Generator<'_>, list: &ChildList) -> String {
    if list.values.is_empty() {
        return "nodes![]".to_owned();
    }
    let codes: Vec<String> = list.values.iter().map(|value| value.code.clone()).collect();
    format!("nodes![{}]", codes.join(", "))
}

/// A string-list field gives each expression its `zant::Str` result target.
pub fn strings(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    let mut spread = false;
    if let ast::ExprData::Array(items) = &e.data {
        spread = items
            .iter()
            .any(|item| matches!(item.data, ast::ExprData::Spread(_)));
    }
    if !matches!(e.data, ast::ExprData::Array(_)) || spread {
        let expected = model::array(g, Type::String)?;
        let code = expr::emit(g, c, e, Some(expected), false)?.code;
        // `SelectOptions.options` is a `Vec<Str>` (List<Str> → Vec<Str>)
        return Ok(format!("({code}).as_slice().to_vec()"));
    }
    let mut codes: Vec<String> = Vec::new();
    for item in array(g, prop, e)? {
        codes.push(expr::emit(g, c, &item, Some(Type::String), false)?.code);
    }
    if codes.is_empty() {
        return Ok("Vec::new()".to_owned());
    }
    Ok(format!("vec![{}]", codes.join(", ")))
}

/// Typed item arrays also work when empty, without depending on peer typing.
pub fn structs(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
) -> Result<String> {
    let items = array(g, prop, e)?;
    let mut codes: Vec<String> = Vec::new();
    let item_path = type_path(prop.item_type);
    for (index, item) in items.iter().enumerate() {
        let owner = format!("{}[{}]", prop.name, index);
        codes.push(struct_item(g, c, prop, &item_path, item, &owner)?);
    }
    if codes.is_empty() {
        return Ok(format!("Vec::<{item_path}>::new()"));
    }
    Ok(format!("vec![{}]", codes.join(", ")))
}

/// Item keys use the same prop conversions recursively, with index/key diagnostics.
fn struct_item(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    item_path: &str,
    item: &ast::Expr,
    owner: &str,
) -> Result<String> {
    if matches!(item.data, ast::ExprData::Spread(_)) {
        return fail(
            g,
            item.loc,
            format!("{owner}: array spread is not supported yet"),
        );
    }
    let ast::ExprData::Object(properties) = &item.data else {
        return fail(g, item.loc, format!("{owner} must be an object literal"));
    };
    let schema = Tag {
        name: "",
        builder: "",
        instance_id: false,
        content: tags::Content::None,
        content_field: "",
        props: prop.item_props.clone(),
    };
    let mut fields = Fields::default();
    for property in properties {
        if property.spread {
            return fail(
                g,
                property.value.loc,
                format!("{owner}: object spread is not supported yet"),
            );
        }
        let Some(field) = tags::find_prop(&schema, &String::from_utf8_lossy(&property.key.name))
        else {
            return fail(
                g,
                property.key.loc,
                format!(
                    "{owner}: unknown key `{}`; valid keys: {}",
                    String::from_utf8_lossy(&property.key.name),
                    jsx::valid_props(&schema)
                ),
            );
        };
        let result = jsx::property(
            g,
            c,
            prop.item_type,
            field,
            &mut fields,
            Some(property.value.as_ref()),
            property.key.loc,
        );
        if let Err(err) = result {
            if err == super::Error::Tsx {
                let prefix = format!("in {owner}.{}", String::from_utf8_lossy(&property.key.name));
                let _ = super::contextualize(g, &prefix);
            }
            return Err(err);
        }
    }
    required(g, &prop.item_props, &fields, item.loc, owner, "key")?;
    // item structs with only required fields (e.g. `ui::TabItem`) do not
    // implement `Default`
    let defaulted = prop
        .item_props
        .iter()
        .any(|field| !field.required && !fields.has_prop(field));
    Ok(fields.emit_with_default(item_path, defaulted))
}

/// `ui.MenuItem` → `ui::MenuItem` (Rust paths use `::`).
pub fn type_path(name: &str) -> String {
    name.replace('.', "::")
}

/// Check emitted field paths so aliases and JSX content satisfy required props too.
pub fn required(
    g: &mut Generator<'_>,
    props: &[Prop],
    fields: &Fields,
    loc: ast::Loc,
    owner: &str,
    noun: &str,
) -> Result<()> {
    for prop in props {
        if prop.required && !fields.has_prop(prop) {
            return fail(
                g,
                loc,
                format!("{owner} is missing required {noun} `{}`", prop.name),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_merge_later_writes_and_keep_order() {
        let mut fields = Fields::default();
        fields.put("gap", "8.0f32".to_owned());
        fields.put("key", "\"a\".into()".to_owned());
        fields.put("gap", "4.0f32".to_owned());
        let emit = fields.emit("ui::BoxOptions");
        assert!(emit.starts_with("ui::BoxOptions { "));
        assert!(emit.contains("gap: 4.0f32, "), "{emit}");
        assert!(emit.contains("key: \"a\".into(), "), "{emit}");
        assert!(emit.ends_with("..Default::default() }"), "{emit}");
        assert!(!emit.contains("8.0f32"));
    }

    #[test]
    fn content_first_follows_key() {
        let mut fields = Fields::default();
        fields.put("disabled", "true".to_owned());
        fields.put("key", "\"b\".into()".to_owned());
        fields.put("text", "\"Save\".into()".to_owned());
        fields.content_first("text");
        let emit = fields.emit("ui::ButtonOptions");
        let key = emit.find("key:").expect("key");
        let text = emit.find("text:").expect("text");
        let disabled = emit.find("disabled:").expect("disabled");
        assert!(key < text && text < disabled, "{emit}");
    }

    #[test]
    fn equal_edges_become_one_inset() {
        let mut fields = Fields::default();
        let entry = fields.entry("padding");
        for side in 0..4 {
            entry.edges[side] = Some(Edge {
                code: "12.0f32".to_owned(),
                source: 7,
            });
        }
        assert!(
            fields
                .emit("ui::BoxOptions")
                .contains("padding: ui::inset(12.0f32)")
        );
    }

    #[test]
    fn mixed_edges_emit_the_geometry_struct() {
        let mut fields = Fields::default();
        let entry = fields.entry("margin");
        entry.edges[0] = Some(Edge {
            code: "1.0f32".to_owned(),
            source: 1,
        });
        entry.edges[1] = Some(Edge {
            code: "2.0f32".to_owned(),
            source: 2,
        });
        let emit = fields.emit("ui::BoxOptions");
        assert!(
            emit.contains(
                "margin: zant::core::geometry::Edges { top: 1.0f32, right: 2.0f32, bottom: 0.0, left: 0.0 }"
            ),
            "{emit}"
        );
    }

    #[test]
    fn nested_fields_group_under_the_parent_field() {
        let mut fields = Fields::default();
        fields.put(
            "on_click",
            "Some(ui::action(|s: &mut C| s._hit()))".to_owned(),
        );
        fields.group("box").put("gap", "6.0f32".to_owned());
        let emit = fields.emit("ui::ClickableOptions");
        assert!(
            emit.contains("box: ui::BoxOptions { gap: 6.0f32, ..Default::default() }"),
            "{emit}"
        );
        assert!(emit.contains("on_click: Some("), "{emit}");
    }
}
