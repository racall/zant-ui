//! Imported windows and calls on a window value.

use super::*;

/// The window a file exports as `name`, if any.
pub fn exported(module: &ast::Module, name: &str) -> Option<ast::TopConst> {
    module
        .consts
        .iter()
        .find(|constant| {
            constant.exported
                && is_create(&constant.value)
                && String::from_utf8_lossy(&constant.name.name) == name
        })
        .cloned()
}

/// `import { panel } from "./App"`: the accessor of the declaring module.
pub fn import(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    alias: &str,
    loc: ast::Loc,
) -> Result<()> {
    let accessor = format!(
        "{alias}::{}_with",
        rust_text::id_at(g, imported.as_bytes(), loc)?
    );
    app_state::with_module(|state| {
        state.windows.push(Window {
            name: local.to_owned(),
            rust: accessor,
        })
    });
    Ok(())
}

pub(super) fn find(_g: &Generator<'_>, c: &scope::Context, e: &ast::Expr) -> Option<Window> {
    let ast::ExprData::Ident(name) = &e.data else {
        return None;
    };
    if scope::local(c, name).is_some() {
        return None;
    }
    app_state::with_module(|state| {
        state
            .windows
            .iter()
            .find(|window| window.name.as_bytes() == name.as_slice())
            .cloned()
    })
}

/// `panel.show()`, `hide`, `activate`, `toggle`, `visible`, `active`,
/// `minimized`, `fullscreen`, `place`, `focus`, `setTitle`, `setSize`,
/// `setMinSize`, `setMaxSize`, `setAlwaysOnTop`, `setFullscreen`.
pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        return Ok(None);
    };
    let Some(window) = find(g, c, &member.object) else {
        return Ok(None);
    };
    let method = String::from_utf8_lossy(&member.name.name).into_owned();
    let begin = format!("{}(", window.rust);
    for name in ["show", "hide", "activate", "toggle"] {
        if method != name {
            continue;
        }
        if !call.args.is_empty() {
            return fail(g, loc, format!("{name}() takes no arguments"));
        }
        return Ok(Some(Value::new(
            Type::Void,
            format!("{begin}|window| window.{method}())"),
        )));
    }
    for name in ["visible", "active", "minimized", "fullscreen"] {
        if method != name {
            continue;
        }
        if !call.args.is_empty() {
            return fail(g, loc, format!("{name}() takes no arguments"));
        }
        return Ok(Some(Value::new(
            Type::Boolean,
            format!("{begin}|window| window.{method}())"),
        )));
    }
    if method == "place" {
        if call.args.is_empty() || call.args.len() > 2 {
            return fail(g, loc, "place(where, margin?)");
        }
        let place = place_literal(g, &call.args[0])?;
        let margin = match call.args.get(1) {
            Some(margin) => expr::emit(g, c, margin, Some(Type::F32), false)?.code,
            None => "0.0".to_owned(),
        };
        return Ok(Some(Value::new(
            Type::Void,
            format!("{begin}|window| window.place({place}, {margin}))"),
        )));
    }
    if method == "focus" {
        if call.args.len() != 1 {
            return fail(g, loc, "focus(key) takes the key of a node");
        }
        let key = expr::emit(g, c, &call.args[0], Some(Type::String), false)?;
        return Ok(Some(Value::new(
            Type::Void,
            format!("{begin}|window| window.focus({}.as_str()))", key.code),
        )));
    }
    if method == "setTitle" {
        if call.args.len() != 1 {
            return fail(g, loc, "setTitle(title) takes the title");
        }
        let title = expr::emit(g, c, &call.args[0], Some(Type::String), false)?;
        let title = types::coerce(g, title, Type::String, call.args[0].loc)?;
        return Ok(Some(Value::new(
            Type::Void,
            format!("{begin}|window| window.set_title({}.as_str()))", title.code),
        )));
    }
    for (name, rust) in [
        ("setSize", "set_size"),
        ("setMinSize", "set_min_size"),
        ("setMaxSize", "set_max_size"),
    ] {
        if method != name {
            continue;
        }
        if call.args.len() != 2 {
            return fail(g, loc, format!("{name}(width, height) takes two numbers"));
        }
        let mut sizes = Vec::new();
        for arg in &call.args {
            let value = expr::emit(g, c, arg, Some(Type::F64), false)?;
            sizes.push(types::coerce(g, value, Type::F64, arg.loc)?.code);
        }
        return Ok(Some(Value::new(
            Type::Void,
            format!("{begin}|window| window.{rust}({}, {}))", sizes[0], sizes[1]),
        )));
    }
    for (name, rust) in [
        ("setAlwaysOnTop", "set_always_on_top"),
        ("setFullscreen", "set_fullscreen"),
    ] {
        if method != name {
            continue;
        }
        if call.args.len() != 1 {
            return fail(g, loc, format!("{name}(on) takes a boolean"));
        }
        let on = expr::emit(g, c, &call.args[0], Some(Type::Boolean), false)?;
        let on = types::coerce(g, on, Type::Boolean, call.args[0].loc)?;
        return Ok(Some(Value::new(
            Type::Void,
            format!("{begin}|window| window.{rust}({}))", on.code),
        )));
    }
    fail(
        g,
        member.name.loc,
        "a window has show, hide, activate, toggle, visible, active, minimized, fullscreen, place, focus, \
         setTitle, setSize, setMinSize, setMaxSize, setAlwaysOnTop and setFullscreen",
    )
}
