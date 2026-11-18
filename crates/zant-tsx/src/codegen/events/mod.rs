//! Event signatures, inferred parameters, generated handlers, and reusable
//! setter adapters.
//!
//! Handler discovery runs before emission (so names and inferred signatures are
//! stable); the JSX emitter then refers to the registered method. The
//! generated value is a closure,
//! `Some(ui::text_action(|s: &mut Counter, v: &str| s._set_name(v)))`, and the
//! method signature lives on the component.
//!
//! Event expressions are identified by AST address: nodes of the parsed module
//! are stable while a component is generated.

use crate::ast;
use crate::tags::{self, Kind};

use super::app_state;
use super::expr;
use super::functions;
use super::model;
use super::rust_text;
use super::scope;
use super::types::{self, Type};
use super::{Generator, Method, Result, fail};

mod actions;
mod arrows;
#[cfg(test)]
mod tests;
mod timers;
mod values;

pub use self::actions::*;
pub use self::arrows::*;
pub use self::timers::*;
pub use self::values::*;

/// A handler expression registered before emission.
///
/// The AST owns its nodes and the spread / JSX shape helpers clone
/// expressions, so a node's address is not stable there; the source location
/// is: a clone keeps it, and one source position holds at most one expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub loc: ast::Loc,
    pub kind: Kind,
    pub method: usize,
}

