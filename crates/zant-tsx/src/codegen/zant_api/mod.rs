//! Desktop and platform functions imported from `"zant"` (docs/tsx.md,
//! "桌面函数"):
//!
//! ```tsx
//! import { clipboard, app, mainWindow, wakeAfter, keyLabel, isMacos } from "zant";
//! clipboard.setText(text);  app.quit();  mainWindow.show();
//! const hint = keyLabel("mod+f");        // "⌘F" / "Ctrl+F"
//! ```
//!
//! One table maps each TSX function to its wrapper in `zant::tsx::platform`
//! (`tsx::platform::*`). Functions that change something (the clipboard, a
//! window, the app) run in handlers and effects, not while rendering.

use crate::ast;

use super::builtins;
use super::expr;
use super::model;
use super::rust_imports;
use super::rust_text;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod calls;
mod table;

use self::calls::*;
use self::table::*;

/// The names `"zant"` exports from this table (for the import check).
pub fn exports(name: &str) -> bool {
    NAMESPACES.iter().any(|ns| ns.name == name)
        || FUNCTIONS.iter().any(|function| function.name == name)
        || CONSTANTS.iter().any(|(constant, _)| *constant == name)
        || name == "keyLabel"
        || name == "titlebarInsets"
        || super::i18n_calls::NAMES.contains(&name)
}

/// `name` is imported from `"zant"` in this file and not shadowed.
fn visible(g: &mut Generator<'_>, c: &mut scope::Context, name: &str) -> bool {
    if builtins::shadowed(g, c, name) {
        return false;
    }
    g.react_imports.iter().any(|import| {
        String::from_utf8_lossy(&import.source) == "zant"
            && !import.type_only
            && import
                .names
                .iter()
                .any(|n| !n.type_only && String::from_utf8_lossy(&n.local.name) == name)
    })
}

/// `isMacos` / `isWindows`; a namespace used as a value is an error.
pub fn read(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    name: &str,
    loc: ast::Loc,
) -> Result<Option<Value>> {
    if !visible(g, c, name) {
        return Ok(None);
    }
    if let Some((_, code)) = CONSTANTS.iter().find(|(constant, _)| *constant == name) {
        return Ok(Some(Value::new(Type::Boolean, *code)));
    }
    if let Some(namespace) = NAMESPACES.iter().find(|namespace| namespace.name == name) {
        return fail(
            g,
            loc,
            format!(
                "`{name}` is a namespace; call its functions: {}",
                names(namespace)
            ),
        );
    }
    fail(g, loc, format!("`{name}` is a function; call it"))
}

/// `clipboard.text()`, `app.quit()`, `wakeAfter(ms)`, `keyLabel("mod+f")`, …
pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    match &call.callee.data {
        ast::ExprData::Ident(name) => {
            let name = String::from_utf8_lossy(name).into_owned();
            if !visible(g, c, &name) {
                return Ok(None);
            }
            if name == "keyLabel" {
                return key_label(g, loc, call).map(Some);
            }
            if super::i18n_calls::NAMES.contains(&name.as_str()) {
                return super::i18n_calls::call(g, c, loc, &name, call).map(Some);
            }
            if name == "titlebarInsets" {
                return titlebar_insets(g, c, loc, call).map(Some);
            }
            if let Some(function) = FUNCTIONS.iter().find(|function| function.name == name) {
                return emit_call(g, c, loc, call, function, function.rust).map(Some);
            }
            Ok(None)
        }
        ast::ExprData::Member(member) => {
            let ast::ExprData::Ident(object) = &member.object.data else {
                return Ok(None);
            };
            let object = String::from_utf8_lossy(object).into_owned();
            if !visible(g, c, &object) {
                return Ok(None);
            }
            let Some(namespace) = NAMESPACES.iter().find(|namespace| namespace.name == object)
            else {
                return Ok(None);
            };
            let method = String::from_utf8_lossy(&member.name.name).into_owned();
            let Some(function) = namespace
                .functions
                .iter()
                .find(|function| function.name == method)
            else {
                return fail(
                    g,
                    member.name.loc,
                    format!(
                        "unknown function `{object}.{method}`; {object} has: {}",
                        names(namespace)
                    ),
                );
            };
            emit_call(
                g,
                c,
                loc,
                call,
                function,
                &format!("{}::{}", namespace.rust, function.rust),
            )
            .map(Some)
        }
        _ => Ok(None),
    }
}

/// Keeps `model::InfoKind` reachable for tests that build compounds.
#[allow(dead_code)]
fn _info(_: model::InfoKind) {}
