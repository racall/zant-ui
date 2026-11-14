//! Desktop and platform functions imported from `"zant"` (docs/tsx.md,
//! "桌面函数"):
//!
//! ```tsx
//! import { clipboard, app, mainWindow, wakeAfter, keyLabel, isMacos } from "zant";
//! clipboard.setText(text);  app.quit();  mainWindow.show();
//! const hint = keyLabel("mod+f");        // "⌘F" / "Ctrl+F"
//! ```
//!
//! Translation of `tools/tsx/codegen/zant_api.zig`. One table maps each TSX
//! function to its wrapper in `zant::tsx::platform` (`tsx::platform::*`).
//! Functions that change something (the clipboard, a window, the app) run in
//! handlers and effects, not while rendering.

use crate::ast;

use super::builtins;
use super::expr;
use super::model;
use super::rust_imports;
use super::rust_text;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Void,
    String,
    Boolean,
    Number,
    Bytes,
    OptString,
    OptBytes,
    OptImage,
}

struct Function {
    name: &'static str,
    rust: &'static str,
    params: &'static [Kind],
    ret: Kind,
    /// Changes the clipboard, a window or the app: not while rendering.
    effect: bool,
}

struct Namespace {
    name: &'static str,
    rust: &'static str,
    functions: &'static [Function],
}

