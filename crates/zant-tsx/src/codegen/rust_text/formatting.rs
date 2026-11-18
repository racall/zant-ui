//! `Format`: building a `format!` string from text pieces and values.

use super::*;

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
        if crate::codegen::model::optional_child(g, raw.ty).is_some() {
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
        let raw = match crate::codegen::unions::text(g, &raw, loc)? {
            Some(text) => text,
            None => raw,
        };
        let previous_args = self.args.len();
        let previous_pattern = self.pattern.clone();
        let raw_ty = raw.ty.concrete();
        let coerced = types::coerce(g, raw, raw_ty, loc)?;
        let v = types::format_value(g, coerced)?;
        let v_ty = v.ty;
        let (spec, argument): (&[u8], String) = if v_ty == Type::String || v_ty == Type::Boolean {
            (b"{}", v.code.clone())
        } else if crate::codegen::model::enum_type(g, v_ty) {
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
        let value = crate::codegen::expr::emit(g, c, e, None, false)?;
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
