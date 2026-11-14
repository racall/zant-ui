//! Event arrows capture render locals by value: a handler that reads
//! render locals keeps them in a small `_CaptureN` struct built at render time;
//! the action closure calls the handler method with `&mut self` plus that
//! struct (`ui::action(move |s: &mut App| App::_on_click1(s, &cap))`).
//!
//! A named component function that reads render locals is emitted like an
//! arrow where it is passed, and a call to one from such a handler gets its own
//! copy ([`nested`]).
//!
//! # Limitations
//!
//! `scope::Context::capture_source` keeps only the visible names of the
//! enclosing context, not a reference to the context itself. This module
//! therefore snapshots the enclosing context's typed bindings in
//! [`captures::prepare`] and keeps them in `Generator::capture_scope` while the
//! method is emitted. Consequences:
//!
//! - narrowing facts are not carried into captures (a narrowed optional is
//!   captured with its declared type);
//! - an arrow nested inside another arrow can't re-read the outer arrow's
//!   captures.
//!
//! TODO: lift both by extending `scope::CaptureSource` to carry the enclosing
//! context's types and narrowing facts.

use crate::ast;
use crate::codegen::{self, Method};

use super::rust_text;
use super::scope;
use super::types::Value;
use super::{Generator, Result, ScopeEntry, fail};

/// Discover captures while generating the inline method in its lexical source
/// context.
pub fn prepare(g: &mut Generator<'_>, c: &scope::Context, index: usize) -> Result<()> {
    if g.methods[index].emitted.is_some() {
        return Ok(());
    }
    let previous = g.capture_scope.take();
    let mut scoped: Vec<ScopeEntry> = Vec::new();
    // an outer handler's snapshot first (a handler nested in another handler
    // reads its captures through the enclosing `cap`), then the enclosing
    // scopes of this render-time closure, then this context's own bindings
    if let Some(previous) = &previous {
        scoped.extend(previous.iter().cloned());
    }
    scoped.extend(snapshot(c));
    g.capture_scope = Some(scoped);
    let method = g.methods[index].clone();
    let code = super::generate::method(g, &method, Some(c), Some(index));
    g.capture_scope = previous;
    g.methods[index].emitted = Some(code?);
    Ok(())
}

/// The typed bindings visible in `c`, innermost last.
fn snapshot(c: &scope::Context) -> Vec<ScopeEntry> {
    c.active
        .iter()
        .map(|index| {
            let binding = &c.bindings[*index];
            // capture a narrowed local (`if (peer) … onClick={…}`) with the
            // proven type and its `unwrap()` accessor
            match super::narrowing::find(c, binding.name.as_bytes()) {
                Some(value) => ScopeEntry {
                    name: binding.name.clone(),
                    ty: value.ty,
                    code: value.code,
                },
                None => ScopeEntry {
                    name: binding.name.clone(),
                    ty: binding.ty,
                    code: binding.rust.clone(),
                },
            }
        })
        .collect()
}

pub fn read(g: &mut Generator<'_>, c: &mut scope::Context, n: &str) -> Result<Option<Value>> {
    if c.capture_method.is_none() {
        return Ok(None);
    }
    // a local in the handler shadows the render local
    if scope::local(c, n.as_bytes()).is_some() {
        return Ok(None);
    }
    let index = c.capture_method.expect("capture method");
    let existing = g.methods[index]
        .captures
        .iter()
        .find(|capture| capture.name == n)
        .cloned();
    if let Some(capture) = existing {
        let field = rust_text::id_at(g, capture.field.as_bytes(), 0)?;
        return Ok(Some(super::scope::owned_read(
            g,
            capture.ty,
            format!("cap.{field}"),
        )));
    }
    let Some(entry) = g
        .capture_scope
        .as_ref()
        .and_then(|scope| scope.iter().rev().find(|entry| entry.name == n))
        .cloned()
    else {
        return Ok(None);
    };
    let mut field = n.to_owned();
    if n == "self" {
        field = "_captured_self".to_owned();
    }
    let mut suffix = 2usize;
    while capture_field_taken(&g.methods[index], &field) {
        field = format!("{n}_{suffix}");
        suffix += 1;
    }
    let ty = entry.ty;
    g.methods[index].captures.push(codegen::Capture {
        name: n.to_owned(),
        field: field.clone(),
        ty,
        source: entry.code,
    });
    let field = rust_text::id_at(g, field.as_bytes(), 0)?;
    Ok(Some(super::scope::owned_read(
        g,
        ty,
        format!("cap.{field}"),
    )))
}

