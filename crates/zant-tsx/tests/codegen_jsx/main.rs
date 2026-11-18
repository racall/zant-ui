//! JSX code generation tests (`jsx.rs`, `jsx_values.rs`, `jsx_children.rs`,
//! `rich_text.rs`, `list.rs`, `events.rs`).
//!
//! These tests compile small TSX components end to end (lexer → parser →
//! `events::scan` → `jsx::emit` → `scope::finish`), assert hand-reviewed Rust
//! snippets, and require every generated module to pass `syn::parse_file`.
//!
//! The component layer is not under test here: the harness installs the
//! component state and scans the render expression itself, which is what the
//! component entry point does before calling the `hooks.emit_jsx` hook.

use zant_tsx::ast;
use zant_tsx::codegen::types::Type;
use zant_tsx::codegen::{self, Generator, StateKind, events, jsx, model, scope};
use zant_tsx::parser::Parser;

mod advanced;
mod elements;
mod props;

struct Generated {
    /// The whole Rust module (header + component + handlers).
    module: String,
    /// The generated `render` body, before formatting.
    render: String,
    /// Registered setter adapters (`_set_name` …).
    methods: String,
}

impl Generated {
    fn assert_parses(&self) -> &Generated {
        let formatted = codegen::parse_and_format(&self.module);
        // The formatted output must still carry the same render body.
        assert!(
            formatted.contains("fn render"),
            "formatted module lost the render function:\n{formatted}"
        );
        self
    }

    fn assert_render(&self, expected: &str) -> &Generated {
        self.assert_parses();
        assert!(
            self.render.contains(expected),
            "missing `{expected}` in generated render:\n{}",
            self.render
        );
        self
    }

    fn assert_methods(&self, expected: &str) -> &Generated {
        self.assert_parses();
        assert!(
            self.methods.contains(expected),
            "missing `{expected}` in generated methods:\n{}",
            self.methods
        );
        self
    }
}

struct StateSpec {
    name: String,
    setter: String,
    ty: Type,
}

fn state(name: &str, setter: &str, ty: Type) -> StateSpec {
    StateSpec {
        name: name.to_owned(),
        setter: setter.to_owned(),
        ty,
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

/// The JSX returned by the first function of the module.
fn render_jsx(module: &ast::Module) -> (ast::Jsx, ast::Loc) {
    for top in &module.functions {
        match &top.func.body {
            ast::Body::Expr(e) => {
                if let ast::ExprData::Jsx(element) = &e.data {
                    return (element.as_ref().clone(), e.loc);
                }
            }
            ast::Body::Block(statements) => {
                for statement in statements {
                    if let ast::StmtData::Ret(Some(e)) = &statement.data
                        && let ast::ExprData::Jsx(element) = &e.data
                    {
                        return (element.as_ref().clone(), e.loc);
                    }
                }
            }
        }
    }
    panic!("no function returning JSX in the test source");
}

/// The generated module for one component: state first, then JSX scanning and
/// emission with the same order the component entry point uses.
fn compile(
    src: &str,
    setup: impl FnOnce(&mut Generator<'_>) -> codegen::Result<()>,
) -> Result<Generated, String> {
    let module = parse_module(src);
    let (element, loc) = render_jsx(&module);
    let mut diag = None;
    let result = {
        let mut g = Generator::new(&mut diag);
        g.name = "App".to_owned();
        g.type_decls = module.types.clone();
        g.resolving = vec![false; module.types.len()];
        g.react_imports = module.imports.clone();
        (|| -> codegen::Result<Generated> {
            // The hooks the component entry point installs.
            g.extensions.hooks.emit_jsx = Some(jsx::emit);
            // Event hooks: `event_value` emits a handler whose capture
            // struct is known before it is used, so the method pass can run.
            g.extensions.hooks.scan_body = Some(events::scan_body);
            g.extensions.hooks.scan_expr = Some(events::scan_expr);
            g.extensions.hooks.emit_setter = Some(events::emit_setter);
            g.extensions.hooks.method_param = Some(events::method_param_type);
            g.extensions.hooks.parameter_type = Some(events::parameter_type);
            g.extensions.hooks.captures_read = Some(codegen::captures::read);
            g.extensions.hooks.nested = Some(codegen::captures::nested);
            setup(&mut g)?;
            let expr = ast::Expr {
                loc,
                data: ast::ExprData::Jsx(Box::new(element)),
            };
            events::scan_expr(&mut g, &expr)?;
            let ast::ExprData::Jsx(element) = &expr.data else {
                unreachable!("the harness wraps JSX")
            };
            let mut c = scope::Context {
                render: true,
                ret: Type::Node,
                ..scope::Context::default()
            };
            let value = jsx::emit(&mut g, &mut c, element)?;
            let render = scope::finish(&mut g, &c, value.code)?;
            // Arrow handler bodies are emitted by the component layer's method
            // pass; the setter adapters are part of `events.rs` and generated here.
            let mut methods = String::new();
            let registered = g.methods.clone();
            for method in &registered {
                if method.setter.is_some() {
                    methods.push_str(&events::emit_setter(&mut g, method)?);
                }
            }
            let declarations = model::declarations(&g);
            let header = "#![allow(clippy::redundant_clone)]\n\
                          #![allow(clippy::too_many_arguments)]\n\
                          use zant::nodes;\nuse zant::tsx;\nuse zant::ui;\n\n";
            let body = format!(
                "{declarations}pub struct App {{}}\n\nimpl App {{\n    pub fn render(&mut self) -> ui::Node {{\n{render}\n    }}\n{methods}}}\n"
            );
            Ok(Generated {
                module: format!("{header}{body}"),
                render,
                methods,
            })
        })()
    };
    result.map_err(|_| match diag {
        Some(diag) => format!("{} @{}", diag.msg, diag.loc),
        None => "codegen failed without a diagnostic".to_owned(),
    })
}

fn compiled(src: &str, states: Vec<StateSpec>) -> Generated {
    match compile(src, |g| {
        for state in states {
            g.states.push(codegen::State {
                name: state.name,
                setter: state.setter,
                ty: state.ty,
                initial: String::new(),
                kind: StateKind::State,
            });
        }
        Ok(())
    }) {
        Ok(generated) => {
            generated.assert_parses();
            generated
        }
        Err(message) => panic!("unexpected diagnostic: {message}"),
    }
}

fn diagnostic(src: &str, states: Vec<StateSpec>) -> String {
    match compile(src, |g| {
        for state in states {
            g.states.push(codegen::State {
                name: state.name,
                setter: state.setter,
                ty: state.ty,
                initial: String::new(),
                kind: StateKind::State,
            });
        }
        Ok(())
    }) {
        Ok(generated) => panic!("expected a diagnostic, generated:\n{}", generated.render),
        Err(message) => message,
    }
}

fn number_state(name: &str, setter: &str, ty: Type) -> StateSpec {
    state(name, setter, ty)
}

// ── golden components ────────────────────────────────────────────────