/// Register event methods before local allocation, so names and inferred
/// signatures are stable.
pub fn scan_expr(g: &mut Generator<'_>, e: &ast::Expr) -> Result<()> {
    match &e.data {
        ast::ExprData::Jsx(element) => scan_jsx(g, element),
        ast::ExprData::Binary(binary) => {
            scan_expr(g, &binary.lhs)?;
            scan_expr(g, &binary.rhs)
        }
        ast::ExprData::Unary(unary) => scan_expr(g, &unary.operand),
        ast::ExprData::NonNull(inner) => scan_expr(g, inner),
        ast::ExprData::As(cast) => scan_expr(g, &cast.expr),
        ast::ExprData::Index(index) => {
            scan_expr(g, &index.object)?;
            scan_expr(g, &index.index)
        }
        ast::ExprData::Member(member) => scan_expr(g, &member.object),
        ast::ExprData::Spread(inner) => scan_expr(g, inner),
        ast::ExprData::Array(items) => {
            for item in items {
                scan_expr(g, item)?;
            }
            Ok(())
        }
        ast::ExprData::Cond(value) => scan_conditional(g, value),
        ast::ExprData::Call(call) => {
            scan_timer(g, call)?;
            super::promises::scan(g, call)?;
            // a chain: `fetch(u).then(a).then(b)` holds the inner calls here
            scan_expr(g, &call.callee)?;
            // arguments (including inline arrows) still register handlers
            for arg in &call.args {
                scan_expr(g, arg)?;
            }
            Ok(())
        }
        ast::ExprData::Template(template) => {
            for expr in &template.exprs {
                scan_expr(g, expr)?;
            }
            Ok(())
        }
        ast::ExprData::Arrow(function) => scan_body(g, &function.body),
        ast::ExprData::Assign(assign) => scan_expr(g, &assign.value),
        ast::ExprData::Object(properties) => {
            for property in properties {
                scan_expr(g, &property.value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The name of a plain call (`setTimeout(…)`) or `None`.
fn call_ident(call: &ast::Call, name: &str) -> bool {
    matches!(&call.callee.data, ast::ExprData::Ident(n) if String::from_utf8_lossy(n) == name)
}

/// Register attribute callbacks before recursively scanning JSX children.
pub fn scan_jsx(g: &mut Generator<'_>, element: &ast::Jsx) -> Result<()> {
    let name = String::from_utf8_lossy(&element.name).into_owned();
    if let Some(component) = super::components::find(g, &name) {
        let component = component.clone();
        super::components::scan(g, element, &component)?;
    }
    if element.name == b"List" {
        return super::list::scan(g, element);
    }
    if let Some(tag) = tags::find(&String::from_utf8_lossy(&element.name)) {
        if super::jsx::spread_any(&element.attrs) {
            let attrs = super::jsx::spread_literals(g, &element.attrs)?;
            for attr in &attrs {
                scan_attribute(g, tag, attr)?;
            }
        } else {
            // Original attribute nodes: handler expressions keep the
            // addresses the emitter will look up.
            for attr in &element.attrs {
                scan_attribute(g, tag, attr)?;
            }
        }
    }
    for child in &element.children {
        match child {
            ast::Child::Element(element) => scan_jsx(g, element)?,
            ast::Child::Expr(Some(value)) => scan_expr(g, value)?,
            ast::Child::Expr(None) | ast::Child::Text(_) => {}
        }
    }
    Ok(())
}

/// One tag attribute for handler discovery: `style` objects recurse into the
/// flattened option fields, action props register their handler.
fn scan_attribute(g: &mut Generator<'_>, tag: &tags::Tag, attr: &ast::Attr) -> Result<()> {
    let Some(value) = attr.value.as_deref() else {
        return Ok(());
    };
    if attr.name.name == b"style"
        && let ast::ExprData::Object(properties) = &value.data
    {
        for property in properties {
            if property.spread {
                continue;
            }
            if let Some(prop) = tags::find_prop(tag, &String::from_utf8_lossy(&property.key.name)) {
                scan_property(g, prop, &property.value, property.key.loc)?;
            } else {
                scan_expr(g, &property.value)?;
            }
        }
        return Ok(());
    }
    if let Some(prop) = tags::find_prop(tag, &String::from_utf8_lossy(&attr.name.name)) {
        scan_property(g, prop, value, attr.name.loc)
    } else {
        scan_expr(g, value)
    }
}

pub fn scan_body(g: &mut Generator<'_>, body: &ast::Body) -> Result<()> {
    match body {
        ast::Body::Expr(e) => scan_expr(g, e),
        ast::Body::Block(ss) => {
            for s in ss {
                scan_stmt(g, s)?;
            }
            Ok(())
        }
    }
}

pub fn scan_stmt(g: &mut Generator<'_>, s: &ast::Stmt) -> Result<()> {
    match &s.data {
        ast::StmtData::For(value) => {
            if let Some(init) = value.init.as_deref() {
                scan_stmt(g, init)?;
            }
            if let Some(cond) = value.cond.as_deref() {
                scan_expr(g, cond)?;
            }
            if let Some(update) = value.update.as_deref() {
                scan_expr(g, update)?;
            }
            scan_stmt(g, &value.body)
        }
        ast::StmtData::ForOf(value) => {
            scan_expr(g, &value.iterable)?;
            scan_stmt(g, &value.body)
        }
        ast::StmtData::While(value) => {
            scan_expr(g, &value.cond)?;
            scan_stmt(g, &value.body)
        }
        ast::StmtData::Switch(value) => {
            scan_expr(g, &value.value)?;
            for case in &value.cases {
                if let Some(test) = case.value.as_deref() {
                    scan_expr(g, test)?;
                }
                for child in &case.body {
                    scan_stmt(g, child)?;
                }
            }
            Ok(())
        }
        ast::StmtData::Break | ast::StmtData::Continue => Ok(()),
        ast::StmtData::Decl(decl) => match decl.init.as_deref() {
            Some(init) => {
                // `const load = async () => { … }` in a handler
                if let (crate::ast::Pattern::Ident(name), ast::ExprData::Arrow(function)) =
                    (&decl.pattern, &init.data)
                    && function.ret.is_none()
                {
                    super::local_functions::scan(g, &name.name, function)?;
                }
                scan_expr(g, init)
            }
            None => Ok(()),
        },
        ast::StmtData::Func(function) => {
            super::local_functions::scan(g, &function.name, function)?;
            scan_body(g, &function.body)
        }
        ast::StmtData::Ret(e) => match e.as_deref() {
            Some(value) => scan_expr(g, value),
            None => Ok(()),
        },
        ast::StmtData::Expr(e) => scan_expr(g, e),
        ast::StmtData::Throw(e) => scan_expr(g, e),
        ast::StmtData::Try(t) => {
            for s in t
                .body
                .iter()
                .chain(t.catch.iter().flat_map(|c| c.body.iter()))
                .chain(t.finally.iter().flatten())
            {
                scan_stmt(g, s)?;
            }
            Ok(())
        }
        ast::StmtData::Block(ss) => {
            for s in ss {
                scan_stmt(g, s)?;
            }
            Ok(())
        }
        ast::StmtData::If(value) => {
            scan_expr(g, &value.cond)?;
            scan_stmt(g, &value.then)?;
            match value.else_.as_deref() {
                Some(otherwise) => scan_stmt(g, otherwise),
                None => Ok(()),
            }
        }
    }
}

fn scan_conditional(g: &mut Generator<'_>, value: &ast::Cond) -> Result<()> {
    scan_expr(g, &value.cond)?;
    scan_expr(g, &value.then)?;
    scan_expr(g, &value.else_)
}

/// Kept so the JSX composer can refer to the same diagnostic wording.
pub fn handler_diagnostic(g: &mut Generator<'_>, loc: ast::Loc) -> Result<()> {
    fail(
        g,
        loc,
        "event handler must be a component function, plain function, setter, or inline arrow",
    )
}
