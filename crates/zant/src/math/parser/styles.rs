//! Spacing, sizing, styles, fonts, text, colors, and boxes.

use super::{
    Atom, CancelKind, Class, Color, LapSide, Node, ParseError, Parser, Stop, StyleSwitch, Text,
    parse,
};
use crate::math::fonts;

impl<'a> Parser<'a> {
    pub(super) fn spacing_command(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        // spacing
        if name == "," || name == "thinspace" {
            return Ok(Some(Node::Space(3.0 / 18.0)));
        }
        if name == ":" || name == ">" || name == "medspace" {
            return Ok(Some(Node::Space(4.0 / 18.0)));
        }
        if name == ";" || name == "thickspace" {
            return Ok(Some(Node::Space(5.0 / 18.0)));
        }
        if name == "!" || name == "negthinspace" {
            return Ok(Some(Node::Space(-3.0 / 18.0)));
        }
        if name == "negmedspace" {
            return Ok(Some(Node::Space(-4.0 / 18.0)));
        }
        if name == "negthickspace" {
            return Ok(Some(Node::Space(-5.0 / 18.0)));
        }
        if name == "quad" {
            return Ok(Some(Node::Space(1.0)));
        }
        if name == "qquad" {
            return Ok(Some(Node::Space(2.0)));
        }
        if name == " " || name == "space" || name == "nobreakspace" {
            return Ok(Some(Node::Space(0.25)));
        }
        if name == "enspace" {
            return Ok(Some(Node::Space(0.5)));
        }
        if name == "hspace"
            || name == "kern"
            || name == "mkern"
            || name == "mskip"
            || name == "hskip"
        {
            let raw = self.raw_arg_or_dimen()?;
            return Ok(Some(Node::Space(parse_dimen(raw))));
        }
        Ok(None)
    }

    pub(super) fn size_command(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        for (n, factor) in [
            ("tiny", 0.5),
            ("scriptsize", 0.7),
            ("footnotesize", 0.8),
            ("small", 0.9),
            ("normalsize", 1.0),
            ("large", 1.2),
            ("Large", 1.44),
            ("LARGE", 1.728),
            ("huge", 2.074),
            ("Huge", 2.488),
        ] {
            if name == n {
                return Ok(Some(Node::Sized {
                    factor,
                    body: self.parse_list(Stop::Brace)?,
                }));
            }
        }
        Ok(None)
    }

    pub(super) fn lap_or_choice(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        for (n, side) in [
            ("mathrlap", LapSide::Right),
            ("rlap", LapSide::Right),
            ("mathllap", LapSide::Left),
            ("llap", LapSide::Left),
            ("mathclap", LapSide::Center),
            ("clap", LapSide::Center),
        ] {
            if name == n {
                let body = if n.as_bytes()[0] == b'm' {
                    self.parse_arg()?
                } else {
                    self.text_or_math_arg()?
                };
                return Ok(Some(Node::Lap { side, body }));
            }
        }
        if name == "mathchoice" {
            let display = self.parse_arg()?;
            let text = self.parse_arg()?;
            let script = self.parse_arg()?;
            let scriptscript = self.parse_arg()?;
            return Ok(Some(Node::Choice([display, text, script, scriptscript])));
        }
        Ok(None)
    }

    pub(super) fn box_command(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        if name == "colorbox" {
            let c = parse_color(self.raw_arg()?).unwrap_or(0xffffff00);
            return Ok(Some(Node::ColorBox {
                background: c,
                frame: None,
                body: self.text_or_math_arg()?,
            }));
        }
        if name == "fcolorbox" {
            let frame = parse_color(self.raw_arg()?).unwrap_or(0xff000000);
            let bg = parse_color(self.raw_arg()?).unwrap_or(0xffffffff);
            return Ok(Some(Node::ColorBox {
                background: bg,
                frame: Some(frame),
                body: self.text_or_math_arg()?,
            }));
        }
        if name == "smash" {
            self.skip_space();
            if self.peek_byte() == b'[' {
                let _ = self.optional_arg()?;
            }
            return Ok(Some(Node::Smash(self.parse_arg()?)));
        }
        if name == "mathstrut" {
            return Ok(Some(Node::Phantom {
                body: vec![Node::Atom(Atom {
                    text: "(".to_string(),
                    class: Class::Ord,
                    variant: fonts::Variant::Normal,
                })],
                horizontal: false,
                vertical: true,
            }));
        }
        if name == "rule" {
            self.skip_space();
            if self.peek_byte() == b'[' {
                let _ = self.optional_arg()?;
            }
            let w = parse_dimen(self.raw_arg()?);
            let h = parse_dimen(self.raw_arg()?);
            return Ok(Some(Node::RuleBox {
                width: w,
                height: h,
            }));
        }
        if name == "raisebox" {
            let amount = parse_dimen(self.raw_arg()?);
            return Ok(Some(Node::Raise {
                amount,
                body: self.text_or_math_arg()?,
            }));
        }
        Ok(None)
    }

