//! Fractions, radicals, delimiters, and stacked annotations.

use super::tokens::strip_braces;
use super::{
    Class, Frac, LeftRight, Node, ParseError, Parser, SizedDelim, Sqrt, Stop, StyleSwitch,
};
use crate::math::symbols;

impl<'a> Parser<'a> {
    pub(super) fn fraction_radical_or_delimiter(
        &mut self,
        name: &str,
        out: &mut Vec<Node>,
    ) -> Result<Option<Node>, ParseError> {
        // fractions
        if name == "frac" || name == "dfrac" || name == "tfrac" || name == "cfrac" {
            let num = self.parse_arg()?;
            let den = self.parse_arg()?;
            let style = if name == "dfrac" || name == "cfrac" {
                Some(StyleSwitch::Display)
            } else if name == "tfrac" {
                Some(StyleSwitch::Text)
            } else {
                None
            };
            return Ok(Some(Node::Frac(Frac {
                num,
                den,
                bar: true,
                left: String::new(),
                right: String::new(),
                style,
                continued: name == "cfrac",
            })));
        }
        if name == "binom" || name == "dbinom" || name == "tbinom" {
            let num = self.parse_arg()?;
            let den = self.parse_arg()?;
            let style = if name == "dbinom" {
                Some(StyleSwitch::Display)
            } else if name == "tbinom" {
                Some(StyleSwitch::Text)
            } else {
                None
            };
            return Ok(Some(Node::Frac(Frac {
                num,
                den,
                bar: false,
                left: "(".to_string(),
                right: ")".to_string(),
                style,
                continued: false,
            })));
        }
        if name == "over" || name == "choose" || name == "atop" {
            // infix: everything before in this list is the numerator
            let num = std::mem::take(out);
            let den = self.parse_list(Stop::Brace)?;
            let is_choose = name == "choose";
            return Ok(Some(Node::Frac(Frac {
                num,
                den,
                bar: name == "over",
                left: if is_choose {
                    "(".to_string()
                } else {
                    String::new()
                },
                right: if is_choose {
                    ")".to_string()
                } else {
                    String::new()
                },
                style: None,
                continued: false,
            })));
        }
        if name == "sqrt" {
            let index = self.optional_arg()?;
            let body = self.parse_arg()?;
            return Ok(Some(Node::Sqrt(Sqrt { body, index })));
        }
        if name == "left" {
            let left = self.delim_token()?.to_string();
            let body = self.parse_list(Stop::Right)?;
            if !self.looking_at_command("right") {
                return Err(ParseError);
            }
            self.i += "\\right".len();
            let right = self.delim_token()?.to_string();
            return Ok(Some(Node::LeftRight(LeftRight { left, right, body })));
        }
        if name == "middle" {
            return Ok(Some(Node::SizedDelim(SizedDelim {
                delim: self.delim_token()?.to_string(),
                size: 0,
                class: Class::Rel,
            })));
        }
        if name == "right" {
            return Err(ParseError);
        }
        for (base, size) in [("big", 1u8), ("Big", 2), ("bigg", 3), ("Bigg", 4)] {
            if let Some(rest) = name.strip_prefix(base) {
                for (suffix, class) in [
                    ("", Class::Ord),
                    ("l", Class::Open),
                    ("r", Class::Close),
                    ("m", Class::Rel),
                ] {
                    if rest == suffix {
                        return Ok(Some(Node::SizedDelim(SizedDelim {
                            delim: self.delim_token()?.to_string(),
                            size,
                            class,
                        })));
                    }
                }
            }
        }
        Ok(None)
    }

    pub(super) fn genfrac_set_or_stack(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        if name == "genfrac" {
            let left = strip_braces(self.raw_arg()?);
            let right = strip_braces(self.raw_arg()?);
            let thickness = self.raw_arg()?;
            let style_arg = self.raw_arg()?;
            let num = self.parse_arg()?;
            let den = self.parse_arg()?;
            let bar = !thickness.trim_matches(' ').starts_with('0');
            let style_trimmed = style_arg.trim_matches(' ');
            let style = if style_trimmed == "0" {
                Some(StyleSwitch::Display)
            } else if style_trimmed == "1" {
                Some(StyleSwitch::Text)
            } else {
                None
            };
            return Ok(Some(Node::Frac(Frac {
                num,
                den,
                bar,
                left: symbols::delimiter(&left).unwrap_or("").to_string(),
                right: symbols::delimiter(&right).unwrap_or("").to_string(),
                style,
                continued: false,
            })));
        }
        if name == "Set" || name == "set" {
            let body = self.parse_arg()?;
            return Ok(Some(Node::LeftRight(LeftRight {
                left: "{".to_string(),
                right: "}".to_string(),
                body,
            })));
        }
        if name == "overset" || name == "underset" || name == "stackrel" {
            let annotation = self.parse_arg()?;
            let base = self.parse_arg()?;
            let a = Box::new(Node::Group(annotation));
            let b = Box::new(Node::Group(base));
            let (over, under) = if name == "underset" {
                (None, Some(a))
            } else {
                (Some(a), None)
            };
            return Ok(Some(Node::Stack {
                base: b,
                over,
                under,
            }));
        }
        Ok(None)
    }

    fn delim_token(&mut self) -> Result<&'a str, ParseError> {
        self.skip_space();
        if self.i >= self.src.len() {
            return Err(ParseError);
        }
        let token: &'a str;
        if self.peek_byte() == b'\\' {
            let start = self.i;
            let _ = self.read_command_name();
            token = self.slice(start, self.i);
        } else {
            let len = self.next_char_len();
            token = self.slice(self.i, self.i + len);
            self.i += len;
        }
        match symbols::delimiter(token) {
            Some(delim) => Ok(delim),
            None => Err(ParseError),
        }
    }
}
