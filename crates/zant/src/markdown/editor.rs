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
use crate::layout::flex;
use crate::markdown::cache::Cache;
use crate::markdown::edit;
use crate::markdown::render::view::{self, Props as ViewProps};
use crate::ui;
use crate::ui::actions::{Action, ActionCx, IndexAction, InputKeyAction, RangeAction, TextAction};
use crate::ui::options::{BoxOptions, IconButtonOptions, InputOptions, SpaceOptions, TextOptions};
use crate::ui::styled::Style;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(usize)]
pub enum Mode {
    #[default]
    Edit = 0,
    Split = 1,
    Preview = 2,
}

impl Mode {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(index: usize) -> Mode {
        match index {
            1 => Mode::Split,
            2 => Mode::Preview,
            _ => Mode::Edit,
        }
    }
}

#[derive(Clone)]
pub struct Labels {
    pub edit: Str,
    pub split: Str,
    pub preview: Str,
    /// Toolbar tooltips, in `edit::Command` order.
    pub tools: [Str; 15],
}

fn tool(label: &str, spec: &str) -> Str {
    if spec.is_empty() {
        Str::copy(label)
    } else {
        Str::from(format!("{label} {}", keys::label(spec)))
    }
}

impl Default for Labels {
    fn default() -> Self {
        Labels {
            edit: "Edit".into(),
            split: "Split".into(),
            preview: "Preview".into(),
            tools: [
                tool("Bold", "mod+b"),
                tool("Italic", "mod+i"),
                tool("Strikethrough", "mod+shift+x"),
                tool("Inline code", "mod+e"),
                tool("Link", "mod+k"),
                tool("Heading", ""),
                tool("Quote", ""),
                tool("Bulleted list", ""),
                tool("Numbered list", ""),
                tool("Task list", ""),
                tool("Code block", ""),
                tool("Inline math", ""),
                tool("Math block", ""),
                tool("Divider", ""),
                tool("Table", ""),
            ],
        }
    }
}

impl Labels {
    /// Chinese labels.
    pub fn zh() -> Labels {
        Labels {
            edit: "编辑".into(),
            split: "分栏".into(),
            preview: "预览".into(),
            tools: [
                tool("粗体", "mod+b"),
                tool("斜体", "mod+i"),
                tool("删除线", "mod+shift+x"),
                tool("行内代码", "mod+e"),
                tool("链接", "mod+k"),
                tool("标题", ""),
                tool("引用", ""),
                tool("无序列表", ""),
                tool("有序列表", ""),
                tool("任务列表", ""),
                tool("代码块", ""),
                tool("行内公式", ""),
                tool("公式块", ""),
                tool("分隔线", ""),
                tool("表格", ""),
            ],
        }
    }
}

pub struct Props<'a> {
    pub key: Str,
    pub value: &'a str,
    /// Controlled selection `(anchor, cursor)`; feed `on_selection` back in.
    pub selection: Option<[usize; 2]>,
    pub mode: Mode,
    pub on_change: Option<TextAction>,
    pub on_selection: Option<RangeAction>,
    /// Receives the new [`Mode`] as an index.
    pub on_mode: Option<IndexAction>,
    pub on_undo: Option<Action>,
    pub on_redo: Option<Action>,
    /// Block cache for the preview (required for split / preview modes).
    pub cache: Option<&'a mut Cache>,
    pub preview: ViewProps,
    pub placeholder: Str,
    pub font_size: f32,
    pub line_height: f32,
    pub toolbar: bool,
    pub mode_switch: bool,
    pub shortcuts: bool,
    /// Width of the preview column in preview mode (`None` = fill).
    pub preview_width: Option<f32>,
    pub labels: Labels,
    /// Centre the text column at this width while the editor and preview
    /// (and their scroll bars) span the full available width.
    pub content_max_width: Option<f32>,
    pub toolbar_commands: &'a [edit::Command],
}

pub static DEFAULT_TOOLBAR: [edit::Command; 13] = [
    edit::Command::Heading,
    edit::Command::Bold,
    edit::Command::Italic,
    edit::Command::Strikethrough,
    edit::Command::InlineCode,
    edit::Command::Link,
    edit::Command::Quote,
    edit::Command::BulletList,
    edit::Command::OrderedList,
    edit::Command::TaskList,
    edit::Command::CodeBlock,
    edit::Command::MathBlock,
    edit::Command::Table,
];

impl Default for Props<'_> {
    fn default() -> Self {
        Props {
            key: "md-editor".into(),
            value: "",
            selection: None,
            mode: Mode::Edit,
            on_change: None,
            on_selection: None,
            on_mode: None,
            on_undo: None,
            on_redo: None,
            cache: None,
            preview: ViewProps::default(),
            placeholder: Str::default(),
            font_size: 16.0,
            line_height: 1.8,
            toolbar: true,
            mode_switch: true,
            shortcuts: true,
            preview_width: None,
            labels: Labels::default(),
            content_max_width: None,
            toolbar_commands: &DEFAULT_TOOLBAR,
        }
    }
}

/// Callbacks shared by the key hook and the toolbar.
#[derive(Clone)]
struct Sink {
    on_change: Option<TextAction>,
    on_selection: Option<RangeAction>,
}

