//! Parameter requirements shared by code generation and translation validation.

use super::message::{Message, MessageError, Part};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    String,
    Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub kind: ParamKind,
    pub loc: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contract {
    pub params: Vec<Param>,
}

#[derive(Default)]
struct Usage {
    first: Option<u32>,
    number: Option<u32>,
    select: Option<u32>,
}

/// Plain display can share a numeric parameter, but select cannot.
pub fn contract(message: &Message) -> Result<Contract, MessageError> {
    let mut uses = BTreeMap::new();
    collect(&message.parts, &mut uses);
    // Exact and category branches are stored separately, so traversal order is
    // not source order. Use locations for both first use and the first conflict.
    if let Some((offset, name)) = uses
        .iter()
        .filter_map(|(name, usage)| Some((usage.number?.max(usage.select?), name)))
        .min()
    {
        return Err(MessageError {
            offset,
            msg: format!("`{name}` is used as a number and as a select value"),
        });
    }
    Ok(Contract {
        params: uses
            .into_iter()
            .map(|(name, usage)| Param {
                name,
                kind: if usage.number.is_some() {
                    ParamKind::Number
                } else {
                    ParamKind::String
                },
                loc: usage.first.unwrap(),
            })
            .collect(),
    })
}

fn collect(parts: &[Part], uses: &mut BTreeMap<String, Usage>) {
    for part in parts {
        let (name, loc) = match part {
            Part::Arg { name, loc }
            | Part::Number { name, loc }
            | Part::Plural { name, loc, .. }
            | Part::Select { name, loc, .. } => (name, *loc),
            Part::Text(_) | Part::Pound { .. } => continue,
        };
        let usage = uses.entry(name.clone()).or_default();
        usage.first = Some(usage.first.unwrap_or(loc).min(loc));
        let required = match part {
            Part::Number { .. } | Part::Plural { .. } => Some(&mut usage.number),
            Part::Select { .. } => Some(&mut usage.select),
            _ => None,
        };
        if let Some(first) = required {
            *first = Some(first.unwrap_or(loc).min(loc));
        }
        match part {
            Part::Plural {
                exact, categories, ..
            } => {
                for (_, parts) in exact {
                    collect(parts, uses);
                }
                for (_, parts) in categories {
                    collect(parts, uses);
                }
            }
            Part::Select { cases, .. } => {
                for (_, parts) in cases {
                    collect(parts, uses);
                }
            }
            _ => {}
        }
    }
}

