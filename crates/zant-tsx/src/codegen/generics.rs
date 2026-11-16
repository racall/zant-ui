//! Reject generic syntax before codegen or hook expansion can erase it.
//! Parsing retains this syntax for the later monomorphization pass.

use crate::ast::*;
use crate::error::Diag;

type Result = std::result::Result<(), Diag>;

fn error(loc: Loc, msg: &str) -> Result {
    Err(Diag {
        loc,
        msg: msg.to_owned(),
    })
}

thread_local! {
    /// Generic declarations of the module being validated (`Box` in `Box<T>`).
    static GENERIC: std::cell::RefCell<Vec<Vec<u8>>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) fn validate(module: &Module) -> Result {
    let names: Vec<Vec<u8>> = module
        .types
        .iter()
        .filter(|decl| !decl.type_params.is_empty())
        .map(|decl| decl.name.name.clone())
        .collect();
    GENERIC.with(|generic| *generic.borrow_mut() = names);
    let result = validate_module(module);
    GENERIC.with(|generic| generic.borrow_mut().clear());
    result
}

fn validate_module(module: &Module) -> Result {
    for top in &module.declared {
        if !top.func.type_params.is_empty() {
            return error(top.func.loc, "native functions are not generic");
        }
        function(&top.func)?;
    }
    for decl in &module.types {
        type_ref(&decl.ty)?;
    }
    for top in &module.functions {
        function(&top.func)?;
    }
    for c in &module.consts {
        optional_type(c.ty.as_ref())?;
        expr(&c.value)?;
    }
    Ok(())
}

pub(crate) fn function(f: &Function) -> Result {
    if !f.type_params.is_empty() {
        return error(f.loc, "generic functions are not supported yet");
    }
    params(&f.params)?;
    optional_type(f.ret.as_ref())?;
    match &f.body {
        Body::Block(ss) => statements(ss),
        Body::Expr(e) => expr(e),
    }
}

fn params(ps: &[Param]) -> Result {
    for p in ps {
        optional_type(p.ty.as_ref())?;
        optional_expr(p.default.as_deref())?;
        for e in &p.destructure_defaults {
            optional_expr(e.as_deref())?;
        }
    }
    Ok(())
}

fn optional_type(t: Option<&TypeRef>) -> Result {
    t.map_or(Ok(()), type_ref)
}

fn type_ref(t: &TypeRef) -> Result {
    if !t.args.is_empty()
        && !GENERIC.with(|generic| generic.borrow().contains(&t.name))
        && let Some(diag) = super::types::generic_error(t)
    {
        return Err(diag);
    }
    for arg in &t.args {
        type_ref(arg)?;
    }
    match t.detail.as_deref() {
        Some(TypeDetail::Array(t)) => type_ref(t),
        Some(TypeDetail::Union(ts) | TypeDetail::Intersection(ts)) => {
            for t in ts {
                type_ref(t)?;
            }
            Ok(())
        }
        Some(TypeDetail::Object(fs)) => {
            for f in fs {
                type_ref(&f.ty)?;
            }
            Ok(())
        }
        Some(TypeDetail::Func(f)) => {
            if !f.type_params.is_empty() {
                return error(t.loc, "generic functions are not supported yet");
            }
            params(&f.params)?;
            type_ref(&f.ret)
        }
        _ => Ok(()),
    }
}

fn optional_expr(e: Option<&Expr>) -> Result {
    e.map_or(Ok(()), expr)
}

pub(crate) fn jsx_type_args(j: &Jsx) -> Result {
    if !j.type_args.is_empty() {
        return error(
            j.loc,
            &format!(
                "`{}` takes no type arguments",
                String::from_utf8_lossy(&j.name)
            ),
        );
    }
    Ok(())
}

fn jsx(j: &Jsx) -> Result {
    jsx_type_args(j)?;
    for a in &j.attrs {
        optional_expr(a.value.as_deref())?;
    }
    for c in &j.children {
        match c {
            Child::Element(j) => jsx(j)?,
            Child::Expr(e) => optional_expr(e.as_deref())?,
            Child::Text(_) => {}
        }
    }
    Ok(())
}

fn expr(e: &Expr) -> Result {
    match &e.data {
        ExprData::Arrow(f) => function(f),
        ExprData::Jsx(j) => jsx(j),
        ExprData::As(a) => {
            type_ref(&a.ty)?;
            expr(&a.expr)
        }
        ExprData::Call(c) => {
            expr(&c.callee)?;
            for t in &c.type_args {
                type_ref(t)?;
            }
            for a in &c.args {
                expr(a)?;
            }
            Ok(())
        }
        ExprData::Template(t) => {
            for e in &t.exprs {
                expr(e)?;
            }
            Ok(())
        }
        ExprData::Unary(u) => expr(&u.operand),
        ExprData::Binary(b) => {
            expr(&b.lhs)?;
            expr(&b.rhs)
        }
        ExprData::Cond(c) => {
            expr(&c.cond)?;
            expr(&c.then)?;
            expr(&c.else_)
        }
        ExprData::Member(m) => expr(&m.object),
        ExprData::Index(i) => {
            expr(&i.object)?;
            expr(&i.index)
        }
        ExprData::Object(ps) => {
            for p in ps {
                expr(&p.value)?;
            }
            Ok(())
        }
        ExprData::Array(es) => {
            for e in es {
                expr(e)?;
            }
            Ok(())
        }
        ExprData::Spread(e) | ExprData::NonNull(e) => expr(e),
        ExprData::Assign(a) => {
            expr(&a.target)?;
            expr(&a.value)
        }
        ExprData::Update(u) => expr(&u.target),
        ExprData::Number(_)
        | ExprData::String(_)
        | ExprData::Boolean(_)
        | ExprData::NullLit
        | ExprData::Ident(_) => Ok(()),
    }
}

fn statements(ss: &[Box<Stmt>]) -> Result {
    for s in ss {
        stmt(s)?;
    }
    Ok(())
}

fn stmt(s: &Stmt) -> Result {
    match &s.data {
        StmtData::Func(f) => function(f),
        StmtData::Decl(d) => {
            optional_type(d.ty.as_ref())?;
            optional_expr(d.init.as_deref())
        }
        StmtData::Ret(e) => optional_expr(e.as_deref()),
        StmtData::Expr(e) => expr(e),
        StmtData::Block(ss) => statements(ss),
        StmtData::If(i) => {
            expr(&i.cond)?;
            stmt(&i.then)?;
            i.else_.as_deref().map_or(Ok(()), stmt)
        }
        StmtData::For(f) => {
            if let Some(s) = &f.init {
                stmt(s)?;
            }
            optional_expr(f.cond.as_deref())?;
            optional_expr(f.update.as_deref())?;
            stmt(&f.body)
        }
        StmtData::ForOf(f) => {
            expr(&f.iterable)?;
            stmt(&f.body)
        }
        StmtData::While(w) => {
            expr(&w.cond)?;
            stmt(&w.body)
        }
        StmtData::Switch(s) => {
            expr(&s.value)?;
            for c in &s.cases {
                optional_expr(c.value.as_deref())?;
                statements(&c.body)?;
            }
            Ok(())
        }
        StmtData::Break | StmtData::Continue => Ok(()),
    }
}
