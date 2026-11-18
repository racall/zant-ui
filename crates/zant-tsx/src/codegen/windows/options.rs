//! `createWindow` options: the option table, styles, places and their Rust values.

use super::*;

pub(super) struct WindowOption {
    ts: &'static str,
    rust: &'static str,
    kind: OptionKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OptionKind {
    Number,
    Boolean,
    Color,
    Style,
}

/// `createWindow` options → `zant::desktop::WindowOptions` fields (`title` is
/// apart).
pub(super) const OPTIONS: [WindowOption; 11] = [
    WindowOption {
        ts: "style",
        rust: "style",
        kind: OptionKind::Style,
    },
    WindowOption {
        ts: "width",
        rust: "width",
        kind: OptionKind::Number,
    },
    WindowOption {
        ts: "height",
        rust: "height",
        kind: OptionKind::Number,
    },
    WindowOption {
        ts: "minWidth",
        rust: "min_width",
        kind: OptionKind::Number,
    },
    WindowOption {
        ts: "minHeight",
        rust: "min_height",
        kind: OptionKind::Number,
    },
    WindowOption {
        ts: "background",
        rust: "background",
        kind: OptionKind::Color,
    },
    WindowOption {
        ts: "resizable",
        rust: "resizable",
        kind: OptionKind::Boolean,
    },
    WindowOption {
        ts: "alwaysOnTop",
        rust: "always_on_top",
        kind: OptionKind::Boolean,
    },
    WindowOption {
        ts: "movableByBackground",
        rust: "movable_by_background",
        kind: OptionKind::Boolean,
    },
    WindowOption {
        ts: "center",
        rust: "center",
        kind: OptionKind::Boolean,
    },
    WindowOption {
        ts: "titlebarHeight",
        rust: "titlebar_height",
        kind: OptionKind::Number,
    },
];

pub(super) const STYLES: [(&str, &str); 5] = [
    ("native", "Native"),
    ("hidden-titlebar", "HiddenTitlebar"),
    ("hiddenTitlebar", "HiddenTitlebar"),
    ("borderless", "Borderless"),
    ("panel", "Panel"),
];

pub(super) const PLACES: [(&str, &str); 7] = [
    ("center", "Center"),
    ("topLeft", "TopLeft"),
    ("topRight", "TopRight"),
    ("bottomLeft", "BottomLeft"),
    ("bottomRight", "BottomRight"),
    ("trayCorner", "TrayCorner"),
    ("pointer", "Pointer"),
];

pub(super) fn default_options(name: &str) -> String {
    format!(
        "zant::desktop::OpenOptions {{ name: Some({name}.into()), title: \"\".into(), ..Default::default() }}"
    )
}

pub(super) fn options_value(
    g: &mut Generator<'_>,
    e: &ast::Expr,
    window_name: &str,
) -> Result<String> {
    let ast::ExprData::Object(properties) = &e.data else {
        return fail(g, e.loc, "window options are an object literal");
    };
    let mut c = scope::Context {
        render: true,
        ret: Type::Void,
        constant: true,
        ..scope::Context::default()
    };
    let mut title = "\"\"".to_owned();
    let mut fields = String::new();
    for property in properties {
        if property.spread {
            return fail(g, property.value.loc, "list the window options one by one");
        }
        let name = String::from_utf8_lossy(&property.key.name).into_owned();
        if name == "title" {
            let ast::ExprData::String(value) = &property.value.data else {
                return fail(g, property.value.loc, "the title is a string literal");
            };
            title = rust_text::quote_at(g, value, property.value.loc)?;
            continue;
        }
        let Some(option) = OPTIONS.iter().find(|option| option.ts == name) else {
            return fail(
                g,
                property.key.loc,
                format!(
                    "unknown window option `{name}`; options: title, {}",
                    option_names()
                ),
            );
        };
        let code = match option.kind {
            OptionKind::Number => {
                expr::emit(g, &mut c, &property.value, Some(Type::F32), false)?.code
            }
            OptionKind::Boolean => {
                expr::emit(g, &mut c, &property.value, Some(Type::Boolean), false)?.code
            }
            OptionKind::Color => rust_text::color(g, &mut c, &property.value)?,
            OptionKind::Style => style_literal(g, &property.value)?,
        };
        fields.push_str(&format!("{}: {code}, ", option.rust));
    }
    Ok(format!(
        "zant::desktop::OpenOptions {{\n    name: Some({window_name}.into()),\n    title: {title}.into(),\n    window: zant::desktop::WindowOptions {{\n        {fields}..Default::default()\n    }},\n    ..Default::default()\n}}"
    ))
}

pub(super) fn option_names() -> String {
    OPTIONS
        .iter()
        .map(|option| option.ts)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn style_literal(g: &mut Generator<'_>, e: &ast::Expr) -> Result<String> {
    if let ast::ExprData::String(value) = &e.data {
        let text = String::from_utf8_lossy(value).into_owned();
        if let Some((_, variant)) = STYLES.iter().find(|(ts, _)| *ts == text) {
            return Ok(format!("zant::desktop::WindowStyle::{variant}"));
        }
    }
    let names = STYLES
        .iter()
        .map(|(ts, _)| *ts)
        .collect::<Vec<_>>()
        .join(", ");
    fail(g, e.loc, format!("style is one of: {names}"))
}

pub(super) fn place_literal(g: &mut Generator<'_>, e: &ast::Expr) -> Result<String> {
    if let ast::ExprData::String(value) = &e.data {
        let text = String::from_utf8_lossy(value).into_owned();
        if let Some((_, variant)) = PLACES.iter().find(|(ts, _)| *ts == text) {
            return Ok(format!("zant::desktop::Place::{variant}"));
        }
    }
    let names = PLACES
        .iter()
        .map(|(ts, _)| *ts)
        .collect::<Vec<_>>()
        .join(", ");
    fail(g, e.loc, format!("the place is one of: {names}"))
}
