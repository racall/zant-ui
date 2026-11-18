//! Whether statements exit or jump out of a loop or switch.

use super::*;

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

/// A possible break/continue escaping the body prevents treating a loop as
/// always returning.
pub fn jumps(s: &ast::Stmt) -> bool {
    escaping_jump(s, true)
}

pub(super) fn escaping_jump(s: &ast::Stmt, include_break: bool) -> bool {
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

pub(super) fn sequence_jumps(ss: &[Box<ast::Stmt>], include_break: bool) -> bool {
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
