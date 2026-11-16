//! Which functions can let a throw escape (effect inference).
//!
//! TypeScript does not mark throwing functions, so the compiler infers it,
//! like Koka / Swift's inferred `rethrows`: a function throws when its body
//! has a `throw`, or calls a function that throws (of its own file, or
//! imported from another), or a throwing builtin (`JSON.parse`), outside a
//! `try` with a `catch`. Calls form a graph across the project's files; the
//! answer is its least fixed point (recursion alone does not throw).
//!
//! Creating an arrow does not run it, so arrow bodies do not count here;
//! where an arrow's body can throw is decided when it is emitted.

use std::collections::HashSet;

use crate::ast;
use crate::ast_walk::{self, StmtChild};

use super::stateful::ModuleSource;

/// A top-level function: `(file stem, function name)`.
type Key = (String, Vec<u8>);

/// The throwing functions of a project.
#[derive(Clone, Debug, Default)]
pub struct Throwing {
    keys: HashSet<Key>,
}

impl Throwing {
    pub fn throws(&self, module: &str, function: &[u8]) -> bool {
        self.keys.contains(&(module.to_owned(), function.to_vec()))
    }
}

/// What a body does directly, before following calls.
#[derive(Default)]
struct Facts {
    throws: bool,
    /// Plain names called outside a catching `try`.
    calls: Vec<Vec<u8>>,
}

