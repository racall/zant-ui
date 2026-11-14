//! File-level constants and type declarations, resolved before component
//! state. Translation of `tools/tsx/codegen/globals.zig`.
//!
//! Asset imports (`import icon from "../assets/icon.png"`) belong to the build
//! layer (WP-5.6): [`assets`] is a hook the entry point calls, and without it
//! asset imports are a diagnostic.

use crate::ast;

use super::expr;
use super::model;
use super::rust_text;
use super::scope;
use super::types::{self, Type};
use super::{Generator, Result, fail};

pub fn collect(g: &mut Generator<'_>, constants: &[&ast::TopConst]) -> Result<String> {
    let mut code = String::new();
    let mut c = scope::Context {
        render: true,
        ret: Type::Void,
        constant: true,
        ..scope::Context::default()
    };
    for constant in constants {
        let name = String::from_utf8_lossy(&constant.name.name).into_owned();
        if scope::global_name(g, constant.name.name.as_slice()) {
            return fail(
                g,
                constant.name.loc,
                format!("duplicate or reserved constant `{name}`"),
            );
        }
        let expected = match constant.ty.as_ref() {
            Some(reference) => Some(types::type_ref(g, reference, false)?),
            None => None,
        };
        let mut value = expr::emit(g, &mut c, &constant.value, expected, false)?;
        let concrete = expected.unwrap_or_else(|| value.ty.concrete());
        value = types::coerce(g, value, concrete, constant.value.loc)?;
        let id = rust_text::id_at(g, &constant.name.name, 0)?;
        code.push_str(&format!(
            "{}const {id}: {} = {};\n\n",
            if constant.exported { "pub " } else { "" },
            value.ty.rust(g),
            value.code
        ));
        value.code = id;
        value.inference = types::Inference::Known;
        value.text = None;
        value.formatted = None;
        g.constants.push(crate::codegen::Constant { name, value });
    }
    Ok(code)
}

/// `import icon from "../assets/icon.png"` (WP-5.6 `assets.zig`).
pub fn assets(
    g: &mut Generator<'_>,
    module: &ast::Module,
    _source_path: Option<&str>,
) -> Result<String> {
    for import in &module.imports {
        let source = String::from_utf8_lossy(&import.source);
        if import.default_name.is_some()
            && !source.starts_with('.')
            && source != "zant"
            && source != "react"
            && !source.contains('/')
        {
            continue;
        }
        if is_asset(&source) {
            return fail(
                g,
                import.source_loc,
                "asset imports need a project build (zant_build::tsx_app, or zant-tsx --out)",
            );
        }
    }
    Ok(String::new())
}

/// Whether an import names an image asset (Zig `assets.isAsset`).
pub fn is_asset(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"]
        .iter()
        .any(|extension| lower.ends_with(extension))
}

/// Anonymous nested types have stable file-level names too.
pub fn declarations(g: &Generator<'_>) -> String {
    model::declarations(g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse_module(src: &str) -> ast::Module {
        let mut parser = Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    #[test]
    fn constants_carry_their_declared_type() {
        let module = parse_module("export const limit: u32 = 10;\n");
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        g.resolving = vec![false; module.types.len()];
        let refs: Vec<&ast::TopConst> = module.consts.iter().collect();
        let code = collect(&mut g, &refs).expect("constants lower");
        assert!(code.contains("pub const limit: u32 ="), "{code}");
        assert_eq!(1, g.constants.len());
    }

    #[test]
    fn image_imports_are_assets() {
        assert!(is_asset("../assets/icon.png"));
        assert!(is_asset("logo.svg"));
        assert!(!is_asset("./TodoRow.tsx"));
        assert!(!is_asset("zant"));
    }
}