/// Translations may omit parameters or display numeric parameters as plain text.
pub fn check_translation(default: &Contract, translated: &Contract) -> Result<(), MessageError> {
    for param in &translated.params {
        let original = default.params.iter().find(|p| p.name == param.name);
        let msg = match original {
            None => format!(
                "`{}` is not a parameter of the default-language message",
                param.name
            ),
            Some(p) if p.kind == ParamKind::String && param.kind == ParamKind::Number => {
                format!(
                    "`{}` is a string in the default-language message but is used as a number",
                    param.name
                )
            }
            _ => continue,
        };
        return Err(MessageError {
            offset: param.loc,
            msg,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::message::{Category, parse};
    use super::*;

    fn requirements(text: &str) -> Contract {
        contract(&parse(text).unwrap()).unwrap()
    }

    #[test]
    fn syntax_errors_include_decoded_byte_offsets() {
        let names = "argument names are identifiers such as `count`";
        let styles = "number styles are not supported yet; use `{n, number}`";
        let exact = "plural exact selectors must be non-negative u64 integers";
        let category = "plural category must be zero, one, two, few, many, or other";
        for (text, offset, msg) in [
            ("中}", 3, "unbalanced `}`"),
            ("{", 0, "unbalanced `{`"),
            ("中{n", 3, "unbalanced `{`"),
            ("{n,", 0, "unbalanced `{`"),
            ("{n, number", 0, "unbalanced `{`"),
            ("{n, plural,", 0, "unbalanced `{`"),
            ("{n, plural, other {", 18, "unbalanced `{`"),
            ("{n, plural, other {ok}", 0, "unbalanced `{`"),
            ("{n, select, other {'{end}", 18, "unbalanced `{`"),
            ("{}", 1, names),
            ("{0}", 1, names),
            ("{a.b}", 1, names),
            ("{a-b}", 1, names),
            ("{中}", 1, names),
            ("{a b}", 3, names),
            ("{n, number, integer}", 10, styles),
            ("{n, number, ::percent}", 10, styles),
            ("{n, plural offset:0}", 11, "expected `,` before branches"),
            (
                "{n, plural, offset:0 other {}}",
                12,
                "plural `offset` is not supported",
            ),
            ("{n, plural, unknown {}}", 12, category),
            ("{n, plural, =-1 {}}", 12, exact),
            ("{n, plural, = {}}", 12, exact),
            ("{n, plural, =1.0 {}}", 12, exact),
            ("{n, plural, =18446744073709551616 {}}", 12, exact),
            (
                "{n, plural, one {}}",
                0,
                "plural requires an `other` branch",
            ),
            ("{s, select, a {}}", 0, "select requires an `other` branch"),
            (
                "{s, select, a.b {}}",
                12,
                "select keys must match [A-Za-z0-9_-]+",
            ),
            (
                "{s, select, {}}",
                12,
                "select keys must match [A-Za-z0-9_-]+",
            ),
            ("{s, select, a x}", 14, "expected `{` after selector"),
            (
                "{s, select, a {} a {} other {}}",
                17,
                "duplicate selector `a`",
            ),
            (
                "{n, plural, one {} one {} other {}}",
                19,
                "duplicate selector `one`",
            ),
            (
                "{n, plural, =0 {} =00 {} other {}}",
                18,
                "duplicate selector `=00`",
            ),
            (
                "{s, select, other {} other {}}",
                21,
                "duplicate selector `other`",
            ),
        ] {
            assert_eq!(
                parse(text),
                Err(MessageError {
                    offset,
                    msg: msg.into()
                }),
                "{text}"
            );
        }
        for kind in "selectordinal date time spellout ordinal duration custom".split_whitespace() {
            assert_eq!(
                parse(&format!("中{{n, {kind}}}")),
                Err(MessageError {
                    offset: 7,
                    msg: format!("`{kind}` is not supported yet"),
                })
            );
        }
    }

    #[test]
    fn merges_and_sorts_parameters_at_their_first_use() {
        for text in [
            "{z}{a}{z, number}",
            "{z, number}{a}{z}",
            "{z, plural, other {#}}{a}",
        ] {
            let params = requirements(text).params;
            assert_eq!(
                params
                    .iter()
                    .map(|p| (p.name.as_str(), p.kind))
                    .collect::<Vec<_>>(),
                [("a", ParamKind::String), ("z", ParamKind::Number)]
            );
            assert_eq!(params[1].loc, 0);
        }
        let text = "{n, plural, one {{x}} =0 {{x, number}} other {}}";
        let params = requirements(text).params;
        assert_eq!((params[1].kind, params[1].loc), (ParamKind::Number, 17));
        assert!(requirements("<b>text</b> #").params.is_empty());
    }

    #[test]
    fn select_and_number_conflicts_report_the_later_use() {
        for (text, offset) in [
            ("{x, number}{x, select, other {}}", 11),
            ("{x, select, other {}}{x, number}", 21),
            ("{x}{x, select, other {}}{x, plural, other {#}}", 24),
            (
                "{n, plural, one {{x, select, other {}}} =0 {{x, number}} other {}}",
                44,
            ),
        ] {
            let err = contract(&parse(text).unwrap()).unwrap_err();
            assert_eq!(
                err,
                MessageError {
                    offset,
                    msg: "`x` is used as a number and as a select value".into()
                },
                "{text}"
            );
        }
        for text in ["{x}{x, select, other {}}", "{x, select, other {}}{x}"] {
            assert_eq!(requirements(text).params[0].kind, ParamKind::String);
        }
    }

    #[test]
    fn translation_requirements_are_directional() {
        for (default, translated) in [
            ("{x}", ""),
            ("{x, number}", "{x}"),
            ("{x}", "{x, select, other {}}"),
            ("{x, plural, other {#}}", "{x, number}"),
            ("{x}{y}", "{y}"),
        ] {
            check_translation(&requirements(default), &requirements(translated)).unwrap();
        }
        for (default, translated, offset, msg) in [
            (
                "",
                "中{x}",
                3,
                "`x` is not a parameter of the default-language message",
            ),
            (
                "{x}",
                "a{x, number}",
                1,
                "`x` is a string in the default-language message but is used as a number",
            ),
            (
                "{x, select, other {}}",
                "{x, plural, other {#}}",
                0,
                "`x` is a string in the default-language message but is used as a number",
            ),
        ] {
            assert_eq!(
                check_translation(&requirements(default), &requirements(translated)),
                Err(MessageError {
                    offset,
                    msg: msg.into()
                })
            );
        }
    }

    #[test]
    fn nested_pounds_follow_the_nearest_plural_through_selects() {
        let text =
            "{outer, plural, other {#{s, select, 0-x {#{inner, plural, other {#}}#} other {#}}#}}#";
        let message = parse(text).unwrap();
        let Part::Plural { categories, .. } = &message.parts[0] else {
            panic!()
        };
        assert_eq!(categories[0].0, Category::Other);
        let outer = &categories[0].1;
        let Part::Select { cases, .. } = &outer[1] else {
            panic!()
        };
        let Part::Plural { categories, .. } = &cases[0].1[1] else {
            panic!()
        };
        assert!(matches!(&categories[0].1[..], [Part::Pound { loc: 65 }]));
        assert!(matches!(
            &cases[0].1[..],
            [Part::Pound { .. }, Part::Plural { .. }, Part::Pound { .. }]
        ));
        assert!(matches!(
            &outer[..],
            [Part::Pound { .. }, Part::Select { .. }, Part::Pound { .. }]
        ));
        assert_eq!(message.parts[1], Part::Text("#".into()));
        assert_eq!(
            requirements(text)
                .params
                .iter()
                .map(|p| (p.name.as_str(), p.kind))
                .collect::<Vec<_>>(),
            [
                ("inner", ParamKind::Number),
                ("outer", ParamKind::Number),
                ("s", ParamKind::String)
            ]
        );
    }

    #[test]
    fn nesting_and_part_limits_count_the_entire_message() {
        for kind in ["plural", "select"] {
            let text = format!(
                "{}#{}",
                format!("{{x, {kind}, other {{").repeat(8),
                "}}".repeat(8)
            );
            assert!(parse(&text).is_ok());
            let err = parse(&format!("{{x, {kind}, other {{{text}}}}}")).unwrap_err();
            assert_eq!(err.msg, "message nesting exceeds 8 levels");
            assert_eq!(
                err.offset as usize,
                format!("{{x, {kind}, other {{").len() * 8
            );
        }
        for (text, offset) in [
            ("{x}".repeat(2001), 6000),
            ("a{x}".repeat(1001), 4000),
            (
                format!("{{n, plural, other {{{}}}}}", "#".repeat(2000)),
                2018,
            ),
        ] {
            let err = parse(&text).unwrap_err();
            assert_eq!(
                err,
                MessageError {
                    offset,
                    msg: "message exceeds 2000 parts".into()
                }
            );
        }
        assert_eq!(parse(&"{x}".repeat(2000)).unwrap().parts.len(), 2000);
        assert_eq!(parse(&"''a".repeat(4000)).unwrap().parts.len(), 1);
    }
}
