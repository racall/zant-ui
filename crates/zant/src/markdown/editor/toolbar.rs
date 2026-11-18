//! The toolbar, the mode switcher and the editing keys of the editor.

use super::*;

/// Callbacks shared by the key hook and the toolbar.
#[derive(Clone)]
pub(super) struct Sink {
    pub(super) on_change: Option<TextAction>,
    pub(super) on_selection: Option<RangeAction>,
}

impl Sink {
    pub(super) fn emit(&self, cx: &mut ActionCx<'_>, result: &edit::Result) {
        if let Some(action) = &self.on_change {
            action.call(cx, &result.text);
        }
        if let Some(action) = &self.on_selection {
            action.call(cx, result.selection.anchor, result.selection.cursor);
        }
    }
}

pub(super) struct KeyCtx {
    pub(super) sink: Sink,
}

impl KeyCtx {
    pub(super) fn on_key(
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
pub(super) struct ToolbarCtx {
    pub(super) sink: Sink,
    pub(super) text: String,
    pub(super) selection: edit::Selection,
    pub(super) commands: Vec<edit::Command>,
}

impl ToolbarCtx {
    pub(super) fn run(&self, cx: &mut ActionCx<'_>, index: usize) {
        let Some(command) = self.commands.get(index) else {
            return;
        };
        let result = edit::apply(&self.text, self.selection, *command);
        self.sink.emit(cx, &result);
    }
}

pub(super) fn command_icon(cmd: edit::Command) -> Icon {
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

pub(super) fn key_with(base: &str, suffix: &str) -> String {
    format!("{base}-{suffix}")
}

/// Centres `child` in a full-width row, capped at `max` (plus side gutters).
pub(super) fn centered(key: &str, max: Option<f32>, gutter: f32, child: ui::Node) -> ui::Node {
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
pub(super) fn mode_switcher(
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
