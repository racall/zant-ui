//! `return` statements and the check that a body returns on every path.

use super::*;

/// Validate the return value against the surrounding render or handler
/// result type.
pub(super) fn return_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    e: Option<&ast::Expr>,
) -> Result<String> {
    if let Some(x) = e
        && matches!(x.data, ast::ExprData::Arrow(_))
        && c.effect_field.is_some()
    {
        return match g.extensions.hooks.cleanup_return {
            Some(hook) => hook(g, c, x),
            None => fail(g, loc, "effect cleanups need the hook support installed"),
        };
    }
    if let Some(x) = e {
        if let Some(label) = c.return_label.clone() {
            if c.return_values.is_some() {
                let value = expr::emit(g, c, x, None, false)?;
                let index = c
                    .return_values
                    .as_ref()
                    .map(|values| values.len())
                    .unwrap_or(0);
                if let Some(values) = c.return_values.as_mut() {
                    values.push(scope::ReturnValue { value, loc: x.loc });
                }
                // parenthesized: a block value after a label reads like a
                // labeled loop (rustc's break_with_label_and_loop)
                return Ok(format!("break '{label} ({}{index}{});\n", '\u{5}', '\u{6}'));
            }
            let value = expr::emit(g, c, x, Some(c.ret), false)?;
            if let Some(code) = crate::codegen::exceptions::return_code(c, Some(&value.code)) {
                return Ok(code);
            }
            return Ok(format!("break '{label} ({});\n", value.code));
        }
        if c.ret == Type::Void {
            if !matches!(x.data, ast::ExprData::Call(_)) {
                return fail(g, x.loc, "void functions cannot return a value");
            }
            if let ast::ExprData::Call(call) = &x.data
                && let ast::ExprData::Ident(name) = &call.callee.data
                && scope::setter_index(g, name).is_some()
            {
                let done = void_return(c);
                return Ok(format!("{}{done}", expression_statement(g, c, x)?));
            }
            let v = expr::emit(g, c, x, Some(Type::Void), false)?;
            return Ok(format!("{};\n{}", v.code, void_return(c)));
        }
        let v = expr::emit(g, c, x, Some(c.ret), false)?;
        if let Some(code) = crate::codegen::exceptions::return_code(c, Some(&v.code)) {
            return Ok(code);
        }
        return Ok(format!("return {};\n", v.code));
    }
    if c.return_label.is_some() {
        return fail(g, loc, "array callbacks must return a value");
    }
    if c.ret != Type::Void {
        return fail(
            g,
            loc,
            format!("return needs a value of type {}", c.ret.rust(g)),
        );
    }
    Ok(void_return(c))
}

/// `return;` from here (through a `finally`, out of a throwing function).
pub(super) fn void_return(c: &scope::Context) -> String {
    crate::codegen::exceptions::return_code(c, None).unwrap_or_else(|| "return;\n".to_owned())
}

pub fn stmt_returns(s: &ast::Stmt) -> bool {
    match &s.data {
        ast::StmtData::Ret(_) => true,
        ast::StmtData::Switch(v) => loops::returns(v),
        ast::StmtData::For(v) => {
            let unconditional = match v.cond.as_deref() {
                None => true,
                Some(cond) => matches!(cond.data, ast::ExprData::Boolean(true)),
            };
            unconditional && !loops::jumps(&v.body) && stmt_returns(&v.body)
        }
        ast::StmtData::While(v) => {
            let unconditional = matches!(v.cond.data, ast::ExprData::Boolean(true));
            (v.do_first || unconditional) && !loops::jumps(&v.body) && stmt_returns(&v.body)
        }
        ast::StmtData::Block(ss) => returns_all(ss),
        ast::StmtData::If(v) => match v.else_.as_deref() {
            Some(otherwise) => stmt_returns(&v.then) && stmt_returns(otherwise),
            None => false,
        },
        // a throw never falls through (TypeScript's `never`)
        ast::StmtData::Throw(_) => true,
        // every way out returns or throws: the protected block and the
        // catch block do, or the finally block does
        ast::StmtData::Try(t) => try_returns(t),
        _ => false,
    }
}

/// Every way out of the `try` returns or throws: the protected block and
/// the catch block do, or the finally block does.
pub fn try_returns(t: &ast::Try) -> bool {
    t.finally.as_deref().is_some_and(returns_all)
        || (returns_all(&t.body) && t.catch.as_ref().is_none_or(|c| returns_all(&c.body)))
}

pub fn returns_all(ss: &[Box<ast::Stmt>]) -> bool {
    for s in ss {
        if stmt_returns(s) {
            return true;
        }
        if loops::exits(s) {
            return false;
        }
    }
    false
}
