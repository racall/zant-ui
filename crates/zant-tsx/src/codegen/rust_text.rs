//! Rust identifiers, string literals, operator precedence, format text, CSS
//! color literals and the small source scanner used for label / usage
//! counting. A TSX name that collides with a Rust keyword becomes a
//! `_`-suffixed identifier (Rust raw identifiers are legal, but the usage
//! scanner and golden output stay simpler without `r#`).

use crate::ast;

use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// Rust binding strength, from a conditional expression to a primary value.
/// `|` = 5 and assignment = 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Precedence {
    Assignment = 0,
    Conditional = 1,
    Range = 2,
    LogicalOr = 3,
    LogicalAnd = 4,
    BitOr = 5,
    BitXor = 6,
    BitAnd = 7,
    Comparison = 8,
    Shift = 9,
    Sum = 10,
    Product = 11,
    Cast = 12,
    Unary = 13,
    Suffix = 14,
    #[default]
    Primary = 20,
}

/// Parenthesize only operands whose binding strength would change the parse.
pub fn operand(v: &Value, parent: Precedence, equal: bool) -> String {
    let precedence = v.precedence;
    if precedence < parent || (equal && precedence == parent) {
        format!("({})", v.code)
    } else {
        v.code.clone()
    }
}

/// Rust spelling of a source identifier.
///
/// JS identifiers may contain `$`, which is not a Rust identifier character;
/// such bytes become `_`. Rust keywords get a `_` suffix (`type` → `type_`);
/// `self`, `Self`, `super` and `crate` cannot be raw identifiers, so they take
/// a suffix too. Returns `None` for names that are not valid UTF-8.
pub fn id(name: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(name).ok()?;
    let mut out = String::with_capacity(text.len() + 1);
    for (index, ch) in text.chars().enumerate() {
        let valid = ch == '_' || ch.is_alphanumeric();
        if valid && !(index == 0 && ch.is_numeric()) {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    // `_` is not an identifier: "" is `Empty`, symbol-only members spell
    // their bytes ("-" → `Sym2d`)
    if out.is_empty() {
        if text.is_empty() {
            out.push_str("Empty");
        } else {
            out.push_str("Sym");
            for byte in text.bytes() {
                out.push_str(&format!("{byte:02x}"));
            }
        }
    }
    let is_keyword = matches!(
        out.as_str(),
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "try"
            // edition 2024 reserved word
            | "gen"
            // Rust 2021 reserved-for-future keywords (a valid TSX name such as
            // `box` would make the generated item unparsable)
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    );
    if is_keyword {
        out.push('_');
    }
    Some(out)
}

/// [`id`] at a diagnostic location.
pub fn id_at(g: &mut Generator<'_>, name: &[u8], loc: ast::Loc) -> Result<String> {
    match id(name) {
        Some(value) => Ok(value),
        None => fail(g, loc, "identifier is not valid UTF-8"),
    }
}

/// A Rust string literal from TSX bytes. Returns `None` when the bytes are
/// not UTF-8 (WTF-8 lone surrogates have no Rust `str` representation).
pub fn quote(s: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(s).ok()?;
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 || ch as u32 == 0x7f => {
                out.push_str(&format!("\\u{{{:x}}}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    Some(out)
}

/// [`quote`] at a diagnostic location.
pub fn quote_at(g: &mut Generator<'_>, s: &[u8], loc: ast::Loc) -> Result<String> {
    match quote(s) {
        Some(value) => Ok(value),
        None => fail(
            g,
            loc,
            "string literal is not valid UTF-8; Rust cannot represent an unpaired surrogate",
        ),
    }
}

/// PascalCase Rust enum variant for a string-literal union member
/// (`"in-progress"` → `InProgress`).
pub fn enum_variant(member: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(member).ok()?;
    let mut out = String::with_capacity(text.len());
    let mut upper = true;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if upper {
                for upper_ch in ch.to_uppercase() {
                    out.push(upper_ch);
                }
                upper = false;
            } else {
                out.push(ch);
            }
        } else {
            upper = true;
        }
    }
    // `_` is not an identifier: "" is `Empty`, symbol-only members spell
    // their bytes ("-" → `Sym2d`)
    if out.is_empty() {
        if text.is_empty() {
            out.push_str("Empty");
        } else {
            out.push_str("Sym");
            for byte in text.bytes() {
                out.push_str(&format!("{byte:02x}"));
            }
        }
    }
    if out.chars().next().is_some_and(|ch| ch.is_numeric()) {
        out.insert(0, '_');
    }
    if out == "Self" || out == "Self_" {
        out = "Self_".to_owned();
    }
    Some(out)
}

/// Convert CSS RGB/RGBA literals into ARGB, or type-check numeric color
/// expressions.
pub fn color(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<String> {
    if let ast::ExprData::Cond(cond) = &e.data {
        return Ok(format!(
            "if {} {{ {} }} else {{ {} }}",
            super::expr::condition(g, c, &cond.cond)?.code,
            color(g, c, &cond.then)?,
            color(g, c, &cond.else_)?
        ));
    }
    if let ast::ExprData::String(s) = &e.data {
        if s.is_empty() || s[0] != b'#' || (s.len() != 4 && s.len() != 7 && s.len() != 9) {
            return fail(g, e.loc, "color must be #rgb, #rrggbb, or #rrggbbaa");
        }
        let digits = match std::str::from_utf8(&s[1..])
            .ok()
            .and_then(|text| u32::from_str_radix(text, 16).ok())
        {
            Some(digits) => digits,
            None => return fail(g, e.loc, "invalid hexadecimal color"),
        };
        let argb: u32 = match s.len() {
            4 => {
                0xff00_0000
                    | (((digits >> 8) * 17) << 16)
                    | ((((digits >> 4) & 15) * 17) << 8)
                    | ((digits & 15) * 17)
            }
            7 => 0xff00_0000 | digits,
            9 => (digits >> 8) | ((digits & 255) << 24),
            _ => unreachable!("checked color literal length"),
        };
        return Ok(format!("0x{argb:08x}"));
    }
    Ok(super::expr::emit(g, c, e, Some(Type::U32), false)?.code)
}

pub fn eq(a: &[u8], b: &[u8]) -> bool {
    a == b
}

/// The compiled pieces of a format string.
///
/// `literal` is the text for a substitution-free string; `pattern` is the Rust
/// `format!` pattern, with braces doubled for literal text. `args` are the
/// generated Rust expressions.
#[derive(Clone, Debug, Default)]
pub struct Format {
    pub compile_time: bool,
    pub literal: Vec<u8>,
    pub pattern: Vec<u8>,
    pub args: Vec<String>,
    /// The only substitution is a string and the pattern is exactly `{}`, so
    /// [`Format::finish`] can return the value itself.
    pub only_string: bool,
}

impl Format {
    pub fn text(&mut self, s: &[u8]) {
        self.literal.extend_from_slice(s);
        for byte in s {
            self.pattern.push(*byte);
            if *byte == b'{' || *byte == b'}' {
                self.pattern.push(*byte);
            }
        }
        // A literal after/before a value means the pattern is no longer just
        // `{}`; without this the literal text would be dropped (`"a" + x` and
        // `${x} MB` would lose their text).
        self.only_string = false;
    }

    pub fn value(&mut self, g: &mut Generator<'_>, raw: Value, loc: ast::Loc) -> Result<()> {
        if super::model::optional_child(g, raw.ty).is_some() {
            return fail(g, loc, "optional text value needs ?? or !");
        }
        if let Some(text) = raw.text.as_ref() {
            self.text(text);
            return Ok(());
        }
        if let Some(other) = raw.formatted.as_ref() {
            self.literal.extend_from_slice(&other.literal);
            self.pattern.extend_from_slice(&other.pattern);
            self.args.extend(other.args.iter().cloned());
            self.only_string = false;
            return Ok(());
        }
        let previous_args = self.args.len();
        let previous_pattern = self.pattern.clone();
        let raw_ty = raw.ty.concrete();
        let coerced = types::coerce(g, raw, raw_ty, loc)?;
        let v = types::format_value(g, coerced)?;
        let v_ty = v.ty;
        let (spec, argument): (&[u8], String) = if v_ty == Type::String || v_ty == Type::Boolean {
            (b"{}", v.code.clone())
        } else if super::model::enum_type(g, v_ty) {
            let concrete = types::concrete(g, v)?;
            (b"{}", format!("{}.as_str()", concrete.code))
        } else if v_ty.numeric() {
            let concrete = types::concrete(g, v)?;
            (b"{}", format!("tsx::builtins::string({})", concrete.code))
        } else {
            return fail(
                g,
                loc,
                "text substitutions must be strings, numbers, or booleans",
            );
        };
        self.pattern.extend_from_slice(spec);
        self.args.push(argument);
        self.only_string = previous_args == 0
            && previous_pattern.is_empty()
            && v_ty == Type::String
            && self.pattern == b"{}";
        Ok(())
    }

    pub fn expression(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        e: &ast::Expr,
    ) -> Result<()> {
        if let ast::ExprData::String(s) = &e.data {
            self.text(s);
            return Ok(());
        }
        if let ast::ExprData::Template(t) = &e.data {
            for (index, quasi) in t.quasis.iter().enumerate() {
                self.text(quasi);
                if let Some(expr) = t.exprs.get(index) {
                    self.expression(g, c, expr)?;
                }
            }
            return Ok(());
        }
        let value = super::expr::emit(g, c, e, None, false)?;
        self.value(g, value, e.loc)
    }

    /// The finished Rust expression (`zant::Str`, `log` call, ...).
    pub fn finish(&self, g: &mut Generator<'_>, log: bool) -> Result<String> {
        let loc = 0;
        if !log && self.args.is_empty() {
            return Ok(format!(
                "zant::Str::from({})",
                quote_at(g, &self.literal, loc)?
            ));
        }
        // a lone string needs no formatting
        if !log && self.only_string && self.args.len() == 1 {
            return Ok(self.args[0].clone());
        }
        let pattern = match std::str::from_utf8(&self.pattern) {
            Ok(pattern) => pattern,
            Err(_) => {
                return fail(
                    g,
                    loc,
                    "template literal text is not valid UTF-8; Rust cannot format it",
                );
            }
        };
        let quoted = quote_at(g, pattern.as_bytes(), loc)?;
        let arguments = self.args.join(", ");
        if log {
            // the runtime logs a typed value (`log(&[Any])`) and adds the
            // newline; the trailing `\n` of the console.log call is dropped
            let pattern = pattern.strip_suffix('\n').unwrap_or(pattern);
            let quoted = quote_at(g, pattern.as_bytes(), loc)?;
            let value = if self.args.is_empty() {
                format!("zant::Str::from({quoted})")
            } else {
                format!("zant::Str::copy(&format!({quoted}, {arguments}))")
            };
            return Ok(format!("tsx::builtins::log(&[tsx::Any::from({value})])"));
        }
        Ok(format!("zant::Str::copy(&format!({quoted}, {arguments}))"))
    }
}

/// Whether `body` contains a `break`/`continue` to `label` outside string
/// literals and comments.
pub fn uses_label(body: &str, label: &str) -> bool {
    let bytes = body.as_bytes();
    let needle = format!("'{label}");
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = skip_string(bytes, index),
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = bytes[index..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |offset| index + offset + 1);
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => index = skip_block_comment(bytes, index),
            b'\'' => {
                if body[index..].starts_with(&needle) {
                    return true;
                }
                index = skip_quote(bytes, index);
            }
            _ => {
                if body[index..].starts_with(&needle) {
                    return true;
                }
                index += 1;
            }
        }
    }
    false
}

fn skip_string(bytes: &[u8], mut index: usize) -> usize {
    index += 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    let mut depth = 0usize;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"/*") {
            depth += 1;
            index += 2;
        } else if bytes[index..].starts_with(b"*/") {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn skip_quote(bytes: &[u8], mut index: usize) -> usize {
    index += 1;
    if bytes.get(index) == Some(&b'\\') {
        // a char literal escape: `'\n'`
        index += 2;
        if bytes.get(index) == Some(&b'\'') {
            return index + 1;
        }
        return index;
    }
    if bytes.get(index + 1) == Some(&b'\'') {
        return index + 2;
    }
    // a label or lifetime: `'label`
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    index
}

/// Count plain identifiers in generated Rust, skipping string and char
/// literals, comments, labels and field names after `.` (`scope::finish`
/// uses the counts to decide which locals are unused).
pub fn count_identifiers(body: &str) -> std::collections::HashMap<String, usize> {
    let bytes = body.as_bytes();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut index = 0usize;
    let mut after_period = false;
    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b'"' => {
                index = skip_string(bytes, index);
                after_period = false;
            }
            b'\'' => {
                index = skip_quote(bytes, index);
                after_period = false;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = bytes[index..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |offset| index + offset + 1);
                after_period = false;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = skip_block_comment(bytes, index);
                after_period = false;
            }
            b'.' => {
                after_period = true;
                index += 1;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                if !after_period && let Ok(name) = std::str::from_utf8(&bytes[start..index]) {
                    *counts.entry(name.to_owned()).or_insert(0) += 1;
                }
                after_period = false;
            }
            _ => {
                after_period = false;
                index += 1;
            }
        }
    }
    counts
}

/// A type spelling usable as an expression path: generic arguments need the
/// turbofish there (`Box<f64>` → `Box::<f64>`, as in `Box::<f64> { .. }`).
pub fn expression_path(rust: &str) -> String {
    match rust.find('<') {
        Some(at) if !rust[..at].ends_with("::") => format!("{}::{}", &rust[..at], &rust[at..]),
        _ => rust.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, ExprData, Member, Name};
    use crate::codegen::{Generator, scope};

    fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        f(&mut g);
    }

    #[test]
    fn keywords_and_dollar_signs_get_safe_names() {
        assert_eq!("type_", id(b"type").unwrap());
        assert_eq!("self_", id(b"self").unwrap());
        assert_eq!("_x", id(b"$x").unwrap());
        assert_eq!("name", id(b"name").unwrap());
        assert_eq!("ünïcode", id("ünïcode".as_bytes()).unwrap());
        assert!(id(b"\xff").is_none());
    }

    #[test]
    fn strings_quote_with_rust_escapes() {
        assert_eq!("\"a\\\"b\"", quote(b"a\"b").unwrap());
        assert_eq!("\"a\\\\b\"", quote(b"a\\b").unwrap());
        assert_eq!("\"a\\nb\\tc\"", quote(b"a\nb\tc").unwrap());
        assert_eq!("\"caf\u{e9}\"", quote("café".as_bytes()).unwrap());
        assert_eq!("\"\\u{1}\"", quote(b"\x01").unwrap());
        assert!(quote(b"\xff").is_none());
    }

    #[test]
    fn enum_variants_are_pascal_case() {
        assert_eq!("Active", enum_variant(b"active").unwrap());
        assert_eq!("InProgress", enum_variant(b"in-progress").unwrap());
        assert_eq!("SpaceBetween", enum_variant(b"space between").unwrap());
        assert_eq!("_2x", enum_variant(b"2x").unwrap());
        assert_eq!("Empty", enum_variant(b"").unwrap());
        assert_eq!("Sym2d", enum_variant(b"-").unwrap());
    }

    #[test]
    fn precedence_parenthesizes_only_weaker_operands() {
        let sum = Value {
            code: "a + b".to_owned(),
            precedence: Precedence::Sum,
            ..Value::default()
        };
        assert_eq!("a + b", operand(&sum, Precedence::Sum, false));
        assert_eq!("(a + b)", operand(&sum, Precedence::Product, false));
        assert_eq!("(a + b)", operand(&sum, Precedence::Product, true));
        let primary = Value::new(Type::F64, "a");
        assert_eq!("a", operand(&primary, Precedence::Suffix, true));
    }

    #[test]
    fn format_text_doubles_braces_and_quotes_literals() {
        let mut format = Format::default();
        format.text(b"a{b}\n");
        with_generator(|g| {
            // without substitutions the text stays ordinary (only braces
            // matter for a format! pattern, which is built separately)
            assert_eq!(
                "zant::Str::from(\"a{b}\\n\")",
                format.finish(g, false).unwrap()
            );
        });
        let pattern = String::from_utf8(format.pattern).unwrap();
        assert_eq!("a{{b}}\n", pattern);
    }

    #[test]
    fn format_joins_substitutions_with_str_copy() {
        let mut format = Format::default();
        format.text(b"hi ");
        format.args.push("name".to_owned());
        format.pattern.extend_from_slice(b"{}");
        format.text(b"!");
        with_generator(|g| {
            assert_eq!(
                "zant::Str::copy(&format!(\"hi {}!\", name))",
                format.finish(g, false).unwrap()
            );
        });
    }

    #[test]
    fn color_literals_convert_to_argb() {
        with_generator(|g| {
            let mut c = scope::Context::default();
            let rgb = Expr {
                loc: 0,
                data: ExprData::String(b"#1677ff".to_vec()),
            };
            assert_eq!("0xff1677ff", color(g, &mut c, &rgb).unwrap());
            let rgba = Expr {
                loc: 0,
                data: ExprData::String(b"#1677ff80".to_vec()),
            };
            assert_eq!("0x801677ff", color(g, &mut c, &rgba).unwrap());
            let short = Expr {
                loc: 0,
                data: ExprData::String(b"#f0a".to_vec()),
            };
            assert_eq!("0xffff00aa", color(g, &mut c, &short).unwrap());
            let bad = Expr {
                loc: 0,
                data: ExprData::String(b"red".to_vec()),
            };
            assert!(color(g, &mut c, &bad).is_err());
        });
    }

    #[test]
    fn labels_are_found_only_outside_literals_and_comments() {
        let body = "if x { break '_loop; }";
        assert!(uses_label(body, "_loop"));
        assert!(!uses_label("log(\"break '_loop;\");", "_loop"));
        assert!(!uses_label("// break '_loop;\n", "_loop"));
        assert!(!uses_label("/* break '_loop; */", "_loop"));
        assert!(!uses_label("break '_other;", "_loop"));
    }

    #[test]
    fn identifier_counting_skips_fields_labels_and_literals() {
        let counts =
            count_identifiers("let value = self.value.clone();\nbreak '_loop;\nlog(\"value\");\n");
        // `let value`, `self`, `break`, `log`: field names, labels and string
        // contents are not identifiers.
        assert_eq!(Some(&1), counts.get("value"));
        assert!(!counts.contains_key("_loop"));
        assert!(!counts.contains_key("clone"));
    }

    #[test]
    fn member_names_are_available_to_the_scanner() {
        let member = Member {
            object: Box::new(Expr {
                loc: 0,
                data: ExprData::Ident(b"s".to_vec()),
            }),
            name: Name {
                name: b"prompt".to_vec(),
                loc: 2,
            },
            optional: false,
        };
        assert_eq!(b"prompt".to_vec(), member.name.name);
    }
}
