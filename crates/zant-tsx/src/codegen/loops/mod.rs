//! Structured statement control flow. Targets stay lexical and never cross
//! callback boundaries.
//!
//! Rust has labels on `loop`, `for`, `while` and plain blocks, so break and
//! continue map directly. A C-style `for` becomes a `loop` that runs the update
//! expression at the top of every iteration after the first — that way
//! `continue` still executes the update.
//! `switch` remains a labeled if/else chain: case tests are arbitrary
//! expressions with side effects, so a Rust `match` cannot represent the
//! evaluation order.

use crate::ast;

use super::aggregate;
use super::expr;
use super::model;
use super::rust_text;
use super::scope;
use super::stmt;
use super::types::{self, Type};
use super::{Generator, Result, fail};

mod jumps;
mod switch;

pub use self::jumps::*;
pub use self::switch::*;

/// Emit a loop, switch, or jump using the surrounding function's bindings and
/// return target.
pub fn emit(g: &mut Generator<'_>, c: &mut scope::Context, s: &ast::Stmt) -> Result<String> {
    if matches!(s.data, ast::StmtData::Break | ast::StmtData::Continue) {
        let is_break = matches!(s.data, ast::StmtData::Break);
        let label = if is_break {
            c.break_label.clone()
        } else {
            c.continue_label.clone()
        };
        let Some(label) = label else {
            return fail(
                g,
                s.loc,
                if is_break {
                    "break is only allowed inside a loop or switch"
                } else {
                    "continue is only allowed inside a loop"
                },
            );
        };
        // a loop outside a `try` with a `finally`: that block runs first
        if let Some(code) = super::exceptions::jump_code(c, is_break, &label) {
            return Ok(code);
        }
        return Ok(format!(
            "{} '{label};\n",
            if is_break { "break" } else { "continue" }
        ));
    }
    if c.render && c.return_label.is_none() {
        return fail(
            g,
            s.loc,
            "loops run in handlers and functions; use `.map` / `.filter` to render lists",
        );
    }
    super::narrowing::enter_loop(c, s);
    let depth = c.active.len();
    let previous_break = c.break_label.clone();
    let previous_continue = c.continue_label.clone();
    let base: &[u8] = if matches!(s.data, ast::StmtData::Switch(_)) {
        b"_switch"
    } else {
        b"_loop"
    };
    let label = aggregate::temporary(g, c, base)?;
    c.break_label = Some(label.clone());
    if !matches!(s.data, ast::StmtData::Switch(_)) {
        c.continue_label = Some(label.clone());
    }
    c.jump_labels.push(label.clone());
    let result = match &s.data {
        ast::StmtData::Switch(v) => switch_statement(g, c, v, &label),
        ast::StmtData::For(v) => for_statement(g, c, v, &label),
        ast::StmtData::ForOf(v) => for_of(g, c, v, &label),
        ast::StmtData::While(v) => while_statement(g, c, v, &label),
        _ => unreachable!("loop statement"),
    };
    c.jump_labels.pop();
    c.active.truncate(depth);
    c.break_label = previous_break;
    c.continue_label = previous_continue;
    result
}

fn for_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    v: &ast::For,
    label: &str,
) -> Result<String> {
    let init = match v.init.as_deref() {
        Some(init) => stmt::statement(g, c, init, false)?,
        None => String::new(),
    };
    let cond = match v.cond.as_deref() {
        Some(cond) => Some(expr::condition(g, c, cond)?.code),
        None => None,
    };
    let update = match v.update.as_deref() {
        Some(update) => Some(stmt::expression_statement(g, c, update)?),
        None => None,
    };
    let mark = match v.cond.as_deref() {
        Some(condition) => super::narrowing::push(g, c, condition, true)?,
        None => c.narrowed.len(),
    };
    let body = stmt::scoped(g, c, &v.body);
    super::narrowing::restore(c, mark);
    let body = body?;
    let first = if update.is_some() {
        Some(aggregate::temporary(g, c, b"_first")?)
    } else {
        None
    };
    let mut out = String::from("{\n");
    out.push_str(&init);
    if let Some(first) = first.as_ref() {
        out.push_str(&format!("let mut {first} = true;\n"));
    }
    let prefix = if rust_text::uses_label(&body, label) {
        format!("'{label}: ")
    } else {
        String::new()
    };
    out.push_str(&format!("{prefix}loop {{\n"));
    if let (Some(first), Some(update)) = (first.as_ref(), update.as_ref()) {
        out.push_str(&format!(
            "if {first} {{\n{first} = false;\n}} else {{\n{update}}}\n"
        ));
    }
    if let Some(cond) = cond.as_ref() {
        out.push_str(&format!("if !({cond}) {{\nbreak;\n}}\n"));
    }
    out.push_str(&body);
    out.push_str("\n}\n}\n");
    Ok(out)
}

