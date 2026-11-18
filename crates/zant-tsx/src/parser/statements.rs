//! Statements: blocks, `try` and the statement dispatcher.

use super::*;

impl<'a> Parser<'a> {
    /// `try { … } catch (e) { … } finally { … }`.
    pub(super) fn parse_try(&mut self, loc: u32) -> Result<Box<Stmt>, Error> {
        self.advance(); // try
        let body = self.parse_block()?;
        let catch = if self.is_kw("catch") {
            self.advance();
            let name = if self.eat_p("(") {
                let name = self.expect_ident("the error's name")?;
                if self.eat_p(":") {
                    // `catch (e: unknown)`: the error type is fixed
                    self.parse_type()?;
                }
                self.expect_p(")")?;
                Some(name)
            } else {
                None
            };
            Some(crate::ast::Catch {
                name,
                body: self.parse_block()?,
            })
        } else {
            None
        };
        let finally = if self.is_kw("finally") {
            self.advance();
            Some(self.parse_block()?)
        } else {
            None
        };
        if catch.is_none() && finally.is_none() {
            return Err(self.fail(loc, "`try` needs a `catch` or `finally`"));
        }
        Ok(Box::new(Stmt {
            loc,
            data: StmtData::Try(crate::ast::Try {
                body,
                catch,
                finally,
            }),
        }))
    }

    pub fn parse_block(&mut self) -> Result<Vec<Box<Stmt>>, Error> {
        self.expect_p("{")?;
        let mut stmts: Vec<Box<Stmt>> = Vec::new();
        while !self.is_p("}") {
            if self.cur.kind == Kind::Eof {
                return Err(self.unexpected("}"));
            }
            if self.eat_p(";") {
                continue;
            }
            stmts.push(self.parse_stmt()?);
        }
        self.advance();
        Ok(stmts)
    }

    pub fn parse_stmt(&mut self) -> Result<Box<Stmt>, Error> {
        let loc = self.cur.start;
        if self.is_kw("const") || self.is_kw("let") || self.is_kw("var") {
            let is_const = self.is_kw("const");
            self.advance();
            let pattern: Pattern = if self.is_p("[") {
                self.advance();
                let mut names: Vec<Option<Name>> = Vec::new();
                while !self.is_p("]") {
                    if self.is_p(",") {
                        self.advance();
                        names.push(None);
                        continue;
                    }
                    names.push(Some(self.expect_ident("a name")?));
                    if !self.eat_p(",") {
                        break;
                    }
                }
                self.expect_p("]")?;
                Pattern::Array(names)
            } else if self.is_p("{") {
                self.advance();
                let mut bindings: Vec<ObjectBinding> = Vec::new();
                while !self.is_p("}") {
                    if self.is_p("...") {
                        return Err(self.fail(self.cur.start, "rest patterns are not supported"));
                    }
                    let key = self.expect_ident("a property name")?;
                    let local = if self.eat_p(":") {
                        self.expect_ident("a variable name")?
                    } else {
                        key.clone()
                    };
                    if self.is_p("=") {
                        return Err(self.fail(
                            self.cur.start,
                            "defaults in destructuring are not supported",
                        ));
                    }
                    bindings.push(ObjectBinding { key, local });
                    if !self.eat_p(",") {
                        break;
                    }
                }
                self.expect_p("}")?;
                Pattern::Object(bindings)
            } else {
                Pattern::Ident(self.expect_ident("a variable name")?)
            };
            let mut ty: Option<TypeRef> = None;
            if self.eat_p(":") {
                ty = Some(self.parse_type()?);
            }
            let mut init_expr: Option<Box<Expr>> = None;
            if self.eat_p("=") {
                init_expr = Some(expr_parser::parse_expr(self)?);
            }
            if self.is_p(",") {
                return Err(self.fail(self.cur.start, "declare one variable per statement"));
            }
            self.end_statement()?;
            return Ok(Box::new(Stmt {
                loc,
                data: StmtData::Decl(Decl {
                    is_const,
                    pattern,
                    ty,
                    init: init_expr,
                }),
            }));
        }
        if self.is_async_function() {
            self.advance(); // async
            self.pending_async = true;
        }
        if self.is_kw("function") {
            let func = self.parse_function()?;
            return Ok(Box::new(Stmt {
                loc,
                data: StmtData::Func(func),
            }));
        }
        if self.is_kw("return") {
            self.advance();
            let mut value: Option<Box<Expr>> = None;
            if !self.is_p(";") && !self.is_p("}") && self.cur.kind != Kind::Eof {
                if self.cur.nl_before {
                    // JS inserts `;` here and returns nothing
                    return Err(self.fail(
                        self.cur.start,
                        "a line break after `return` ends the statement; start the value on the same line (`return (`)",
                    ));
                }
                value = Some(expr_parser::parse_expr(self)?);
            }
            self.end_statement()?;
            return Ok(Box::new(Stmt {
                loc,
                data: StmtData::Ret(value),
            }));
        }
        if self.is_kw("if") {
            self.advance();
            self.expect_p("(")?;
            let cond = expr_parser::parse_expr(self)?;
            self.expect_p(")")?;
            let then = self.parse_stmt()?;
            let mut else_: Option<Box<Stmt>> = None;
            if self.is_kw("else") {
                self.advance();
                else_ = Some(self.parse_stmt()?);
            }
            return Ok(Box::new(Stmt {
                loc,
                data: StmtData::If(ast::If { cond, then, else_ }),
            }));
        }
        if self.is_p("{") {
            let body = self.parse_block()?;
            return Ok(Box::new(Stmt {
                loc,
                data: StmtData::Block(body),
            }));
        }
        for kw in ["for", "while", "do", "switch", "break", "continue"] {
            if self.is_kw(kw) {
                return loop_parser::parse(self);
            }
        }
        if self.is_kw("throw") {
            self.advance();
            if self.cur.nl_before {
                return Err(self.fail(self.cur.start, "`throw` needs a value on the same line"));
            }
            let value = expr_parser::parse_expr(self)?;
            self.end_statement()?;
            return Ok(Box::new(Stmt {
                loc,
                data: StmtData::Throw(value),
            }));
        }
        if self.is_kw("try") {
            return self.parse_try(loc);
        }
        if self.is_kw("class") {
            return Err(self.fail(loc, "`class` is not supported yet"));
        }
        let e = expr_parser::parse_expr(self)?;
        self.end_statement()?;
        Ok(Box::new(Stmt {
            loc,
            data: StmtData::Expr(e),
        }))
    }
}
