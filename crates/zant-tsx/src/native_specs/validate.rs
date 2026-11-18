//! Checking a native spec module: only declared functions and named types.

use super::*;

/// A spec has types and declared functions only; the implementation is Rust.
/// Object and string-union types must be named so both sides can refer to
/// them.
pub fn validate(g: &mut Generator<'_>, module: &ast::Module, name: &str) -> Result<()> {
    if let Err(diag) = crate::codegen::generics::validate(module) {
        return fail(g, diag.loc, diag.msg);
    }
    if let Some(top) = module.functions.first() {
        return fail(
            g,
            top.func.loc,
            format!(
                "a spec declares functions (`export declare function {}(…): …;`); write them in native/{name}.rs",
                String::from_utf8_lossy(&top.func.name)
            ),
        );
    }
    if let Some(constant) = module.consts.first() {
        return fail(
            g,
            constant.name.loc,
            format!(
                "a spec has types and declared functions only; constants belong in native/{name}.rs"
            ),
        );
    }
    for top in &module.declared {
        if !top.exported {
            return fail(
                g,
                top.func.loc,
                format!(
                    "export the declared function: `export declare function {}`",
                    String::from_utf8_lossy(&top.func.name)
                ),
            );
        }
    }
    // Rust names every type the implementation uses: no inline objects or
    // enums (arrays and `| null` of named types are fine)
    for decl in &module.types {
        let Some(detail) = decl.ty.detail.as_deref() else {
            continue;
        };
        match detail {
            ast::TypeDetail::Object(fields) => {
                for field in fields {
                    require_named(g, &field.ty)?;
                }
            }
            ast::TypeDetail::Union(members) => {
                for member in members {
                    let Some(detail) = member.detail.as_deref() else {
                        continue;
                    };
                    match detail {
                        ast::TypeDetail::StringLit(_) | ast::TypeDetail::Null => {}
                        _ => require_named(g, member)?,
                    }
                }
            }
            ast::TypeDetail::Intersection(members) => {
                for member in members {
                    match member.detail.as_deref() {
                        Some(ast::TypeDetail::Object(fields)) => {
                            for field in fields {
                                require_named(g, &field.ty)?;
                            }
                        }
                        _ => require_named(g, member)?,
                    }
                }
            }
            _ => require_named(g, &decl.ty)?,
        }
    }
    for top in &module.declared {
        for param in &top.func.params {
            if let Some(ty) = param.ty.as_ref() {
                require_named(g, ty)?;
            }
        }
        if let Some(ty) = top.func.ret.as_ref() {
            require_named(g, ty)?;
        }
    }
    Ok(())
}

/// `ty` names its object and string-union types.
pub(super) fn require_named(g: &mut Generator<'_>, ty: &ast::TypeRef) -> Result<()> {
    let Some(detail) = ty.detail.as_deref() else {
        return Ok(());
    };
    match detail {
        ast::TypeDetail::Array(item) => require_named(g, item),
        ast::TypeDetail::Union(members) => {
            let mut others = 0usize;
            for member in members {
                if matches!(member.detail.as_deref(), Some(ast::TypeDetail::Null)) {
                    continue;
                }
                others += 1;
                require_named(g, member)?;
            }
            if others > 1 {
                return unnamed(g, ty, "a union");
            }
            Ok(())
        }
        ast::TypeDetail::StringLit(_) => unnamed(g, ty, "a union"),
        ast::TypeDetail::Object(_) => unnamed(g, ty, "an object type"),
        ast::TypeDetail::Intersection(_) => unnamed(g, ty, "an intersection"),
        ast::TypeDetail::Keyof(_)
        | ast::TypeDetail::Typeof(_)
        | ast::TypeDetail::Indexed(..)
        | ast::TypeDetail::NumberLit(_) => fail(
            g,
            ty.loc,
            "native specs use named types; declare this type with a name",
        ),
        ast::TypeDetail::Tuple(_) => fail(
            g,
            ty.loc,
            "native functions take named types; use an interface instead of a tuple",
        ),
        ast::TypeDetail::Func(_) => fail(
            g,
            ty.loc,
            "native functions take and return data, not callbacks",
        ),
        ast::TypeDetail::Null => Ok(()),
    }
}

pub(super) fn unnamed<T>(g: &mut Generator<'_>, ty: &ast::TypeRef, what: &str) -> Result<T> {
    fail(
        g,
        ty.loc,
        format!(
            "name {what} the Rust side uses: `export type Name = …;` / `export interface Name {{ … }}`"
        ),
    )
}
