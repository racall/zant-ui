//! Pure Markdown editing commands used by the editor component: formatting
//! toggles, line prefixes, list continuation on Enter and list indentation.
//! Every command returns the new text plus the selection to apply.
//!
//! The results are owned `String`s (the editor's callbacks own what they hand
//! to the app).

use crate::markdown::scan;

mod lists;
mod prefixes;
#[cfg(test)]
mod tests;

pub use self::lists::*;
use self::prefixes::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

impl Selection {
    pub fn start(self) -> usize {
        self.anchor.min(self.cursor)
    }

    pub fn end(self) -> usize {
        self.anchor.max(self.cursor)
    }

    pub fn collapsed(at: usize) -> Selection {
        Selection {
            anchor: at,
            cursor: at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Result {
    pub text: String,
    pub selection: Selection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Bold,
    Italic,
    Strikethrough,
    InlineCode,
    Link,
    Heading,
    Quote,
    BulletList,
    OrderedList,
    TaskList,
    CodeBlock,
    InlineMath,
    MathBlock,
    Rule,
    Table,
}

impl Command {
    pub fn label(self) -> &'static str {
        match self {
            Command::Bold => "B",
            Command::Italic => "I",
            Command::Strikethrough => "S",
            Command::InlineCode => "`",
            Command::Link => "🔗",
            Command::Heading => "H",
            Command::Quote => "❝",
            Command::BulletList => "•",
            Command::OrderedList => "1.",
            Command::TaskList => "☑",
            Command::CodeBlock => "{}",
            Command::InlineMath => "∑",
            Command::MathBlock => "$$",
            Command::Rule => "—",
            Command::Table => "▦",
        }
    }
}

fn clamp_sel(text: &str, sel: Selection) -> Selection {
    Selection {
        anchor: sel.anchor.min(text.len()),
        cursor: sel.cursor.min(text.len()),
    }
}

fn splice(text: &str, start: usize, end: usize, insert: &str) -> String {
    let mut out = String::with_capacity(text.len() - (end - start) + insert.len());
    out.push_str(&text[..start]);
    out.push_str(insert);
    out.push_str(&text[end..]);
    out
}

/// Toggles `open`…`close` around the selection (or inserts an empty pair).
pub fn wrap(text: &str, sel_in: Selection, open: &str, close: &str) -> Result {
    let sel = clamp_sel(text, sel_in);
    let s = sel.start();
    let e = sel.end();
    // already wrapped outside the selection → unwrap
    if s >= open.len()
        && e + close.len() <= text.len()
        // compared as bytes: the marker-sized window before / after the
        // selection may start inside a multi-byte character
        && text.as_bytes()[s - open.len()..s] == *open.as_bytes()
        && text.as_bytes()[e..e + close.len()] == *close.as_bytes()
    {
        let out = splice(text, s - open.len(), e + close.len(), &text[s..e]);
        return Result {
            text: out,
            selection: Selection {
                anchor: s - open.len(),
                cursor: e - open.len(),
            },
        };
    }
    // selection includes the markers → unwrap
    let inner = &text[s..e];
    if inner.len() >= open.len() + close.len() && inner.starts_with(open) && inner.ends_with(close)
    {
        let body = &inner[open.len()..inner.len() - close.len()];
        let out = splice(text, s, e, body);
        return Result {
            text: out,
            selection: Selection {
                anchor: s,
                cursor: s + body.len(),
            },
        };
    }
    let out = splice(text, s, e, &format!("{open}{inner}{close}"));
    Result {
        text: out,
        selection: Selection {
            anchor: s + open.len(),
            cursor: e + open.len(),
        },
    }
}

pub fn apply(text: &str, sel: Selection, cmd: Command) -> Result {
    match cmd {
        Command::Bold => wrap(text, sel, "**", "**"),
        Command::Italic => wrap(text, sel, "*", "*"),
        Command::Strikethrough => wrap(text, sel, "~~", "~~"),
        Command::InlineCode => wrap(text, sel, "`", "`"),
        Command::InlineMath => wrap(text, sel, "$$", "$$"),
        Command::Link => {
            let c = clamp_sel(text, sel);
            let label = if c.end() > c.start() {
                &text[c.start()..c.end()]
            } else {
                "link"
            };
            let mut out = String::with_capacity(text.len() + label.len() + 16);
            out.push_str(&text[..c.start()]);
            out.push('[');
            out.push_str(label);
            out.push_str("](https://)");
            out.push_str(&text[c.end()..]);
            let url_start = c.start() + 1 + label.len() + 2;
            Result {
                text: out,
                selection: Selection {
                    anchor: url_start,
                    cursor: url_start + "https://".len(),
                },
            }
        }
        Command::Heading => cycle_heading(text, sel),
        Command::Quote => toggle_prefix(text, sel, Prefix::Quote),
        Command::BulletList => toggle_prefix(text, sel, Prefix::Bullet),
        Command::OrderedList => toggle_prefix(text, sel, Prefix::Ordered),
        Command::TaskList => toggle_prefix(text, sel, Prefix::Task),
        Command::CodeBlock => insert_block(text, sel, "```\n", "\n```", "code"),
        Command::MathBlock => insert_block(text, sel, "$$\n", "\n$$", "E = mc^2"),
        Command::Rule => insert_block(text, sel, "", "", "---"),
        Command::Table => insert_block(
            text,
            sel,
            "| Column 1 | Column 2 |\n| --- | --- |\n| ",
            " |  |",
            "Content",
        ),
    }
}
