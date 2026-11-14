//! `<List>` (React Native `FlatList`) → `ui::list`, the virtual list: only the
//! rows on screen are built (docs/tsx.md, "长列表"). Translation of
//! `tools/tsx/codegen/list.zig` adapted to Rust closures:
//!
//! ```tsx
//! <List data={todos} renderItem={(t, i) => <Row …/>} keyExtractor={(t) => t.id} />
//! ```
//!
//! Zig registered a hidden `ui.ListState` field, a capture struct and item /
//! id methods. Rust has closures and `Rc<RefCell<..>>` state, so the list is
//! emitted inline:
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
//! locals before the closure (the Zig `captures.zig` role); state reads
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

/// One `<List>` of the component being generated (Zig `ListInfo`).
#[derive(Clone, Debug)]
pub struct ListInfo {
    /// The element's address: connects [`scan`] and [`emit`].
    pub jsx: usize,
    /// The hidden `ui::ListState` field and the block name (`_list1`).
    pub field: String,
}

/// `ui::ListOptions` fields settable from TSX (camelCase); `data`,
/// `renderItem` and `keyExtractor` are handled here.
fn option_props() -> Vec<Prop> {
    vec![
        Prop {
            name: "key",
            field: "key",
            kind: Kind::String,
            ..Prop::default()
        },
        Prop {
            name: "estimatedItemHeight",
            field: "estimated_height",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "itemHeight",
            field: "item_height",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "followTail",
            field: "follow_tail",
            kind: Kind::Boolean,
            ..Prop::default()
        },
        Prop {
            name: "overdraw",
            field: "overdraw",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "gap",
            field: "gap",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "padding",
            field: "padding",
            kind: Kind::Edges,
            sides: Sides::ALL,
            ..Prop::default()
        },
        Prop {
            name: "background",
            field: "background",
            kind: Kind::Color,
            ..Prop::default()
        },
        Prop {
            name: "width",
            field: "width",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "height",
            field: "height",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "grow",
            field: "grow",
            kind: Kind::Number,
            ..Prop::default()
        },
        Prop {
            name: "selectable",
            field: "selectable",
            kind: Kind::Boolean,
            ..Prop::default()
        },
        Prop {
            name: "focusable",
            field: "focusable",
            kind: Kind::Boolean,
            ..Prop::default()
        },
        Prop {
            name: "scrollbar",
            field: "scrollbar",
            kind: Kind::Boolean,
            ..Prop::default()
        },
        Prop {
            name: "alignment",
            field: "alignment",
            kind: Kind::Enumeration,
            values: vec![
                tags::Value {
                    ts: "top",
                    zig: "top",
                },
                tags::Value {
                    ts: "bottom",
                    zig: "bottom",
                },
            ],
            ..Prop::default()
        },
    ]
}

