//! Math alphabets (`\mathbb`, `\mathcal`, ...) mapped to Unicode
//! Mathematical Alphanumeric Symbols, which STIX Two Math covers. This is
//! how KaTeX's font commands look without loading extra font files.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Default math: Latin letters and lowercase Greek italic, the rest upright.
    Normal,
    Italic,
    Bold,
    BoldItalic,
    Roman,
    DoubleStruck,
    Script,
    Fraktur,
    SansSerif,
    SansSerifBold,
    Monospace,
}

pub fn from_command(name: &str) -> Option<Variant> {
    let map: &[(&str, Variant)] = &[
        ("mathbb", Variant::DoubleStruck),
        ("Bbb", Variant::DoubleStruck),
        ("mathcal", Variant::Script),
        ("mathscr", Variant::Script),
        ("mathfrak", Variant::Fraktur),
        ("frak", Variant::Fraktur),
        ("mathbf", Variant::Bold),
        ("bf", Variant::Bold),
        ("boldsymbol", Variant::BoldItalic),
        ("bm", Variant::BoldItalic),
        ("pmb", Variant::BoldItalic),
        ("mathit", Variant::Italic),
        ("it", Variant::Italic),
        ("mathrm", Variant::Roman),
        ("rm", Variant::Roman),
        ("mathup", Variant::Roman),
        ("mathnormal", Variant::Normal),
        ("mathsf", Variant::SansSerif),
        ("sf", Variant::SansSerif),
        ("mathtt", Variant::Monospace),
        ("tt", Variant::Monospace),
        ("bold", Variant::Bold),
        ("Bbb", Variant::DoubleStruck),
        ("cal", Variant::Script),
    ];
    for (n, variant) in map {
        if *n == name {
            return Some(*variant);
        }
    }
    None
}

fn exception(variant: Variant, c: u8) -> Option<u32> {
    match variant {
        Variant::Italic | Variant::Normal => {
            if c == b'h' {
                Some(0x210E)
            } else {
                None
            }
        }
        Variant::DoubleStruck => match c {
            b'C' => Some(0x2102),
            b'H' => Some(0x210D),
            b'N' => Some(0x2115),
            b'P' => Some(0x2119),
            b'Q' => Some(0x211A),
            b'R' => Some(0x211D),
            b'Z' => Some(0x2124),
            _ => None,
        },
        Variant::Script => match c {
            b'B' => Some(0x212C),
            b'E' => Some(0x2130),
            b'F' => Some(0x2131),
            b'H' => Some(0x210B),
            b'I' => Some(0x2110),
            b'L' => Some(0x2112),
            b'M' => Some(0x2133),
            b'R' => Some(0x211B),
            b'e' => Some(0x212F),
            b'g' => Some(0x210A),
            b'o' => Some(0x2134),
            _ => None,
        },
        Variant::Fraktur => match c {
            b'C' => Some(0x212D),
            b'H' => Some(0x210C),
            b'I' => Some(0x2111),
            b'R' => Some(0x211C),
            b'Z' => Some(0x2128),
            _ => None,
        },
        _ => None,
    }
}

fn letter_base(variant: Variant) -> Option<u32> {
    match variant {
        Variant::Bold => Some(0x1D400),
        Variant::Italic | Variant::Normal => Some(0x1D434),
        Variant::BoldItalic => Some(0x1D468),
        Variant::Script => Some(0x1D49C),
        Variant::Fraktur => Some(0x1D504),
        Variant::DoubleStruck => Some(0x1D538),
        Variant::SansSerif => Some(0x1D5A0),
        Variant::SansSerifBold => Some(0x1D5D4),
        Variant::Monospace => Some(0x1D670),
        Variant::Roman => None,
    }
}

fn digit_base(variant: Variant) -> Option<u32> {
    match variant {
        Variant::Bold | Variant::BoldItalic => Some(0x1D7CE),
        Variant::DoubleStruck => Some(0x1D7D8),
        Variant::SansSerif => Some(0x1D7E2),
        Variant::SansSerifBold => Some(0x1D7EC),
        Variant::Monospace => Some(0x1D7F6),
        _ => None,
    }
}

/// Lowercase Greek α..ω (U+03B1..U+03C9) in math italic.
fn greek_italic(cp: u32, bold: bool) -> Option<u32> {
    let base: u32 = if bold { 0x1D736 } else { 0x1D6FC };
    if (0x03B1..=0x03C9).contains(&cp) {
        return Some(base + (cp - 0x03B1));
    }
    match cp {
        0x03F5 => Some(base + 0x1A), // ϵ lunate epsilon → 𝜖 (base+26)
        0x03D1 => Some(base + 27),   // ϑ
        0x03F0 => Some(base + 28),   // ϰ
        0x03D5 => Some(base + 29),   // ϕ
        0x03F1 => Some(base + 30),   // ϱ
        0x03D6 => Some(base + 31),   // ϖ
        _ => None,
    }
}

