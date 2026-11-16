//! Deterministic AST dump, for parser tests and golden comparisons.
//!
//! Format rules:
//! - one node per line, two spaces per nesting level;
//! - a line is `label key=value ...`; string leaves are always quoted and
//!   escaped (bytes outside printable ASCII become `\xNN`, so WTF-8 decoded
//!   strings dump deterministically);
//! - `loc=N` is the node's byte offset in the source;
//! - every node kind is printed; list sections print their header even when
//!   empty, optional fields are omitted when absent, and wrappers (`lhs`,
//!   `arg`, `body`, ...) name the field the child belongs to.

use crate::ast::{
    self, Attr, Case, Child, Expr, ExprData, FuncType, Function, Import, Jsx, Module,
    ObjectBinding, Param, Pattern, Property, Stmt, StmtData, TopConst, TopFunction, TypeDecl,
    TypeDetail, TypeField, TypeRef,
};

/// The AST root: a whole module.
pub type Program = Module;

/// A deterministic, human-readable dump of a whole module.
pub fn dump_program(program: &Program) -> String {
    let mut d = Dumper::default();
    d.module(program);
    d.out
}

#[derive(Default)]
struct Dumper {
    out: String,
    depth: usize,
}

impl Dumper {
    fn line(&mut self, text: &str) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn line_fmt(&mut self, text: String) {
        self.line(&text);
    }

    fn module(&mut self, m: &Module) {
        self.line("module");
        self.depth += 1;
        self.section("imports", |d| {
            for imp in &m.imports {
                d.import(imp);
            }
        });
        self.section("functions", |d| {
            for top in &m.functions {
                d.top_function(top);
            }
        });
        self.section("types", |d| {
            for t in &m.types {
                d.type_decl(t);
            }
        });
        self.section("consts", |d| {
            for c in &m.consts {
                d.top_const(c);
            }
        });
        self.section("declared", |d| {
            for top in &m.declared {
                d.top_function(top);
            }
        });
        self.depth -= 1;
    }

    fn section(&mut self, label: &str, f: impl FnOnce(&mut Self)) {
        self.line(label);
        self.depth += 1;
        f(self);
        self.depth -= 1;
    }

    fn import(&mut self, imp: &Import) {
        self.line_fmt(format!(
            "import loc={} source_loc={} type_only={} source={}",
            imp.loc,
            imp.source_loc,
            imp.type_only,
            quote(&imp.source)
        ));
        self.depth += 1;
        for n in &imp.names {
            self.line_fmt(format!(
                "import-name type_only={} imported={} local={}",
                n.type_only,
                quote(&n.imported.name),
                quote(&n.local.name)
            ));
        }
        if let Some(n) = &imp.default_name {
            self.line_fmt(format!("default {}", quote(&n.name)));
        }
        if let Some(n) = &imp.namespace_name {
            self.line_fmt(format!("namespace {}", quote(&n.name)));
        }
        self.depth -= 1;
    }

    fn top_function(&mut self, top: &TopFunction) {
        self.line_fmt(format!(
            "top-function exported={} default={}",
            top.exported, top.default
        ));
        self.depth += 1;
        self.function(&top.func);
        self.depth -= 1;
    }

    fn type_decl(&mut self, t: &TypeDecl) {
        self.line_fmt(format!(
            "type-decl name={} loc={} exported={}",
            quote(&t.name.name),
            t.name.loc,
            t.exported
        ));
        self.depth += 1;
        self.type_params(&t.type_params);
        self.type_ref(&t.ty);
        self.depth -= 1;
    }

    fn top_const(&mut self, c: &TopConst) {
        self.line_fmt(format!(
            "top-const name={} loc={} exported={}",
            quote(&c.name.name),
            c.name.loc,
            c.exported
        ));
        self.depth += 1;
        if let Some(ty) = &c.ty {
            self.wrapped("ty", |d| d.type_ref(ty));
        }
        self.wrapped("value", |d| d.expr(&c.value));
        self.depth -= 1;
    }

