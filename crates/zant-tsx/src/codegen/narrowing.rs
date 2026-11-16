//! Optional values as conditions, and narrowing them (TS control flow):
//!
//! ```tsx
//! const name = peer ? peer.name : "none";      // peer: Peer | null
//! if (label) { label.length }                  // label: string | null
//! {peer && <Text>{peer.name}</Text>}
//! if (!entry) return <Empty/>;  entry.text     // the rest of the block
//! s.prompt !== null && s.prompt.row == row      // member paths too
//! ```
//!
//! A condition proves "this name is non-null when the condition is true /
//! false"; while a branch is emitted, reading the name gives the child type
//! and `x.unwrap()` (on a clone of the `Option`, so the binding itself is
//! never moved out of).

use crate::ast;

use super::expr;
use super::model;
use super::rust_text::{self, Precedence};
use super::scope;
use super::types::{Type, Value};
use super::{Generator, Result, fail};

#[derive(Clone, Debug)]
pub struct Narrow {
    /// The name, or a member path (`s.prompt`).
    pub name: String,
    /// The local it narrows (null: a prop, state or constant), so an inner
    /// local with the same name is not narrowed.
    pub binding: Option<usize>,
    pub ty: Type,
    pub code: String,
}

/// The narrowed value of `n`, if a condition around this point narrowed it.
pub fn find(c: &scope::Context, n: &[u8]) -> Option<Value> {
    find_path(c, n, n)
}

/// The narrowed value of the member path `path` (`s.prompt`) whose base name
/// is `base`.
pub fn find_path(c: &scope::Context, member_path: &[u8], base_name: &[u8]) -> Option<Value> {
    if c.narrowed.is_empty() {
        return None;
    }
    let binding = scope::local_index(c, base_name);
    for item in c.narrowed.iter().rev() {
        if item.name.as_bytes() == member_path && item.binding == binding {
            return Some(Value {
                ty: item.ty,
                code: item.code.clone(),
                ..Value::default()
            });
        }
    }
    None
}

/// Narrows what `cond` proves when it is `positive`; `restore(c, mark)` ends
/// it.
pub fn push(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    cond: &ast::Expr,
    positive: bool,
) -> Result<usize> {
    let mark = c.narrowed.len();
    facts(g, c, cond, positive)?;
    Ok(mark)
}

pub fn restore(c: &mut scope::Context, mark: usize) {
    c.narrowed.truncate(mark);
}

fn facts(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    positive: bool,
) -> Result<()> {
    match &e.data {
        ast::ExprData::Ident(_) | ast::ExprData::Member(_) => {
            if positive {
                candidate(g, c, e)?;
            }
            Ok(())
        }
        ast::ExprData::Unary(u) => {
            if u.op == b"!" {
                facts(g, c, &u.operand, !positive)?;
            }
            Ok(())
        }
        ast::ExprData::Binary(b) => {
            let logical_and = b.op == b"&&";
            let logical_or = b.op == b"||";
            if (logical_and && positive) || (logical_or && !positive) {
                facts(g, c, &b.lhs, positive)?;
                facts(g, c, &b.rhs, positive)?;
                return Ok(());
            }
            // `typeof x === "string"` / `s.kind === "circle"` on a union
            if super::unions::fact(g, c, b, positive)? {
                return Ok(());
            }
            let not_equal = b.op == b"!=" || b.op == b"!==";
            let equal = b.op == b"==" || b.op == b"===";
            let proves = (not_equal && positive) || (equal && !positive);
            if !proves {
                return Ok(());
            }
            let other = if is_null(&b.rhs) {
                &b.lhs
            } else if is_null(&b.lhs) {
                &b.rhs
            } else {
                return Ok(());
            };
            candidate(g, c, other)
        }
        _ => Ok(()),
    }?;
    Ok(())
}

fn is_null(e: &ast::Expr) -> bool {
    matches!(e.data, ast::ExprData::NullLit)
        || matches!(&e.data, ast::ExprData::Ident(n) if n == b"undefined")
}

/// `peer` → "peer", `s.prompt` → "s.prompt" (plain member chains only).
pub fn path(e: &ast::Expr) -> Option<Vec<u8>> {
    match &e.data {
        ast::ExprData::Ident(n) => Some(n.clone()),
        ast::ExprData::Member(m) => {
            if m.optional {
                None
            } else {
                path(&m.object).map(|mut object| {
                    object.push(b'.');
                    object.extend_from_slice(&m.name.name);
                    object
                })
            }
        }
        _ => None,
    }
}

pub fn base(e: &ast::Expr) -> Vec<u8> {
    match &e.data {
        ast::ExprData::Member(m) => base(&m.object),
        ast::ExprData::Ident(n) => n.clone(),
        _ => Vec::new(),
    }
}

