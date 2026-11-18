//! Markdown editor component: edit / split / preview modes, a formatting
//! toolbar and editing shortcuts (mod+B/I/K/E, mod+Shift+X — ⌘ on macOS,
//! Ctrl on Windows; list continuation on Enter, Tab / ⇧Tab list indentation).
//!
//! Stateless like every Zant component: the text, selection and mode live in
//! the app and come back through callbacks.

use crate::Str;
use crate::core::events::{self, Key, KeyEvent};
use crate::core::geometry::Edges;
use crate::core::icons::Icon;
use crate::core::keys;
use crate::flex;
use crate::i18n::builtin::{self, Language};
use crate::markdown::cache::Cache;
use crate::markdown::edit;
use crate::markdown::render::view::{self, Props as ViewProps};
use crate::ui;
use crate::ui::actions::{Action, ActionCx, IndexAction, InputKeyAction, RangeAction, TextAction};
use crate::ui::options::{BoxOptions, IconButtonOptions, InputOptions, SpaceOptions, TextOptions};
use crate::ui::styled::Style;

mod props;
mod toolbar;

pub use self::props::*;
use self::toolbar::*;

pub fn editor(p: Props<'_>) -> ui::Node {
    let Props {
        key,
        value,
        selection,
        mode,
        on_change,
        on_selection,
        on_mode,
        on_undo,
        on_redo,
        cache,
        preview,
        placeholder,
        font_size,
        line_height,
        toolbar,
        mode_switch,
        shortcuts,
        preview_width,
        labels,
        content_max_width,
        toolbar_commands,
    } = p;
    let theme = preview.render.theme;
    let sink = Sink {
        on_change,
        on_selection,
    };
    let sel: edit::Selection = match selection {
        Some(s) => edit::Selection {
            anchor: s[0].min(value.len()),
            cursor: s[1].min(value.len()),
        },
        None => edit::Selection::collapsed(value.len()),
    };
    let gutter: f32 = if content_max_width.is_some() {
        24.0
    } else {
        0.0
    };

    let mut rows: Vec<ui::Node> = Vec::new();
    let mut pane_bar: Option<ui::Node> = None;
    if toolbar || mode_switch {
        let mut left: Vec<ui::Node> = Vec::new();
        if toolbar && mode != Mode::Preview {
            let toolbar_ctx = ToolbarCtx {
                sink: sink.clone(),
                text: value.to_owned(),
                selection: sel,
                commands: toolbar_commands.to_vec(),
            };
            for (i, command) in toolbar_commands.iter().enumerate() {
                let ctx = toolbar_ctx.clone();
                let key = key_with(&key, &format!("tool-{i}"));
                left.push(ui::icon_button(IconButtonOptions {
                    key: Some(Str::copy(&key)),
                    icon: command_icon(*command),
                    size: 17.0,
                    r#box: 30.0,
                    color: Some(theme.muted_foreground),
                    tooltip: Some(labels.tools[*command as usize].clone()),
                    index_action: Some(IndexAction::new(move |cx, index| ctx.run(cx, index))),
                    index: i,
                    ..Default::default()
                }));
            }
        }
        let switcher = if mode_switch {
            mode_switcher(&key_with(&key, "mode"), mode, &labels, on_mode.clone())
        } else {
            ui::space(SpaceOptions {
                width: Some(1.0),
                ..Default::default()
            })
        };
        let bar = centered(
            &key_with(&key, "bar-wrap"),
            content_max_width,
            gutter,
            ui::row(BoxOptions {
                key: Some(Str::copy(&key_with(&key, "bar"))),
                width_percent: Some(100.0),
                height: Some(38.0),
                align_items: flex::Align::Center,
                justify: flex::Justify::SpaceBetween,
                children: vec![
                    ui::row(BoxOptions {
                        gap: 2.0,
                        align_items: flex::Align::Center,
                        children: left,
                        ..Default::default()
                    }),
                    switcher,
                ],
                ..Default::default()
            }),
        );
        // split view without a mode switch: the formatting tools belong to
        // the editing pane, so they sit above it rather than the whole window
        if mode == Mode::Split && !mode_switch {
            pane_bar = Some(bar);
        } else {
            rows.push(bar);
        }
    }

    let key_ctx = KeyCtx { sink: sink.clone() };
    let input = ui::styled(
        ui::input(InputOptions {
            key: Some(Str::copy(&key_with(&key, "input"))),
            value: Some(Str::copy(value)),
            multiline: true,
            height: 120.0,
            font_size,
            line_height,
            focus_ring: false,
            placeholder,
            on_change: sink.on_change.clone(),
            on_undo,
            on_redo,
            selection,
            on_selection: sink.on_selection.clone(),
            on_key: if shortcuts {
                Some(InputKeyAction::new(move |cx, key, text, anchor, cursor| {
                    key_ctx.on_key(cx, key, text, anchor, cursor)
                }))
            } else {
                None
            },
            content_max_width,
            ..Default::default()
        }),
        Style {
            grow: Some(1.0),
            shrink: Some(1.0),
            basis: Some(0.0),
            background: Some(theme.background),
            border_color: Some(0x00000000),
            border_width: Some(0.0),
            radius: Some(0.0),
            padding: Some(Edges::default()),
            ..Default::default()
        },
    );
    // a growing column lets the input fill the height in rows (split mode)
    let mut input_children = Vec::with_capacity(2);
    if let Some(bar) = &pane_bar {
        input_children.push(bar.clone());
    }
    input_children.push(input);
    let input_column = ui::column(BoxOptions {
        key: Some(Str::copy(&key_with(&key, "input-col"))),
        grow: 1.0,
        shrink: 1.0,
        basis: Some(0.0),
        align_items: flex::Align::Stretch,
        gap: 6.0,
        children: input_children,
        ..Default::default()
    });

    let preview_node = if mode != Mode::Edit {
        let content = match cache {
            Some(cache) => view::streamdown(cache, value, preview.clone()),
            None => ui::text(
                "(no preview cache)",
                TextOptions {
                    color: Some(theme.muted_foreground),
                    ..Default::default()
                },
            ),
        };
        let max = content_max_width.or(preview_width);
        // the scroll container spans the full width (scroll bar on the
        // window edge); the text column is centred inside it
        Some(ui::column(BoxOptions {
            key: Some(Str::copy(&key_with(&key, "preview"))),
            grow: 1.0,
            shrink: 1.0,
            basis: Some(0.0),
            scroll: true,
            padding: Edges {
                top: 8.0,
                bottom: 32.0,
                ..Edges::default()
            },
            align_items: flex::Align::Stretch,
            children: vec![centered(
                &key_with(&key, "preview-col"),
                max,
                if max.is_some() { 24.0 } else { 12.0 },
                content,
            )],
            ..Default::default()
        }))
    } else {
        None
    };

    let body = match mode {
        Mode::Edit => input_column,
        Mode::Preview => preview_node.expect("preview is built outside edit mode"),
        Mode::Split => ui::row(BoxOptions {
            key: Some(Str::copy(&key_with(&key, "split"))),
            grow: 1.0,
            align_items: flex::Align::Stretch,
            children: vec![
                input_column,
                ui::styled(
                    ui::space(SpaceOptions {
                        width: Some(1.0),
                        ..Default::default()
                    }),
                    Style {
                        background: Some(theme.border),
                        ..Default::default()
                    },
                ),
                preview_node.expect("preview is built outside edit mode"),
            ],
            ..Default::default()
        }),
    };
    rows.push(ui::styled(
        body,
        Style {
            grow: Some(1.0),
            shrink: Some(1.0),
            ..Default::default()
        },
    ));
    ui::column(BoxOptions {
        key: Some(Str::copy(&key)),
        grow: 1.0,
        align_items: flex::Align::Stretch,
        gap: 6.0,
        children: rows,
        ..Default::default()
    })
}