    fn function(&mut self, f: &Function) {
        self.line_fmt(format!("function loc={} name={}", f.loc, quote(&f.name)));
        self.depth += 1;
        self.type_params(&f.type_params);
        self.section("params", |d| {
            for p in &f.params {
                d.param(p);
            }
        });
        if let Some(ret) = &f.ret {
            self.wrapped("ret", |d| d.type_ref(ret));
        }
        match &f.body {
            ast::Body::Block(ss) => {
                self.line("body block");
                self.depth += 1;
                for s in ss {
                    self.stmt(s);
                }
                self.depth -= 1;
            }
            ast::Body::Expr(e) => {
                self.line("body expr");
                self.depth += 1;
                self.expr(e);
                self.depth -= 1;
            }
        }
        self.depth -= 1;
    }

    fn type_params(&mut self, params: &[ast::TypeParam]) {
        if params.is_empty() {
            return;
        }
        self.section("type-params", |d| {
            for p in params {
                d.line_fmt(format!(
                    "type-param name={} loc={} const={}",
                    quote(&p.name.name),
                    p.name.loc,
                    p.is_const
                ));
                d.depth += 1;
                if let Some(t) = &p.constraint {
                    d.wrapped("constraint", |d| d.type_ref(t));
                }
                if let Some(t) = &p.default {
                    d.wrapped("default", |d| d.type_ref(t));
                }
                d.depth -= 1;
            }
        });
    }

    fn param(&mut self, p: &Param) {
        self.line_fmt(format!(
            "param name={} loc={} optional={}",
            quote(&p.name.name),
            p.name.loc,
            p.optional
        ));
        self.depth += 1;
        if let Some(ty) = &p.ty {
            self.wrapped("ty", |d| d.type_ref(ty));
        }
        for n in &p.destructure {
            self.line_fmt(format!("destructure {}", quote(&n.name)));
        }
        for (i, default) in p.destructure_defaults.iter().enumerate() {
            if let Some(e) = default {
                self.line_fmt(format!("destructure-default #{i}"));
                self.depth += 1;
                self.expr(e);
                self.depth -= 1;
            }
        }
        if let Some(e) = &p.default {
            self.wrapped("default", |d| d.expr(e));
        }
        self.depth -= 1;
    }

    fn type_ref(&mut self, t: &TypeRef) {
        self.line_fmt(format!("type-ref loc={} name={}", t.loc, quote(&t.name)));
        if !t.args.is_empty() {
            self.depth += 1;
            self.section("args", |d| {
                for arg in &t.args {
                    d.type_ref(arg);
                }
            });
            self.depth -= 1;
        }
        let Some(detail) = &t.detail else {
            return;
        };
        self.depth += 1;
        match &**detail {
            TypeDetail::Array(item) => {
                self.line("array");
                self.depth += 1;
                self.type_ref(item);
                self.depth -= 1;
            }
            TypeDetail::Union(members) => {
                self.line("union");
                self.depth += 1;
                for m in members {
                    self.type_ref(m);
                }
                self.depth -= 1;
            }
            TypeDetail::StringLit(value) => self.line_fmt(format!("string-lit {}", quote(value))),
            TypeDetail::Null => self.line("null"),
            TypeDetail::Object(fields) => {
                self.line("object");
                self.depth += 1;
                for f in fields {
                    self.type_field(f);
                }
                self.depth -= 1;
            }
            TypeDetail::Func(f) => self.func_type(f),
            TypeDetail::Keyof(inner) => {
                self.line("keyof");
                self.depth += 1;
                self.type_ref(inner);
                self.depth -= 1;
            }
            TypeDetail::Typeof(name) => self.line_fmt(format!("typeof {}", quote(&name.name))),
            TypeDetail::Indexed(inner, key) => {
                self.line_fmt(format!("indexed {}", quote(key)));
                self.depth += 1;
                self.type_ref(inner);
                self.depth -= 1;
            }
            TypeDetail::NumberLit(value) => self.line_fmt(format!("number-lit {}", quote(value))),
            TypeDetail::Tuple(members) => {
                self.line("tuple");
                self.depth += 1;
                for m in members {
                    self.type_ref(m);
                }
                self.depth -= 1;
            }
            TypeDetail::Intersection(members) => {
                self.line("intersection");
                self.depth += 1;
                for m in members {
                    self.type_ref(m);
                }
                self.depth -= 1;
            }
        }
        self.depth -= 1;
    }

    fn type_field(&mut self, field: &TypeField) {
        self.line_fmt(format!(
            "type-field loc={} optional={} name={}",
            field.name.loc,
            field.optional,
            quote(&field.name.name)
        ));
        self.depth += 1;
        self.type_ref(&field.ty);
        self.depth -= 1;
    }

