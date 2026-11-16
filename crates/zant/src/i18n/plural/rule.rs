use super::Operands;

#[derive(Clone, Debug)]
pub(super) struct Relation {
    operand: u8,
    modulus: Option<u64>,
    negated: bool,
    ranges: Vec<(u64, u64)>,
}

pub(super) type Condition = Vec<Vec<Relation>>;

pub(super) fn matches(condition: &Condition, operands: &Operands) -> bool {
    condition.iter().any(|and| {
        and.iter().all(|relation| {
            let value = operands.integer_value(relation.operand, relation.modulus);
            let included = value.is_some_and(|value| {
                relation
                    .ranges
                    .iter()
                    .any(|&(start, end)| start <= value && value <= end)
            });
            included != relation.negated
        })
    })
}

pub(super) fn parse(text: &str) -> Result<Condition, String> {
    let mut parser = Parser { rest: text };
    let mut condition = vec![vec![parser.relation()?]];
    loop {
        if parser.take("and") {
            condition.last_mut().unwrap().push(parser.relation()?);
        } else if parser.take("or") {
            condition.push(vec![parser.relation()?]);
        } else if parser.rest.trim_matches(pattern_space).is_empty() {
            return Ok(condition);
        } else {
            return Err(format!("unexpected plural rule input: {}", parser.rest));
        }
    }
}

struct Parser<'a> {
    rest: &'a str,
}

impl Parser<'_> {
    fn take(&mut self, token: &str) -> bool {
        let rest = self.rest.trim_start_matches(pattern_space);
        if let Some(tail) = rest.strip_prefix(token) {
            // Keywords must not consume prefixes of unknown identifiers.
            if token.bytes().all(|b| b.is_ascii_alphabetic())
                && tail.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                return false;
            }
            self.rest = tail;
            true
        } else {
            false
        }
    }

    fn number(&mut self) -> Result<u64, String> {
        let rest = self.rest.trim_start_matches(pattern_space);
        let end = rest.bytes().take_while(u8::is_ascii_digit).count();
        if end == 0 {
            return Err(format!("expected integer value at: {rest}"));
        }
        let number = rest[..end]
            .parse()
            .map_err(|_| "plural rule integer exceeds u64".to_string())?;
        self.rest = &rest[end..];
        Ok(number)
    }

    fn relation(&mut self) -> Result<Relation, String> {
        let operand = [b'n', b'i', b'v', b'w', b'f', b't', b'c', b'e']
            .into_iter()
            .find(|&b| self.take(std::str::from_utf8(&[b]).unwrap()))
            .ok_or_else(|| format!("expected plural operand at: {}", self.rest))?;
        let modulus = if self.take("%") || self.take("mod") {
            let value = self.number()?;
            if value == 0 {
                return Err("plural modulus must be positive".into());
            }
            Some(value)
        } else {
            None
        };
        let negated = if self.take("!=") {
            true
        } else if self.take("=") {
            false
        } else {
            return Err(format!("expected '=' or '!=' at: {}", self.rest));
        };
        let mut ranges = Vec::new();
        loop {
            let start = self.number()?;
            let end = if self.take("..") {
                self.number()?
            } else {
                start
            };
            if start > end {
                return Err("plural range start exceeds end".into());
            }
            ranges.push((start, end));
            if !self.take(",") {
                break;
            }
        }
        Ok(Relation {
            operand,
            modulus,
            negated,
            ranges,
        })
    }
}

pub(super) fn pattern_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}' | ' ' | '\u{85}' | '\u{200e}' | '\u{200f}' | '\u{2028}' | '\u{2029}'
    )
}
