//! `<List>` (React Native `FlatList`) → `ui::list`, the virtual list: only the
//! rows on screen are built (docs/tsx.md, "长列表"):
//!
//! ```tsx
//! <List data={todos} renderItem={(t, i) => <Row …/>} keyExtractor={(t) => t.id} />
//! ```
//!
//! The component keeps a hidden `ui::ListState` field; the item and id
//! callbacks are closures over `Rc<RefCell<..>>` state, so the list is emitted
//! inline:
//!
//! ```ignore
//! {
//!     let _data = self.todos.clone();
//!     let _rows = _data.clone();
//!     ui::list(&self._list1, ui::ListOptions {
//!         count: _data.len(),
//!         item: std::rc::Rc::new(move |_index: usize| -> ui::Node {
//!             let item: Todo = _rows[_index].clone();
//!             ui::text(item.title.clone(), ui::TextOptions { ..Default::default() })
//!         }),
//!         ..Default::default()
//!     })
//! }
//! ```
//!
//! `renderItem` bodies that read render locals capture them into `_cap_*`
//! locals before the closure (see [`super::captures`]); state reads
//! capture `self.field` the same way. `keyExtractor` supports the item / index
//! parameters and constants (reading render locals there is a diagnostic).

use crate::ast;
use crate::tags::{self, Kind, Prop, Sides, Tag};

use super::events;
use super::expr;
use super::jsx;
use super::jsx_values::Fields;
use super::model;
use super::scope;
use super::stmt;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod capture_names;
mod item_closures;
mod list_props;

use self::capture_names::*;
use self::item_closures::*;
use self::list_props::*;

/// One `<List>` of the component being generated.
#[derive(Clone, Debug)]
pub struct ListInfo {
    /// The element's address: connects [`scan`] and [`emit`].
    pub jsx: usize,
    /// The hidden `ui::ListState` field and the block name (`_list1`).
    pub field: String,
}

fn address(jsx: &ast::Jsx) -> usize {
    std::ptr::from_ref(jsx) as usize
}

fn attr<'j>(jsx: &'j ast::Jsx, name: &str) -> Option<&'j ast::Attr> {
    jsx.attrs
        .iter()
        .find(|attr| attr.name.name == name.as_bytes())
}

fn arrow<'j>(
    g: &mut Generator<'_>,
    jsx: &'j ast::Jsx,
    name: &str,
    required: bool,
) -> Result<Option<&'j ast::Function>> {
    let Some(attr) = attr(jsx, name) else {
        if required {
            return fail(g, jsx.loc, format!("<List> needs `{name}`"));
        }
        return Ok(None);
    };
    let Some(value) = attr.value.as_deref() else {
        return fail(g, attr.name.loc, format!("`{name}` needs a function"));
    };
    match &value.data {
        ast::ExprData::Arrow(function) => Ok(Some(function)),
        _ => fail(
            g,
            value.loc,
            format!("`{name}` must be an arrow function: (item, index) => ..."),
        ),
    }
}

/// Registers the `<List>` for emission (before JSX emission, like handlers).
pub fn scan(g: &mut Generator<'_>, jsx: &ast::Jsx) -> Result<()> {
    for attr in &jsx.attrs {
        if attr.spread {
            return fail(
                g,
                attr.name.loc,
                "<List> takes its props one by one (no spread)",
            );
        }
    }
    if attr(jsx, "data").is_none() {
        return fail(g, jsx.loc, "<List> needs `data`");
    }
    let render_item = arrow(g, jsx, "renderItem", true)?.expect("required renderItem");
    if render_item.params.is_empty() || render_item.params.len() > 2 {
        return fail(
            g,
            render_item.loc,
            "renderItem takes (item) or (item, index)",
        );
    }
    g.counter += 1;
    let n = g.counter;
    let field = scope::unique_method(g, &format!("_list{n}"))?;
    let info = ListInfo {
        jsx: address(jsx),
        field,
    };
    events::scan_body(g, &render_item.body)?;
    if let Some(key_extractor) = arrow(g, jsx, "keyExtractor", false)?
        && key_extractor.params.len() != 1
    {
        return fail(g, key_extractor.loc, "keyExtractor takes (item)");
    }
    for attr in &jsx.attrs {
        let special = attr.name.name == b"renderItem" || attr.name.name == b"keyExtractor";
        if !special && let Some(value) = attr.value.as_deref() {
            events::scan_expr(g, value)?;
        }
    }
    g.lists.push(info);
    Ok(())
}