fn for_of(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    v: &ast::ForOf,
    label: &str,
) -> Result<String> {
    let mut iterable = expr::emit(g, c, &v.iterable, None, false)?;
    // a Map iterates its `[key, value]` entries, a Set its values
    match super::dicts::kind(g, iterable.ty) {
        Some(model::InfoKind::Map) => {
            let info = model::info(g, iterable.ty).expect("map");
            let pair = model::tuple(g, vec![info.params[0], info.child])?;
            iterable = types::Value {
                ty: model::array(g, pair)?,
                code: format!(
                    "{}.entries()",
                    rust_text::operand(&iterable, rust_text::Precedence::Suffix, false)
                ),
                ..types::Value::default()
            };
        }
        Some(model::InfoKind::Set) => {
            let info = model::info(g, iterable.ty).expect("set");
            iterable = types::Value {
                ty: model::array(g, info.child)?,
                code: format!(
                    "{}.values()",
                    rust_text::operand(&iterable, rust_text::Precedence::Suffix, false)
                ),
                ..types::Value::default()
            };
        }
        Some(_) => {
            return fail(
                g,
                v.iterable.loc,
                "a record is not iterable; loop over Object.entries(record) / Object.keys(record)",
            );
        }
        None => {}
    }
    if iterable.ty == Type::String {
        // by code point, like JS: `for (const ch of name)`
        iterable = types::Value {
            ty: model::array(g, Type::String)?,
            code: format!(
                "tsx::strings::chars(&{})",
                rust_text::operand(&iterable, rust_text::Precedence::Suffix, false)
            ),
            ..types::Value::default()
        };
    }
    let Some(element) = model::element(g, iterable.ty) else {
        return fail(g, v.iterable.loc, "for...of requires an array or a string");
    };
    let source = aggregate::temporary(g, c, b"_items")?;
    let binding = scope::bind(g, c, &v.name.name, element, !v.is_const)?;
    let body = stmt::scoped(g, c, &v.body)?;
    let prefix = if rust_text::uses_label(&body, label) {
        format!("'{label}: ")
    } else {
        String::new()
    };
    // `let` loop bindings are only mutable when the body assigns them
    // (`scope::finish` decides the same way): an unused `mut` would warn.
    let mutable = if c.bindings[binding].assigned {
        "mut "
    } else {
        ""
    };
    Ok(format!(
        "{{\nlet {source}: {} = {};\n{prefix}for {mutable}{} in {source}.iter().cloned() {{\n{body}\n}}\n}}\n",
        iterable.ty.rust(g),
        iterable.code,
        c.bindings[binding].rust
    ))
}

fn while_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    v: &ast::While,
    label: &str,
) -> Result<String> {
    let cond = expr::condition(g, c, &v.cond)?;
    // `while (m !== null) { m.… }`: the body sees what the condition
    // proves (a do-while body runs before its first check)
    let mark = if v.do_first {
        c.narrowed.len()
    } else {
        super::narrowing::push(g, c, &v.cond, true)?
    };
    let body = stmt::scoped(g, c, &v.body);
    super::narrowing::restore(c, mark);
    let body = body?;
    let prefix = if rust_text::uses_label(&body, label) {
        format!("'{label}: ")
    } else {
        String::new()
    };
    if !v.do_first {
        return Ok(format!("{prefix}while {} {body}\n", cond.code));
    }
    // The first flag is set at the start of the body, so `continue` re-checks
    // the condition.
    let first = aggregate::temporary(g, c, b"_first")?;
    Ok(format!(
        "{{\nlet mut {first} = true;\n{prefix}while {first} || ({}) {{\n{first} = false;\n{body}\n}}\n}}\n",
        cond.code
    ))
}

thread_local! {
    /// Switches (by discriminant location) whose cases cover every member of
    /// a string union: with no `default` they still cannot fall through.
    static EXHAUSTIVE: std::cell::RefCell<Vec<ast::Loc>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Forgets the exhaustive switches of the previous module.
pub fn reset_exhaustive() {
    EXHAUSTIVE.with(|switches| switches.borrow_mut().clear());
}