fn candidate(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<()> {
    let Some(name) = path(e) else {
        return Ok(());
    };
    let base_name = base(e);
    let binding = scope::local_index(c, &base_name);
    if let Some(index) = binding
        && c.bindings[index].mutable
    {
        return Ok(());
    }
    let value = match expr::emit(g, c, e, None, false) {
        Ok(value) => value,
        // not a name this condition can narrow (an error is reported where it
        // is read)
        Err(super::Error::Tsx) => {
            *g.diag = None;
            return Ok(());
        }
    };
    let Some(child) = model::optional_child(g, value.ty) else {
        return Ok(());
    };
    c.narrowed.push(Narrow {
        name: String::from_utf8_lossy(&name).into_owned(),
        binding,
        ty: child,
        code: format!(
            "{}.unwrap()",
            rust_text::operand(&value, Precedence::Suffix, false)
        ),
    });
    Ok(())
}

/// JS truthiness of an optional: present, and for strings / numbers /
/// booleans also non-empty / non-zero / true.
pub fn truthy(g: &mut Generator<'_>, v: Value) -> Result<Value> {
    let Some(child) = model::optional_child(g, v.ty) else {
        return fail(g, 0, "internal: optional value expected for truthiness");
    };
    if child == Type::Boolean {
        return Ok(Value {
            ty: Type::Boolean,
            code: format!(
                "{}.unwrap_or(false)",
                rust_text::operand(&v, Precedence::Suffix, false)
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    if child != Type::String && !child.numeric() {
        return Ok(Value {
            ty: Type::Boolean,
            code: format!(
                "{}.is_some()",
                rust_text::operand(&v, Precedence::Suffix, false)
            ),
            precedence: Precedence::Suffix,
            ..Value::default()
        });
    }
    g.counter += 1;
    let name = format!("_present{}", g.counter);
    let test = if child == Type::String {
        format!("{name}.len() > 0")
    } else {
        format!("{name} != 0")
    };
    Ok(Value {
        ty: Type::Boolean,
        code: format!(
            "{}.map_or(false, |{name}| {test})",
            rust_text::operand(&v, Precedence::Suffix, false)
        ),
        precedence: Precedence::Suffix,
        ..Value::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, ExprData, Member, Name};
    use crate::codegen::types;

    fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        f(&mut g);
    }

    #[test]
    fn member_paths_and_bases_are_plain_chains() {
        let expression = Expr {
            loc: 4,
            data: ExprData::Member(Member {
                object: Box::new(Expr {
                    loc: 0,
                    data: ExprData::Member(Member {
                        object: Box::new(Expr {
                            loc: 0,
                            data: ExprData::Ident(b"s".to_vec()),
                        }),
                        name: Name {
                            name: b"prompt".to_vec(),
                            loc: 2,
                        },
                        optional: false,
                    }),
                }),
                name: Name {
                    name: b"row".to_vec(),
                    loc: 6,
                },
                optional: false,
            }),
        };
        assert_eq!(Some(b"s.prompt.row".to_vec()), path(&expression));
        assert_eq!(b"s".to_vec(), base(&expression));
    }

    #[test]
    fn find_returns_the_narrowed_value_for_the_same_binding() {
        let mut c = scope::Context::default();
        c.narrowed.push(Narrow {
            name: "s.prompt".to_owned(),
            binding: None,
            ty: Type::Boolean,
            code: "s.prompt.clone().unwrap()".to_owned(),
        });
        let value = find_path(&c, b"s.prompt", b"s").expect("narrowed");
        assert_eq!(Type::Boolean, value.ty);
        assert_eq!("s.prompt.clone().unwrap()", value.code);
        assert!(find_path(&c, b"s.other", b"s").is_none());
    }

    #[test]
    fn restore_truncates_the_narrowed_facts() {
        let mut c = scope::Context::default();
        c.narrowed.push(Narrow {
            name: "a".to_owned(),
            binding: None,
            ty: Type::Boolean,
            code: "a".to_owned(),
        });
        c.narrowed.push(Narrow {
            name: "b".to_owned(),
            binding: None,
            ty: Type::Boolean,
            code: "b".to_owned(),
        });
        restore(&mut c, 1);
        assert_eq!(1, c.narrowed.len());
        assert_eq!("a", c.narrowed[0].name);
    }

    #[test]
    fn truthiness_of_an_optional_number_checks_zero() {
        with_generator(|g| {
            let optional = types::Type::F64;
            let ty = model::optional(g, optional).unwrap();
            let value = types::Value::new(ty, "value");
            let truthy = truthy(g, value).unwrap();
            assert_eq!(Type::Boolean, truthy.ty);
            assert_eq!(
                "value.map_or(false, |_present1| _present1 != 0)",
                truthy.code
            );
        });
    }

    #[test]
    fn truthiness_of_an_optional_value_uses_is_some() {
        with_generator(|g| {
            let list = model::array(g, Type::F64).unwrap();
            let ty = model::optional(g, list).unwrap();
            let value = types::Value::new(ty, "items");
            assert_eq!("items.is_some()", truthy(g, value).unwrap().code);
        });
    }
}
