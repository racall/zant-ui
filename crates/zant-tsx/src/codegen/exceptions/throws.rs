//! `throw`, calls that may throw, and `return` / `break` / `continue` leaving a `try` region.

use super::*;

/// Code (diverging, `!`) that throws the `Thrown` expression `value` from
/// here.
pub fn throw_code(
    g: &mut Generator<'_>,
    c: &scope::Context,
    value: &str,
    loc: ast::Loc,
) -> Result<String> {
    if let Some(frame) = c.try_frames.last() {
        return Ok(if frame.finally {
            format!("break '{} {COMPLETION}::Throw({value})", frame.label)
        } else {
            format!("break '{} Err({value})", frame.label)
        });
    }
    match c.throw_mode {
        ThrowMode::Propagate => Ok(format!("return Err({value})")),
        ThrowMode::Report => Ok(format!(
            "{{ zant::tsx::error::uncaught(&({value})); return; }}"
        )),
        ThrowMode::Forbid if c.render => fail(
            g,
            loc,
            "a component cannot throw while it renders; catch the error here with try / catch",
        ),
        ThrowMode::Forbid => fail(
            g,
            loc,
            "a throw cannot leave this function yet; catch it inside with try / catch",
        ),
    }
}

/// `throw value;`
pub fn throw_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    value: &ast::Expr,
) -> Result<String> {
    let emitted = expr::emit(g, c, value, None, false)?;
    let thrown = thrown(g, emitted, value.loc)?;
    Ok(format!("{};\n", throw_code(g, c, &thrown, loc)?))
}

/// The call `code` of a throwing function, as a value: propagated, caught
/// by the enclosing `try`, or reported.
pub fn checked_call(
    g: &mut Generator<'_>,
    c: &scope::Context,
    value: Value,
    loc: ast::Loc,
) -> Result<Value> {
    if c.try_frames.is_empty() && c.throw_mode == ThrowMode::Propagate {
        return Ok(Value {
            code: format!("{}?", rust_text::operand(&value, Precedence::Suffix, false)),
            ..value
        });
    }
    let throw = throw_code(g, c, "_thrown", loc)?;
    Ok(Value {
        code: format!(
            "(match {} {{ Ok(_value) => _value, Err(_thrown) => {throw} }})",
            value.code
        ),
        precedence: Precedence::default(),
        ..value
    })
}

/// `return value` from here: through the enclosing `finally`, if any;
/// `Ok(..)` out of a throwing function. `value` is `None` for a void return.
/// `None` when no rewrite is needed.
pub fn return_code(c: &scope::Context, value: Option<&str>) -> Option<String> {
    if let Some(frame) = c.try_frames.iter().rev().find(|frame| frame.finally) {
        return Some(format!(
            "break '{} {COMPLETION}::Return({});\n",
            frame.label,
            value.unwrap_or("()")
        ));
    }
    (c.throw_mode == ThrowMode::Propagate && c.return_label.is_none())
        .then(|| format!("return Ok({});\n", value.unwrap_or("()")))
}

/// `break` / `continue` to `label`: through the enclosing `finally` when the
/// loop is outside it; `None` for a plain jump.
pub fn jump_code(c: &mut scope::Context, is_break: bool, label: &str) -> Option<String> {
    let position = c.jump_labels.iter().position(|known| known == label)?;
    let frame = c.try_frames.iter_mut().rev().find(|frame| frame.finally)?;
    if position >= frame.jump_depth {
        return None;
    }
    let id = frame.jumps.len();
    frame.jumps.push((is_break, label.to_owned()));
    Some(format!(
        "break '{} {COMPLETION}::Jump({id});\n",
        frame.label
    ))
}

/// The plain form of a jump (after the `finally` ran).
pub(super) fn resume_jump(c: &mut scope::Context, is_break: bool, label: &str) -> String {
    jump_code(c, is_break, label).unwrap_or_else(|| {
        format!(
            "{} '{label};\n",
            if is_break { "break" } else { "continue" }
        )
    })
}