    fn func_type(&mut self, f: &FuncType) {
        self.line("func");
        self.depth += 1;
        self.type_params(&f.type_params);
        self.section("params", |d| {
            for p in &f.params {
                d.param(p);
            }
        });
        self.wrapped("ret", |d| d.type_ref(&f.ret));
        self.depth -= 1;
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.data {
            StmtData::Decl(decl) => {
                self.line_fmt(format!("stmt loc={} decl const={}", s.loc, decl.is_const));
                self.depth += 1;
                self.pattern(&decl.pattern);
                if let Some(ty) = &decl.ty {
                    self.wrapped("ty", |d| d.type_ref(ty));
                }
                if let Some(e) = &decl.init {
                    self.wrapped("init", |d| d.expr(e));
                }
                self.depth -= 1;
            }
            StmtData::Func(f) => {
                self.line_fmt(format!("stmt loc={} func", s.loc));
                self.depth += 1;
                self.function(f);
                self.depth -= 1;
            }
            StmtData::Ret(e) => {
                self.line_fmt(format!("stmt loc={} ret", s.loc));
                self.depth += 1;
                if let Some(e) = e {
                    self.expr(e);
                }
                self.depth -= 1;
            }
            StmtData::If(v) => {
                self.line_fmt(format!("stmt loc={} if", s.loc));
                self.depth += 1;
                self.wrapped("cond", |d| d.expr(&v.cond));
                self.wrapped("then", |d| d.stmt(&v.then));
                if let Some(el) = &v.else_ {
                    self.wrapped("else", |d| d.stmt(el));
                }
                self.depth -= 1;
            }
            StmtData::For(v) => {
                self.line_fmt(format!("stmt loc={} for", s.loc));
                self.depth += 1;
                if let Some(init) = &v.init {
                    self.wrapped("init", |d| d.stmt(init));
                }
                if let Some(cond) = &v.cond {
                    self.wrapped("cond", |d| d.expr(cond));
                }
                if let Some(update) = &v.update {
                    self.wrapped("update", |d| d.expr(update));
                }
                self.wrapped("body", |d| d.stmt(&v.body));
                self.depth -= 1;
            }
            StmtData::ForOf(v) => {
                self.line_fmt(format!(
                    "stmt loc={} for-of name={} const={}",
                    s.loc,
                    quote(&v.name.name),
                    v.is_const
                ));
                self.depth += 1;
                self.wrapped("iterable", |d| d.expr(&v.iterable));
                self.wrapped("body", |d| d.stmt(&v.body));
                self.depth -= 1;
            }
            StmtData::While(v) => {
                self.line_fmt(format!("stmt loc={} while do-first={}", s.loc, v.do_first));
                self.depth += 1;
                self.wrapped("cond", |d| d.expr(&v.cond));
                self.wrapped("body", |d| d.stmt(&v.body));
                self.depth -= 1;
            }
            StmtData::Switch(v) => {
                self.line_fmt(format!("stmt loc={} switch", s.loc));
                self.depth += 1;
                self.wrapped("value", |d| d.expr(&v.value));
                for case in &v.cases {
                    self.case(case);
                }
                self.depth -= 1;
            }
            StmtData::Break => self.line_fmt(format!("stmt loc={} break", s.loc)),
            StmtData::Continue => self.line_fmt(format!("stmt loc={} continue", s.loc)),
            StmtData::Expr(e) => {
                self.line_fmt(format!("stmt loc={} expr", s.loc));
                self.depth += 1;
                self.expr(e);
                self.depth -= 1;
            }
            StmtData::Block(ss) => {
                self.line_fmt(format!("stmt loc={} block", s.loc));
                self.depth += 1;
                for s in ss {
                    self.stmt(s);
                }
                self.depth -= 1;
            }
            StmtData::Throw(e) => {
                self.line_fmt(format!("stmt loc={} throw", s.loc));
                self.depth += 1;
                self.expr(e);
                self.depth -= 1;
            }
            StmtData::Try(t) => {
                self.line_fmt(format!("stmt loc={} try", s.loc));
                self.depth += 1;
                for s in &t.body {
                    self.stmt(s);
                }
                if let Some(c) = &t.catch {
                    let name = c.name.as_ref().map(|n| quote(&n.name)).unwrap_or_default();
                    self.line_fmt(format!("catch {name}"));
                    for s in &c.body {
                        self.stmt(s);
                    }
                }
                if let Some(f) = &t.finally {
                    self.line("finally");
                    for s in f {
                        self.stmt(s);
                    }
                }
                self.depth -= 1;
            }
        }
    }

