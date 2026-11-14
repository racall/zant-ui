//! Plain-text rendering of a formula (copy, word counts, accessibility).

use crate::math::parser::{self, Node, XArrowKind};

/// Plain-text rendering of a formula: `\frac{a}{b}` → `(a)/(b)`, `x^2` →
/// `x²`, `\alpha` → `α`. A formula that does not parse is returned unchanged.
pub fn inline_text(source: &str) -> String {
    let Ok(nodes) = parser::parse(source) else {
        return source.to_owned();
    };
    let mut out = String::new();
    flatten(&mut out, &nodes);
    out
}

const SUPERSCRIPTS: [(&str, &str); 14] = [
    ("0", "⁰"),
    ("1", "¹"),
    ("2", "²"),
    ("3", "³"),
    ("4", "⁴"),
    ("5", "⁵"),
    ("6", "⁶"),
    ("7", "⁷"),
    ("8", "⁸"),
    ("9", "⁹"),
    ("+", "⁺"),
    ("−", "⁻"),
    ("n", "ⁿ"),
    ("i", "ⁱ"),
];

const SUBSCRIPTS: [(&str, &str); 14] = [
    ("0", "₀"),
    ("1", "₁"),
    ("2", "₂"),
    ("3", "₃"),
    ("4", "₄"),
    ("5", "₅"),
    ("6", "₆"),
    ("7", "₇"),
    ("8", "₈"),
    ("9", "₉"),
    ("+", "₊"),
    ("−", "₋"),
    ("i", "ᵢ"),
    ("n", "ₙ"),
];

/// A script as Unicode super/subscript characters when every character has
/// one, otherwise as `^(..)` / `_(..)`.
fn script_text(out: &mut String, node: &Node, sup: bool) {
    let mut tmp = String::new();
    flatten(&mut tmp, std::slice::from_ref(node));
    let table = if sup { &SUPERSCRIPTS } else { &SUBSCRIPTS };
    let mut mapped = String::new();
    let mut buf = [0u8; 4];
    for ch in tmp.chars() {
        let key: &str = ch.encode_utf8(&mut buf);
        match table.iter().find(|(plain, _)| *plain == key) {
            Some((_, script)) => mapped.push_str(script),
            None => {
                out.push_str(if sup { "^(" } else { "_(" });
                out.push_str(&tmp);
                out.push(')');
                return;
            }
        }
    }
    out.push_str(&mapped);
}

fn arrow(kind: XArrowKind) -> &'static str {
    match kind {
        XArrowKind::Right => "→",
        XArrowKind::Left => "←",
        XArrowKind::LeftRight => "↔",
        XArrowKind::DoubleRight => "⇒",
        XArrowKind::DoubleLeft => "⇐",
        XArrowKind::DoubleLeftRight => "⇔",
        XArrowKind::HookRight => "↪",
        XArrowKind::HookLeft => "↩",
        XArrowKind::MapsTo => "↦",
        XArrowKind::LongEqual => "=",
        XArrowKind::HarpoonsRl => "⇌",
        XArrowKind::HarpoonsLr => "⇋",
        XArrowKind::ArrowsRl => "⇄",
        XArrowKind::TwoHeadRight => "↠",
        XArrowKind::TwoHeadLeft => "↞",
    }
}

fn flatten(out: &mut String, nodes: &[Node]) {
    for node in nodes {
        match node {
            Node::Atom(a) => out.push_str(&a.text),
            Node::Group(g) => flatten(out, g),
            Node::SupSub(s) => {
                if let Some(base) = &s.base {
                    flatten(out, std::slice::from_ref(&**base));
                }
                if let Some(sub) = &s.sub {
                    script_text(out, sub, false);
                }
                if let Some(sup) = &s.sup {
                    script_text(out, sup, true);
                }
            }
            Node::Frac(f) => {
                out.push('(');
                flatten(out, &f.num);
                out.push_str(")/(");
                flatten(out, &f.den);
                out.push(')');
            }
            Node::Sqrt(s) => {
                out.push_str("√(");
                flatten(out, &s.body);
                out.push(')');
            }
            Node::LeftRight(lr) => {
                out.push_str(&lr.left);
                flatten(out, &lr.body);
                out.push_str(&lr.right);
            }
            Node::SizedDelim(d) => out.push_str(&d.delim),
            Node::Accent(a) => flatten(out, &a.base),
            Node::OverUnder(o) => flatten(out, &o.body),
            Node::Op(o) => {
                out.push_str(&o.text);
                if !o.symbol {
                    out.push(' ');
                }
            }
            Node::Text(t) => out.push_str(&t.text),
            Node::Space(_) => out.push(' '),
            Node::Array(a) => {
                for (r, row) in a.rows.iter().enumerate() {
                    if r > 0 {
                        out.push_str("; ");
                    }
                    for (c, cell) in row.iter().enumerate() {
                        if c > 0 {
                            out.push_str(", ");
                        }
                        flatten(out, cell);
                    }
                }
            }
            Node::Color(c) => flatten(out, &c.body),
            Node::Style { body, .. } => flatten(out, body),
            Node::Phantom { .. } => {}
            Node::Boxed(body) => flatten(out, body),
            Node::Cancel { body, .. } => flatten(out, body),
            Node::Stack { base, .. } => flatten(out, std::slice::from_ref(&**base)),
            Node::Tag(tag) => {
                out.push_str(" (");
                out.push_str(tag);
                out.push(')');
            }
            // An extensible arrow reads as its glyph; the labels are dropped
            // like the over / under parts of a stack.
            Node::XArrow(x) => out.push_str(arrow(x.kind)),
            Node::Sized { body, .. }
            | Node::ColorBox { body, .. }
            | Node::Lap { body, .. }
            | Node::Raise { body, .. } => flatten(out, body),
            Node::Smash(body) => flatten(out, body),
            Node::RuleBox { .. } => {}
            // The text-style branch: this is the inline rendering.
            Node::Choice(branches) => flatten(out, &branches[1]),
            Node::Unknown(name) => {
                out.push('\\');
                out.push_str(name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::inline_text;

    #[test]
    fn fractions_scripts_and_symbols_become_plain_text() {
        assert_eq!(inline_text("\\frac{a}{b}"), "(a)/(b)");
        assert_eq!(inline_text("x^2"), "x²");
        assert_eq!(inline_text("\\alpha"), "α");
        assert_eq!(inline_text("a_1^n"), "a₁ⁿ");
        assert_eq!(inline_text("\\sqrt{x}"), "√(x)");
    }

    #[test]
    fn scripts_without_a_unicode_form_are_parenthesised() {
        assert_eq!(inline_text("x^{ab}"), "x^(ab)");
        assert_eq!(inline_text("x_{k}"), "x_(k)");
        assert_eq!(inline_text("x^{12}"), "x¹²");
    }

    #[test]
    fn a_formula_that_does_not_parse_is_returned_unchanged() {
        assert_eq!(inline_text("\\frac{a}{"), "\\frac{a}{");
    }
}
