//! Top-level declarations: the module, imports, type declarations, file constants and default exports.

use super::*;

impl<'a> Parser<'a> {
    pub fn parse_module(&mut self) -> Result<Module, Error> {
        let mut m = ModuleBuilder::default();
        while self.cur.kind != Kind::Eof {
            if self.eat_p(";") {
                continue;
            }
            if self.is_kw("import") {
                m.imports.push(self.parse_import()?);
                continue;
            }
            let mut exported = false;
            let mut default = false;
            if self.is_kw("export") {
                self.advance();
                exported = true;
                default = self.is_kw("default");
                if default {
                    self.advance();
                }
                if default
                    && self.cur.kind == Kind::Identifier
                    && !self.is_kw("function")
                    && !self.is_async_function()
                    && !self.is_kw("enum")
                {
                    self.mark_default(&mut m)?;
                    continue;
                }
            }
            // `enum E { … }` / `const enum E { … }`: a type and its members
            let const_enum = !default && self.is_kw("const") && {
                let state = self.save();
                self.advance();
                let is_enum = self.is_kw("enum");
                self.restore(state);
                is_enum
            };
            if const_enum {
                self.advance(); // const
            }
            if self.is_kw("enum") {
                if default {
                    return Err(self.fail(self.cur.start, "an enum cannot be the default export"));
                }
                let lowered = crate::enum_parser::parse(self, exported)?;
                m.types.push(lowered.ty);
                m.enums.push(lowered.decl);
                continue;
            }
            if !default && self.is_kw("declare") {
                // `export declare function copy(row: i64): CopyResult;` (native module specs)
                self.advance();
                if !self.is_kw("function") {
                    return Err(self.fail(self.cur.start, "only `declare function` is supported"));
                }
                let func = self.parse_declared_function()?;
                m.declared.push(TopFunction {
                    func,
                    exported,
                    default: false,
                });
            } else if self.is_kw("function") || self.is_async_function() {
                if self.is_kw("async") {
                    self.advance();
                    self.pending_async = true;
                }
                let func = self.parse_function_with_default(default)?;
                m.functions.push(TopFunction {
                    func,
                    exported,
                    default,
                });
            } else if !default && self.is_kw("const") {
                self.parse_top_const(&mut m, exported)?;
            } else if !default && (self.is_kw("interface") || self.is_kw("type")) {
                m.types.push(self.parse_type_decl(exported)?);
            } else if exported {
                return Err(self.fail(
                    self.cur.start,
                    "only functions, constants and types can be exported",
                ));
            } else {
                return Err(self.fail(
                    self.cur.start,
                    "only imports, functions, constants and types are supported at the top level",
                ));
            }
        }
        let mut module = Module {
            imports: m.imports,
            functions: m.functions,
            types: m.types,
            consts: m.consts,
            declared: m.declared,
            enums: m.enums,
        };
        react_compat::normalize(&mut module);
        Ok(module)
    }

    /// `export default App;` marks an earlier function component.
    pub(super) fn mark_default(&mut self, m: &mut ModuleBuilder) -> Result<(), Error> {
        let name = self.expect_ident("a component name")?;
        self.end_statement()?;
        for f in &mut m.functions {
            if f.func.name == name.name {
                f.exported = true;
                f.default = true;
                return Ok(());
            }
        }
        let msg = format!(
            "`{}` is not a function declared above",
            bytes_lossy(&name.name)
        );
        Err(self.fail(name.loc, msg))
    }

    /// `interface Todo { ... }` / `type Filter = "all" | "done";`
    pub(super) fn parse_type_decl(&mut self, exported: bool) -> Result<TypeDecl, Error> {
        let is_interface = self.is_kw("interface");
        self.advance();
        let name = self.expect_ident("a type name")?;
        let type_params = crate::type_params::parse_params(self)?;
        if is_interface {
            // `interface B extends A, C { .. }`: the body plus the bases' fields
            let mut bases: Vec<TypeRef> = Vec::new();
            if self.is_kw("extends") {
                self.advance();
                loop {
                    bases.push(self.parse_type()?);
                    if !self.eat_p(",") {
                        break;
                    }
                }
            }
            let loc = self.cur.start;
            // `{ [key: string]: V }` is a Record
            let body = type_parser::parse_object_type(self, loc)?;
            let ty = if bases.is_empty() {
                body
            } else {
                bases.push(body);
                TypeRef {
                    args: Vec::new(),
                    name: Vec::new(),
                    loc,
                    detail: Some(Box::new(TypeDetail::Intersection(bases))),
                }
            };
            return Ok(TypeDecl {
                type_params,
                name,
                ty,
                exported,
            });
        }
        self.expect_p("=")?;
        let ty = self.parse_type()?;
        self.end_statement()?;
        Ok(TypeDecl {
            type_params,
            name,
            ty,
            exported,
        })
    }

    /// A top-level `const`: an arrow function is a component, anything else
    /// a constant.
    pub(super) fn parse_top_const(
        &mut self,
        m: &mut ModuleBuilder,
        exported: bool,
    ) -> Result<(), Error> {
        self.advance(); // const
        let name = self.expect_ident("a name")?;
        let mut ty: Option<TypeRef> = None;
        if self.eat_p(":") {
            ty = Some(self.parse_type()?);
        }
        self.expect_p("=")?;
        let value = expr_parser::parse_expr(self)?;
        if self.is_p(",") {
            return Err(self.fail(self.cur.start, "declare one constant per statement"));
        }
        self.end_statement()?;
        let mut func = match *value {
            Expr {
                data: ExprData::Arrow(f),
                ..
            } => f,
            value => {
                m.consts.push(TopConst {
                    name,
                    ty,
                    value: Box::new(value),
                    exported,
                });
                return Ok(());
            }
        };
        func.name = name.name;
        func.body = match func.body {
            // a concise body becomes `{ return body; }`
            Body::Expr(e) => {
                let ret = Box::new(Stmt {
                    loc: e.loc,
                    data: StmtData::Ret(Some(e)),
                });
                Body::Block(vec![ret])
            }
            block => block,
        };
        m.functions.push(TopFunction {
            func,
            exported,
            default: false,
        });
        Ok(())
    }

    /// Parse a named import or React's default / namespace import.
    pub fn parse_import(&mut self) -> Result<Import, Error> {
        crate::import_parser::parse(self)
    }

    // ── functions and types ─────────────────────────────────────────────
}