    fn case(&mut self, c: &Case) {
        self.line_fmt(format!("case loc={}", c.loc));
        self.depth += 1;
        if let Some(value) = &c.value {
            self.wrapped("test", |d| d.expr(value));
        }
        self.section("body", |d| {
            for s in &c.body {
                d.stmt(s);
            }
        });
        self.depth -= 1;
    }

    fn pattern(&mut self, p: &Pattern) {
        match p {
            Pattern::Ident(n) => {
                self.line_fmt(format!("pattern ident {} loc={}", quote(&n.name), n.loc));
            }
            Pattern::Array(names) => {
                self.line("pattern array");
                self.depth += 1;
                for n in names {
                    match n {
                        Some(n) => self.line_fmt(format!("binding {}", quote(&n.name))),
                        None => self.line("hole"),
                    }
                }
                self.depth -= 1;
            }
            Pattern::Object(bindings) => {
                self.line("pattern object");
                self.depth += 1;
                for b in bindings {
                    self.object_binding(b);
                }
                self.depth -= 1;
            }
        }
    }

    fn object_binding(&mut self, b: &ObjectBinding) {
        self.line_fmt(format!(
            "object-binding key={} local={} loc={}",
            quote(&b.key.name),
            quote(&b.local.name),
            b.key.loc
        ));
    }

