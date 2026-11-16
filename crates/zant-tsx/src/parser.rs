//! Recursive-descent parser for the TSX subset (`docs/tsx.md`): the parser
//! state, the module, functions, types and statements. Expressions live in
//! `expr_parser`, JSX in `jsx_parser` (text rules in `jsx_text`).
//!
//! Shape follows esbuild's parser: one token of lookahead (`cur`, with the
//! lexer positioned right after it) and explicit mode switches for JSX tags,
//! JSX text and template literals. Nodes are `Box`ed and lists are `Vec`s,
//! while AST string leaves are `Vec<u8>` (see `ast.rs`).

use crate::ast::{
    self, Body, Decl, Expr, ExprData, Function, Import, Module, Name, ObjectBinding, Param,
    Pattern, Stmt, StmtData, TopConst, TopFunction, TypeDecl, TypeDetail, TypeRef,
};
pub use crate::dump::dump_program;
pub use crate::error::{Diag, ParseError as Error};
use crate::expr_parser;
use crate::lexer::{Kind, Lexer, Token};
use crate::loop_parser;
use crate::react_compat;
use crate::type_parser;

/// AST bytes as text for diagnostics. `String` cannot hold the WTF-8 byte
/// escapes a decoded string may produce, so diagnostics use the lossy form
/// (identifiers and type names are always valid UTF-8).
pub(crate) fn bytes_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub struct Parser<'a> {
    pub src: &'a str,
    pub lx: Lexer<'a>,
    pub cur: Token,
    pub diag: Option<Diag>,
    /// Inside the body of an `async` function: `await` is an operator.
    pub in_async: bool,
    /// The next function parsed is `async` (set by `async function`).
    pub pending_async: bool,
}

#[derive(Clone, Copy)]
pub struct State {
    pub pos: u32,
    pub cur: Token,
}

impl<'a> Parser<'a> {
    pub fn init(src: &'a str) -> Parser<'a> {
        let mut lx = Lexer::init(src);
        let first = lx.next();
        Parser {
            src,
            lx,
            cur: first,
            diag: None,
            in_async: false,
            pending_async: false,
        }
    }

    // ── helpers ─────────────────────────────────────────────────────────

    pub fn fail(&mut self, loc: u32, msg: impl Into<String>) -> Error {
        if self.diag.is_none() {
            self.diag = Some(Diag {
                loc,
                msg: msg.into(),
            });
        }
        Error::Syntax
    }

    /// The current token's text. The lifetime comes from the source, not from
    /// the parser, so callers may hold it across `advance`.
    pub fn text(&self) -> &'a str {
        self.cur.text(self.src)
    }

