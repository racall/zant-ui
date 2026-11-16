//! Building the promise chain: identifiers, calls and reaction arrows.

// AST statement lists hold boxed statements (`ast::Body::Block`)
#![allow(clippy::vec_box)]

use crate::ast::{Body, Call, Expr, ExprData, Function, Member, Name, Param, Stmt};

pub(super) fn ident(loc: u32, name: &[u8]) -> Box<Expr> {
    Box::new(Expr {
        loc,
        data: ExprData::Ident(name.to_vec()),
    })
}

pub(super) fn member(object: Box<Expr>, name: &[u8], loc: u32) -> Box<Expr> {
    Box::new(Expr {
        loc,
        data: ExprData::Member(Member {
            object,
            name: Name {
                name: name.to_vec(),
                loc,
            },
            optional: false,
        }),
    })
}

pub(super) fn call(callee: Box<Expr>, args: Vec<Box<Expr>>, loc: u32) -> Box<Expr> {
    Box::new(Expr {
        loc,
        data: ExprData::Call(Call {
            callee,
            type_args: Vec::new(),
            args,
            optional: false,
            is_new: false,
        }),
    })
}

pub(super) fn param(name: Name, ty: Option<crate::ast::TypeRef>) -> Param {
    Param {
        name,
        ty,
        optional: false,
        destructure: Vec::new(),
        destructure_defaults: Vec::new(),
        rest: false,
        default: None,
    }
}

/// `promise.then((param) => { body })`.
pub(super) fn then(
    promise: Box<Expr>,
    param: Option<Param>,
    body: Vec<Box<Stmt>>,
    loc: u32,
) -> Box<Expr> {
    reaction(promise, b"then", param, body, loc)
}

pub(super) fn reaction(
    promise: Box<Expr>,
    method: &[u8],
    param: Option<Param>,
    body: Vec<Box<Stmt>>,
    loc: u32,
) -> Box<Expr> {
    let arrow = Box::new(Expr {
        loc,
        data: ExprData::Arrow(Box::new(Function {
            type_params: Vec::new(),
            loc,
            name: Vec::new(),
            params: param.into_iter().collect(),
            ret: None,
            body: Body::Block(body),
        })),
    });
    call(member(promise, method, loc), vec![arrow], loc)
}
