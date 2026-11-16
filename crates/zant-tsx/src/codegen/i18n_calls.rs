//! The i18n functions TSX imports from `"zant"`: `t`, `useLocale`,
//! `setLocale`, `locales`, `localeName` and `localePreference`.
//!
//! `t("home.welcome", { name })` is checked against the project's catalogs
//! at compile time — the key must exist in the default language and the
//! parameters must match its message — and lowers to a call by key id
//! (`zant::i18n::context::format(3, &[Arg::Str(..)])`). A key may also be a
//! value of a string-literal union whose members are all keys with the same
//! parameters (`type Status = "status.open" | "status.done"`), lowered to a
//! `match` over the union's variants.

use crate::ast;
use crate::i18n::contract::{Contract, ParamKind};

use super::app_state;
use super::expr;
use super::model;
use super::rust_text;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// The project's message keys with their contracts (key id = index).
#[derive(Clone, Debug, Default)]
pub struct Index {
    pub keys: Vec<(String, Contract)>,
    pub locales: Vec<String>,
    pub default: String,
}

/// The i18n names `"zant"` exports.
pub const NAMES: [&str; 6] = [
    "t",
    "useLocale",
    "setLocale",
    "locales",
    "localeName",
    "localePreference",
];

const CONTEXT: &str = "zant::i18n::context";

/// A call of one of [`NAMES`] (already known to be imported from `"zant"`).
pub fn call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    name: &str,
    call: &ast::Call,
) -> Result<Value> {
    if c.constant {
        return fail(
            g,
            loc,
            format!(
                "`{name}` reads the current language: call it while rendering or in a handler, not in a top-level constant"
            ),
        );
    }
    let arity = |g: &mut Generator<'_>, count: usize| -> Result<()> {
        if call.args.len() != count {
            return fail(g, loc, format!("`{name}` takes {count} arguments"));
        }
        Ok(())
    };
    match name {
        "t" => translate(g, c, loc, call),
        "useLocale" | "localePreference" => {
            arity(g, 0)?;
            let function = if name == "useLocale" {
                "locale"
            } else {
                "preference"
            };
            Ok(Value::new(Type::String, format!("{CONTEXT}::{function}()")))
        }
        "locales" => {
            arity(g, 0)?;
            let ty = model::array(g, Type::String)?;
            Ok(Value::new(
                ty,
                format!("tsx::List::from_vec({CONTEXT}::locales())"),
            ))
        }
        "localeName" | "setLocale" => {
            arity(g, 1)?;
            if name == "setLocale" && c.render {
                return fail(
                    g,
                    loc,
                    "`setLocale` changes every window: call it in a handler or an effect, not while rendering",
                );
            }
            let value = expr::emit(g, c, &call.args[0], Some(Type::String), false)?;
            let value = types::coerce(g, value, Type::String, call.args[0].loc)?;
            let (function, ty) = if name == "setLocale" {
                ("set_locale", Type::Boolean)
            } else {
                ("name", Type::String)
            };
            Ok(Value::new(
                ty,
                format!("{CONTEXT}::{function}(({}).as_str())", value.code),
            ))
        }
        _ => unreachable!("i18n function"),
    }
}

fn index(g: &mut Generator<'_>, loc: ast::Loc) -> Result<std::rc::Rc<Index>> {
    match app_state::with_project(|project| project.and_then(|project| project.i18n.clone())) {
        Some(index) => Ok(index),
        None => fail(
            g,
            loc,
            "`t` needs translations: add locales/<language>.json (for example locales/en.json) to the app",
        ),
    }
}

