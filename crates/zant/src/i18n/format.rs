//! Formatting a catalog [`Message`] with its arguments, following ICU
//! MessageFormat for the supported subset: `{x}`, `{n, number}`, `plural`
//! (exact `=N` branches first, then the CLDR category of the locale the
//! message came from), `select` (else `other`) and `#`.
//!
//! Numbers print as JavaScript's `String(n)` does, the same as a template
//! literal `${n}`; locale digit grouping arrives with number formatting.

use super::catalog::{Locale, Message, Node};
use super::plural::{Category, Operands};
use crate::Str;

/// One argument, in the message's parameter order.
#[derive(Clone, Copy, Debug)]
pub enum Arg<'a> {
    Str(&'a str),
    Num(f64),
}

/// The text of `message` in `locale` with `args`.
pub fn format(message: &Message, locale: &Locale, args: &[Arg<'_>]) -> Str {
    match message {
        Message::Text(text) => text.clone(),
        Message::Nodes(nodes) => {
            let mut out = String::new();
            write(&mut out, nodes, locale, args, None);
            Str::from(out)
        }
    }
}

fn number(value: f64) -> String {
    crate::tsx::builtins::string(value).as_str().to_owned()
}

fn write(out: &mut String, nodes: &[Node], locale: &Locale, args: &[Arg<'_>], pound: Option<f64>) {
    for node in nodes {
        match node {
            Node::Text(text) => out.push_str(text),
            Node::Arg(arg) | Node::Number(arg) => match args.get(*arg as usize) {
                Some(Arg::Str(text)) => out.push_str(text),
                Some(Arg::Num(value)) => out.push_str(&number(*value)),
                None => {}
            },
            Node::Pound => {
                if let Some(value) = pound {
                    out.push_str(&number(value));
                }
            }
            Node::Plural {
                arg,
                exact,
                categories,
            } => {
                let value = match args.get(*arg as usize) {
                    Some(Arg::Num(value)) => *value,
                    Some(Arg::Str(text)) => text.parse().unwrap_or(f64::NAN),
                    None => f64::NAN,
                };
                let exact_branch = exact
                    .iter()
                    .find(|(n, _)| value >= 0.0 && value == *n as f64)
                    .map(|(_, nodes)| nodes);
                let branch = exact_branch.or_else(|| {
                    let category = locale.rules.select(&Operands::from_f64(value));
                    categories
                        .iter()
                        .find(|(c, _)| *c == category)
                        .or_else(|| categories.iter().find(|(c, _)| *c == Category::Other))
                        .map(|(_, nodes)| nodes)
                });
                if let Some(branch) = branch {
                    write(out, branch, locale, args, Some(value));
                }
            }
            Node::Select { arg, cases } => {
                let value = match args.get(*arg as usize) {
                    Some(Arg::Str(text)) => (*text).to_owned(),
                    Some(Arg::Num(value)) => number(*value),
                    None => String::new(),
                };
                let branch = cases
                    .iter()
                    .find(|(key, _)| **key == *value)
                    .or_else(|| cases.iter().find(|(key, _)| &**key == "other"))
                    .map(|(_, nodes)| nodes);
                if let Some(branch) = branch {
                    write(out, branch, locale, args, pound);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::catalog::{Catalog, Description, LocaleDesc, Op};
    use super::*;

    static RU_RULES: [(Category, &str); 3] = [
        (Category::One, "v = 0 and i % 10 = 1 and i % 100 != 11"),
        (
            Category::Few,
            "v = 0 and i % 10 = 2..4 and i % 100 != 12..14",
        ),
        (
            Category::Many,
            "v = 0 and i % 10 = 0 or v = 0 and i % 10 = 5..9 or v = 0 and i % 100 = 11..14",
        ),
    ];
    static FILES: [Op; 1] = [Op::Plural {
        arg: 0,
        exact: &[(0, &[Op::Text("нет файлов")])],
        categories: &[
            (Category::One, &[Op::Pound, Op::Text(" файл")]),
            (Category::Few, &[Op::Pound, Op::Text(" файла")]),
            (Category::Many, &[Op::Pound, Op::Text(" файлов")]),
            (Category::Other, &[Op::Pound, Op::Text(" файла")]),
        ],
    }];
    static GREETING: [Op; 3] = [
        Op::Select {
            arg: 1,
            cases: &[
                ("morning", &[Op::Text("Good morning")]),
                ("other", &[Op::Text("Hello")]),
            ],
        },
        Op::Text(", "),
        Op::Arg(0),
    ];
    static DESCRIPTION: Description = Description {
        keys: &["files", "greeting"],
        locales: &[LocaleDesc {
            tag: "ru",
            name: "Русский",
            plural: &RU_RULES,
            messages: &[Some(&FILES), Some(&GREETING)],
        }],
        default: 0,
    };

    fn text(key: usize, args: &[Arg<'_>]) -> String {
        let catalog = Catalog::new(&DESCRIPTION);
        let (message, locale) = catalog.lookup(&[0], key).unwrap();
        format(message, locale, args).as_str().to_owned()
    }

    #[test]
    fn plurals_pick_exact_then_the_locale_category() {
        for (n, expected) in [
            (0.0, "нет файлов"),
            (1.0, "1 файл"),
            (3.0, "3 файла"),
            (5.0, "5 файлов"),
            (11.0, "11 файлов"),
            (21.0, "21 файл"),
            (1.5, "1.5 файла"),
            (f64::NAN, "NaN файла"),
        ] {
            assert_eq!(text(0, &[Arg::Num(n)]), expected, "{n}");
        }
    }

    #[test]
    fn selects_fall_back_to_other() {
        assert_eq!(
            text(1, &[Arg::Str("Ada"), Arg::Str("morning")]),
            "Good morning, Ada"
        );
        assert_eq!(text(1, &[Arg::Str("Ada"), Arg::Str("night")]), "Hello, Ada");
    }
}
