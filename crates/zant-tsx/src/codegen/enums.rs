//! Reading an `enum` member (`Mode.Dark`, `Level.High`): inlined as its
//! value, as tsc does for `const enum` — the string-literal union's variant
//! for a string enum, an `f64` for a numeric one (see `enum_parser`).
//! Enums of this file and enums imported from another file both work.

use crate::ast;

use super::app_state;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// The enum `name` refers to here: declared in this file, or imported.
fn find(g: &Generator<'_>, name: &[u8]) -> Option<ast::EnumDecl> {
    if let Some(decl) = g.enums.iter().find(|decl| decl.name.name == name) {
        return Some(decl.clone());
    }
    let (source, imported) = g.react_imports.iter().find_map(|import| {
        import
            .names
            .iter()
            .find(|entry| entry.local.name == name)
            .map(|entry| (import.source.clone(), entry.imported.name.clone()))
    })?;
    let source = String::from_utf8_lossy(&source).into_owned();
    app_state::with_project(|project| {
        let module = project?.find_module(&source)?;
        module
            .module
            .enums
            .iter()
            .find(|decl| decl.exported && decl.name.name == imported)
            .cloned()
    })
}

/// `E.Member` when `E` names an enum (and no local shadows it).
pub fn member(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    m: &ast::Member,
) -> Result<Option<Value>> {
    let ast::ExprData::Ident(name) = &m.object.data else {
        return Ok(None);
    };
    if scope::local(c, name).is_some() {
        return Ok(None);
    }
    let Some(decl) = find(g, name) else {
        return Ok(None);
    };
    let Some(member) = decl
        .members
        .iter()
        .find(|member| member.name.name == m.name.name)
    else {
        let names: Vec<String> = decl
            .members
            .iter()
            .map(|member| String::from_utf8_lossy(&member.name.name).into_owned())
            .collect();
        return fail(
            g,
            m.name.loc,
            format!(
                "`{}` has no member `{}`; members: {}",
                String::from_utf8_lossy(name),
                String::from_utf8_lossy(&m.name.name),
                names.join(", ")
            ),
        );
    };
    let reference = ast::TypeRef {
        name: name.clone(),
        loc: m.name.loc,
        detail: None,
        args: Vec::new(),
    };
    let ty = types::type_ref(g, &reference, false)?;
    let value = match &member.value {
        ast::EnumValue::Number(number) => Value::new(Type::F64, format!("({number}f64)")),
        ast::EnumValue::Text(text) => {
            let literal = Value {
                ty: Type::String,
                code: String::new(),
                text: Some(text.clone()),
                ..Value::default()
            };
            types::coerce(g, literal, ty, m.name.loc)?
        }
    };
    Ok(Some(value))
}