/// The row builder closure and the declarations it needs.
pub fn emit(g: &mut Generator<'_>, c: &mut scope::Context, jsx: &ast::Jsx) -> Result<Value> {
    let Some(info) = g
        .lists
        .iter()
        .find(|list| list.jsx == address(jsx))
        .cloned()
    else {
        return fail(g, jsx.loc, "internal: <List> was not scanned");
    };
    let Some(data_attr) = attr(jsx, "data") else {
        return fail(g, jsx.loc, "`data` needs an array");
    };
    let Some(data_expr) = data_attr.value.as_deref() else {
        return fail(g, jsx.loc, "`data` needs an array");
    };
    let data = expr::emit(g, c, data_expr, None, false)?;
    let Some(element) = model::element(g, data.ty) else {
        return fail(g, data_expr.loc, "`data` must be an array");
    };
    let render_item = arrow(g, jsx, "renderItem", true)?.expect("required renderItem");
    // rows repeat (keys) and scroll inside the list (no nested scrolling lists)
    g.loop_depth += 1;
    g.scroll_depth += 1;
    let captures = collect_captures(g, c, &render_item.body)?;
    let item = item_closure(g, c, render_item, element, &captures, "_rows")?;
    g.loop_depth -= 1;
    g.scroll_depth -= 1;
    let item_id = match arrow(g, jsx, "keyExtractor", false)? {
        Some(key_extractor) => Some(id_closure(g, key_extractor, element, "_ids", &captures)?),
        None => None,
    };
    let mut capture_decls = String::new();
    for capture in &captures {
        capture_decls.push_str(&format!(
            "let {}: {} = {};\n",
            capture.local,
            capture.ty.rust(g),
            capture.init
        ));
    }
    let tag = list_tag();
    let mut fields = Fields::default();
    for attr in &jsx.attrs {
        let name = attr.name.name.as_slice();
        if name == b"data"
            || name == b"renderItem"
            || name == b"keyExtractor"
            || is_scroll_prop(name)
        {
            continue;
        }
        jsx::attribute(g, c, &tag, &mut fields, attr)?;
    }
    let follow = scrolling(g, c, jsx, &info.field)?;
    fields.put("count", "_data.len()".to_owned());
    fields.put("item", item);
    if let Some(id) = item_id {
        fields.put("item_id", format!("Some({id})"));
    }
    let options = fields.emit("ui::ListOptions");
    let field = scope::state_field(g, info.field.as_bytes())?;
    let mut block = format!("let _data = {};\n", data.code);
    if !fields.has("item_id") {
        block.push_str("let _rows = _data.clone();\n");
    } else {
        block.push_str("let _rows = _data.clone();\nlet _ids = _data.clone();\n");
    }
    block.push_str(&capture_decls);
    block.push_str(&follow);
    // the row closures must not borrow `self`: clone the list state, and the
    // component's local-state slot the actions upgrade, into locals first
    block.push_str(&format!("let _list = self.{field}.clone();\n"));
    if g.current_component.is_some() {
        // rows build child instances under this id (the closure cannot read `self`)
        block.push_str("let _state = self._state.clone();\nlet _parent_id = self._id;\n");
    }
    block.push_str(&format!("ui::list(&_list, {options})\n"));
    Ok(Value::new(Type::Node, format!("{{\n{block}}}")))
}

fn is_scroll_prop(name: &[u8]) -> bool {
    name == b"scrollToIndex" || name == b"scrollAlign" || name == b"layoutKey"
}

/// `scrollToIndex` / `scrollAlign` / `layoutKey` → `self._list1.follow(..);`
fn scrolling(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    jsx: &ast::Jsx,
    field: &str,
) -> Result<String> {
    let index = attr(jsx, "scrollToIndex");
    let key = attr(jsx, "layoutKey");
    let align_attr = attr(jsx, "scrollAlign");
    if index.is_none() && key.is_none() {
        if let Some(align) = align_attr {
            return fail(g, align.name.loc, "scrollAlign goes with scrollToIndex");
        }
        return Ok(String::new());
    }
    let index_code = match index {
        Some(attr) => {
            let Some(value) = attr.value.as_deref() else {
                return fail(g, attr.name.loc, "scrollToIndex needs a number");
            };
            let emitted = expr::emit(g, c, value, Some(Type::F64), false)?;
            let value = types::coerce(g, emitted, Type::F64, value.loc)?;
            format!("tsx::list_index({})", value.code)
        }
        None => "None".to_owned(),
    };
    let key_code = match key {
        Some(attr) => {
            let Some(value) = attr.value.as_deref() else {
                return fail(g, attr.name.loc, "layoutKey needs a number");
            };
            let emitted = expr::emit(g, c, value, Some(Type::F64), false)?;
            let value = types::coerce(g, emitted, Type::F64, value.loc)?;
            format!("tsx::list_key({})", value.code)
        }
        None => "0".to_owned(),
    };
    let mut position = "ui::ListScrollPosition::Start".to_owned();
    if let Some(attr) = align_attr {
        let Some(value) = attr.value.as_deref() else {
            return fail(
                g,
                attr.name.loc,
                "scrollAlign is \"start\", \"center\" or \"end\"",
            );
        };
        let name = match &value.data {
            ast::ExprData::String(text) if text == b"start" => "Start",
            ast::ExprData::String(text) if text == b"center" => "Center",
            ast::ExprData::String(text) if text == b"end" => "End",
            _ => {
                return fail(
                    g,
                    value.loc,
                    "scrollAlign is \"start\", \"center\" or \"end\"",
                );
            }
        };
        position = format!("ui::ListScrollPosition::{name}");
    }
    let field = scope::state_field(g, field.as_bytes())?;
    Ok(format!(
        "self.{field}.borrow_mut().follow({key_code}, {index_code}, {position});\n"
    ))
}