fn list_tag() -> Tag {
    Tag {
        name: "List",
        builder: "ui.list",
        instance_id: false,
        content: tags::Content::None,
        content_field: "",
        props: option_props(),
    }
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

/// A render-local or state value captured by a list closure.
struct Capture {
    /// The TSX name, visible inside the closure body.
    name: Vec<u8>,
    /// The Rust local declared before the closure (`_cap_count`).
    local: String,
    ty: Type,
    /// The initialiser (`self.count.clone()`, `label.clone()`, …).
    init: String,
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
    block.push_str(&format!("ui::list(&self.{field}, {options})\n"));
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

/// Bind `renderItem`'s / `keyExtractor`'s params to `rows[index]` and the
/// index, and return the declarations that must open the closure body. The
/// index parameter is allocated first, so a user param called `index` gets
/// renamed instead of shadowing it.
fn bind_item(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    function: &ast::Function,
    element: Type,
    rows: &str,
) -> Result<String> {
    let index_param = scope::bind(g, c, b"_index", Type::Usize, false)?;
    let index_name = c.bindings[index_param].zig.clone();
    let item = &function.params[0];
    if let Some(reference) = item.ty.as_ref()
        && types::type_ref(g, reference, false)? != element
    {
        return fail(g, item.name.loc, "the item type does not match `data`");
    }
    let binding = scope::bind(g, c, &item.name.name, element, false)?;
    let item_name = c.bindings[binding].zig.clone();
    let item_code = if element.copy(g) {
        format!("{rows}[{index_name}]")
    } else {
        format!("{rows}[{index_name}].clone()")
    };
    let mut prologue = format!("let {item_name}: {} = {item_code};\n", element.rust(g));
    if function.params.len() == 2 {
        let index = &function.params[1];
        if let Some(reference) = index.ty.as_ref()
            && types::type_ref(g, reference, false)? != Type::Usize
        {
            return fail(g, index.name.loc, "the index is a usize");
        }
        let binding = scope::bind(g, c, &index.name.name, Type::Usize, false)?;
        let name = c.bindings[binding].zig.clone();
        prologue.push_str(&format!("let {name}: usize = {index_name};\n"));
    }
    Ok(prologue)
}

/// `move |_index: usize| -> ui::Node`: renderItem in render context.
fn item_closure(
    g: &mut Generator<'_>,
    source: &mut scope::Context,
    function: &ast::Function,
    element: Type,
    captures: &[Capture],
    rows: &str,
) -> Result<String> {
    let mut c = scope::Context {
        render: false,
        ret: Type::Node,
        capture_source: Some(source.capture_source()),
        ..scope::Context::default()
    };
    // captured values are declared before the closure; the body reads them by
    // name (the binding's Rust identifier is the `_cap_*` local)
    for capture in captures {
        let binding = scope::bind(g, &mut c, &capture.name, capture.ty, false)?;
        c.bindings[binding].zig = capture.local.clone();
    }
    // inside the closure prop reads go through the `_cap_props` local, not
    // `self.props` (the closure outlives the `&mut self` borrow)
    let component = std::mem::take(&mut g.current_component);
    let prologue = bind_item(g, &mut c, function, element, rows)?;
    let mut body = prologue;
    match &function.body {
        ast::Body::Expr(e) => {
            let value = expr::emit(g, &mut c, e, Some(Type::Node), false)?;
            body.push_str(&format!("return {};\n", value.code));
        }
        ast::Body::Block(statements) => {
            if !stmt::returns_all(statements) {
                return fail(
                    g,
                    function.loc,
                    "renderItem must return an element on every path",
                );
            }
            body.push_str(&stmt::statements(g, &mut c, statements, true)?);
        }
    }
    let finished = scope::finish(g, &c, body)?;
    g.current_component = component;
    Ok(format!(
        "std::rc::Rc::new(move |_index: usize| -> ui::Node {{\n{finished}}})"
    ))
}

/// `move |_index: usize| -> u64` from keyExtractor: integer keys as they are,
/// floats truncated, strings hashed (Zig `std.hash.Wyhash.hash`).
fn id_closure(
    g: &mut Generator<'_>,
    function: &ast::Function,
    element: Type,
    rows: &str,
    captures: &[Capture],
) -> Result<String> {
    let mut c = scope::Context {
        render: false,
        ret: Type::U64,
        ..scope::Context::default()
    };
    for capture in captures {
        let binding = scope::bind(g, &mut c, &capture.name, capture.ty, false)?;
        c.bindings[binding].zig = capture.local.clone();
    }
    let component = std::mem::take(&mut g.current_component);
    let prologue = bind_item(g, &mut c, function, element, rows)?;
    let e = match &function.body {
        ast::Body::Expr(e) => e,
        ast::Body::Block(_) => {
            return fail(
                g,
                function.loc,
                "keyExtractor must be a single expression: (item) => item.id",
            );
        }
    };
    let emitted = expr::emit(g, &mut c, e, None, false)?;
    let value = types::concrete(g, emitted)?;
    let id = if value.ty == Type::String {
        format!("zant::core::hash::hash(0, ({}).as_str())", value.code)
    } else if value.ty.integer() || value.ty.float() {
        format!("({}) as u64", value.code)
    } else {
        return fail(g, e.loc, "keyExtractor must return a string or a number");
    };
    let finished = scope::finish(g, &c, format!("{prologue}return {id};\n"))?;
    g.current_component = component;
    Ok(format!(
        "std::rc::Rc::new(move |_index: usize| -> u64 {{\n{finished}}})"
    ))
}

/// Render-local and state names read by a renderItem body, in first-use order.
fn collect_captures(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    body: &ast::Body,
) -> Result<Vec<Capture>> {
    let mut names: Vec<Vec<u8>> = Vec::new();
    collect_body_names(body, &mut names);
    let mut captures: Vec<Capture> = Vec::new();
    for name in names {
        if captures.iter().any(|capture| capture.name == name) {
            continue;
        }
        if let Some(binding) = scope::local(c, &name) {
            let ty = binding.ty;
            let init = if ty.copy(g) {
                binding.zig.clone()
            } else {
                format!("{}.clone()", binding.zig)
            };
            let local = format!("_cap_{}", String::from_utf8_lossy(&name));
            captures.push(Capture {
                name,
                local,
                ty,
                init,
            });
            continue;
        }
        if let Some(component) = g.current_component.map(|index| g.components[index].clone())
            && component.destructure.is_empty()
            && !component.param.is_empty()
            && component.param.as_bytes() == name.as_slice()
        {
            // Staged port fix (M7.1): a renderItem body reading `props.x`
            // (a non-destructured props parameter) must capture the props
            // struct before the closure, like render locals and state.
            let Some(props) = component.props else {
                continue;
            };
            let init = if props.copy(g) {
                "self.props".to_owned()
            } else {
                "self.props.clone()".to_owned()
            };
            let local = format!("_cap_{}", String::from_utf8_lossy(&name));
            captures.push(Capture {
                name,
                local,
                ty: props,
                init,
            });
            continue;
        }
        if let Some(index) = scope::state_index(g, &name) {
            let state = g.states[index].clone();
            let field = scope::state_field(g, state.name.as_bytes())?;
            let init = if state.ty.copy(g) {
                format!("self.{field}")
            } else {
                format!("self.{field}.clone()")
            };
            let local = format!("_cap_{}", String::from_utf8_lossy(&name));
            captures.push(Capture {
                name,
                local,
                ty: state.ty,
                init,
            });
        }
    }
    Ok(captures)
}

fn collect_body_names(body: &ast::Body, out: &mut Vec<Vec<u8>>) {
    match body {
        ast::Body::Expr(e) => collect_expr_names(e, out),
        ast::Body::Block(statements) => {
            for statement in statements {
                collect_stmt_names(statement, out);
            }
        }
    }
}

fn collect_stmt_names(statement: &ast::Stmt, out: &mut Vec<Vec<u8>>) {
    match &statement.data {
        ast::StmtData::Decl(decl) => {
            if let Some(init) = decl.init.as_deref() {
                collect_expr_names(init, out);
            }
        }
        ast::StmtData::Func(function) => collect_body_names(&function.body, out),
        ast::StmtData::Ret(Some(e)) => collect_expr_names(e, out),
        ast::StmtData::Ret(None) => {}
        ast::StmtData::If(value) => {
            collect_expr_names(&value.cond, out);
            collect_stmt_names(&value.then, out);
            if let Some(otherwise) = value.else_.as_deref() {
                collect_stmt_names(otherwise, out);
            }
        }
        ast::StmtData::For(value) => {
            if let Some(init) = value.init.as_deref() {
                collect_stmt_names(init, out);
            }
            if let Some(cond) = value.cond.as_deref() {
                collect_expr_names(cond, out);
            }
            if let Some(update) = value.update.as_deref() {
                collect_expr_names(update, out);
            }
            collect_stmt_names(&value.body, out);
        }
        ast::StmtData::ForOf(value) => {
            collect_expr_names(&value.iterable, out);
            collect_stmt_names(&value.body, out);
        }
        ast::StmtData::While(value) => {
            collect_expr_names(&value.cond, out);
            collect_stmt_names(&value.body, out);
        }
        ast::StmtData::Switch(value) => {
            collect_expr_names(&value.value, out);
            for case in &value.cases {
                if let Some(test) = case.value.as_deref() {
                    collect_expr_names(test, out);
                }
                for child in &case.body {
                    collect_stmt_names(child, out);
                }
            }
        }
        ast::StmtData::Break | ast::StmtData::Continue => {}
        ast::StmtData::Expr(e) => collect_expr_names(e, out),
        ast::StmtData::Block(statements) => {
            for statement in statements {
                collect_stmt_names(statement, out);
            }
        }
    }
}

fn collect_expr_names(e: &ast::Expr, out: &mut Vec<Vec<u8>>) {
    match &e.data {
        ast::ExprData::Ident(name) => out.push(name.clone()),
        ast::ExprData::Template(template) => {
            for expr in &template.exprs {
                collect_expr_names(expr, out);
            }
        }
        ast::ExprData::Unary(unary) => collect_expr_names(&unary.operand, out),
        ast::ExprData::Binary(binary) => {
            collect_expr_names(&binary.lhs, out);
            collect_expr_names(&binary.rhs, out);
        }
        ast::ExprData::Cond(value) => {
            collect_expr_names(&value.cond, out);
            collect_expr_names(&value.then, out);
            collect_expr_names(&value.else_, out);
        }
        ast::ExprData::Call(call) => {
            collect_expr_names(&call.callee, out);
            for arg in &call.args {
                collect_expr_names(arg, out);
            }
        }
        ast::ExprData::Member(member) => collect_expr_names(&member.object, out),
        ast::ExprData::Arrow(function) => collect_body_names(&function.body, out),
        ast::ExprData::Jsx(jsx) => {
            for attr in &jsx.attrs {
                if let Some(value) = attr.value.as_deref() {
                    collect_expr_names(value, out);
                }
            }
            for child in &jsx.children {
                match child {
                    ast::Child::Element(element) => {
                        for attr in &element.attrs {
                            if let Some(value) = attr.value.as_deref() {
                                collect_expr_names(value, out);
                            }
                        }
                        for child in &element.children {
                            if let ast::Child::Expr(Some(value)) = child {
                                collect_expr_names(value, out);
                            }
                        }
                    }
                    ast::Child::Expr(Some(value)) => collect_expr_names(value, out),
                    ast::Child::Expr(None) | ast::Child::Text(_) => {}
                }
            }
        }
        ast::ExprData::Object(properties) => {
            for property in properties {
                collect_expr_names(&property.value, out);
            }
        }
        ast::ExprData::Array(items) => {
            for item in items {
                collect_expr_names(item, out);
            }
        }
        ast::ExprData::Spread(inner) | ast::ExprData::NonNull(inner) => {
            collect_expr_names(inner, out);
        }
        ast::ExprData::Index(index) => {
            collect_expr_names(&index.object, out);
            collect_expr_names(&index.index, out);
        }
        ast::ExprData::As(cast) => collect_expr_names(&cast.expr, out),
        ast::ExprData::Assign(assign) => {
            collect_expr_names(&assign.target, out);
            collect_expr_names(&assign.value, out);
        }
        ast::ExprData::Update(update) => collect_expr_names(&update.target, out),
        _ => {}
    }
}
