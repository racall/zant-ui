//! Does a function body mention one of these names?
//!
//! Used to find component functions (`const copy = () => …`) that read render
//! locals: those are emitted like inline arrows where they are passed
//! (`onClick={copy}`), so they capture the values of that render.
//!
//! Over-approximates (a parameter or inner local with the same name counts):
//! inlining a function that did not need it is still correct.

use crate::ast;

pub fn function(f: &ast::Function, names: &[&str]) -> bool {
    match &f.body {
        ast::Body::Block(ss) => statements(ss, names),
        ast::Body::Expr(e) => expr(e, names),
    }
}

fn statements(ss: &[Box<ast::Stmt>], names: &[&str]) -> bool {
    ss.iter().any(|s| statement(s, names))
}

fn opt(e: Option<&ast::Expr>, names: &[&str]) -> bool {
    e.is_some_and(|e| expr(e, names))
}

fn statement(s: &ast::Stmt, names: &[&str]) -> bool {
    match &s.data {
        ast::StmtData::Decl(d) => opt(d.init.as_deref(), names),
        ast::StmtData::Func(f) => function(f, names),
        ast::StmtData::Ret(e) => opt(e.as_deref(), names),
        ast::StmtData::If(v) => {
            expr(&v.cond, names)
                || statement(&v.then, names)
                || v.else_.as_deref().is_some_and(|el| statement(el, names))
        }
        ast::StmtData::For(v) => {
            v.init.as_deref().is_some_and(|i| statement(i, names))
                || opt(v.cond.as_deref(), names)
                || opt(v.update.as_deref(), names)
                || statement(&v.body, names)
        }
        ast::StmtData::ForOf(v) => expr(&v.iterable, names) || statement(&v.body, names),
        ast::StmtData::While(v) => expr(&v.cond, names) || statement(&v.body, names),
        ast::StmtData::Switch(v) => {
            if expr(&v.value, names) {
                return true;
            }
            v.cases
                .iter()
                .any(|case| opt(case.value.as_deref(), names) || statements(&case.body, names))
        }
        ast::StmtData::Break | ast::StmtData::Continue => false,
        ast::StmtData::Expr(e) => expr(e, names),
        ast::StmtData::Block(ss) => statements(ss, names),
    }
}

fn exprs(es: &[Box<ast::Expr>], names: &[&str]) -> bool {
    es.iter().any(|e| expr(e, names))
}

fn expr(e: &ast::Expr, names: &[&str]) -> bool {
    match &e.data {
        ast::ExprData::Number(_) | ast::ExprData::String(_) | ast::ExprData::Boolean(_) => false,
        ast::ExprData::NullLit => false,
        ast::ExprData::Ident(n) => names.iter().any(|name| name.as_bytes() == n.as_slice()),
        ast::ExprData::Template(t) => exprs(&t.exprs, names),
        ast::ExprData::Unary(u) => expr(&u.operand, names),
        ast::ExprData::Binary(b) => expr(&b.lhs, names) || expr(&b.rhs, names),
        ast::ExprData::Cond(v) => {
            expr(&v.cond, names) || expr(&v.then, names) || expr(&v.else_, names)
        }
        ast::ExprData::Call(v) => expr(&v.callee, names) || exprs(&v.args, names),
        ast::ExprData::Member(m) => expr(&m.object, names),
        ast::ExprData::Arrow(f) => function(f, names),
        ast::ExprData::Jsx(j) => jsx(j, names),
        ast::ExprData::Object(ps) => ps.iter().any(|p| expr(&p.value, names)),
        ast::ExprData::Array(items) => exprs(items, names),
        ast::ExprData::Spread(inner) | ast::ExprData::NonNull(inner) => expr(inner, names),
        ast::ExprData::Index(ix) => expr(&ix.object, names) || expr(&ix.index, names),
        ast::ExprData::As(cast) => expr(&cast.expr, names),
        ast::ExprData::Assign(v) => expr(&v.target, names) || expr(&v.value, names),
        ast::ExprData::Update(v) => expr(&v.target, names),
    }
}

fn jsx(j: &ast::Jsx, names: &[&str]) -> bool {
    if j.attrs.iter().any(|a| opt(a.value.as_deref(), names)) {
        return true;
    }
    j.children.iter().any(|child| match child {
        ast::Child::Text(_) => false,
        ast::Child::Expr(e) => opt(e.as_deref(), names),
        ast::Child::Element(el) => jsx(el, names),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse_module(src: &str) -> ast::Module {
        let mut parser = Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    /// The body of the function called `name`.
    fn function_named(module: &ast::Module, name: &str) -> ast::Function {
        module
            .functions
            .iter()
            .find(|top| top.func.name == name.as_bytes())
            .map(|top| (*top.func).clone())
            .expect("function exists")
    }

    #[test]
    fn a_body_mentioning_a_render_local_is_detected() {
        let module = parse_module("function save(count: number) { console.log(count); }\n");
        let save = function_named(&module, "save");
        assert!(function(&save, &["count"]));
        assert!(!function(&save, &["other"]));
    }

    #[test]
    fn parameters_and_jsx_attributes_count_as_mentions() {
        let module = parse_module(
            "function pick(props: { label: string }) { console.log(props.label); }\nfunction view() { console.log(1); }\nfunction inner() { return <Text>{label}</Text>; }\n",
        );
        // a parameter with the same name over-approximates (documented)
        assert!(function(&function_named(&module, "pick"), &["props"]));
        assert!(!function(&function_named(&module, "pick"), &["label"]));
        assert!(function(&function_named(&module, "inner"), &["label"]));
        assert!(!function(&function_named(&module, "view"), &["label"]));
    }
}
