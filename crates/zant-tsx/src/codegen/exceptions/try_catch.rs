//! `try` / `catch` / `finally` statements.

use super::*;

/// The statements of a region, in their own scope.
pub(super) fn region(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    statements: &[Box<ast::Stmt>],
) -> Result<String> {
    let depth = c.active.len();
    let narrowed = c.narrowed.len();
    let out = stmt::statements(g, c, statements, false);
    c.active.truncate(depth);
    narrowing::restore(c, narrowed);
    out
}

pub(super) fn open(g: &mut Generator<'_>, c: &mut scope::Context, finally: bool) -> Result<String> {
    let label = aggregate::temporary(g, c, b"_try")?;
    c.try_frames.push(Frame {
        label: label.clone(),
        finally,
        jump_depth: c.jump_labels.len(),
        jumps: Vec::new(),
    });
    Ok(label)
}

/// The catch block, `e` bound to `thrown` (an expression).
pub(super) fn catch_body(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    catch: &ast::Catch,
    thrown: &str,
) -> Result<String> {
    let depth = c.active.len();
    let mut out = String::new();
    if let Some(name) = &catch.name {
        let ty = thrown_type(g)?;
        let binding = scope::bind(g, c, &name.name, ty, false)?;
        let rust = c.bindings[binding].rust.clone();
        out.push_str(&format!(
            "let {rust}: {THROWN} = {thrown};\nlet _ = &{rust};\n"
        ));
    }
    out.push_str(&region(g, c, &catch.body)?);
    c.active.truncate(depth);
    Ok(out)
}

/// `try { … } catch (e) { … } finally { … }`.
pub fn try_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    t: &ast::Try,
) -> Result<String> {
    let Some(finally) = &t.finally else {
        let Some(catch) = &t.catch else {
            return fail(g, loc, "`try` needs a `catch` or `finally`");
        };
        // try / catch: the region's value is `Result<(), Thrown>`
        let label = open(g, c, false)?;
        let body = region(g, c, &t.body);
        c.try_frames.pop();
        let body = body?;
        let handler = catch_body(g, c, catch, "_thrown")?;
        return Ok(format!(
            "if let Err(_thrown) = '{label}: {{\n{body}Ok::<(), {THROWN}>(())\n}} {{\n{handler}}}\n{}",
            diverges(t)
        ));
    };
    if c.return_values.is_some() {
        return fail(
            g,
            loc,
            "`finally` needs the callback's return type written out (`(x): number => …`)",
        );
    }
    let ret = match c.ret {
        Type::Void => "()".to_owned(),
        ty => ty.rust(g).to_owned(),
    };
    let completion = aggregate::temporary(g, c, b"_completion")?;
    let label = open(g, c, true)?;
    let body = region(g, c, &t.body);
    let mut frame = c.try_frames.pop().expect("try frame");
    let body = body?;
    let mut out = format!(
        "let {completion}: {COMPLETION}<{ret}> = '{label}: {{\n{body}{COMPLETION}::Normal\n}};\n"
    );
    if let Some(catch) = &t.catch {
        // the catch block is a region of its own: its exits also run the
        // finally block; jumps keep their numbers
        let catch_label = aggregate::temporary(g, c, b"_catch")?;
        c.try_frames.push(Frame {
            label: catch_label.clone(),
            finally: true,
            jump_depth: frame.jump_depth,
            jumps: std::mem::take(&mut frame.jumps),
        });
        let handler = catch_body(g, c, catch, "_thrown");
        frame.jumps = c.try_frames.pop().expect("catch frame").jumps;
        let handler = handler?;
        out.push_str(&format!(
            "let {completion}: {COMPLETION}<{ret}> = match {completion} {{\n{COMPLETION}::Throw(_thrown) => '{catch_label}: {{\n{handler}{COMPLETION}::Normal\n}},\n_other => _other,\n}};\n"
        ));
    }
    // the finally block runs where the try statement is
    out.push_str(&format!("{{\n{}}}\n", region(g, c, finally)?));
    // then the completion resumes
    let mut arms = format!("{COMPLETION}::Normal => {{}}\n");
    let returned = match return_code(c, Some("_value")) {
        Some(code) => code,
        None if c.return_label.is_some() => {
            let target = c.return_label.clone().unwrap_or_default();
            format!("break '{target} (_value);\n")
        }
        None if c.ret == Type::Void => "return;\n".to_owned(),
        None => "return _value;\n".to_owned(),
    };
    arms.push_str(&format!(
        "{COMPLETION}::Return(_value) => {{\nlet _ = &_value;\n{returned}}}\n"
    ));
    let escapes = match &t.catch {
        Some(catch) => may_throw(g, &catch.body),
        None => may_throw(g, &t.body),
    };
    let rethrow = if escapes {
        throw_code(g, c, "_thrown", loc)?
    } else {
        "unreachable!(\"nothing throws here\")".to_owned()
    };
    arms.push_str(&format!("{COMPLETION}::Throw(_thrown) => {rethrow},\n"));
    for (id, (is_break, target)) in frame.jumps.iter().enumerate() {
        arms.push_str(&format!(
            "{COMPLETION}::Jump({id}) => {{\n{}}}\n",
            resume_jump(c, *is_break, target)
        ));
    }
    arms.push_str(&format!(
        "{COMPLETION}::Jump(_) => unreachable!(\"jump\"),\n"
    ));
    out.push_str(&format!("match {completion} {{\n{arms}}}\n{}", diverges(t)));
    Ok(out)
}

/// After a `try` that leaves on every path (TypeScript checked it), tell
/// Rust: it cannot see that the region's normal end is never reached.
pub(super) fn diverges(t: &ast::Try) -> &'static str {
    if stmt::try_returns(t) {
        "unreachable!(\"every path of the try statement left\");\n"
    } else {
        ""
    }
}

/// Whether running `statements` may throw: a `throw`, or a call of a
/// throwing function or builtin (conservative: a nested `try` is not
/// subtracted).
pub(super) fn may_throw(g: &Generator<'_>, statements: &[Box<ast::Stmt>]) -> bool {
    use crate::ast_walk::{self, StmtChild};
    fn child(g: &Generator<'_>, item: StmtChild<'_>) -> bool {
        match item {
            StmtChild::Stmt(s) => {
                if matches!(s.data, ast::StmtData::Throw(_)) {
                    return true;
                }
                let mut found = false;
                ast_walk::stmt_children(s, &mut |inner| found = found || child(g, inner));
                found
            }
            StmtChild::Expr(e) => expression(g, e),
        }
    }
    fn expression(g: &Generator<'_>, e: &ast::Expr) -> bool {
        if let ast::ExprData::Call(call) = &e.data {
            if crate::codegen::throws::builtin_throws(call) {
                return true;
            }
            if let ast::ExprData::Ident(name) = &call.callee.data
                && g.functions
                    .iter()
                    .any(|function| function.throws && function.name.as_bytes() == name)
            {
                return true;
            }
        }
        let mut found = false;
        ast_walk::expr_children(e, &mut |inner| found = found || expression(g, inner));
        found
    }
    statements.iter().any(|s| child(g, StmtChild::Stmt(s)))
}