/// One capture struct per arrow, shared by all instances (including map rows).
pub fn declarations(g: &mut Generator<'_>) -> Result<String> {
    let mut code = String::new();
    for index in 0..g.methods.len() {
        let method = g.methods[index].clone();
        if method.captures.is_empty() {
            continue;
        }
        let name = method
            .capture_name
            .as_deref()
            .unwrap_or("_Capture")
            .to_owned();
        let name = rust_text::id_at(g, name.as_bytes(), 0)?;
        code.push_str("#[derive(Clone)]\n");
        code.push_str(&format!("struct {name} {{\n"));
        for capture in &method.captures {
            let field = rust_text::id_at(g, capture.field.as_bytes(), 0)?;
            code.push_str(&format!("    {field}: {},\n", capture.ty.rust(g)));
        }
        code.push_str("}\n\n");
    }
    Ok(code)
}

/// The capture struct instance of a handler: `_Capture1 { name: self.name, … }`
/// (it holds no `self` pointer: the closure passes `&mut self` to the method).
pub fn value(g: &mut Generator<'_>, m: &Method) -> Result<String> {
    let name = m.capture_name.as_deref().unwrap_or("_Capture").to_owned();
    let mut fields = String::new();
    for capture in &m.captures {
        let field = rust_text::id_at(g, capture.field.as_bytes(), 0)?;
        // the render value is moved into the capture struct once: clone a
        // non-Copy value (the render still reads it afterwards)
        let source = if capture.ty.copy(g) {
            capture.source.clone()
        } else {
            format!("({}).clone()", capture.source)
        };
        fields.push_str(&format!("    {field}: {source},\n"));
    }
    Ok(format!("{name} {{\n{fields}}}"))
}

fn capture_field_taken(method: &Method, field: &str) -> bool {
    if field == "self" {
        return true;
    }
    method.captures.iter().any(|entry| entry.field == field)
}

/// A call to component function `index` (it reads render locals) from a
/// handler emitted at a render site: a copy of the function capturing from
/// that render, its captured values read through the calling handler.
pub fn nested(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    index: usize,
) -> Result<codegen::NestedCall> {
    if c.capture_source.is_none() {
        return fail(
            g,
            loc,
            format!(
                "`{}` reads render locals; only handlers made while rendering (events, effects) can call it",
                g.methods[index].name
            ),
        );
    }
    let method = g.methods[index].clone();
    g.counter += 1;
    let name = scope::unique_method(g, &format!("_{}{}", method.name, g.counter))?;
    let capture_name = scope::unique_method(g, &format!("_Capture{}", g.counter))?;
    g.methods.push(Method {
        name,
        func: method.func.clone(),
        ret: method.ret,
        capture_name: Some(capture_name),
        ..Method::default()
    });
    let copy = g.methods.len() - 1;
    prepare(g, c, copy)?;
    let inner = g.methods[copy].clone();
    let component_name = g.name.clone();
    let inner_name = inner.name.clone();
    let target = format!(
        "{}::{}",
        rust_text::id_at(g, component_name.as_bytes(), 0)?,
        rust_text::id_at(g, inner_name.as_bytes(), 0)?
    );
    if inner.captures.is_empty() {
        return Ok(codegen::NestedCall {
            receiver: "self".to_owned(),
            target,
        });
    }
    let mut fields = String::new();
    for capture in &inner.captures {
        let Some(value) = read(g, c, &capture.name)? else {
            unreachable!("a local of the same render")
        };
        let field = rust_text::id_at(g, capture.field.as_bytes(), 0)?;
        fields.push_str(&format!("    {field}: {},\n", value.code));
    }
    Ok(codegen::NestedCall {
        receiver: format!(
            "{} {{\n{fields}}}",
            inner.capture_name.as_deref().unwrap_or("_Capture")
        ),
        target,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_fields_are_unique() {
        let method = Method::default();
        assert!(!capture_field_taken(&method, "name"));
        assert!(capture_field_taken(&method, "self"));
    }
}
