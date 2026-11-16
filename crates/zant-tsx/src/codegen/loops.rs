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

fn switch_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    v: &ast::Switch,
    label: &str,
) -> Result<String> {
    let value = expr::emit(g, c, &v.value, None, false)?;
    let concrete_ty = value.ty.concrete();
    let value = types::coerce(g, value, concrete_ty, v.value.loc)?;
    if !value.ty.numeric()
        && value.ty != Type::String
        && value.ty != Type::Boolean
        && !model::enum_type(g, value.ty)
    {
        return fail(
            g,
            v.value.loc,
            "switch requires a number, string, boolean or string-literal union",
        );
    }
    // every member of a string union handled and no default: exhaustive
    // (TypeScript's control-flow analysis knows nothing falls through)
    let exhaustive = model::enum_type(g, value.ty)
        && v.cases.iter().all(|case| case.value.is_some())
        && model::info(g, value.ty).is_some_and(|info| {
            info.members.iter().all(|member| {
                v.cases.iter().any(|case| {
                    matches!(case.value.as_deref().map(|e| &e.data), Some(ast::ExprData::String(text)) if text == member)
                        || case.value.as_deref().is_some_and(|e| enum_member_text(g, e).as_deref() == Some(member.as_slice()))
                })
            })
        });
    // `switch (s.kind)` / `switch (typeof x)` over a union's members
    let literals: Option<Vec<Vec<u8>>> = v
        .cases
        .iter()
        .map(|case| match case.value.as_deref().map(|e| &e.data) {
            Some(ast::ExprData::String(text)) => Some(text.clone()),
            _ => None,
        })
        .collect();
    let exhaustive = exhaustive
        || match &literals {
            Some(literals) => super::unions::switch_covers(g, c, &v.value, literals)?,
            None => false,
        };
    if exhaustive {
        EXHAUSTIVE.with(|switches| switches.borrow_mut().push(v.value.loc));
    }
    let discriminant = aggregate::temporary(g, c, b"_value")?;
    let default_label = if grouped_default(v) {
        Some(aggregate::temporary(g, c, b"_default")?)
    } else {
        None
    };
    let mut branches = String::new();
    let mut fallback: Option<String> = None;
    let mut start = 0usize;
    for (index, case) in v.cases.iter().enumerate() {
        if !case.body.is_empty() && index + 1 < v.cases.len() && !ends_case(&case.body) {
            return fail(
                g,
                case.loc,
                "add `break`; falling through to the next case is not supported",
            );
        }
        if case.body.is_empty() && index + 1 < v.cases.len() {
            continue;
        }
        let mut conditions: Vec<String> = Vec::new();
        let mut is_default = false;
        for grouped in v.cases.iter().take(index + 1).skip(start) {
            match grouped.value.as_deref() {
                Some(test_expr) => {
                    let mut test_value = expr::emit(g, c, test_expr, Some(value.ty), false)?;
                    // `Str` has several `PartialEq` implementations, so a bare
                    // `"...".into()` pattern would be ambiguous; name the
                    // static variant explicitly.
                    test_value = explicit_string(g, test_value)?;
                    conditions.push(format!("({discriminant} == {})", test_value.code));
                }
                None => is_default = true,
            }
        }
        let depth = c.active.len();
        // a case of `switch (s.kind)` narrows `s` like `s.kind === "…"`
        let mark = match v.cases[start..=index].as_ref() {
            [only]
                if matches!(
                    only.value.as_deref().map(|e| &e.data),
                    Some(ast::ExprData::String(_))
                ) =>
            {
                let test = ast::Expr {
                    loc: only.loc,
                    data: ast::ExprData::Binary(ast::Binary {
                        op: b"===".to_vec(),
                        lhs: v.value.clone(),
                        rhs: only.value.clone().expect("case value"),
                    }),
                };
                Some(super::narrowing::push(g, c, &test, true)?)
            }
            _ => None,
        };
        let body = stmt::statements(g, c, &case.body, false)?;
        if let Some(mark) = mark {
            super::narrowing::restore(c, mark);
        }
        c.active.truncate(depth);
        let end = if default_label.is_some() && !is_default && !ends_case(&case.body) {
            format!("break '{label};\n")
        } else {
            String::new()
        };
        let block = format!("{{\n{body}{end}}}");
        if is_default {
            fallback = Some(block.clone());
        }
        if !conditions.is_empty() {
            let condition = conditions.join(" || ");
            let joined = if default_label.is_some() && is_default {
                let target = default_label.as_deref().unwrap_or("_default");
                format!("{{ break '{target}; }}")
            } else {
                block
            };
            if branches.is_empty() {
                branches.push_str(&format!("if {condition} {joined}"));
            } else {
                branches.push_str(&format!(" else if {condition} {joined}"));
            }
        }
        start = index + 1;
    }
    if let Some(target) = default_label.as_ref() {
        // A grouped default body must appear only once (locals and captures
        // have one lexical binding).
        branches = format!("'{target}: {{\n{branches}\n}}\n");
    }
    if let Some(block) = fallback {
        if !branches.is_empty() && default_label.is_none() {
            branches.push_str(" else ");
        }
        branches.push_str(&block);
    } else if exhaustive && !branches.is_empty() {
        branches.push_str(" else { unreachable!(\"every member is handled\") }");
    }
    // Even an empty switch evaluates its discriminant once.
    let discard = if v.cases.is_empty() || branches.is_empty() || !has_tests(v) {
        format!("let _ = {discriminant};\n")
    } else {
        String::new()
    };
    let prefix = if rust_text::uses_label(&branches, label) {
        format!("'{label}: ")
    } else {
        String::new()
    };
    Ok(format!(
        "{prefix}{{\nlet {discriminant}: {} = {};\n{discard}{branches}\n}}\n",
        value.ty.rust(g),
        value.code
    ))
}