    fn expr(&mut self, e: &Expr) {
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

    fn property(&mut self, p: &Property) {
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

    fn jsx(&mut self, j: &Jsx) {
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

    fn attr(&mut self, a: &Attr) {
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

    fn wrapped(&mut self, label: &str, f: impl FnOnce(&mut Self)) {
        self.line(label);
        self.depth += 1;
        f(self);
        self.depth -= 1;
    }
}

/// Quote and escape a byte string: printable ASCII stays, everything else is
/// `\xNN`, plus the usual C escapes.
fn quote(bytes: &[u8]) -> String {
    let mut out = String::from("\"");
    for &b in bytes {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINNED_SRC: &str = r#"import { View } from "zant";
function App({ title }: Props) {
  const [n, setN] = useState(0);
  if (n > 0) return <View>{n}</View>;
  for (let i = 0; i < 3; i++) n++;
  return null;
}"#;

    const PINNED_DUMP: &str = "\
module
  imports
    import loc=0 source_loc=21 type_only=false source=\"zant\"
      import-name type_only=false imported=\"View\" local=\"View\"
  functions
    top-function exported=false default=false
      function loc=29 name=\"App\"
        params
          param name=\"props\" loc=42 optional=false
            ty
              type-ref loc=53 name=\"Props\"
            destructure \"title\"
        body block
          stmt loc=64 decl const=true
            pattern array
              binding \"n\"
              binding \"setN\"
            init
              expr loc=82 call
                callee
                  expr loc=82 ident \"useState\"
                arg
                  expr loc=91 number \"0\"
          stmt loc=97 if
            cond
              expr loc=101 binary op=\">\"
                lhs
                  expr loc=101 ident \"n\"
                rhs
                  expr loc=105 number \"0\"
            then
              stmt loc=108 ret
                expr loc=115 jsx
                  jsx loc=115 name=\"View\"
                    child expr
                      expr loc=122 ident \"n\"
          stmt loc=135 for
            init
              stmt loc=140 decl const=false
                pattern ident \"i\" loc=144
                init
                  expr loc=148 number \"0\"
            cond
              expr loc=151 binary op=\"<\"
                lhs
                  expr loc=151 ident \"i\"
                rhs
                  expr loc=155 number \"3\"
            update
              expr loc=158 update op=\"++\"
                target
                  expr loc=158 ident \"i\"
            body
              stmt loc=163 expr
                expr loc=163 update op=\"++\"
                  target
                    expr loc=163 ident \"n\"
          stmt loc=170 ret
            expr loc=177 null
  types
  consts
  declared
";

    /// The pinned shape: byte-for-byte dump of a component (the golden form).
    #[test]
    fn dump_program_pins_a_component_shape() {
        let m = crate::parser::parse_ok(PINNED_SRC);
        assert_eq!(PINNED_DUMP, dump_program(&m));
    }

    // Total coverage: every node kind shows up in the dump.
    #[test]
    fn dump_program_covers_every_node_kind() {
        let m = crate::parser::parse_ok(
            r#"import Def, * as NS from "react";
import { type ReactNode, x as y } from "zant";
import icon from "./icon.png";
export declare function copy(row: i64): CopyResult;
export interface Todo { id: number; title?: string; tags: string[]; done: boolean | null; onToggle: (id: number) => void; }
export type Filter = "all" | "active";
export type Rows = Array<Todo>;
export const LIMIT: u32 = 3;
const later = (a: number = 1): string => `x${a}`;
export default function App({ size = 14 }: Props) {
  function helper() { return; }
  const { a, b } = obj;
  let [c, , d] = arr;
  const flag = true;
  const nothing = null;
  const t = !a;
  const u = -2;
  const v = a + b * 2;
  const w = a ?? b;
  const x1 = a === b ? 1 : 2;
  const x2 = f<Todo>(1, 2);
  const x3 = obj.prop?.inner;
  const x4 = arr[idx];
  const x5 = value!;
  const x6 = value as u32;
  const g = items.map((x: number) => x);
  a = 1;
  a += 2;
  ++a;
  if (a) { b(); } else c();
  for (let i = 0; i < 2; i++) continue;
  for (const item of items) break;
  while (a) a--;
  do a++; while (a);
  switch (a) { case 1: break; default: continue; }
  const obj2 = { key: 1, ...rest, short };
  const arr2 = [1, ...items];
  const esc = "\u00e9";
  const el = <View key={a} {...rest} disabled>text {a} <Text /></View>;
  const frag = <><Text /></>;
  return;
}"#,
        );
        let dump = dump_program(&m);

        // expression kinds
        for kind in [
            "number", "string", "boolean", "null", "ident", "template", "unary", "binary", "cond",
            "call", "member", "arrow", "jsx", "object", "array", "spread", "index", "non-null",
            "as", "assign", "update",
        ] {
            assert!(has_expr(&dump, kind), "missing expression kind {kind}");
        }
        // statement kinds
        for kind in [
            "decl", "func", "ret", "if", "for", "for-of", "while", "switch", "break", "continue",
            "expr", "block",
        ] {
            assert!(has_stmt(&dump, kind), "missing statement kind {kind}");
        }
        // type detail kinds
        for kind in ["array", "union", "null", "object", "func"] {
            assert!(
                dump.lines().any(|l| l.trim() == kind),
                "missing type detail {kind}"
            );
        }
        assert!(
            dump.lines()
                .any(|l| l.trim_start().starts_with("string-lit ")),
            "missing type detail string-lit"
        );
        // patterns, imports, JSX and other node kinds
        for line in [
            "pattern ident",
            "pattern array",
            "pattern object",
            "binding",
            "hole",
            "object-binding",
        ] {
            assert!(
                dump.lines().any(|l| l.trim_start().starts_with(line)),
                "missing pattern line {line}"
            );
        }
        for line in [
            "default ",
            "namespace ",
            "import-name",
            "type-decl",
            "top-const",
            "top-function",
            "type-field",
            "attr",
            "child text",
            "child expr",
            "child element",
            "quasi",
            "subexpr",
            "property",
            "case",
            "func",
            "string-lit",
        ] {
            assert!(
                dump.lines().any(|l| l.trim_start().starts_with(line)),
                "missing node line {line}"
            );
        }
        assert!(dump.contains("spread=true"));
        assert!(dump.contains("optional=true"));
        assert!(dump.contains("type_only=true"));
        assert!(dump.contains("exported=true"));
        assert!(dump.contains("default=true"));
        assert!(dump.contains("do-first=true"));
        assert!(dump.contains("const=true"));
        assert!(dump.contains("name=\"\""));
        assert!(dump.contains("\\x"));
    }

    fn has_expr(dump: &str, kind: &str) -> bool {
        dump.lines().any(|line| {
            let mut parts = line.split_whitespace();
            parts.next() == Some("expr")
                && parts.next().is_some_and(|loc| loc.starts_with("loc="))
                && parts.next() == Some(kind)
        })
    }

    fn has_stmt(dump: &str, kind: &str) -> bool {
        dump.lines().any(|line| {
            let mut parts = line.split_whitespace();
            parts.next() == Some("stmt")
                && parts.next().is_some_and(|loc| loc.starts_with("loc="))
                && parts.next() == Some(kind)
        })
    }
}
