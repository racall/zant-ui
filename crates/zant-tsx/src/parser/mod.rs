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

mod functions;
mod module;
mod statements;
#[cfg(test)]
mod tests;

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