/// Maps one code point; returns it unchanged when the alphabet has no form.
pub fn map_codepoint(variant: Variant, cp: char) -> char {
    let cp_u32 = cp as u32;
    let mapped = if cp_u32 < 0x80 {
        let c = cp_u32 as u8;
        if c.is_ascii_alphabetic() {
            if let Some(e) = exception(variant, c) {
                e
            } else if let Some(base) = letter_base(variant) {
                let offset: u32 = if c.is_ascii_uppercase() {
                    u32::from(c - b'A')
                } else {
                    26 + u32::from(c - b'a')
                };
                base + offset
            } else {
                cp_u32
            }
        } else if c.is_ascii_digit() {
            match digit_base(variant) {
                Some(base) => base + u32::from(c - b'0'),
                None => cp_u32,
            }
        } else {
            cp_u32
        }
    } else {
        match variant {
            Variant::Normal | Variant::Italic => greek_italic(cp_u32, false).unwrap_or(cp_u32),
            Variant::BoldItalic => greek_italic(cp_u32, true).unwrap_or(cp_u32),
            Variant::Bold => {
                if (0x0391..=0x03A9).contains(&cp_u32) {
                    0x1D6A8 + (cp_u32 - 0x0391)
                } else if (0x03B1..=0x03C9).contains(&cp_u32) {
                    0x1D6C2 + (cp_u32 - 0x03B1)
                } else {
                    cp_u32
                }
            }
            _ => cp_u32,
        }
    };
    char::from_u32(mapped).expect("math alphabet maps to a valid code point")
}

/// Maps a UTF-8 string into a new `String`.
pub fn map_text(variant: Variant, text: &str) -> String {
    if variant == Variant::Roman {
        return text.to_string();
    }
    text.chars().map(|cp| map_codepoint(variant, cp)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabets() {
        let r = map_text(Variant::DoubleStruck, "RZx1");
        assert_eq!(r, "ℝℤ𝕩𝟙");
        let h = map_text(Variant::Italic, "hx");
        assert_eq!(h, "ℎ𝑥");
    }

    #[test]
    fn from_command_table() {
        assert_eq!(from_command("mathbb"), Some(Variant::DoubleStruck));
        assert_eq!(from_command("Bbb"), Some(Variant::DoubleStruck));
        assert_eq!(from_command("cal"), Some(Variant::Script));
        assert_eq!(from_command("boldsymbol"), Some(Variant::BoldItalic));
        assert_eq!(from_command("mathnormal"), Some(Variant::Normal));
        assert_eq!(from_command("unknown"), None);
    }

    #[test]
    fn map_codepoint_samples() {
        assert_eq!(map_codepoint(Variant::Normal, 'h'), 'ℎ');
        assert_eq!(map_codepoint(Variant::Normal, 'A'), '𝐴');
        assert_eq!(map_codepoint(Variant::Normal, 'α'), '𝛼');
        assert_eq!(map_codepoint(Variant::Normal, 'Γ'), 'Γ');
        assert_eq!(map_codepoint(Variant::Normal, '\u{03F5}'), '𝜖');
        assert_eq!(map_codepoint(Variant::Bold, 'A'), '𝐀');
        assert_eq!(map_codepoint(Variant::Bold, '1'), '𝟏');
        assert_eq!(map_codepoint(Variant::Bold, 'α'), '𝛂');
        assert_eq!(map_codepoint(Variant::Bold, 'Γ'), '𝚪');
        assert_eq!(map_codepoint(Variant::BoldItalic, 'α'), '𝜶');
        assert_eq!(map_codepoint(Variant::Script, 'B'), 'ℬ');
        assert_eq!(map_codepoint(Variant::DoubleStruck, 'R'), 'ℝ');
        assert_eq!(map_codepoint(Variant::DoubleStruck, 'A'), '𝔸');
        assert_eq!(map_codepoint(Variant::Monospace, '5'), '𝟻');
        assert_eq!(map_codepoint(Variant::SansSerifBold, 'A'), '𝗔');
        assert_eq!(map_codepoint(Variant::Roman, 'x'), 'x');
        assert_eq!(map_codepoint(Variant::Roman, 'α'), 'α');
    }

    #[test]
    fn map_text_keeps_roman() {
        assert_eq!(map_text(Variant::Roman, "abc α"), "abc α");
    }
}
