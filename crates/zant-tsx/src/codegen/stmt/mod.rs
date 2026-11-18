//! Render and handler statements, local mutation, setters, and functional
//! updates.
//!
//! Declarations are emitted as placeholders and expanded by
//! [`scope::finish`], which picks `let` or `let mut`.

use crate::ast;

use super::expr;
use super::loops;
use super::model;
use super::narrowing;
use super::rust_text;
use super::scope;
use super::types::{self, Type};
use super::{Generator, Result, fail};

mod declarations;
mod expressions;
mod returns;

use self::declarations::*;
pub use self::expressions::*;
pub use self::returns::*;

pub fn statements(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    ss: &[Box<ast::Stmt>],
    top: bool,
) -> Result<String> {
    let mut out = String::new();
    let mark = c.narrowed.len();
    for s in ss {
        out.push_str(&statement(g, c, s, top)?);
        if loops::exits(s) {
            break;
        }
        // `if (!entry) return …;` narrows `entry` for the rest of the block
        if let ast::StmtData::If(v) = &s.data
            && v.else_.is_none()
            && leaves(&v.then)
        {
            narrowing::push(g, c, &v.cond, false)?;
        }
    }
    narrowing::restore(c, mark);
    Ok(out)
}

/// The statement always leaves the block (return, break, continue).
fn leaves(s: &ast::Stmt) -> bool {
    let body: Vec<Box<ast::Stmt>> = match &s.data {
        ast::StmtData::Block(ss) => ss.clone(),
        _ => vec![Box::new(s.clone())],
    };
    if returns_all(&body) {
        return true;
    }
    body.last()
        .is_some_and(|last| matches!(last.data, ast::StmtData::Break | ast::StmtData::Continue))
}

pub fn scoped(g: &mut Generator<'_>, c: &mut scope::Context, s: &ast::Stmt) -> Result<String> {
    let depth = c.active.len();
    let inner = match &s.data {
        ast::StmtData::Block(ss) => statements(g, c, ss, false)?,
        _ => statement(g, c, s, false)?,
    };
    c.active.truncate(depth);
    Ok(format!("{{\n{inner}}}"))
}

pub fn statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    s: &ast::Stmt,
    top: bool,
) -> Result<String> {
    match &s.data {
        ast::StmtData::For(_)
        | ast::StmtData::ForOf(_)
        | ast::StmtData::While(_)
        | ast::StmtData::Switch(_)
        | ast::StmtData::Break
        | ast::StmtData::Continue => loops::emit(g, c, s),
        ast::StmtData::Decl(d) => declaration(g, c, s.loc, d, top),
        ast::StmtData::Func(function) => {
            if c.render && top {
                return Ok(String::new());
            }
            if !c.render && super::local_functions::registered(g, function) {
                return super::local_functions::declare(g, c, s.loc, &function.name, function);
            }
            fail(
                g,
                s.loc,
                "nested functions are not supported yet (closures arrive in phase 2)",
            )
        }
        ast::StmtData::Ret(e) => return_statement(g, c, s.loc, e.as_deref()),
        ast::StmtData::Expr(e) => {
            if super::hook(g, e) {
                if let Some(hook) = g.extensions.hooks.hook_statement {
                    return hook(g, c, e, top);
                }
                return fail(g, e.loc, "hook statements need the hook support installed");
            }
            expression_statement(g, c, e)
        }
        ast::StmtData::Block(ss) => block(g, c, ss),
        ast::StmtData::If(v) => if_statement(g, c, v),
        ast::StmtData::Throw(value) => super::exceptions::throw_statement(g, c, s.loc, value),
        ast::StmtData::Try(t) => super::exceptions::try_statement(g, c, s.loc, t),
    }
}

/// Restore the enclosing lexical scope after emitting a nested block.
fn block(g: &mut Generator<'_>, c: &mut scope::Context, ss: &[Box<ast::Stmt>]) -> Result<String> {
    let depth = c.active.len();
    let inner = statements(g, c, ss, false)?;
    c.active.truncate(depth);
    Ok(format!("{{\n{inner}}}\n"))
}

/// Emit boolean branches with a separate scope for each branch.
fn if_statement(g: &mut Generator<'_>, c: &mut scope::Context, v: &ast::If) -> Result<String> {
    let cond = expr::condition(g, c, &v.cond)?;
    let mut mark = narrowing::push(g, c, &v.cond, true)?;
    let then = scoped(g, c, &v.then)?;
    narrowing::restore(c, mark);
    mark = narrowing::push(g, c, &v.cond, false)?;
    let otherwise = match v.else_.as_deref() {
        Some(otherwise) => format!(" else {}", scoped(g, c, otherwise)?),
        None => String::new(),
    };
    narrowing::restore(c, mark);
    Ok(format!("if {} {then}{otherwise}\n", cond.code))
}

/// `let _ = x;`: marks a binding as used; shared by tests.
pub fn discard(binding: &str) -> String {
    format!("let _ = {binding};\n")
}