/// `t(key, params?)`.
fn translate(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Value> {
    if call.args.is_empty() || call.args.len() > 2 {
        return fail(
            g,
            loc,
            "`t` takes a key and, when the message has parameters, an object: t(\"home.title\") / t(\"home.welcome\", { name })",
        );
    }
    let index = index(g, loc)?;
    let key = &call.args[0];
    let (id_code, contract) = key_id(g, c, &index, key)?;
    let params = call.args.get(1);
    if contract.params.is_empty() {
        if let Some(params) = params {
            return fail(
                g,
                params.loc,
                "this message has no parameters; remove the object",
            );
        }
        return Ok(Value::new(
            Type::String,
            format!("{CONTEXT}::text({id_code})"),
        ));
    }
    let Some(params) = params else {
        let names: Vec<&str> = contract
            .params
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        return fail(
            g,
            key.loc,
            format!("this message needs {{ {} }}", names.join(", ")),
        );
    };
    let ast::ExprData::Object(properties) = &params.data else {
        return fail(
            g,
            params.loc,
            "pass the message's parameters as an object literal: t(\"key\", { name, count })",
        );
    };
    let mut setup = String::new();
    let mut args = Vec::new();
    for (position, param) in contract.params.iter().enumerate() {
        let Some(property) = properties
            .iter()
            .rev()
            .find(|property| !property.spread && property.key.name == param.name.as_bytes())
        else {
            return fail(g, params.loc, format!("missing parameter `{}`", param.name));
        };
        let ty = match param.kind {
            ParamKind::String => Type::String,
            ParamKind::Number => Type::F64,
            // shown as text: a string or a number, as the caller has it
            ParamKind::Text => {
                let value = expr::emit(g, c, &property.value, None, false)?;
                if value.ty == Type::String {
                    Type::String
                } else if value.ty.numeric() {
                    Type::F64
                } else {
                    return fail(
                        g,
                        property.value.loc,
                        format!(
                            "`{}` is shown as text: pass a string or a number",
                            param.name
                        ),
                    );
                }
            }
        };
        let wrap = if ty == Type::String { "Str" } else { "Num" };
        let value = expr::emit(g, c, &property.value, Some(ty), false)?;
        let value = types::coerce(g, value, ty, property.value.loc)?;
        let temp = format!("_t{position}");
        setup.push_str(&format!("let {temp}: {} = {};\n", ty.rust(g), value.code));
        args.push(if wrap == "Str" {
            format!("zant::i18n::format::Arg::Str({temp}.as_str())")
        } else {
            format!("zant::i18n::format::Arg::Num({temp})")
        });
    }
    for property in properties {
        if property.spread {
            return fail(
                g,
                property.value.loc,
                "list the parameters one by one; `...` is not supported here",
            );
        }
        if !contract
            .params
            .iter()
            .any(|param| param.name.as_bytes() == property.key.name.as_slice())
        {
            return fail(
                g,
                property.key.loc,
                format!(
                    "`{}` is not a parameter of this message",
                    String::from_utf8_lossy(&property.key.name)
                ),
            );
        }
    }
    Ok(Value::new(
        Type::String,
        format!(
            "{{\n{setup}{CONTEXT}::format({id_code}, &[{}])\n}}",
            args.join(", ")
        ),
    ))
}

/// The key id (code) and contract of `t`'s first argument.
fn key_id(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    index: &Index,
    key: &ast::Expr,
) -> Result<(String, Contract)> {
    if let ast::ExprData::String(text) = &key.data {
        let (id, contract) = lookup(g, index, text, key.loc)?;
        return Ok((id.to_string(), contract));
    }
    let value = expr::emit(g, c, key, None, false)?;
    let Some(info) =
        model::info(g, value.ty).filter(|info| info.kind == model::InfoKind::Enumeration)
    else {
        return fail(
            g,
            key.loc,
            "`t` keys are string literals, or a value of a union of literal keys (`type Status = \"status.open\" | \"status.done\"`)",
        );
    };
    let mut arms = String::new();
    let mut shared: Option<Contract> = None;
    for member in &info.members {
        let (id, contract) = lookup(g, index, member, key.loc)?;
        match &shared {
            Some(first)
                if first
                    .params
                    .iter()
                    .map(|p| (&p.name, p.kind))
                    .ne(contract.params.iter().map(|p| (&p.name, p.kind))) =>
            {
                return fail(
                    g,
                    key.loc,
                    format!(
                        "the keys of this union need the same parameters (`{}` differs); call `t` per key instead",
                        String::from_utf8_lossy(member)
                    ),
                );
            }
            Some(_) => {}
            None => shared = Some(contract),
        }
        let Some(variant) = rust_text::enum_variant(member) else {
            return fail(g, key.loc, "this key cannot be a union member");
        };
        arms.push_str(&format!("{}::{variant} => {id},\n", info.rust));
    }
    let code = format!("match {} {{\n{arms}}}", value.code);
    Ok((code, shared.unwrap_or(Contract { params: Vec::new() })))
}

fn lookup(
    g: &mut Generator<'_>,
    index: &Index,
    key: &[u8],
    loc: ast::Loc,
) -> Result<(usize, Contract)> {
    let text = String::from_utf8_lossy(key);
    if let Some(id) = index.keys.iter().position(|(name, _)| *name == text) {
        return Ok((id, index.keys[id].1.clone()));
    }
    let suggestion = index
        .keys
        .iter()
        .map(|(name, _)| (model::edit_distance(name.as_bytes(), key), name))
        .filter(|(distance, _)| *distance <= 3)
        .min()
        .map(|(_, name)| format!("; did you mean `{name}`?"))
        .unwrap_or_default();
    fail(
        g,
        loc,
        format!(
            "unknown message key `{text}` (not in locales/{}.json){suggestion}",
            index.default
        ),
    )
}
