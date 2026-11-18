//! Codegen core tests: expression / statement / type generation
//! against hand-reviewed Rust, and `syn::parse_file` validation of everything
//! that is emitted.
//!
//! The cases only need plain functions (functions, loops, emission and
//! builtins); components and JSX have their own suites.

use zant_tsx::ast;
use zant_tsx::codegen::{self, Generator, functions, model};
use zant_tsx::parser::Parser;

mod callables;
mod control_flow;
mod expressions;
mod types;

struct Compiled {
    declarations: String,
    functions: String,
}

impl Compiled {
    /// The whole generated module wrapped in the preamble the application layer adds.
    fn module(&self) -> String {
        format!(
            "#![allow(clippy::redundant_clone)]\n#![allow(clippy::too_many_arguments)]\n\
             use zant::tsx;\nuse zant::ui;\n\n{}{}",
            self.declarations, self.functions
        )
    }

    fn assert_parses(&self) {
        let source = self.module();
        if let Err(error) = syn::parse_file(&source) {
            panic!("generated code does not parse: {error}\n---\n{source}");
        }
    }
}

fn parse_module(src: &str) -> ast::Module {
    let mut parser = Parser::init(src);
    match parser.parse_module() {
        Ok(module) => module,
        Err(_) => match parser.diag {
            Some(diag) => panic!("parse error at {}: {}", diag.loc, diag.msg),
            None => panic!("parse error without a diagnostic"),
        },
    }
}

/// Generate Rust for a module that only holds plain functions and types.
fn compile(src: &str) -> Result<Compiled, String> {
    let module = parse_module(src);
    let mut diag = None;
    let result = {
        let mut g = Generator::new(&mut diag);
        g.type_decls = module.types.clone();
        g.react_imports = module.imports.clone();
        g.resolving = vec![false; module.types.len()];
        (|| -> codegen::Result<Compiled> {
            for decl in &module.types {
                let _ = model::named(&mut g, &decl.name.name)?;
            }
            functions::collect(&mut g, &module)?;
            let declarations = model::declarations(&g);
            let functions = functions::emit_all(&mut g)?;
            Ok(Compiled {
                declarations,
                functions,
            })
        })()
    };
    result.map_err(|_| match diag {
        Some(diag) => format!("{} @{}", diag.msg, diag.loc),
        None => "codegen failed without a diagnostic".to_owned(),
    })
}

fn generated(src: &str) -> Compiled {
    match compile(src) {
        Ok(compiled) => {
            compiled.assert_parses();
            compiled
        }
        Err(message) => panic!("unexpected diagnostic: {message}"),
    }
}

fn diagnostic(src: &str) -> String {
    match compile(src) {
        Ok(compiled) => panic!("expected a diagnostic, generated:\n{}", compiled.functions),
        Err(message) => message,
    }
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "missing `{needle}` in:\n---\n{haystack}\n---"
    );
}

fn assert_functions(src: &str, expected: &str) {
    let compiled = generated(src);
    assert_eq!(expected, compiled.functions, "for source: {src}");
}

// ── arithmetic, operators, precedence ────────────────────────────────
