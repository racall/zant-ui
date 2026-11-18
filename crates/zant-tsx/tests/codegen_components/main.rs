//! Golden tests: components, state, hooks, stores and array methods.
//!
//! Most tests run the real generator. Where a test isolates the component
//! layer from the JSX / event emitters (`codegen/jsx.rs` / `events.rs`), it
//! installs a small **test-only** stand-in ([`install_stubs`]) that
//!
//! - emits `ui::text("stub", …)` for every element (so the module parses and
//!   the render statements around it are generated), and
//! - registers inline event arrows / setters the way `events.rs` does, so the
//!   handler methods of the component are exercised.
//!
//! Everything the component layer emits — the struct, `impl Default`, the setter
//! and handler methods, `impl ui::App`, `build` + `ui::local_state`, memo /
//! effect / ref / store lowering and the array adapters — is real generated
//! code. The render body itself is covered by the JSX tests
//! (`tests/codegen_jsx.rs`).

use zant_tsx::ast;
use zant_tsx::codegen::{self, components, generate, scope};
use zant_tsx::parser::Parser;
use zant_tsx::tags;

mod props;
mod state;

use self::props::*;

const STUB_NODE: &str = "ui::text(zant::Str::from(\"stub\"), ui::TextOptions::default())";

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

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The test-only JSX emitter: a stub node plus handler registration.
fn stub_jsx(
    g: &mut codegen::Generator<'_>,
    _c: &mut scope::Context,
    j: &ast::Jsx,
) -> codegen::Result<codegen::types::Value> {
    for attr in &j.attrs {
        let Some(value) = attr.value.as_deref().cloned() else {
            continue;
        };
        let kind = match text(&attr.name.name).as_str() {
            "onClick" => Some(tags::Kind::Action),
            "onChange" => Some(tags::Kind::TextAction),
            "onToggle" | "onChecked" => Some(tags::Kind::BoolAction),
            "onSelect" => Some(tags::Kind::IndexAction),
            _ => None,
        };
        let Some(kind) = kind else {
            // props are lowered by `jsx.rs`; emitting their values
            // here keeps the expression codegen (reads, clones, array calls)
            // under test.
            let _ = codegen::expr::emit(g, _c, &value, None, false)?;
            continue;
        };
        register_handler(g, &value, kind)?;
    }
    for child in &j.children {
        match child {
            ast::Child::Element(element) => {
                stub_jsx(g, _c, element)?;
            }
            ast::Child::Expr(Some(value)) => {
                let _ = codegen::expr::emit(g, _c, value, None, false)?;
            }
            _ => {}
        }
    }
    Ok(codegen::types::Value::new(
        codegen::types::Type::Node,
        STUB_NODE,
    ))
}

/// `events.rs`'s arrow registration / setter reuse, reduced to what the stub
/// emitter needs to exercise the component methods.
fn register_handler(
    g: &mut codegen::Generator<'_>,
    value: &ast::Expr,
    kind: tags::Kind,
) -> codegen::Result<()> {
    match &value.data {
        ast::ExprData::Arrow(function) => {
            g.counter += 1;
            let name = scope::unique_method(g, &format!("_on_click{}", g.counter))?;
            let capture = scope::unique_method(g, &format!("_Capture{}", g.counter))?;
            g.methods.push(codegen::Method {
                name,
                func: Some(function.clone()),
                action: Some(kind),
                capture_name: Some(capture),
                ..codegen::Method::default()
            });
            Ok(())
        }
        ast::ExprData::Ident(name) => {
            if let Some(index) = scope::setter_index(g, name) {
                let already = g
                    .methods
                    .iter()
                    .any(|method| method.setter == Some(index) && method.action == Some(kind));
                if !already {
                    let setter = scope::unique_method(g, &format!("_{}", text(name)))?;
                    g.methods.push(codegen::Method {
                        name: setter,
                        setter: Some(index),
                        action: Some(kind),
                        ..codegen::Method::default()
                    });
                }
            } else if let Some(index) = scope::source_method(g, name) {
                g.methods[index].action = Some(kind);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// `events::emit_setter` for the stub harness (the setter adapter becomes a
/// private method).
fn stub_setter(g: &mut codegen::Generator<'_>, m: &codegen::Method) -> codegen::Result<String> {
    let state = g.states[m.setter.expect("setter")].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    let value = match m.action {
        Some(tags::Kind::IndexAction) if state.ty.integer() => "value as _".to_owned(),
        Some(tags::Kind::IndexAction) => "value as _".to_owned(),
        _ => "value".to_owned(),
    };
    let ty = match m.action {
        Some(tags::Kind::TextAction) => "&str",
        Some(tags::Kind::BoolAction) => "bool",
        Some(tags::Kind::IndexAction) => "usize",
        _ => "()",
    };
    let value = if ty == "&str" {
        format!("Str::copy({value})")
    } else if state.ty.integer() && ty == "usize" {
        format!("{value} as {}", state.ty.rust(g))
    } else {
        value
    };
    let name = zant_tsx::codegen::rust_text::id_at(g, m.name.as_bytes(), 0)?;
    Ok(format!(
        "fn {name}(&mut self, value: {ty}) {{\nlet _ = value;\nself.{field} = {value};\n}}\n"
    ))
}

fn install_stubs(g: &mut codegen::Generator<'_>) {
    g.extensions.hooks.emit_jsx = Some(stub_jsx);
    g.extensions.hooks.emit_setter = Some(stub_setter);
}

fn generated(src: &str) -> generate::GeneratedFile {
    // The real JSX / event emitters: no stub.
    let module = parse_module(src);
    match generate::generate("src/App.tsx", &module, &generate::Options::default()) {
        Ok(file) => file,
        Err(_) => match generate::last_diag() {
            Some(diag) => panic!("codegen error at {}: {}", diag.loc, diag.msg),
            None => panic!("codegen error without a diagnostic"),
        },
    }
}