    pub(super) fn style_command(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        if let Some(variant) = fonts::from_command(name) {
            let body = self.parse_arg()?;
            return Ok(Some(Node::Group(apply_variant(body, variant))));
        }
        if matches!(
            name,
            "text"
                | "textrm"
                | "textnormal"
                | "mbox"
                | "textup"
                | "textsf"
                | "texttt"
                | "textbf"
                | "textit"
                | "hbox"
                | "textmd"
        ) {
            let raw = self.raw_arg()?;
            let variant = if name == "textit" {
                fonts::Variant::Italic
            } else if name == "texttt" {
                fonts::Variant::Monospace
            } else if name == "textsf" {
                fonts::Variant::SansSerif
            } else {
                fonts::Variant::Roman
            };
            return Ok(Some(Node::Text(Text {
                text: text_mode_string(raw),
                variant,
                bold: name == "textbf",
            })));
        }
        if name == "color" || name == "textcolor" {
            let color_name = self.raw_arg()?;
            let Some(color) = parse_color(color_name) else {
                return Ok(Some(Node::Unknown(color_name.to_string())));
            };
            if name == "textcolor" {
                return Ok(Some(Node::Color(Color {
                    color,
                    body: self.parse_arg()?,
                })));
            }
            // \color switches the rest of the current group
            let rest = self.parse_list(Stop::Brace)?;
            return Ok(Some(Node::Color(Color { color, body: rest })));
        }
        for (n, style) in [
            ("displaystyle", StyleSwitch::Display),
            ("textstyle", StyleSwitch::Text),
            ("scriptstyle", StyleSwitch::Script),
            ("scriptscriptstyle", StyleSwitch::ScriptScript),
        ] {
            if name == n {
                return Ok(Some(Node::Style {
                    style,
                    body: self.parse_list(Stop::Brace)?,
                }));
            }
        }
        if name == "phantom" {
            return Ok(Some(Node::Phantom {
                body: self.parse_arg()?,
                horizontal: true,
                vertical: true,
            }));
        }
        if name == "hphantom" {
            return Ok(Some(Node::Phantom {
                body: self.parse_arg()?,
                horizontal: true,
                vertical: false,
            }));
        }
        if name == "vphantom" {
            return Ok(Some(Node::Phantom {
                body: self.parse_arg()?,
                horizontal: false,
                vertical: true,
            }));
        }
        if name == "boxed" || name == "fbox" {
            return Ok(Some(Node::Boxed(self.parse_arg()?)));
        }
        for (n, kind) in [
            ("cancel", CancelKind::Cancel),
            ("bcancel", CancelKind::Bcancel),
            ("xcancel", CancelKind::Xcancel),
            ("sout", CancelKind::Sout),
        ] {
            if name == n {
                return Ok(Some(Node::Cancel {
                    body: self.parse_arg()?,
                    kind,
                }));
            }
        }
        Ok(None)
    }

    /// Text-mode argument (\colorbox, \raisebox): `{text}` as roman text,
    /// `{$math$}` as math.
    fn text_or_math_arg(&mut self) -> Result<Vec<Node>, ParseError> {
        let raw = self.raw_arg()?;
        let t = raw.trim_matches(' ');
        if t.len() >= 2 && t.as_bytes()[0] == b'$' && t.as_bytes()[t.len() - 1] == b'$' {
            return parse(&t[1..t.len() - 1]);
        }
        Ok(vec![Node::Text(Text {
            text: text_mode_string(raw),
            variant: fonts::Variant::Roman,
            bold: false,
        })])
    }

    fn raw_arg_or_dimen(&mut self) -> Result<&'a str, ParseError> {
        self.skip_space();
        if self.peek_byte() == b'{' {
            return self.raw_arg();
        }
        let start = self.i;
        while self.i < self.src.len() {
            let b = self.src.as_bytes()[self.i];
            if b.is_ascii_digit() || b == b'.' || b == b'-' {
                self.i += 1;
            } else {
                break;
            }
        }
        while self.i < self.src.len()
            && self.src.as_bytes()[self.i].is_ascii_alphabetic()
            && self.i - start < 8
        {
            self.i += 1;
        }
        Ok(self.slice(start, self.i))
    }
}

