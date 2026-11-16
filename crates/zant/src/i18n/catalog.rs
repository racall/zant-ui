//! Message catalogs: the static description an app's build emits
//! ([`Description`], generated from `locales/*.json`) and the framework-owned
//! copy the runtime formats from ([`Catalog`]).
//!
//! Hot reload unloads the previous app library, so nothing here may point
//! into it after [`Catalog::new`]: every string is copied into an `Rc<str>`.
//! Messages are indexed by key id (the position of the key in the sorted key
//! list), arguments by their position in the default-language message's
//! parameter list, so a call costs one index per lookup.

use std::rc::Rc;

use super::plural::{Category, Rules};
use crate::Str;

/// One instruction of a compiled ICU message.
#[derive(Clone, Copy, Debug)]
pub enum Op {
    Text(&'static str),
    /// `{name}`: argument `n` as text.
    Arg(u16),
    /// `{name, number}`.
    Number(u16),
    /// `#` in a plural branch: the innermost plural's value.
    Pound,
    Plural {
        arg: u16,
        exact: &'static [(u64, &'static [Op])],
        categories: &'static [(Category, &'static [Op])],
    },
    Select {
        arg: u16,
        cases: &'static [(&'static str, &'static [Op])],
    },
}

/// One language of the description.
#[derive(Clone, Copy, Debug)]
pub struct LocaleDesc {
    /// Canonical BCP-47 tag (`zh-CN`).
    pub tag: &'static str,
    /// The language's own name (`简体中文`), for language pickers.
    pub name: &'static str,
    /// CLDR cardinal rules (`plural::cldr_rules`), copied at build time.
    pub plural: &'static [(Category, &'static str)],
    /// By key id; `None`: not translated (falls back).
    pub messages: &'static [Option<&'static [Op]>],
}

/// What `zant_build` generates for an app.
#[derive(Clone, Copy, Debug)]
pub struct Description {
    /// Sorted message keys; a key id is an index here.
    pub keys: &'static [&'static str],
    pub locales: &'static [LocaleDesc],
    /// The default locale (`locales[default]`), which has every message.
    pub default: usize,
}

/// An owned instruction (see [`Op`]).
#[derive(Clone, Debug)]
pub enum Node {
    Text(Str),
    Arg(u16),
    Number(u16),
    Pound,
    Plural {
        arg: u16,
        exact: Vec<(u64, Vec<Node>)>,
        categories: Vec<(Category, Vec<Node>)>,
    },
    Select {
        arg: u16,
        cases: Vec<(Str, Vec<Node>)>,
    },
}

/// A message: a lone text (no arguments) is kept as one shared string.
#[derive(Clone, Debug)]
pub enum Message {
    Text(Str),
    Nodes(Rc<[Node]>),
}

#[derive(Debug)]
pub struct Locale {
    pub tag: Str,
    pub name: Str,
    pub rules: Rules,
    pub messages: Vec<Option<Message>>,
}

/// The framework-owned catalog of an app.
#[derive(Debug)]
pub struct Catalog {
    pub keys: Vec<Str>,
    pub locales: Vec<Locale>,
    pub default: usize,
}

fn owned(text: &str) -> Str {
    Str::Shared(Rc::from(text))
}

fn nodes(ops: &[Op]) -> Vec<Node> {
    ops.iter()
        .map(|op| match *op {
            Op::Text(text) => Node::Text(owned(text)),
            Op::Arg(arg) => Node::Arg(arg),
            Op::Number(arg) => Node::Number(arg),
            Op::Pound => Node::Pound,
            Op::Plural {
                arg,
                exact,
                categories,
            } => Node::Plural {
                arg,
                exact: exact.iter().map(|(n, ops)| (*n, nodes(ops))).collect(),
                categories: categories
                    .iter()
                    .map(|(category, ops)| (*category, nodes(ops)))
                    .collect(),
            },
            Op::Select { arg, cases } => Node::Select {
                arg,
                cases: cases
                    .iter()
                    .map(|(key, ops)| (owned(key), nodes(ops)))
                    .collect(),
            },
        })
        .collect()
}

fn message(ops: &[Op]) -> Message {
    match ops {
        [] => Message::Text(Str::default()),
        [Op::Text(text)] => Message::Text(owned(text)),
        _ => Message::Nodes(nodes(ops).into()),
    }
}

impl Catalog {
    /// Copies `description` (which may live in an app library that is
    /// unloaded later). Plural rules were validated at build time; a rule
    /// that does not parse falls back to `other` only.
    pub fn new(description: &Description) -> Catalog {
        let locales = description
            .locales
            .iter()
            .map(|locale| Locale {
                tag: owned(locale.tag),
                name: owned(locale.name),
                rules: Rules::parse(locale.plural)
                    .unwrap_or_else(|_| Rules::parse(&[]).expect("empty rules")),
                messages: locale.messages.iter().map(|ops| ops.map(message)).collect(),
            })
            .collect();
        Catalog {
            keys: description.keys.iter().map(|key| owned(key)).collect(),
            locales,
            default: description
                .default
                .min(description.locales.len().saturating_sub(1)),
        }
    }

    pub fn index_of(&self, tag: &str) -> Option<usize> {
        self.locales.iter().position(|locale| &*locale.tag == tag)
    }

    pub fn tags(&self) -> Vec<&str> {
        self.locales.iter().map(|locale| &*locale.tag).collect()
    }

    /// The message for `key` in the first locale of `chain` that has it,
    /// with that locale (its plural rules format the message).
    pub fn lookup(&self, chain: &[usize], key: usize) -> Option<(&Message, &Locale)> {
        chain.iter().find_map(|index| {
            let locale = self.locales.get(*index)?;
            let message = locale.messages.get(key)?.as_ref()?;
            Some((message, locale))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static HELLO: [Op; 1] = [Op::Text("Hello")];
    static ITEMS: [Op; 1] = [Op::Plural {
        arg: 0,
        exact: &[(0, &[Op::Text("none")])],
        categories: &[
            (Category::One, &[Op::Pound, Op::Text(" item")]),
            (Category::Other, &[Op::Pound, Op::Text(" items")]),
        ],
    }];
    pub(crate) static DESCRIPTION: Description = Description {
        keys: &["hello", "items"],
        locales: &[
            LocaleDesc {
                tag: "en",
                name: "English",
                plural: &[(Category::One, "i = 1 and v = 0")],
                messages: &[Some(&HELLO), Some(&ITEMS)],
            },
            LocaleDesc {
                tag: "zh-CN",
                name: "简体中文",
                plural: &[],
                messages: &[Some(&[Op::Text("你好")]), None],
            },
        ],
        default: 0,
    };

    #[test]
    fn the_copy_owns_its_strings_and_falls_back_per_message() {
        let catalog = Catalog::new(&DESCRIPTION);
        assert!(matches!(&catalog.keys[0], Str::Shared(_)));
        assert_eq!(catalog.index_of("zh-CN"), Some(1));
        let chain = [1, 0];
        let (message, locale) = catalog.lookup(&chain, 0).unwrap();
        assert!(matches!(message, Message::Text(text) if &**text == "你好"));
        assert_eq!(&*locale.tag, "zh-CN");
        // untranslated: the default locale's message, with its rules
        let (message, locale) = catalog.lookup(&chain, 1).unwrap();
        assert!(matches!(message, Message::Nodes(_)));
        assert_eq!(&*locale.tag, "en");
        assert!(catalog.lookup(&chain, 9).is_none());
    }
}