/// Analyzes the project's files plus `current` (which replaces the project
/// file of the same name: the one being generated).
pub fn analyze(project: &[ModuleSource<'_>], current: ModuleSource<'_>) -> Throwing {
    let mut units: Vec<ModuleSource<'_>> = project
        .iter()
        .copied()
        .filter(|unit| unit.name != current.name)
        .collect();
    units.push(current);
    let mut nodes: Vec<(Key, Facts, usize)> = Vec::new();
    for (index, unit) in units.iter().enumerate() {
        for top in &unit.module.functions {
            if !super::functions::is_function(&top.func) {
                continue;
            }
            let mut facts = Facts::default();
            body(&top.func.body, false, &mut facts);
            nodes.push(((unit.name.to_owned(), top.func.name.clone()), facts, index));
        }
    }
    // callees resolved once: a function of the same file, or an import
    let edges: Vec<Vec<Key>> = nodes
        .iter()
        .map(|(_, facts, unit)| {
            facts
                .calls
                .iter()
                .filter_map(|name| resolve(&units[*unit], name))
                .collect()
        })
        .collect();
    let mut keys: HashSet<Key> = nodes
        .iter()
        .filter(|(_, facts, _)| facts.throws)
        .map(|(key, _, _)| key.clone())
        .collect();
    loop {
        let before = keys.len();
        for ((key, _, _), callees) in nodes.iter().zip(&edges) {
            if !keys.contains(key) && callees.iter().any(|callee| keys.contains(callee)) {
                keys.insert(key.clone());
            }
        }
        if keys.len() == before {
            return Throwing { keys };
        }
    }
}

/// The function `name` refers to in `unit`.
fn resolve(unit: &ModuleSource<'_>, name: &[u8]) -> Option<Key> {
    if unit
        .module
        .functions
        .iter()
        .any(|top| top.func.name == name)
    {
        return Some((unit.name.to_owned(), name.to_vec()));
    }
    for import in &unit.module.imports {
        if !super::imports::is_relative(&String::from_utf8_lossy(&import.source)) {
            continue;
        }
        if let Some(imported) = import.names.iter().find(|n| n.local.name == name) {
            let module = super::stateful::stem(&import.source);
            return Some((
                String::from_utf8_lossy(&module).into_owned(),
                imported.imported.name.clone(),
            ));
        }
    }
    None
}

/// A builtin that throws on bad input.
pub fn builtin_throws(call: &ast::Call) -> bool {
    // `new RegExp(text)`: a computed pattern may be invalid (SyntaxError)
    if super::regexps::computed_construction(call) {
        return true;
    }
    let ast::ExprData::Member(member) = &call.callee.data else {
        return false;
    };
    // `xs.reduce(f)` with no initial value: a TypeError on an empty array
    if member.name.name == b"reduce" && call.args.len() == 1 {
        return true;
    }
    matches!(&member.object.data, ast::ExprData::Ident(object) if object == b"JSON")
        && member.name.name == b"parse"
}

fn body(b: &ast::Body, caught: bool, facts: &mut Facts) {
    ast_walk::body_children(b, &mut |child| visit(child, caught, facts));
}

fn visit(child: StmtChild<'_>, caught: bool, facts: &mut Facts) {
    match child {
        StmtChild::Expr(e) => expr(e, caught, facts),
        StmtChild::Stmt(s) => stmt(s, caught, facts),
    }
}

fn stmts(list: &[Box<ast::Stmt>], caught: bool, facts: &mut Facts) {
    for s in list {
        stmt(s, caught, facts);
    }
}

fn stmt(s: &ast::Stmt, caught: bool, facts: &mut Facts) {
    match &s.data {
        ast::StmtData::Throw(value) => {
            expr(value, caught, facts);
            facts.throws |= !caught;
        }
        ast::StmtData::Try(t) => {
            // the protected block's throws go to the catch (if any); the
            // catch and finally blocks throw to the outside
            stmts(&t.body, caught || t.catch.is_some(), facts);
            if let Some(catch) = &t.catch {
                stmts(&catch.body, caught, facts);
            }
            if let Some(finally) = &t.finally {
                stmts(finally, caught, facts);
            }
        }
        _ => ast_walk::stmt_children(s, &mut |child| visit(child, caught, facts)),
    }
}

fn expr(e: &ast::Expr, caught: bool, facts: &mut Facts) {
    if let ast::ExprData::Call(call) = &e.data
        && !caught
    {
        if builtin_throws(call) {
            facts.throws = true;
        }
        if let ast::ExprData::Ident(name) = &call.callee.data
            && !call.is_new
        {
            facts.calls.push(name.clone());
        }
    }
    ast_walk::expr_children(e, &mut |child| expr(child, caught, facts));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> ast::Module {
        let mut parser = crate::parser::Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    fn unit<'a>(name: &'a str, module: &'a ast::Module) -> ModuleSource<'a> {
        ModuleSource { name, module }
    }

    #[test]
    fn throws_follow_calls_and_stop_at_catch() {
        let module = parse(
            r#"function check(n: number): number { if (n < 0) { throw new Error("negative"); } return n; }
function twice(n: number): number { return check(n) * 2; }
function safe(n: number): number { try { return check(n); } catch (e) { return 0; } }
function rethrow(n: number): number { try { return check(n); } catch (e) { throw e; } }
function later(): number { const f = (n: number): number => check(n); return 1; }
function parse(text: string): number { return JSON.parse(text); }
function pure(n: number): number { return n; }"#,
        );
        let result = analyze(&[], unit("App", &module));
        for (name, expected) in [
            ("check", true),
            ("twice", true),
            ("safe", false),
            ("rethrow", true),
            ("later", false),
            ("parse", true),
            ("pure", false),
        ] {
            assert_eq!(expected, result.throws("App", name.as_bytes()), "{name}");
        }
    }

    #[test]
    fn recursion_alone_does_not_throw_and_imports_are_followed() {
        let lib = parse(
            r#"export function fail(): number { throw "no"; }
export function even(n: number): boolean { return n == 0 ? true : odd(n - 1); }
export function odd(n: number): boolean { return n == 0 ? false : even(n - 1); }"#,
        );
        let app = parse(
            r#"import { fail as stop, even } from "./lib";
function run(): number { return stop(); }
function count(): boolean { return even(4); }"#,
        );
        let project = [unit("lib", &lib)];
        let result = analyze(&project, unit("App", &app));
        assert!(result.throws("lib", b"fail"));
        assert!(!result.throws("lib", b"even"));
        assert!(result.throws("App", b"run"));
        assert!(!result.throws("App", b"count"));
    }

    #[test]
    fn a_finally_block_does_not_catch() {
        let module = parse(
            r#"function f(): number { try { throw "x"; } finally { } return 1; }
function g(): number { try { return 1; } catch (e) { return 2; } finally { throw "y"; } }"#,
        );
        let result = analyze(&[], unit("App", &module));
        assert!(result.throws("App", b"f"));
        assert!(result.throws("App", b"g"));
    }
}
