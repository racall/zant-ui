//! TSX types, literal inference, numeric peers, and the permitted implicit
//! conversions. Numeric conversions are emitted as Rust `as` casts and
//! suffixed literals.

use std::rc::Rc;

use crate::ast;
use crate::react_compat;

use super::model;
use super::rust_imports;
use super::rust_text::{self, Format, Precedence};
use super::scope;
use super::{Generator, Result, fail};

mod coercion;
mod hints;
mod literals;
#[cfg(test)]
mod tests;

pub use self::coercion::*;
pub use self::hints::*;
pub use self::literals::*;

/// `Type`: primitives plus interned compound types (`model.Info`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Type {
    IntLiteral,
    FloatLiteral,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Usize,
    Isize,
    F32,
    F64,
    Boolean,
    String,
    Node,
    OptionalNode,
    Null,
    #[default]
    Void,
    /// An index into [`Generator::compounds`].
    Compound(u32),
}

/// Literal inference state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Inference {
    /// The code already has its final Rust type.
    #[default]
    Known,
    /// An unsuffixed numeric literal that takes its type from the context.
    Literal,
    /// A conditional or conversion whose type the caller must materialize.
    ResultLocation,
}

#[derive(Clone, Debug)]
pub struct Value {
    pub ty: Type,
    pub code: String,
    /// The decoded literal text (enum member matching).
    pub text: Option<Vec<u8>>,
    /// A reusable format pattern (`a + b` string concatenation).
    pub formatted: Option<Rc<Format>>,
    pub precedence: Precedence,
    pub inference: Inference,
}