    /// A slice of the source between two byte offsets.
    pub fn src_slice(&self, from: u32, to: u32) -> &'a str {
        &self.src[from as usize..to as usize]
    }

    pub fn advance(&mut self) {
        self.cur = self.lx.next();
    }

    pub fn is_p(&self, p: &str) -> bool {
        self.cur.kind == Kind::Punct && self.text() == p
    }

    pub fn is_kw(&self, kw: &str) -> bool {
        self.cur.kind == Kind::Identifier && self.text() == kw
    }

    pub fn eat_p(&mut self, p: &str) -> bool {
        if !self.is_p(p) {
            return false;
        }
        self.advance();
        true
    }

    pub fn expect_p(&mut self, p: &str) -> Result<(), Error> {
        if !self.eat_p(p) {
            return Err(self.unexpected(p));
        }
        Ok(())
    }

    pub fn unexpected(&mut self, wanted: &str) -> Error {
        let loc = self.cur.start;
        match self.cur.kind {
            Kind::Eof => self.fail(loc, format!("expected `{wanted}` but the file ended")),
            Kind::Invalid => {
                let text = self.text();
                self.fail(loc, format!("unexpected character `{text}`"))
            }
            _ => {
                let text = self.text();
                self.fail(loc, format!("expected `{wanted}`, found `{text}`"))
            }
        }
    }

    pub fn expect_ident(&mut self, what: &str) -> Result<Name, Error> {
        if self.cur.kind != Kind::Identifier {
            return Err(self.unexpected(what));
        }
        let name = Name {
            name: self.text().as_bytes().to_vec(),
            loc: self.cur.start,
        };
        self.advance();
        Ok(name)
    }

    /// tsc's rule for the end of a statement: a `;`, or (automatic
    /// semicolon insertion) a `}`, the end of the file or a line break.
    pub fn end_statement(&mut self) -> Result<(), Error> {
        if self.eat_p(";") {
            return Ok(());
        }
        if self.is_p("}") || self.cur.kind == Kind::Eof || self.cur.nl_before {
            return Ok(());
        }
        let msg = format!("expected `;` or a line break before `{}`", self.text());
        Err(self.fail(self.cur.start, msg))
    }

    /// Joins a `>` token with a following `=` into `>=` (binary-operator
    /// position only; see the lexer).
    pub fn rescan_greater(&mut self) {
        if !self.is_p(">") {
            return;
        }
        // `>` `>` `>` `=`: up to three adjacent `>`, then an optional `=`
        let bytes = self.src.as_bytes();
        let mut end = self.cur.end as usize;
        let mut greater = 1;
        while greater < 3 && bytes.get(end) == Some(&b'>') {
            end += 1;
            greater += 1;
        }
        if bytes.get(end) == Some(&b'=') {
            end += 1;
        }
        self.cur.end = end as u32;
        self.lx.pos = self.cur.end;
    }

    /// Allocates a node: boxes the value.
    #[allow(clippy::new_ret_no_self)] // allocates a node, it is not a constructor
    pub fn new<T>(&self, value: T) -> Box<T> {
        Box::new(value)
    }

    pub fn expr(&self, loc: u32, data: ExprData) -> Box<Expr> {
        Box::new(Expr { loc, data })
    }

    pub fn save(&self) -> State {
        State {
            pos: self.lx.pos,
            cur: self.cur,
        }
    }

    pub fn restore(&mut self, s: State) {
        self.lx.pos = s.pos;
        self.cur = s.cur;
    }

    // ── module ──────────────────────────────────────────────────────────

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
    fn mark_default(&mut self, m: &mut ModuleBuilder) -> Result<(), Error> {
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
    fn parse_type_decl(&mut self, exported: bool) -> Result<TypeDecl, Error> {
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
    fn parse_top_const(&mut self, m: &mut ModuleBuilder, exported: bool) -> Result<(), Error> {
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

    pub fn parse_function(&mut self) -> Result<Box<Function>, Error> {
        self.parse_function_with_default(false)
    }

    fn parse_function_with_default(&mut self, default: bool) -> Result<Box<Function>, Error> {
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
    fn parse_declared_function(&mut self) -> Result<Box<Function>, Error> {
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

    /// `try { … } catch (e) { … } finally { … }`.
    fn parse_try(&mut self, loc: u32) -> Result<Box<Stmt>, Error> {
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

    /// `{ a, b: local = 1 }` in a parameter: properties, bindings and defaults.
    fn parse_destructure(&mut self, param: &mut Param) -> Result<(), Error> {
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

// The builder lives outside `impl` because Rust does not allow a nested struct
// there.
#[derive(Default)]
struct ModuleBuilder {
    imports: Vec<Import>,
    functions: Vec<TopFunction>,
    types: Vec<TypeDecl>,
    consts: Vec<TopConst>,
    declared: Vec<TopFunction>,
    enums: Vec<crate::ast::EnumDecl>,
}

#[cfg(test)]
pub(crate) fn parse_ok(src: &str) -> Module {
    let mut p = Parser::init(src);
    match p.parse_module() {
        Ok(module) => module,
        Err(_) => {
            if let Some(d) = &p.diag {
                panic!("parse error at {}: {}", d.loc, d.msg);
            }
            panic!("parse error without a diagnostic");
        }
    }
}

#[cfg(test)]
pub(crate) fn expect_syntax_error(src: &str, msg: &str, at: &str) {
    let mut p = Parser::init(src);
    assert!(
        p.parse_module().is_err(),
        "expected a syntax error in {src:?}"
    );
    let d = p.diag.expect("diagnostic");
    assert_eq!(msg, d.msg);
    let offset = src.find(at).expect("error location text") as u32;
    assert_eq!(offset, d.loc);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{self, Body, Child, Expr, ExprData, Function, Stmt, StmtData};

    fn block(f: &Function) -> &[Box<Stmt>] {
        match &f.body {
            Body::Block(stmts) => stmts,
            Body::Expr(_) => panic!("expected a block body"),
        }
    }

    fn ret_expr(s: &Stmt) -> &Expr {
        match &s.data {
            StmtData::Ret(Some(e)) => e,
            _ => panic!("expected a return with a value"),
        }
    }

    fn text(value: &[u8]) -> &str {
        std::str::from_utf8(value).expect("text is UTF-8")
    }

    #[test]
    fn anonymous_default_components_use_app_without_a_file_name() {
        let source = r#"import { View } from "zant";
export default function () { return <View />; }"#;
        let m = parse_ok(source);
        assert_eq!(m.functions.len(), 1);
        let f = &m.functions[0];
        assert_eq!("App", text(&f.func.name));
        assert!(f.default && f.exported);
        assert!(matches!(
            &ret_expr(&block(&f.func)[0]).data,
            ExprData::Jsx(_)
        ));
        crate::compile::compile(source, "Other.tsx").expect("anonymous default compiles as App");
        expect_syntax_error(
            "function () {}",
            "expected `a function name`, found `(`",
            "(",
        );
        expect_syntax_error(
            "export function () {}",
            "expected `a function name`, found `(`",
            "(",
        );
    }

    #[test]
    fn enums_lower_to_a_type_and_inlined_members() {
        let m = parse_ok(
            "export enum Kind { A = 'a', B = 'b' }\nconst enum Level { Low, High = 10, Max }",
        );
        assert_eq!(m.types.len(), 2);
        assert!(
            m.consts.is_empty(),
            "members are inlined, not a runtime object"
        );
        let values: Vec<_> = m.enums[1]
            .members
            .iter()
            .map(|member| member.value.clone())
            .collect();
        assert_eq!(
            values,
            [
                ast::EnumValue::Number("0.0".into()),
                ast::EnumValue::Number("10.0".into()),
                ast::EnumValue::Number("11.0".into()),
            ]
        );
        for (source, message, at) in [
            (
                "enum Mixed { A = 'a', B = 1 }",
                "enums mixing string and number members are not supported; use one kind",
                "Mixed",
            ),
            (
                "enum After { A = 'a', B }",
                "a member after a string member needs a value (`Member = \"…\"`)",
                "B",
            ),
            (
                "enum Computed { A = 1 + 2 }",
                "enum members are string or number literals; computed members are not supported",
                "1",
            ),
        ] {
            expect_syntax_error(source, message, at);
        }
    }

    #[test]
    fn parses_a_component() {
        let m = parse_ok(
            r#"import { useState, View, Text } from "zant";
import type { u32 } from "zant";
export default function App() {
  const [n, setN] = useState<u32>(0);
  function inc() { setN(n + 1) }
  return (
    <View gap={8} style={{ padding: 4 }}>
      <Text size={20}>Count: {n} {`x${n * 2}y`}</Text>
      {/* comment */}
      <Button onClick={() => setN(0)} disabled>Reset &amp; go</Button>
    </View>
  );
}"#,
        );
        assert_eq!(m.imports.len(), 2);
        assert!(m.imports[1].type_only);
        assert_eq!(m.functions.len(), 1);
        let f = &m.functions[0];
        assert!(f.default);
        assert_eq!("App", text(&f.func.name));
        let body = block(&f.func);
        assert_eq!(body.len(), 3);
        let init = match &body[0].data {
            StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
            _ => panic!("expected a declaration"),
        };
        let call = match &init.data {
            ExprData::Call(c) => c,
            _ => panic!("expected a call"),
        };
        assert_eq!(call.type_args.len(), 1);
        let root = match &ret_expr(&body[2]).data {
            ExprData::Jsx(j) => j,
            _ => panic!("expected JSX"),
        };
        assert_eq!("View", text(&root.name));
        assert_eq!(root.attrs.len(), 2);
        assert_eq!(root.children.len(), 3);
        let text_el = match &root.children[0] {
            Child::Element(el) => el,
            _ => panic!("expected an element"),
        };
        assert_eq!(text_el.children.len(), 4);
        match &text_el.children[0] {
            Child::Text(t) => assert_eq!("Count: ", text(&t.value)),
            _ => panic!("expected text"),
        }
        match &text_el.children[2] {
            Child::Text(t) => assert_eq!(" ", text(&t.value)),
            _ => panic!("expected text"),
        }
        match &text_el.children[3] {
            Child::Expr(Some(e)) => match &e.data {
                ExprData::Template(t) => assert_eq!(t.quasis.len(), 2),
                _ => panic!("expected a template"),
            },
            _ => panic!("expected an expression child"),
        }
        assert!(matches!(&root.children[1], Child::Expr(None)));
        let button = match &root.children[2] {
            Child::Element(el) => el,
            _ => panic!("expected an element"),
        };
        assert!(button.attrs[1].value.is_none());
        match &button.children[0] {
            Child::Text(t) => assert_eq!("Reset & go", text(&t.value)),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn syntax_errors_carry_a_location() {
        let mut p = Parser::init("function App() { return <View></Text>; }");
        assert!(p.parse_module().is_err());
        let d = p.diag.clone().expect("diagnostic");
        assert_eq!("expected `</View>` to close <View>", d.msg);
        assert_eq!(32, d.loc);
    }

    #[test]
    fn arrow_function_components() {
        let m = parse_ok(
            r#"import { View } from "zant";
const Box = () => <View />;
export const Panel = (): JSX.Element => {
  return <View />;
};
export default Box;"#,
        );
        assert_eq!(m.functions.len(), 2);
        assert_eq!("Box", text(&m.functions[0].func.name));
        assert!(m.functions[0].default);
        let body = block(&m.functions[0].func);
        assert_eq!(body.len(), 1);
        assert!(matches!(&ret_expr(&body[0]).data, ExprData::Jsx(_)));
        assert_eq!("Panel", text(&m.functions[1].func.name));
        assert!(m.functions[1].exported && !m.functions[1].default);
    }

    #[test]
    fn statements_end_at_semicolon_brace_end_or_line_break_tscs_asi() {
        // no semicolons at all is fine
        let m = parse_ok("function f() {\n  const a = 1\n  let b = a\n  b = 2\n  return b\n}");
        assert_eq!(block(&m.functions[0].func).len(), 4);
        expect_syntax_error(
            "function f() { g() h() }",
            "expected `;` or a line break before `h`",
            "h()",
        );
        expect_syntax_error(
            "function f() {\n  return\n    <View />\n}",
            "a line break after `return` ends the statement; start the value on the same line (`return (`)",
            "<View",
        );
    }

    #[test]
    fn postfix_increment_needs_the_operand_on_the_same_line() {
        let m = parse_ok("function f() {\n  a\n  ++b\n}");
        let body = block(&m.functions[0].func);
        assert_eq!(body.len(), 2);
        assert!(matches!(
            &body[0].data,
            StmtData::Expr(e) if matches!(&e.data, ExprData::Ident(_))
        ));
        assert!(matches!(
            &body[1].data,
            StmtData::Expr(e) if matches!(&e.data, ExprData::Update(_))
        ));
    }

    #[test]
    fn destructured_props_and_optional_parameters() {
        let m = parse_ok(
            r#"function Row({ todo, onToggle }: RowProps) { return <View />; }
function g(a: number, b?: string) {}"#,
        );
        let p = &m.functions[0].func.params[0];
        assert_eq!("props", text(&p.name.name));
        assert_eq!(p.destructure.len(), 2);
        assert_eq!("onToggle", text(&p.destructure[1].name));
        assert_eq!("RowProps", text(&p.ty.as_ref().expect("type").name));
        assert!(m.functions[1].func.params[1].optional);
    }

    #[test]
    fn destructured_props_keep_property_names_local_names_and_defaults() {
        let m = parse_ok(
            r#"function Label({ id, title: label = "Untitled", onSave: save }: {
                id: number; title?: string; onSave: () => void;
            }) { return <Text>{label}</Text>; }"#,
        );
        let p = &m.functions[0].func.params[0];
        assert_eq!(3, p.destructure.len());
        assert_eq!("id", text(&p.destructure[0].name));
        assert_eq!("id", text(&p.destructure[0].local.name));
        assert_eq!("title", text(&p.destructure[1].name));
        assert_eq!("label", text(&p.destructure[1].local.name));
        assert_eq!("onSave", text(&p.destructure[2].name));
        assert_eq!("save", text(&p.destructure[2].local.name));
        assert!(p.destructure_defaults[0].is_none());
        assert!(matches!(
            &p.destructure_defaults[1].as_ref().expect("default").data,
            ExprData::String(value) if value == b"Untitled"
        ));
        assert!(p.destructure_defaults[2].is_none());
        expect_syntax_error(
            "function f({ title: }: P) {}",
            "expected `a variable name`, found `}`",
            "}",
        );
    }

    #[test]
    fn destructured_defaults_accept_assignment_level_arrows() {
        let m = parse_ok(
            "function Probe({ onClose = () => {}, onPick: pick = (id: number) => save(id), onPair = (a, b) => { save(a); } }: Props) { return <View />; }",
        );
        let p = &m.functions[0].func.params[0];
        for (index, count) in [0, 1, 2].into_iter().enumerate() {
            let ExprData::Arrow(f) = &p.destructure_defaults[index].as_ref().unwrap().data else {
                panic!("expected an arrow default");
            };
            assert_eq!(count, f.params.len());
        }
        assert_eq!(b"pick", p.destructure[1].local.name.as_slice());
    }

    #[test]
    fn typed_local_object_destructuring_keeps_keys_and_bindings() {
        let m = parse_ok(
            r#"function f() {
                const item: Item = { title: "Hello", id: 1 };
                const { title: label, id } = item;
                let { title: other }: Item = item;
            }"#,
        );
        let body = block(&m.functions[0].func);
        let StmtData::Decl(d) = &body[1].data else {
            panic!("declaration");
        };
        let Pattern::Object(bindings) = &d.pattern else {
            panic!("object destructuring");
        };
        assert_eq!("title", text(&bindings[0].key.name));
        assert_eq!("label", text(&bindings[0].local.name));
        assert_eq!(bindings[1].key, bindings[1].local);
        let StmtData::Decl(d) = &body[2].data else {
            panic!("typed declaration");
        };
        assert!(!d.is_const);
        assert_eq!("Item", text(&d.ty.as_ref().expect("type").name));
    }

    // Every TSX file of the repository (the golden corpus, the fixtures and
    // the examples) must parse.
    #[test]
    fn parser_accepts_the_repository_tsx_corpus() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        let testdata = root.join("crates/zant-tsx/testdata/tsx");
        if !testdata.is_dir() {
            return;
        }
        for entry in std::fs::read_dir(&testdata).expect("testdata directory") {
            let path = entry.expect("testdata entry").path();
            if path.extension().is_some_and(|e| e == "tsx") {
                paths.push(path);
            }
        }
        assert_eq!(36, paths.len());
        let examples = root.join("crates/zant-examples/tsx/corpus");
        if examples.is_dir() {
            collect_tsx(&examples, &mut paths);
            assert!(paths.len() > 34);
        }
        for path in paths {
            let src = std::fs::read_to_string(&path).expect("read TSX file");
            let mut p = Parser::init(&src);
            if p.parse_module().is_err() {
                panic!(
                    "{}: {}",
                    path.display(),
                    p.diag.as_ref().map(|d| d.msg.as_str()).unwrap_or("")
                );
            }
        }
    }

    fn collect_tsx(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                collect_tsx(&path, out);
            } else if path.extension().is_some_and(|e| e == "tsx") {
                out.push(path);
            }
        }
    }
}
