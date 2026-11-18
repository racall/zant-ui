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

mod decls;
mod exprs;
mod stmts;
#[cfg(test)]
mod tests;

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
