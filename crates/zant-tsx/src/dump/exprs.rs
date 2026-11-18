//! Dumping expressions, object properties and JSX.

use super::*;

impl Dumper {
    pub(super) fn expr(&mut self, e: &Expr) {
        match &e.data {
            ExprData::Number(raw) => {
                self.line_fmt(format!("expr loc={} number {}", e.loc, quote(raw)));
            }
            ExprData::String(value) => {
                self.line_fmt(format!("expr loc={} string {}", e.loc, quote(value)));
            }
            ExprData::Boolean(value) => {
                self.line_fmt(format!("expr loc={} boolean {value}", e.loc));
            }
            ExprData::NullLit => self.line_fmt(format!("expr loc={} null", e.loc)),
            ExprData::Ident(name) => {
                self.line_fmt(format!("expr loc={} ident {}", e.loc, quote(name)));
            }
            ExprData::Template(t) => {
                self.line_fmt(format!("expr loc={} template", e.loc));
                self.depth += 1;
                for quasi in &t.quasis {
                    self.line_fmt(format!("quasi {}", quote(quasi)));
                }
                for x in &t.exprs {
                    self.wrapped("subexpr", |d| d.expr(x));
                }
                self.depth -= 1;
            }
            ExprData::Unary(u) => {
                self.line_fmt(format!("expr loc={} unary op={}", e.loc, quote(&u.op)));
                self.depth += 1;
                self.wrapped("operand", |d| d.expr(&u.operand));
                self.depth -= 1;
            }
            ExprData::Binary(b) => {
                self.line_fmt(format!("expr loc={} binary op={}", e.loc, quote(&b.op)));
                self.depth += 1;
                self.wrapped("lhs", |d| d.expr(&b.lhs));
                self.wrapped("rhs", |d| d.expr(&b.rhs));
                self.depth -= 1;
            }
            ExprData::Cond(c) => {
                self.line_fmt(format!("expr loc={} cond", e.loc));
                self.depth += 1;
                self.wrapped("cond", |d| d.expr(&c.cond));
                self.wrapped("then", |d| d.expr(&c.then));
                self.wrapped("else", |d| d.expr(&c.else_));
                self.depth -= 1;
            }
            ExprData::Call(c) => {
                self.line_fmt(format!("expr loc={} call", e.loc));
                self.depth += 1;
                self.wrapped("callee", |d| d.expr(&c.callee));
                for ty in &c.type_args {
                    self.wrapped("type-arg", |d| d.type_ref(ty));
                }
                for arg in &c.args {
                    self.wrapped("arg", |d| d.expr(arg));
                }
                self.depth -= 1;
            }
            ExprData::Member(m) => {
                self.line_fmt(format!(
                    "expr loc={} member optional={} name={}",
                    e.loc,
                    m.optional,
                    quote(&m.name.name)
                ));
                self.depth += 1;
                self.wrapped("object", |d| d.expr(&m.object));
                self.depth -= 1;
            }
            ExprData::Arrow(f) => {
                self.line_fmt(format!("expr loc={} arrow", e.loc));
                self.depth += 1;
                self.function(f);
                self.depth -= 1;
            }
            ExprData::Jsx(j) => {
                self.line_fmt(format!("expr loc={} jsx", e.loc));
                self.depth += 1;
                self.jsx(j);
                self.depth -= 1;
            }
            ExprData::Object(props) => {
                self.line_fmt(format!("expr loc={} object", e.loc));
                self.depth += 1;
                for p in props {
                    self.property(p);
                }
                self.depth -= 1;
            }
            ExprData::Array(items) => {
                self.line_fmt(format!("expr loc={} array", e.loc));
                self.depth += 1;
                for item in items {
                    self.wrapped("item", |d| d.expr(item));
                }
                self.depth -= 1;
            }
            ExprData::Spread(x) => {
                self.line_fmt(format!("expr loc={} spread", e.loc));
                self.depth += 1;
                self.wrapped("operand", |d| d.expr(x));
                self.depth -= 1;
            }
            ExprData::Index(i) => {
                self.line_fmt(format!("expr loc={} index optional={}", e.loc, i.optional));
                self.depth += 1;
                self.wrapped("object", |d| d.expr(&i.object));
                self.wrapped("index", |d| d.expr(&i.index));
                self.depth -= 1;
            }
            ExprData::NonNull(x) => {
                self.line_fmt(format!("expr loc={} non-null", e.loc));
                self.depth += 1;
                self.wrapped("operand", |d| d.expr(x));
                self.depth -= 1;
            }
            ExprData::As(a) => {
                self.line_fmt(format!("expr loc={} as", e.loc));
                self.depth += 1;
                self.wrapped("operand", |d| d.expr(&a.expr));
                self.wrapped("ty", |d| d.type_ref(&a.ty));
                self.depth -= 1;
            }
            ExprData::Assign(a) => {
                self.line_fmt(format!("expr loc={} assign op={}", e.loc, quote(&a.op)));
                self.depth += 1;
                self.wrapped("target", |d| d.expr(&a.target));
                self.wrapped("value", |d| d.expr(&a.value));
                self.depth -= 1;
            }
            ExprData::Update(u) => {
                self.line_fmt(format!("expr loc={} update op={}", e.loc, quote(&u.op)));
                self.depth += 1;
                self.wrapped("target", |d| d.expr(&u.target));
                self.depth -= 1;
            }
        }
    }

    pub(super) fn property(&mut self, p: &Property) {
        self.line_fmt(format!(
            "property spread={} key={} loc={}",
            p.spread,
            quote(&p.key.name),
            p.key.loc
        ));
        self.depth += 1;
        self.wrapped("value", |d| d.expr(&p.value));
        self.depth -= 1;
    }

    pub(super) fn jsx(&mut self, j: &Jsx) {
        self.line_fmt(format!("jsx loc={} name={}", j.loc, quote(&j.name)));
        self.depth += 1;
        if !j.type_args.is_empty() {
            self.section("type-args", |d| {
                for arg in &j.type_args {
                    d.type_ref(arg);
                }
            });
        }
        for attr in &j.attrs {
            self.attr(attr);
        }
        for child in &j.children {
            match child {
                Child::Text(t) => {
                    self.line_fmt(format!("child text loc={} {}", t.loc, quote(&t.value)))
                }
                Child::Expr(e) => {
                    self.line("child expr");
                    self.depth += 1;
                    if let Some(e) = e {
                        self.expr(e);
                    }
                    self.depth -= 1;
                }
                Child::Element(el) => {
                    self.line("child element");
                    self.depth += 1;
                    self.jsx(el);
                    self.depth -= 1;
                }
            }
        }
        self.depth -= 1;
    }

    pub(super) fn attr(&mut self, a: &Attr) {
        self.line_fmt(format!(
            "attr name={} loc={} spread={}",
            quote(&a.name.name),
            a.name.loc,
            a.spread
        ));
        self.depth += 1;
        if let Some(value) = &a.value {
            self.wrapped("value", |d| d.expr(value));
        }
        self.depth -= 1;
    }
}
