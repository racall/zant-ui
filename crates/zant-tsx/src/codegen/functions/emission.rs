//! Emitting the file's functions as Rust functions.

use super::*;

/// The Rust declarations of this file's functions.
pub fn emit_all(g: &mut Generator<'_>) -> Result<String> {
    let mut out = String::new();
    // generic instances may be added while bodies are generated
    let mut index = g.emitted_functions;
    while index < g.functions.len() {
        if !g.functions[index].external {
            out.push_str(&emit(g, index)?);
            out.push('\n');
        }
        index += 1;
    }
    g.emitted_functions = index;
    Ok(out)
}

pub(super) fn emit(g: &mut Generator<'_>, index: usize) -> Result<String> {
    let env = g.functions[index].type_env.clone();
    crate::codegen::generic_types::with_env(g, env, |g| emit_function(g, index))
}

pub(super) fn emit_function(g: &mut Generator<'_>, index: usize) -> Result<String> {
    let function = g.functions[index].clone();
    let mut c = scope::Context {
        render: false,
        ret: function.ret,
        no_self: true,
        throw_mode: if function.throws {
            crate::codegen::exceptions::ThrowMode::Propagate
        } else {
            crate::codegen::exceptions::ThrowMode::Forbid
        },
        ..scope::Context::default()
    };
    let mut params: Vec<String> = Vec::new();
    for (param, ty) in function.func.params.iter().zip(function.params.iter()) {
        let binding = scope::bind(g, &mut c, &param.name.name, *ty, false)?;
        c.params.push(binding);
        params.push(format!("{}: {}", c.bindings[binding].rust, ty.rust(g)));
    }
    let body = match &function.func.body {
        ast::Body::Block(ss) => {
            // checked after generating: an exhaustive switch is known then
            let body = stmt::statements(g, &mut c, ss, false)?;
            if function.ret != Type::Void && !stmt::returns_all(ss) {
                return fail(
                    g,
                    function.func.loc,
                    format!("`{}` must return a value on every path", function.name),
                );
            }
            body
        }
        ast::Body::Expr(e) => {
            if function.ret == Type::Void {
                stmt::expression_statement(g, &mut c, e)?
            } else {
                let value = expr::emit(g, &mut c, e, Some(function.ret), false)?;
                match crate::codegen::exceptions::return_code(&c, Some(&value.code)) {
                    Some(code) => code,
                    None => format!("return {};\n", value.code),
                }
            }
        }
    };
    let mut finished = scope::finish(g, &c, body)?;
    let return_type = match (function.ret, function.throws) {
        (Type::Void, false) => String::new(),
        (Type::Void, true) => {
            // falling off the end of a throwing void function
            let ends = matches!(&function.func.body, ast::Body::Block(ss) if stmt::returns_all(ss));
            if !ends {
                finished.push_str("Ok(())\n");
            }
            " -> Result<(), zant::tsx::Thrown>".to_owned()
        }
        (ret, false) => format!(" -> {}", ret.rust(g)),
        (ret, true) => format!(" -> Result<{}, zant::tsx::Thrown>", ret.rust(g)),
    };
    Ok(format!(
        "pub fn {}({}){return_type} {{\n{finished}}}\n",
        function.rust,
        params.join(", ")
    ))
}
