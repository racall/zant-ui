//! The supported ICU MessageFormat subset. `<` is literal; rich-text tags are not parsed.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    Text(String),
    Arg {
        name: String,
        loc: u32,
    },
    Number {
        name: String,
        loc: u32,
    },
    Pound {
        loc: u32,
    },
    Plural {
        name: String,
        loc: u32,
        exact: Vec<(u64, Vec<Part>)>,
        categories: Vec<(Category, Vec<Part>)>,
    },
    Select {
        name: String,
        loc: u32,
        cases: Vec<(String, Vec<Part>)>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub parts: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageError {
    pub offset: u32,
    pub msg: String,
}

fn error(offset: usize, msg: impl Into<String>) -> MessageError {
    MessageError {
        offset: offset as u32,
        msg: msg.into(),
    }
}

/// Parse a decoded message. Locations point at opening braces or pound signs.
/// Complex arguments nest at most eight levels; the entire AST has at most 2000 parts.
pub fn parse(text: &str) -> Result<Message, MessageError> {
    if text.len() > u32::MAX as usize {
        return Err(error(0, "message exceeds the byte offset limit"));
    }
    let mut reader = Reader {
        text,
        at: 0,
        count: 0,
    };
    Ok(Message {
        parts: reader.parts(None, 0, false)?,
    })
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
    count: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<char> {
        self.text[self.at..].chars().next()
    }

    fn take(&mut self, c: char) -> bool {
        if self.peek() != Some(c) {
            return false;
        }
        self.at += c.len_utf8();
        true
    }

    fn space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.at += self.peek().unwrap().len_utf8();
        }
    }

    fn token(&mut self) -> &str {
        let start = self.at;
        while let Some(c) = self.peek() {
            if c.is_whitespace() || matches!(c, '{' | '}' | ',') {
                break;
            }
            self.at += c.len_utf8();
        }
        &self.text[start..self.at]
    }

    fn count(&mut self, at: usize) -> Result<(), MessageError> {
        if self.count == 2000 {
            return Err(error(at, "message exceeds 2000 parts"));
        }
        self.count += 1;
        Ok(())
    }

    fn parts(
        &mut self,
        open: Option<usize>,
        depth: u8,
        plural: bool,
    ) -> Result<Vec<Part>, MessageError> {
        let mut parts = Vec::new();
        while let Some(c) = self.peek() {
            let at = self.at;
            if c == '}' {
                if open.is_none() {
                    return Err(error(at, "unbalanced `}`"));
                }
                self.at += 1;
                return Ok(parts);
            }
            if c == '{' || (c == '#' && plural) {
                self.count(at)?;
                let part = if c == '{' {
                    self.argument(depth, plural)?
                } else {
                    self.at += 1;
                    Part::Pound { loc: at as u32 }
                };
                parts.push(part);
            } else {
                if !matches!(parts.last(), Some(Part::Text(_))) {
                    self.count(at)?;
                    parts.push(Part::Text(String::new()));
                }
                let Some(Part::Text(text)) = parts.last_mut() else {
                    unreachable!()
                };
                if c == '\'' {
                    self.quote(text, plural);
                } else {
                    text.push(c);
                    self.at += c.len_utf8();
                }
            }
        }
        if let Some(at) = open {
            return Err(error(at, "unbalanced `{`"));
        }
        Ok(parts)
    }

    fn quote(&mut self, text: &mut String, plural: bool) {
        self.at += 1;
        if self.take('\'') {
            text.push('\'');
            return;
        }
        if !(matches!(self.peek(), Some('{' | '}' | '|')) || plural && self.peek() == Some('#')) {
            text.push('\'');
            return;
        }
        // ICU DOUBLE_OPTIONAL implicitly closes an unterminated quote at EOF.
        // This subset also quotes `|`, and keeps plural context through select branches.
        while let Some(c) = self.peek() {
            self.at += c.len_utf8();
            if c == '\'' && !self.take('\'') {
                break;
            }
            text.push(c);
        }
    }

    fn argument(&mut self, depth: u8, plural: bool) -> Result<Part, MessageError> {
        let open = self.at;
        let loc = open as u32;
        self.at += 1;
        self.space();
        let at = self.at;
        if self.peek().is_none() {
            return Err(error(open, "unbalanced `{`"));
        }
        let name = self.token().to_owned();
        if !name.bytes().enumerate().all(|(i, b)| {
            b.is_ascii_alphabetic() || b == b'_' || b == b'$' || (i > 0 && b.is_ascii_digit())
        }) || name.is_empty()
        {
            return Err(error(at, "argument names are identifiers such as `count`"));
        }
        self.space();
        if self.take('}') {
            return Ok(Part::Arg { name, loc });
        }
        self.expect(',', open, "argument names are identifiers such as `count`")?;
        self.space();
        let at = self.at;
        if self.peek().is_none() {
            return Err(error(open, "unbalanced `{`"));
        }
        let kind = self.token().to_owned();
        self.space();
        if kind == "number" {
            if self.take('}') {
                return Ok(Part::Number { name, loc });
            }
            if self.peek().is_none() {
                return Err(error(open, "unbalanced `{`"));
            }
            return Err(error(
                self.at,
                format!("number styles are not supported yet; use `{{{name}, number}}`"),
            ));
        }
        if kind != "plural" && kind != "select" {
            return Err(error(at, format!("`{kind}` is not supported yet")));
        }
        if depth == 8 {
            return Err(error(open, "message nesting exceeds 8 levels"));
        }
        self.expect(',', open, "expected `,` before branches")?;
        let is_plural = kind == "plural";
        let mut exact = Vec::new();
        let mut categories = Vec::new();
        let mut cases = Vec::new();
        let mut has_other = false;
        loop {
            self.space();
            if self.take('}') {
                break;
            }
            let at = self.at;
            if self.peek().is_none() {
                return Err(error(open, "unbalanced `{`"));
            }
            let key = self.token().to_owned();
            if is_plural && key.starts_with("offset:") {
                return Err(error(at, "plural `offset` is not supported"));
            }
            let (number, category) = if is_plural {
                if let Some(n) = key.strip_prefix('=') {
                    let number = n
                        .parse::<u64>()
                        .ok()
                        .filter(|_| n.bytes().all(|b| b.is_ascii_digit()));
                    let number = number.ok_or_else(|| {
                        error(
                            at,
                            "plural exact selectors must be non-negative u64 integers",
                        )
                    })?;
                    (Some(number), None)
                } else {
                    let category = match key.as_str() {
                        "zero" => Category::Zero,
                        "one" => Category::One,
                        "two" => Category::Two,
                        "few" => Category::Few,
                        "many" => Category::Many,
                        "other" => Category::Other,
                        _ => {
                            return Err(error(
                                at,
                                "plural category must be zero, one, two, few, many, or other",
                            ));
                        }
                    };
                    (None, Some(category))
                }
            } else {
                if key.is_empty()
                    || !key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                {
                    return Err(error(at, "select keys must match [A-Za-z0-9_-]+"));
                }
                (None, None)
            };
            if number.is_some_and(|n| exact.iter().any(|(k, _)| *k == n))
                || category.is_some_and(|c| categories.iter().any(|(k, _)| *k == c))
                || cases.iter().any(|(k, _)| k == &key)
            {
                return Err(error(at, format!("duplicate selector `{key}`")));
            }
            has_other |= key == "other";
            self.space();
            let branch = self.at;
            self.expect('{', open, "expected `{` after selector")?;
            let parts = self.parts(Some(branch), depth + 1, plural || is_plural)?;
            if let Some(n) = number {
                exact.push((n, parts));
            } else if let Some(c) = category {
                categories.push((c, parts));
            } else {
                cases.push((key, parts));
            }
        }
        if !has_other {
            return Err(error(open, format!("{kind} requires an `other` branch")));
        }
        if is_plural {
            Ok(Part::Plural {
                name,
                loc,
                exact,
                categories,
            })
        } else {
            Ok(Part::Select { name, loc, cases })
        }
    }

    fn expect(&mut self, c: char, open: usize, msg: &str) -> Result<(), MessageError> {
        if self.take(c) {
            return Ok(());
        }
        Err(if self.peek().is_none() {
            error(open, "unbalanced `{`")
        } else {
            error(self.at, msg)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Category::*;

    #[test]
    fn literals_and_optional_apostrophes_merge_adjacent_text() {
        for (input, expected) in [
            ("", ""),
            ("中😀 <b>hi</b> #", "中😀 <b>hi</b> #"),
            ("''", "'"),
            ("'{'", "{"),
            ("'{a}'", "{a}"),
            ("It's", "It's"),
            ("'#'", "'#'"),
            ("'{abc", "{abc"),
            ("'}", "}"),
            ("a'{''b''}'c", "a{'b'}c"),
            ("'|{a}'", "|{a}"),
            ("'<'", "'<'"),
        ] {
            let expected = if expected.is_empty() {
                vec![]
            } else {
                vec![Part::Text(expected.into())]
            };
            assert_eq!(parse(input).unwrap().parts, expected, "{input}");
        }
        let parts = parse("It's {n}").unwrap().parts;
        assert!(
            matches!(&parts[..], [Part::Text(s), Part::Arg { name, loc: 5 }] if s == "It's " && name == "n")
        );
        let Part::Plural { categories, .. } =
            &parse("{n, plural, other {'#' '#''x' #}}").unwrap().parts[0]
        else {
            panic!()
        };
        assert!(
            matches!(&categories[0].1[..], [Part::Text(s), Part::Pound { .. }] if s == "# #'x ")
        );
    }

    #[test]
    fn arguments_allow_whitespace_and_preserve_exact_selectors() {
        let text =
            "中{ $n_2 , plural , =0 {none} one {one} two {} few {} many {} zero {} other {#} }";
        let message = parse(text).unwrap();
        let Part::Plural {
            name,
            loc,
            exact,
            categories,
        } = &message.parts[1]
        else {
            panic!()
        };
        assert_eq!((name.as_str(), *loc, exact[0].0), ("$n_2", 3, 0));
        assert_eq!(exact[0].1, [Part::Text("none".into())]);
        assert_eq!(
            categories.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
            [One, Two, Few, Many, Zero, Other]
        );
        let parts = parse("\n{ count\t, number \r}").unwrap().parts;
        assert!(matches!(&parts[1], Part::Number { name, loc: 1 } if name == "count"));
        assert!(parse("{n, plural, =18446744073709551615 {} other {}}").is_ok());
        assert!(parse("{s, select, 0-a_ {} other {}}").is_ok());
    }
}
