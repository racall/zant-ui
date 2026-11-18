//! The render locals a `<List>` row closure captures, found by walking its JSX, statements and expressions.

use super::*;

/// A render-local or state value captured by a list closure.
pub(super) struct Capture {
    /// The TSX name, visible inside the closure body.
    pub(super) name: Vec<u8>,
    /// The Rust local declared before the closure (`_cap_count`).
    pub(super) local: String,
    pub(super) ty: Type,
    /// The initialiser (`self.count.clone()`, `label.clone()`, …).
    pub(super) init: String,
}

/// Render-local and state names read by a renderItem body, in first-use order.
pub(super) fn collect_captures(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    body: &ast::Body,
) -> Result<Vec<Capture>> {
    let mut names: Vec<Vec<u8>> = Vec::new();
    collect_body_names(body, &mut names);
    let mut captures: Vec<Capture> = Vec::new();
    for name in names {
        if captures.iter().any(|capture| capture.name == name) {
            continue;
        }
        if let Some(binding) = scope::local(c, &name) {
            let ty = binding.ty;
            let init = if ty.copy(g) {
                binding.rust.clone()
            } else {
                format!("{}.clone()", binding.rust)
            };
            let local = format!("_cap_{}", String::from_utf8_lossy(&name));
            captures.push(Capture {
                name,
                local,
                ty,
                init,
            });
            continue;
        }
        if g.current_component.is_some() {
            // a renderItem body reading a prop (`props.x` or a destructured
            // `onDelete`) must capture it before the closure: the row closures
            // outlive the `&mut self` build and cannot read `self.props`
            let name_str = String::from_utf8_lossy(&name).into_owned();
            if let Some(value) = crate::codegen::components::read_prop(g, &name_str)? {
                let ty = value.ty;
                let init = value.code;
                let local = format!("_cap_{}", String::from_utf8_lossy(&name));
                captures.push(Capture {
                    name,
                    local,
                    ty,
                    init,
                });
                continue;
            }
        }
        if let Some(index) = scope::state_index(g, &name) {
            let state = g.states[index].clone();
            let field = scope::state_field(g, state.name.as_bytes())?;
            let init = if state.ty.copy(g) {
                format!("self.{field}")
            } else {
                format!("self.{field}.clone()")
            };
            let local = format!("_cap_{}", String::from_utf8_lossy(&name));
            captures.push(Capture {
                name,
                local,
                ty: state.ty,
                init,
            });
        }
    }
    Ok(captures)
}

pub(super) fn collect_jsx_names(jsx: &ast::Jsx, out: &mut Vec<Vec<u8>>) {
    for attr in &jsx.attrs {
        if let Some(value) = attr.value.as_deref() {
            collect_expr_names(value, out);
        }
    }
    for child in &jsx.children {
        match child {
            ast::Child::Element(element) => collect_jsx_names(element, out),
            ast::Child::Expr(Some(value)) => collect_expr_names(value, out),
            ast::Child::Expr(None) | ast::Child::Text(_) => {}
        }
    }
}

pub(super) fn collect_body_names(body: &ast::Body, out: &mut Vec<Vec<u8>>) {
    match body {
        ast::Body::Expr(e) => collect_expr_names(e, out),
        ast::Body::Block(statements) => {
            for statement in statements {
                collect_stmt_names(statement, out);
            }
        }
    }
}

pub(super) fn collect_stmt_names(statement: &ast::Stmt, out: &mut Vec<Vec<u8>>) {
    match &statement.data {
        ast::StmtData::Decl(decl) => {
            if let Some(init) = decl.init.as_deref() {
                collect_expr_names(init, out);
            }
        }
        ast::StmtData::Func(function) => collect_body_names(&function.body, out),
        ast::StmtData::Ret(Some(e)) => collect_expr_names(e, out),
        ast::StmtData::Ret(None) => {}
        ast::StmtData::If(value) => {
            collect_expr_names(&value.cond, out);
            collect_stmt_names(&value.then, out);
            if let Some(otherwise) = value.else_.as_deref() {
                collect_stmt_names(otherwise, out);
            }
        }
        ast::StmtData::For(value) => {
            if let Some(init) = value.init.as_deref() {
                collect_stmt_names(init, out);
            }
            if let Some(cond) = value.cond.as_deref() {
                collect_expr_names(cond, out);
            }
            if let Some(update) = value.update.as_deref() {
                collect_expr_names(update, out);
            }
            collect_stmt_names(&value.body, out);
        }
        ast::StmtData::ForOf(value) => {
            collect_expr_names(&value.iterable, out);
            collect_stmt_names(&value.body, out);
        }
        ast::StmtData::While(value) => {
            collect_expr_names(&value.cond, out);
            collect_stmt_names(&value.body, out);
        }
        ast::StmtData::Switch(value) => {
            collect_expr_names(&value.value, out);
            for case in &value.cases {
                if let Some(test) = case.value.as_deref() {
                    collect_expr_names(test, out);
                }
                for child in &case.body {
                    collect_stmt_names(child, out);
                }
            }
        }
        ast::StmtData::Break | ast::StmtData::Continue => {}
        ast::StmtData::Expr(e) => collect_expr_names(e, out),
        ast::StmtData::Block(statements) => {
            for statement in statements {
                collect_stmt_names(statement, out);
            }
        }
        ast::StmtData::Throw(e) => collect_expr_names(e, out),
        ast::StmtData::Try(t) => {
            for statement in t
                .body
                .iter()
                .chain(t.catch.iter().flat_map(|c| c.body.iter()))
                .chain(t.finally.iter().flatten())
            {
                collect_stmt_names(statement, out);
            }
        }
    }
}

pub(super) fn collect_expr_names(e: &ast::Expr, out: &mut Vec<Vec<u8>>) {
    match &e.data {
        ast::ExprData::Ident(name) => out.push(name.clone()),
        ast::ExprData::Template(template) => {
            for expr in &template.exprs {
                collect_expr_names(expr, out);
            }
        }
        ast::ExprData::Unary(unary) => collect_expr_names(&unary.operand, out),
        ast::ExprData::Binary(binary) => {
            collect_expr_names(&binary.lhs, out);
            collect_expr_names(&binary.rhs, out);
        }
        ast::ExprData::Cond(value) => {
            collect_expr_names(&value.cond, out);
            collect_expr_names(&value.then, out);
            collect_expr_names(&value.else_, out);
        }
        ast::ExprData::Call(call) => {
            collect_expr_names(&call.callee, out);
            for arg in &call.args {
                collect_expr_names(arg, out);
            }
        }
        ast::ExprData::Member(member) => collect_expr_names(&member.object, out),
        ast::ExprData::Arrow(function) => collect_body_names(&function.body, out),
        ast::ExprData::Jsx(jsx) => collect_jsx_names(jsx, out),
        ast::ExprData::Object(properties) => {
            for property in properties {
                collect_expr_names(&property.value, out);
            }
        }
        ast::ExprData::Array(items) => {
            for item in items {
                collect_expr_names(item, out);
            }
        }
        ast::ExprData::Spread(inner) | ast::ExprData::NonNull(inner) => {
            collect_expr_names(inner, out);
        }
        ast::ExprData::Index(index) => {
            collect_expr_names(&index.object, out);
            collect_expr_names(&index.index, out);
        }
        ast::ExprData::As(cast) => collect_expr_names(&cast.expr, out),
        ast::ExprData::Assign(assign) => {
            collect_expr_names(&assign.target, out);
            collect_expr_names(&assign.value, out);
        }
        ast::ExprData::Update(update) => collect_expr_names(&update.target, out),
        _ => {}
    }
}