/// `\text{...}` content: strip braces, handle `\$`, `\%`, `\&`, `~`.
fn text_mode_string(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'{' || c == b'}' {
            i += 1;
            continue;
        }
        if c == b'~' {
            out.push(b' ');
            i += 1;
            continue;
        }
        if c == b'\\' && i + 1 < bytes.len() {
            let n = bytes[i + 1];
            if !n.is_ascii_alphabetic() {
                out.push(if n == b' ' || n == b',' { b' ' } else { n });
                i += 2;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    String::from_utf8(out).expect("text_mode_string keeps valid UTF-8")
}

fn apply_variant(body: Vec<Node>, variant: fonts::Variant) -> Vec<Node> {
    body.into_iter()
        .map(|node| apply_variant_one(node, variant))
        .collect()
}

fn apply_variant_one(node: Node, variant: fonts::Variant) -> Node {
    match node {
        Node::Atom(mut atom) => {
            atom.variant = variant;
            Node::Atom(atom)
        }
        Node::Group(group) => Node::Group(apply_variant(group, variant)),
        Node::SupSub(mut sup_sub) => {
            if let Some(base) = sup_sub.base.take() {
                sup_sub.base = Some(Box::new(apply_variant_one(*base, variant)));
            }
            Node::SupSub(sup_sub)
        }
        other => other,
    }
}

/// Parses a TeX dimension into em (`3mu`, `1em`, `2pt`, `0.5ex`).
pub fn parse_dimen(raw: &str) -> f32 {
    let t = raw.trim_matches(|c| c == ' ' || c == '\t');
    let bytes = t.as_bytes();
    let mut end: usize = 0;
    while end < bytes.len()
        && (bytes[end].is_ascii_digit()
            || bytes[end] == b'.'
            || bytes[end] == b'-'
            || bytes[end] == b'+')
    {
        end += 1;
    }
    let value: f32 = t[..end].parse().unwrap_or(0.0);
    let unit = t[end..].trim_matches(' ');
    if unit == "mu" {
        value / 18.0
    } else if unit == "pt" {
        value / 10.0
    } else if unit == "ex" {
        value * 0.431
    } else if unit == "px" {
        value / 16.0
    } else if unit == "mm" {
        value * 0.2845
    } else if unit == "cm" {
        value * 2.845
    } else if unit == "in" {
        value * 7.227
    } else {
        value // em (default)
    }
}

pub fn parse_color(raw: &str) -> Option<u32> {
    let t = raw.trim_matches(|c| c == ' ' || c == '\t');
    if t.is_empty() {
        return None;
    }
    let bytes = t.as_bytes();
    if bytes[0] == b'#' || (t.len() == 6 && is_hex(t)) {
        let hex = if bytes[0] == b'#' { &t[1..] } else { t };
        if hex.len() == 6 {
            return Some(0xff000000 | u32::from_str_radix(hex, 16).ok()?);
        }
        if hex.len() == 3 {
            let v = u32::from_str_radix(hex, 16).ok()?;
            let r = (v >> 8) & 0xf;
            let g = (v >> 4) & 0xf;
            let b = v & 0xf;
            return Some(0xff000000 | (r * 17) << 16 | (g * 17) << 8 | (b * 17));
        }
        return None;
    }
    let named: &[(&str, u32)] = &[
        ("red", 0xffdf0030),
        ("blue", 0xff0000ff),
        ("green", 0xff008000),
        ("black", 0xff000000),
        ("white", 0xffffffff),
        ("gray", 0xff808080),
        ("grey", 0xff808080),
        ("orange", 0xffff8000),
        ("purple", 0xff800080),
        ("magenta", 0xffff00ff),
        ("cyan", 0xff00ffff),
        ("yellow", 0xffffd700),
        ("brown", 0xffa52a2a),
        ("pink", 0xffff69b4),
        ("teal", 0xff008080),
        ("olive", 0xff808000),
        ("violet", 0xff8000ff),
        ("lime", 0xff00ff00),
        ("darkgray", 0xff404040),
        ("lightgray", 0xffbfbfbf),
    ];
    for (n, color) in named {
        if t.eq_ignore_ascii_case(n) {
            return Some(*color);
        }
    }
    None
}

fn is_hex(t: &str) -> bool {
    t.as_bytes().iter().all(u8::is_ascii_hexdigit)
}