const NAMESPACES: [Namespace; 3] = [
    Namespace {
        name: "clipboard",
        rust: "tsx::platform::clipboard",
        functions: &[
            Function {
                name: "text",
                rust: "text",
                params: &[],
                ret: Kind::String,
                effect: false,
            },
            Function {
                name: "setText",
                rust: "set_text",
                params: &[Kind::String],
                ret: Kind::Boolean,
                effect: true,
            },
            Function {
                name: "image",
                rust: "image",
                params: &[],
                ret: Kind::OptBytes,
                effect: false,
            },
            Function {
                name: "setImage",
                rust: "set_image",
                params: &[Kind::Bytes],
                ret: Kind::Boolean,
                effect: true,
            },
            Function {
                name: "files",
                rust: "files",
                params: &[],
                ret: Kind::OptString,
                effect: false,
            },
            Function {
                name: "setFiles",
                rust: "set_files",
                params: &[Kind::String],
                ret: Kind::Boolean,
                effect: true,
            },
            Function {
                name: "changeCount",
                rust: "change_count",
                params: &[],
                ret: Kind::Number,
                effect: false,
            },
        ],
    },
    Namespace {
        name: "app",
        rust: "tsx::platform::app",
        functions: &[
            Function {
                name: "quit",
                rust: "quit",
                params: &[],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "launchAtLogin",
                rust: "launch_at_login",
                params: &[],
                ret: Kind::String,
                effect: false,
            },
            Function {
                name: "setLaunchAtLogin",
                rust: "set_launch_at_login",
                params: &[Kind::Boolean],
                ret: Kind::Boolean,
                effect: true,
            },
            Function {
                name: "launchedAtLogin",
                rust: "launched_at_login",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
            Function {
                name: "deviceName",
                rust: "device_name",
                params: &[],
                ret: Kind::String,
                effect: false,
            },
            Function {
                name: "downloadsDir",
                rust: "downloads_dir",
                params: &[],
                ret: Kind::OptString,
                effect: false,
            },
            Function {
                name: "dataDir",
                rust: "data_dir",
                params: &[],
                ret: Kind::OptString,
                effect: false,
            },
        ],
    },
    Namespace {
        name: "mainWindow",
        rust: "tsx::platform::main_window",
        functions: &[
            Function {
                name: "show",
                rust: "show",
                params: &[],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "hide",
                rust: "hide",
                params: &[],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "activate",
                rust: "activate",
                params: &[],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "focus",
                rust: "focus",
                params: &[Kind::String],
                ret: Kind::Void,
                effect: true,
            },
            Function {
                name: "visible",
                rust: "visible",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
            Function {
                name: "active",
                rust: "active",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
            Function {
                name: "minimized",
                rust: "minimized",
                params: &[],
                ret: Kind::Boolean,
                effect: false,
            },
        ],
    },
];

/// Functions imported by name.
const FUNCTIONS: [Function; 3] = [
    Function {
        name: "wakeAfter",
        rust: "tsx::platform::wake_after",
        params: &[Kind::Number],
        ret: Kind::Void,
        effect: false,
    },
    Function {
        name: "shortcutLabel",
        rust: "tsx::platform::shortcut_label",
        params: &[Kind::String],
        ret: Kind::String,
        effect: false,
    },
    Function {
        name: "assetImage",
        rust: "tsx::platform::asset_image",
        params: &[Kind::Bytes],
        ret: Kind::OptImage,
        effect: false,
    },
];

/// Constants imported by name (`tsx::platform::IS_MACOS`).
const CONSTANTS: [(&str, &str); 2] = [
    ("isMacos", "zant::tsx::platform::IS_MACOS"),
    ("isWindows", "zant::tsx::platform::IS_WINDOWS"),
];

/// The names `"zant"` exports from this table (for the import check).
pub fn exports(name: &str) -> bool {
    NAMESPACES.iter().any(|ns| ns.name == name)
        || FUNCTIONS.iter().any(|function| function.name == name)
        || CONSTANTS.iter().any(|(constant, _)| *constant == name)
        || name == "keyLabel"
        || name == "titlebarInsets"
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

fn emit_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    function: &Function,
    rust: &str,
) -> Result<Value> {
    if !call.type_args.is_empty() {
        return fail(
            g,
            loc,
            format!("`{}` takes no type arguments", function.name),
        );
    }
    if call.args.len() != function.params.len() {
        return fail(
            g,
            loc,
            format!(
                "`{}` takes {} argument(s), got {}",
                function.name,
                function.params.len(),
                call.args.len()
            ),
        );
    }
    if c.constant {
        return fail(
            g,
            loc,
            format!("`{}` cannot run in a top-level constant", function.name),
        );
    }
    if function.effect && c.render {
        return fail(
            g,
            loc,
            format!(
                "`{}` changes the desktop; call it in an event handler or an effect, not while rendering",
                function.name
            ),
        );
    }
    let mut arguments: Vec<String> = Vec::new();
    for (argument, kind) in call.args.iter().zip(function.params.iter()) {
        let ty = kind_type(g, *kind)?;
        let value = expr::emit(g, c, argument, Some(ty), false)?;
        arguments.push(types::coerce(g, value, ty, argument.loc)?.code);
    }
    Ok(Value::new(
        kind_type(g, function.ret)?,
        format!("{rust}({})", arguments.join(", ")),
    ))
}

fn kind_type(g: &mut Generator<'_>, kind: Kind) -> Result<Type> {
    Ok(match kind {
        Kind::Void => Type::Void,
        Kind::String => Type::String,
        Kind::Boolean => Type::Boolean,
        Kind::Number => Type::F64,
        Kind::Bytes => rust_imports::bytes(g)?,
        Kind::OptString => model::optional(g, Type::String)?,
        Kind::OptBytes => {
            let bytes = bytes_owned(g)?;
            model::optional(g, bytes)?
        }
        Kind::OptImage => match g.extensions.hooks.image_type {
            Some(hook) => {
                let image = hook(g)?;
                model::optional(g, image)?
            }
            None => {
                return fail(
                    g,
                    0,
                    "`Image` needs the JSX hooks (WP-5.4); `assetImage` returns one",
                );
            }
        },
    })
}

/// `tsx::Bytes` (`Rc<[u8]>`): the runtime's byte buffers (clipboard images).
fn bytes_owned(g: &mut Generator<'_>) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.rust == "tsx::Bytes" {
            return Ok(Type::Compound(index as u32));
        }
    }
    model::add(
        g,
        model::Info {
            kind: model::InfoKind::Array,
            rust: "tsx::Bytes".to_owned(),
            child: Type::U8,
            copy: false,
            ..model::Info::default()
        },
    )
}

/// `keyLabel("mod+shift+s")`: the platform's label (a runtime call in Rust).
fn key_label(g: &mut Generator<'_>, loc: ast::Loc, call: &ast::Call) -> Result<Value> {
    match call.args.first().map(|argument| &argument.data) {
        Some(ast::ExprData::String(value)) if call.args.len() == 1 => Ok(Value::new(
            Type::String,
            format!(
                "tsx::platform::key_label({})",
                rust_text::quote_at(g, value, loc)?
            ),
        )),
        _ => fail(
            g,
            loc,
            "keyLabel takes a string literal: keyLabel(\"mod+f\")",
        ),
    }
}

/// `titlebarInsets()`: `{ left, right }`, the space the system's window
/// buttons take in a hidden title bar.
fn titlebar_insets(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Value> {
    let _ = c;
    if !call.args.is_empty() {
        return fail(g, loc, "titlebarInsets() takes no arguments");
    }
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.rust == "zant::platform::TitlebarInsets" {
            return Ok(Value::new(
                Type::Compound(index as u32),
                "ui::titlebar_insets()",
            ));
        }
    }
    let ty = model::add(
        g,
        model::Info {
            kind: model::InfoKind::Object,
            rust: "zant::platform::TitlebarInsets".to_owned(),
            fields: vec![
                model::Field {
                    name: b"left".to_vec(),
                    ty: Type::F32,
                    optional: false,
                    rust: None,
                },
                model::Field {
                    name: b"right".to_vec(),
                    ty: Type::F32,
                    optional: false,
                    rust: None,
                },
            ],
            copy: true,
            ..model::Info::default()
        },
    )?;
    Ok(Value::new(ty, "ui::titlebar_insets()"))
}

fn names(namespace: &Namespace) -> String {
    namespace
        .functions
        .iter()
        .map(|function| function.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Keeps `model::InfoKind` reachable for tests that build compounds.
#[allow(dead_code)]
fn _info(_: model::InfoKind) {}
