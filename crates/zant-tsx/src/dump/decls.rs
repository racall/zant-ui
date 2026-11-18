//! Dumping declarations: imports, top-level functions, types, constants, functions, parameters and type references.

use super::*;

impl Dumper {
    pub(super) fn import(&mut self, imp: &Import) {
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

    pub(super) fn top_function(&mut self, top: &TopFunction) {
        self.line_fmt(format!(
            "top-function exported={} default={}",
            top.exported, top.default
        ));
        self.depth += 1;
        self.function(&top.func);
        self.depth -= 1;
    }

    pub(super) fn type_decl(&mut self, t: &TypeDecl) {
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

    pub(super) fn top_const(&mut self, c: &TopConst) {
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

    pub(super) fn function(&mut self, f: &Function) {
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

    pub(super) fn type_params(&mut self, params: &[ast::TypeParam]) {
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

    pub(super) fn param(&mut self, p: &Param) {
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

    pub(super) fn type_ref(&mut self, t: &TypeRef) {
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

    pub(super) fn type_field(&mut self, field: &TypeField) {
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

    pub(super) fn func_type(&mut self, f: &FuncType) {
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
}
