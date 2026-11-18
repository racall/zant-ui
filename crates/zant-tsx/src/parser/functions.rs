//! Functions: declarations, async functions, parameters, destructuring and return types.

use super::*;

impl<'a> Parser<'a> {
    pub fn parse_function(&mut self) -> Result<Box<Function>, Error> {
        self.parse_function_with_default(false)
    }

    pub(super) fn parse_function_with_default(
        &mut self,
        default: bool,
    ) -> Result<Box<Function>, Error> {
        let loc = self.cur.start;
        self.advance(); // function
        if self.is_p("*") {
            return Err(self.fail(self.cur.start, "generator functions are not supported"));
        }
        let name = if default && (self.is_p("(") || self.is_p("<")) {
            // The parser has no file name; anonymous default components use App.
            b"App".to_vec()
        } else {
            self.expect_ident("a function name")?.name
        };
        let type_params = crate::type_params::parse_params(self)?;
        let params = self.parse_params_with(true)?;
        let ret = self.parse_return_type()?;
        if !self.is_p("{") {
            return Err(self.unexpected("{"));
        }
        let is_async = std::mem::take(&mut self.pending_async);
        let outer = std::mem::replace(&mut self.in_async, is_async);
        let body = self.parse_block();
        self.in_async = outer;
        let mut function = Box::new(Function {
            type_params,
            loc,
            name,
            params,
            ret,
            body: Body::Block(body?),
        });
        if is_async {
            crate::async_lower::lower(self, &mut function)?;
        }
        Ok(function)
    }

    /// `async function`: the `async` keyword is the current token.
    pub fn is_async_function(&mut self) -> bool {
        if !self.is_kw("async") {
            return false;
        }
        let state = self.save();
        self.advance();
        let yes = self.is_kw("function") && !self.cur.nl_before;
        self.restore(state);
        yes
    }

    /// `function copy(row: i64): CopyResult;`: a signature with no body.
    pub(super) fn parse_declared_function(&mut self) -> Result<Box<Function>, Error> {
        let loc = self.cur.start;
        self.advance(); // function
        let name = self.expect_ident("a function name")?;
        let type_params = crate::type_params::parse_params(self)?;
        let params = self.parse_params()?;
        let ret = self.parse_return_type()?;
        if self.is_p("{") {
            return Err(self.fail(self.cur.start, "a declared function has no body"));
        }
        self.end_statement()?;
        Ok(Box::new(Function {
            type_params,
            loc,
            name: name.name,
            params,
            ret,
            body: Body::Block(Vec::new()),
        }))
    }

    /// `(a: T, b = 1, ...rest: U[])`. Rest parameters are accepted where
    /// `rest` allows them (`function` declarations).
    pub fn parse_params_with(&mut self, rest: bool) -> Result<Vec<Param>, Error> {
        self.expect_p("(")?;
        let mut params: Vec<Param> = Vec::new();
        while !self.is_p(")") {
            if self.is_p("[") {
                return Err(self.fail(
                    self.cur.start,
                    "array destructuring in parameters is not supported",
                ));
            }
            if self.is_p("...") {
                if !rest {
                    return Err(self.fail(
                        self.cur.start,
                        "rest parameters are supported in `function` declarations",
                    ));
                }
                let loc = self.cur.start;
                self.advance();
                let name = self.expect_ident("a parameter name")?;
                if !self.eat_p(":") {
                    return Err(
                        self.fail(loc, "a rest parameter needs an array type: `...items: T[]`")
                    );
                }
                let ty = self.parse_type()?;
                if self.is_p("=") || self.is_p("?") {
                    return Err(self.fail(self.cur.start, "a rest parameter has no default"));
                }
                params.push(Param {
                    name,
                    ty: Some(ty),
                    optional: false,
                    destructure: Vec::new(),
                    destructure_defaults: Vec::new(),
                    rest: true,
                    default: None,
                });
                if self.eat_p(",") && !self.is_p(")") {
                    return Err(
                        self.fail(self.cur.start, "the rest parameter must be the last one")
                    );
                }
                break;
            }
            let mut param;
            if self.is_p("{") {
                // `{ todo, onToggle }: Props` (React props)
                let loc = self.cur.start;
                param = Param {
                    name: Name {
                        name: b"props".to_vec(),
                        loc,
                    },
                    ty: None,
                    optional: false,
                    destructure: Vec::new(),
                    destructure_defaults: Vec::new(),
                    rest: false,
                    default: None,
                };
                self.parse_destructure(&mut param)?;
            } else {
                param = Param {
                    name: self.expect_ident("a parameter name")?,
                    ty: None,
                    optional: false,
                    destructure: Vec::new(),
                    destructure_defaults: Vec::new(),
                    rest: false,
                    default: None,
                };
                param.optional = self.eat_p("?");
            }
            if self.eat_p(":") {
                param.ty = Some(self.parse_type()?);
            }
            if self.eat_p("=") {
                if !param.destructure.is_empty() {
                    return Err(self.fail(
                        self.cur.start,
                        "give defaults per prop: `{ size = 14 }: Props`",
                    ));
                }
                if param.optional {
                    return Err(self.fail(
                        self.cur.start,
                        "a parameter with a default is already optional; drop the `?`",
                    ));
                }
                param.default = Some(expr_parser::parse_cond(self)?);
            }
            params.push(param);
            if !self.eat_p(",") {
                break;
            }
        }
        self.expect_p(")")?;
        Ok(params)
    }

    /// `(a: T, b = 1)`: parameters without a rest parameter.
    pub fn parse_params(&mut self) -> Result<Vec<Param>, Error> {
        self.parse_params_with(false)
    }

    /// `{ a, b: local = 1 }` in a parameter: properties, bindings and defaults.
    pub(super) fn parse_destructure(&mut self, param: &mut Param) -> Result<(), Error> {
        self.advance(); // {
        let mut names = Vec::new();
        let mut defaults: Vec<Option<Box<Expr>>> = Vec::new();
        while !self.is_p("}") {
            if self.is_p("...") {
                return Err(self.fail(self.cur.start, "rest properties are not supported"));
            }
            let name = self.expect_ident("a property name")?;
            let local = if self.eat_p(":") {
                self.expect_ident("a variable name")?
            } else {
                name.clone()
            };
            names.push(ast::DestructuredProp {
                name: name.name,
                loc: name.loc,
                local,
            });
            if self.eat_p("=") {
                defaults.push(Some(expr_parser::parse_assign(self)?));
            } else {
                defaults.push(None);
            }
            if !self.eat_p(",") {
                break;
            }
        }
        self.expect_p("}")?;
        param.destructure = names;
        param.destructure_defaults = defaults;
        Ok(())
    }

    pub fn parse_return_type(&mut self) -> Result<Option<TypeRef>, Error> {
        if !self.eat_p(":") {
            return Ok(None);
        }
        Ok(Some(self.parse_type()?))
    }

    /// A type annotation (`type_parser`).
    pub fn parse_type(&mut self) -> Result<TypeRef, Error> {
        type_parser::parse_type(self)
    }

    // ── statements ──────────────────────────────────────────────────────
}
