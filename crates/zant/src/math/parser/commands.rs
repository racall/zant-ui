//! Control sequence dispatch and macro expansion.

use super::{Atom, CancelKind, Class, Node, ParseError, Parser, Text, parse};
use crate::math::{fonts, mhchem};

impl<'a> Parser<'a> {
    pub(super) fn command(
        &mut self,
        name: &str,
        out: &mut Vec<Node>,
    ) -> Result<Option<Node>, ParseError> {
        if let Some(node) = self.spacing_command(name)? {
            return Ok(Some(node));
        }
        // line break outside environments: ignore (KaTeX: newline in display)
        if name == "\\"
            || name == "newline"
            || name == "nonumber"
            || name == "notag"
            || name == "displaylimits"
            || name == "allowbreak"
            || name == "relax"
        {
            return Ok(None);
        }
        if name == "limits" || name == "nolimits" {
            if let Some(last) = out.last_mut()
                && let Node::Op(op) = last
            {
                op.forced = Some(name == "limits");
            }
            return Ok(None);
        }

        if let Some(node) = self.symbol_or_operator(name)? {
            return Ok(Some(node));
        }
        if let Some(node) = self.fraction_radical_or_delimiter(name, out)? {
            return Ok(Some(node));
        }
        if let Some(node) = self.accent_or_arrow(name)? {
            return Ok(Some(node));
        }
        if let Some(node) = self.size_command(name)? {
            return Ok(Some(node));
        }
        if let Some(node) = self.environment_macro(name)? {
            return Ok(Some(node));
        }
        if let Some(node) = self.lap_or_choice(name)? {
            return Ok(Some(node));
        }
        if name == "ce" || name == "pu" {
            // mhchem: the argument becomes TeX (KaTeX defines these as macros)
            let arg = self.raw_arg()?;
            let machine = if name == "ce" {
                mhchem::Machine::Ce
            } else {
                mhchem::Machine::Pu
            };
            return match mhchem::to_tex(arg, machine) {
                Ok(tex) => Ok(Some(Node::Group(self.sub_parse(&[tex.as_str()])?))),
                Err(_) => Ok(Some(Node::Unknown(name.to_string()))),
            };
        }
        if name == "tripledash" {
            // mhchem's macro: three small dashes (a "tripledash" bond)
            return Ok(Some(Node::Group(self.sub_parse(&[
                "{\\vphantom{-}\\raisebox{2.56mu}{$\\mkern2mu\\tiny\\text{-}\\mkern1mu\\text{-}\\mkern1mu\\text{-}\\mkern2mu$}}",
            ])?)));
        }
        if name == "href" {
            let _ = self.raw_arg()?; // the URL: formulas are not clickable
            return Ok(Some(Node::Group(self.parse_arg()?)));
        }
        if name == "url" {
            return Ok(Some(Node::Text(Text {
                text: self.raw_arg()?.to_string(),
                variant: fonts::Variant::Monospace,
                bold: false,
            })));
        }
        if let Some(node) = self.box_command(name)? {
            return Ok(Some(node));
        }
        if let Some(node) = self.unicode_command(name)? {
            return Ok(Some(node));
        }
        if let Some(node) = self.genfrac_set_or_stack(name)? {
            return Ok(Some(node));
        }
        if let Some(node) = self.style_command(name)? {
            return Ok(Some(node));
        }
        if name == "not" {
            self.skip_space();
            if self.looking_at("=") {
                self.i += 1;
                return Ok(Some(Node::Atom(Atom {
                    text: "≠".to_string(),
                    class: Class::Rel,
                    variant: fonts::Variant::Normal,
                })));
            }
            if self.looking_at_command("in") {
                self.i += 3;
                return Ok(Some(Node::Atom(Atom {
                    text: "∉".to_string(),
                    class: Class::Rel,
                    variant: fonts::Variant::Normal,
                })));
            }
            let next = match self.parse_atom(out)? {
                Some(next) => next,
                None => return Ok(None),
            };
            return Ok(Some(Node::Cancel {
                body: vec![next],
                kind: CancelKind::Cancel,
            }));
        }
        if name == "tag" || name == "tag*" {
            return Ok(Some(Node::Tag(self.raw_arg()?.to_string())));
        }
        if name == "begin" {
            return Ok(Some(self.environment()?));
        }
        if name == "end" {
            return Err(ParseError);
        }
        if let Some(node) = self.class_command(name)? {
            return Ok(Some(node));
        }
        Ok(Some(Node::Unknown(name.to_string())))
    }

    /// Parses TeX assembled from `parts` (macros expanding to environments).
    pub(super) fn sub_parse(&mut self, parts: &[&str]) -> Result<Vec<Node>, ParseError> {
        let src = parts.concat();
        parse(&src)
    }
}