fn grouped_default(v: &ast::Switch) -> bool {
    let mut has_default = false;
    let mut has_value = false;
    for (index, case) in v.cases.iter().enumerate() {
        if case.value.is_none() {
            has_default = true;
        } else {
            has_value = true;
        }
        if case.body.is_empty() && index + 1 < v.cases.len() {
            continue;
        }
        if has_default && has_value {
            return true;
        }
        has_default = false;
        has_value = false;
    }
    false
}

fn has_tests(v: &ast::Switch) -> bool {
    v.cases.iter().any(|case| case.value.is_some())
}

/// Materialize a string literal as `zant::Str::Static` where a generic
/// `PartialEq` / `match` context would leave `.into()` ambiguous.
pub fn explicit_string(
    g: &mut Generator<'_>,
    value: super::types::Value,
) -> Result<super::types::Value> {
    let (Some(text), true) = (value.text.as_ref(), value.ty == Type::String) else {
        return Ok(value);
    };
    let quoted = rust_text::quote_at(g, text, 0)?;
    Ok(types::Value {
        code: format!("zant::Str::Static({quoted})"),
        ..value
    })
}

/// Whether this statement always exits the current statement sequence.
pub fn exits(s: &ast::Stmt) -> bool {
    match &s.data {
        ast::StmtData::Ret(_) | ast::StmtData::Break | ast::StmtData::Continue => true,
        ast::StmtData::Block(ss) => ends_case(ss),
        ast::StmtData::If(v) => match v.else_.as_deref() {
            Some(otherwise) => exits(&v.then) && exits(otherwise),
            None => false,
        },
        _ => stmt::stmt_returns(s),
    }
}

fn ends_case(ss: &[Box<ast::Stmt>]) -> bool {
    ss.iter().any(|s| exits(s))
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

/// A switch returns on every path only with a default (or every member of
/// its string union handled) and a returning body for each group.
pub fn returns(v: &ast::Switch) -> bool {
    let mut has_default = EXHAUSTIVE.with(|switches| switches.borrow().contains(&v.value.loc));
    for (index, case) in v.cases.iter().enumerate() {
        if case.value.is_none() {
            has_default = true;
        }
        if case.body.is_empty() && index + 1 < v.cases.len() {
            continue;
        }
        if !stmt::returns_all(&case.body) || sequence_jumps(&case.body, true) {
            return false;
        }
    }
    has_default
}

/// A possible break/continue escaping the body prevents treating a loop as
/// always returning.
pub fn jumps(s: &ast::Stmt) -> bool {
    escaping_jump(s, true)
}

fn escaping_jump(s: &ast::Stmt, include_break: bool) -> bool {
    match &s.data {
        ast::StmtData::Break => include_break,
        ast::StmtData::Continue => true,
        ast::StmtData::Block(ss) => sequence_jumps(ss, include_break),
        ast::StmtData::If(v) => {
            escaping_jump(&v.then, include_break)
                || v.else_
                    .as_deref()
                    .is_some_and(|otherwise| escaping_jump(otherwise, include_break))
        }
        ast::StmtData::Switch(v) => v.cases.iter().any(|case| sequence_jumps(&case.body, false)),
        ast::StmtData::Try(t) => {
            sequence_jumps(&t.body, include_break)
                || t.catch
                    .as_ref()
                    .is_some_and(|c| sequence_jumps(&c.body, include_break))
                || t.finally
                    .as_deref()
                    .is_some_and(|f| sequence_jumps(f, include_break))
        }
        // Nested loops consume both kinds of jump. Functions have their own
        // boundary.
        _ => false,
    }
}

fn sequence_jumps(ss: &[Box<ast::Stmt>], include_break: bool) -> bool {
    for s in ss {
        if escaping_jump(s, include_break) {
            return true;
        }
        if exits(s) {
            break;
        }
    }
    false
}

/// The text of `E.Member` when `e` reads a string enum member.
fn enum_member_text(g: &Generator<'_>, e: &ast::Expr) -> Option<Vec<u8>> {
    let ast::ExprData::Member(member) = &e.data else {
        return None;
    };
    let ast::ExprData::Ident(name) = &member.object.data else {
        return None;
    };
    let decl = g.enums.iter().find(|decl| decl.name.name == *name)?;
    decl.members
        .iter()
        .find_map(|candidate| match &candidate.value {
            ast::EnumValue::Text(text) if candidate.name.name == member.name.name => {
                Some(text.clone())
            }
            _ => None,
        })
}
