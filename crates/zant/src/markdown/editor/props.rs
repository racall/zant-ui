//! The editor's props: modes, labels (localized) and the default toolbar.

use super::*;

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

pub(super) fn tool(text: builtin::Text, language: Language, spec: &str) -> Str {
    let label = text.get(language);
    if spec.is_empty() {
        Str::Static(label)
    } else {
        Str::from(format!("{label} {}", keys::label(spec)))
    }
}

impl Default for Labels {
    /// The labels in the app's current language (`i18n::builtin`).
    fn default() -> Self {
        Labels::for_language(Language::current())
    }
}

impl Labels {
    /// The built-in labels in `language`.
    pub fn for_language(language: Language) -> Labels {
        use builtin::*;
        let tool = |text, spec| tool(text, language, spec);
        Labels {
            edit: EDITOR_EDIT.str(language),
            split: EDITOR_SPLIT.str(language),
            preview: EDITOR_PREVIEW.str(language),
            tools: [
                tool(EDITOR_BOLD, "mod+b"),
                tool(EDITOR_ITALIC, "mod+i"),
                tool(EDITOR_STRIKETHROUGH, "mod+shift+x"),
                tool(EDITOR_INLINE_CODE, "mod+e"),
                tool(EDITOR_LINK, "mod+k"),
                tool(EDITOR_HEADING, ""),
                tool(EDITOR_QUOTE, ""),
                tool(EDITOR_BULLETED_LIST, ""),
                tool(EDITOR_NUMBERED_LIST, ""),
                tool(EDITOR_TASK_LIST, ""),
                tool(EDITOR_CODE_BLOCK, ""),
                tool(EDITOR_INLINE_MATH, ""),
                tool(EDITOR_MATH_BLOCK, ""),
                tool(EDITOR_DIVIDER, ""),
                tool(EDITOR_TABLE, ""),
            ],
        }
    }

    /// The built-in labels for a BCP-47 tag (`"zh-CN"`).
    pub fn for_locale(tag: &str) -> Labels {
        Labels::for_language(Language::for_tag(tag))
    }

    /// Chinese labels.
    #[deprecated(
        note = "defaults follow the app locale; use `Labels::default()` or `Labels::for_locale(\"zh-CN\")`"
    )]
    pub fn zh() -> Labels {
        Labels::for_language(Language::Chinese)
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