impl Value {
    pub fn new(ty: Type, code: impl Into<String>) -> Value {
        Value {
            ty,
            code: code.into(),
            ..Value::default()
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value {
            ty: Type::Void,
            code: String::new(),
            text: None,
            formatted: None,
            precedence: Precedence::Primary,
            inference: Inference::Known,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IntInfo {
    bits: u16,
    signed: bool,
}

impl Type {
    /// An unsigned Rust integer (`.length` is `usize`).
    pub fn unsigned(self) -> bool {
        self.int_info().is_some_and(|info| !info.signed)
    }

    pub fn integer(self) -> bool {
        matches!(
            self,
            Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::Usize
                | Type::Isize
        )
    }

    pub fn float(self) -> bool {
        self == Type::F32 || self == Type::F64
    }

    pub fn numeric(self) -> bool {
        self.integer() || self.float() || self.literal()
    }

    pub fn literal(self) -> bool {
        self == Type::IntLiteral || self == Type::FloatLiteral
    }

    /// The wider type when it holds every value of both (the peer rule:
    /// `u8` and `usize` → `usize`, `u16` and `i32` → `i32`); otherwise a TS
    /// `number` (`f64`), e.g. `u64` and `i32`.
    pub fn numeric_peer(a: Type, b: Type) -> Option<Type> {
        if !a.numeric() || !b.numeric() {
            return None;
        }
        if a == b {
            return Some(a);
        }
        if a == Type::IntLiteral {
            return Some(b);
        }
        if b == Type::IntLiteral {
            return Some(a);
        }
        if a == Type::FloatLiteral {
            return Some(if b.float() { b } else { Type::F64 });
        }
        if b == Type::FloatLiteral {
            return Some(if a.float() { a } else { Type::F64 });
        }
        if a.float() || b.float() {
            return Some(if a == Type::F64 || b == Type::F64 {
                Type::F64
            } else {
                Type::F32
            });
        }
        Some(integer_peer(a, b))
    }

    fn int_info(self) -> Option<IntInfo> {
        let info = match self {
            Type::U8 => IntInfo {
                bits: 8,
                signed: false,
            },
            Type::U16 => IntInfo {
                bits: 16,
                signed: false,
            },
            Type::U32 => IntInfo {
                bits: 32,
                signed: false,
            },
            Type::U64 | Type::Usize => IntInfo {
                bits: 64,
                signed: false,
            },
            Type::I8 => IntInfo {
                bits: 8,
                signed: true,
            },
            Type::I16 => IntInfo {
                bits: 16,
                signed: true,
            },
            Type::I32 => IntInfo {
                bits: 32,
                signed: true,
            },
            Type::I64 | Type::Isize => IntInfo {
                bits: 64,
                signed: true,
            },
            _ => return None,
        };
        Some(info)
    }

    pub fn concrete(self) -> Type {
        if self.literal() { Type::F64 } else { self }
    }

    /// The Rust spelling of this type.
    pub fn rust<'g>(self, g: &'g Generator<'_>) -> &'g str {
        match self {
            Type::Boolean => "bool",
            Type::String => "zant::Str",
            Type::Node => "ui::Node",
            Type::OptionalNode => "Option<ui::Node>",
            // A null-only value: `Option<()>` is the cheapest runtime shape.
            Type::Null => "Option<()>",
            Type::Void => "()",
            Type::IntLiteral | Type::FloatLiteral => "f64",
            Type::U8 => "u8",
            Type::U16 => "u16",
            Type::U32 => "u32",
            Type::U64 => "u64",
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::I64 => "i64",
            Type::Usize => "usize",
            Type::Isize => "isize",
            Type::F32 => "f32",
            Type::F64 => "f64",
            Type::Compound(index) => &g.compounds[index as usize].rust,
        }
    }

    /// Whether reading this type copies it (every non-`Copy` read appends
    /// `.clone()`).
    pub fn copy(self, g: &Generator<'_>) -> bool {
        match self {
            Type::Boolean
            | Type::Null
            | Type::Void
            | Type::IntLiteral
            | Type::FloatLiteral
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::Usize
            | Type::Isize
            | Type::F32
            | Type::F64 => true,
            Type::String | Type::Node | Type::OptionalNode => false,
            Type::Compound(index) => g.compounds[index as usize].copy,
        }
    }
}

fn integer_peer(a: Type, b: Type) -> Type {
    let ia = a.int_info().expect("integer peer");
    let ib = b.int_info().expect("integer peer");
    if ia.signed == ib.signed {
        return if ia.bits >= ib.bits { a } else { b };
    }
    let signed = if ia.signed { a } else { b };
    let unsigned_bits = if ia.signed { ib.bits } else { ia.bits };
    if signed.int_info().expect("signed integer").bits > unsigned_bits {
        signed
    } else {
        Type::F64
    }
}

/// The type of a TSX `Uint8Array`: the runtime's static byte slice
/// (`&'static [u8]`). This is the same interned compound that asset imports
/// (`rust_imports::bytes`) and `TrayOptions::icon` use, so a byte literal and
/// an imported `.png` are the same value type.
pub fn bytes_type(g: &mut Generator<'_>) -> Result<Type> {
    rust_imports::bytes(g)
}

/// The parser preserves named generic arguments until monomorphization exists.
pub(crate) fn generic_error(reference: &ast::TypeRef) -> Option<super::Diag> {
    if reference.args.is_empty() {
        return None;
    }
    Some(super::Diag {
        loc: reference.loc,
        msg: format!(
            "generic type `{}<…>` is not supported yet; declare it in this file (`interface X<T>`) or use a concrete type",
            String::from_utf8_lossy(&reference.name)
        ),
    })
}

/// Resolve a type annotation.
pub fn type_ref(g: &mut Generator<'_>, reference: &ast::TypeRef, allow_void: bool) -> Result<Type> {
    if !reference.args.is_empty() {
        if let Some(index) = super::generic_types::generic_decl(g, &reference.name) {
            return super::generic_types::instantiate(g, reference, index);
        }
        if let Some(ty) = super::promises::named_type(g, reference)? {
            return Ok(ty);
        }
        if let Some(ty) = super::dicts::named(g, reference)? {
            return Ok(ty);
        }
        if let Some(diag) = generic_error(reference) {
            return fail(g, diag.loc, diag.msg);
        }
    }
    if let Some(detail) = reference.detail.as_deref() {
        // inside a generic declaration the same detail (`{ value: T }`)
        // means a different type per instantiation: no cache
        let cacheable = g.type_env.is_empty();
        if cacheable
            && let Some(resolved) = g
                .resolved_types
                .iter()
                .find(|resolved| resolved.detail == *detail)
        {
            return Ok(resolved.ty);
        }
        let ty = model::resolve(g, reference, None)?;
        if cacheable {
            g.resolved_types.push(super::ResolvedType {
                detail: detail.clone(),
                ty,
            });
        }
        return Ok(ty);
    }
    let name = reference.name.as_slice();
    if let Some(ty) = model::named(g, name)? {
        return Ok(ty);
    }
    // React element types: a prop holding an element (`children`, `icon`)
    if node_name(name) || react_compat::node_type(&g.react_imports, name) {
        return Ok(Type::Node);
    }
    if reference.name == b"number" {
        return Ok(Type::F64);
    }
    if reference.name == b"boolean" {
        return Ok(Type::Boolean);
    }
    if reference.name == b"string" {
        return Ok(Type::String);
    }
    if reference.name == b"KeyEvent" {
        return match g.extensions.hooks.key_event_type {
            Some(hook) => hook(g),
            None => fail(
                g,
                reference.loc,
                "`KeyEvent` needs the application hooks installed",
            ),
        };
    }
    if let Some(ty) = super::exceptions::named_type(g, &reference.name)? {
        return Ok(ty);
    }
    if let Some(ty) = super::regexps::named_type(g, &reference.name)? {
        return Ok(ty);
    }
    if reference.name == b"unknown" {
        // what a `catch` binding holds (`catch (e: unknown)`)
        return super::exceptions::thrown_type(g);
    }
    if reference.name == b"Image" {
        return match g.extensions.hooks.image_type {
            Some(hook) => hook(g),
            None => fail(g, reference.loc, "`Image` needs the JSX hooks installed"),
        };
    }
    // bytes (tray icons, imported assets, file contents): the runtime's
    // `&'static [u8]` (see `rust_imports::bytes`); a byte *literal* is emitted
    // as a promoted `&[..]` slice by `expr::byte_slice`
    if reference.name == b"Uint8Array" {
        return bytes_type(g);
    }
    if allow_void && reference.name == b"void" {
        return Ok(Type::Void);
    }
    if let Some(ty) = numeric_name(name) {
        return Ok(ty);
    }
    fail(
        g,
        reference.loc,
        format!(
            "unsupported type `{}` in module `{}`; use number, boolean, string, Uint8Array, or a numeric type",
            String::from_utf8_lossy(name),
            g.name
        ),
    )
}