impl Sink {
    fn emit(&self, cx: &mut ActionCx<'_>, result: &edit::Result) {
        if let Some(action) = &self.on_change {
            action.call(cx, &result.text);
        }
        if let Some(action) = &self.on_selection {
            action.call(cx, result.selection.anchor, result.selection.cursor);
        }
    }
}

struct KeyCtx {
    sink: Sink,
}

impl KeyCtx {
    fn on_key(
        &self,
        cx: &mut ActionCx<'_>,
        key: KeyEvent,
        text: &str,
        anchor: usize,
        cursor: usize,
    ) -> bool {
        let sel = edit::Selection { anchor, cursor };
        let meta = key.modifiers.meta || key.modifiers.control;
        if meta {
            let cmd: Option<edit::Command> = if events::character_equals(key.key, b'b') {
                Some(edit::Command::Bold)
            } else if events::character_equals(key.key, b'i') {
                Some(edit::Command::Italic)
            } else if events::character_equals(key.key, b'k') {
                Some(edit::Command::Link)
            } else if events::character_equals(key.key, b'e') {
                Some(edit::Command::InlineCode)
            } else if key.modifiers.shift && events::character_equals(key.key, b'x') {
                Some(edit::Command::Strikethrough)
            } else {
                None
            };
            let Some(cmd) = cmd else {
                return false;
            };
            let result = edit::apply(text, sel, cmd);
            self.sink.emit(cx, &result);
            return true;
        }
        match key.key {
            Key::Enter => {
                if key.modifiers.shift {
                    return false;
                }
                let Some(result) = edit::continue_list(text, sel) else {
                    return false;
                };
                self.sink.emit(cx, &result);
                true
            }
            Key::Tab => {
                let result = edit::indent_list(text, sel, key.modifiers.shift).or_else(|| {
                    if key.modifiers.shift {
                        return None;
                    }
                    let s = sel.start();
                    let mut out = String::with_capacity(text.len() + 2);
                    out.push_str(&text[..s]);
                    out.push_str("  ");
                    out.push_str(&text[sel.end()..]);
                    Some(edit::Result {
                        text: out,
                        selection: edit::Selection::collapsed(s + 2),
                    })
                });
                match result {
                    Some(result) => {
                        self.sink.emit(cx, &result);
                        true
                    }
                    // Shift-Tab outside a list is consumed
                    None => key.modifiers.shift,
                }
            }
            _ => false,
        }
    }
}

#[derive(Clone)]
struct ToolbarCtx {
    sink: Sink,
    text: String,
    selection: edit::Selection,
    commands: Vec<edit::Command>,
}

impl ToolbarCtx {
    fn run(&self, cx: &mut ActionCx<'_>, index: usize) {
        let Some(command) = self.commands.get(index) else {
            return;
        };
        let result = edit::apply(&self.text, self.selection, *command);
        self.sink.emit(cx, &result);
    }
}

fn command_icon(cmd: edit::Command) -> Icon {
    match cmd {
        edit::Command::Bold => Icon::Bold,
        edit::Command::Italic => Icon::Italic,
        edit::Command::Strikethrough => Icon::Strikethrough,
        edit::Command::InlineCode => Icon::Code,
        edit::Command::Link => Icon::Link,
        edit::Command::Heading => Icon::Heading,
        edit::Command::Quote => Icon::Quote,
        edit::Command::BulletList => Icon::ListBullet,
        edit::Command::OrderedList => Icon::ListOrdered,
        edit::Command::TaskList => Icon::ListChecks,
        edit::Command::CodeBlock => Icon::CodeBlock,
        edit::Command::InlineMath | edit::Command::MathBlock => Icon::Sigma,
        edit::Command::Rule => Icon::MinusRule,
        edit::Command::Table => Icon::Table,
    }
}

fn key_with(base: &str, suffix: &str) -> String {
    format!("{base}-{suffix}")
}

/// Centres `child` in a full-width row, capped at `max` (plus side gutters).
fn centered(key: &str, max: Option<f32>, gutter: f32, child: ui::Node) -> ui::Node {
    ui::row(BoxOptions {
        key: Some(Str::copy(key)),
        width_percent: Some(100.0),
        justify: flex::Justify::Center,
        padding: Edges {
            left: gutter,
            right: gutter,
            ..Edges::default()
        },
        children: vec![ui::column(BoxOptions {
            width_percent: Some(100.0),
            max_width: max,
            align_items: flex::Align::Stretch,
            children: vec![child],
            ..Default::default()
        })],
        ..Default::default()
    })
}

/// The edit / split / preview selector.
fn mode_switcher(
    key: &str,
    selected: Mode,
    labels: &Labels,
    on_mode: Option<IndexAction>,
) -> ui::Node {
    let option = |label: &Str, icon: Icon| ui::SegmentedOption {
        label: label.clone(),
        icon,
        tooltip: None,
    };
    ui::segmented(ui::SegmentedOptions {
        key: Some(Str::copy(key)),
        options: vec![
            option(&labels.edit, Icon::Pencil),
            option(&labels.split, Icon::Columns),
            option(&labels.preview, Icon::BookOpen),
        ],
        selected: selected.index(),
        on_change: on_mode,
        ..Default::default()
    })
}

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
