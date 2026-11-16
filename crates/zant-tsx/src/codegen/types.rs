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

/// Allow matching types, contextual literals, integer-to-float, and
/// float-width conversions.
pub fn coerce(g: &mut Generator<'_>, v: Value, t: Type, loc: ast::Loc) -> Result<Value> {
    if v.ty == t {
        return Ok(v);
    }
    if t == Type::String && model::enum_type(g, v.ty) {
        return Ok(Value {
            ty: Type::String,
            code: format!("zant::Str::Static({}.as_str())", concrete(g, v)?.code),
            ..Value::default()
        });
    }
    if let Some(child) = model::optional_child(g, t) {
        if v.ty == Type::Null {
            return Ok(Value {
                ty: t,
                code: "None".to_owned(),
                ..Value::default()
            });
        }
        let mut result = coerce(g, v, child, loc)?;
        result.ty = t;
        result.code = format!("Some({})", result.code);
        result.precedence = Precedence::Suffix;
        return Ok(result);
    }
    if model::optional_child(g, v.ty).is_some() {
        return fail(
            g,
            loc,
            "optional value needs a fallback with ?? or a non-null assertion !",
        );
    }
    if model::enum_type(g, t)
        && v.ty == Type::String
        && let Some(text) = v.text.as_ref()
    {
        let members = model::info(g, t)
            .map(|info| info.members)
            .unwrap_or_default();
        for member in &members {
            if member == text {
                let Some(variant) = rust_text::enum_variant(member) else {
                    return fail(g, loc, "enum member is not valid UTF-8");
                };
                return Ok(Value {
                    ty: t,
                    code: format!("{}::{variant}", t.rust(g)),
                    inference: Inference::ResultLocation,
                    ..Value::default()
                });
            }
        }
        let expected = members
            .iter()
            .map(|member| String::from_utf8_lossy(member).into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        return fail(
            g,
            loc,
            format!(
                "unknown {} member `{}`; expected: {expected}",
                t.rust(g),
                String::from_utf8_lossy(text)
            ),
        );
    }
    if (v.ty == Type::IntLiteral && t.numeric()) || (v.ty == Type::FloatLiteral && t.float()) {
        let mut result = v;
        result.code = literal_code(&result.code, t);
        result.ty = t;
        return Ok(result);
    }
    if v.ty.integer() && t.integer() && Type::numeric_peer(v.ty, t) == Some(t) {
        // widening (`u8` → `u32`): Rust needs an explicit `as` cast
        return Ok(Value {
            ty: t,
            code: format!(
                "({} as {})",
                rust_text::operand(&concrete(g, v)?, Precedence::Cast, false),
                t.rust(g)
            ),
            ..Value::default()
        });
    }
    if v.ty.integer() && t.float() {
        return Ok(Value {
            ty: t,
            code: format!(
                "({} as {})",
                rust_text::operand(&concrete(g, v)?, Precedence::Cast, false),
                t.rust(g)
            ),
            inference: Inference::ResultLocation,
            ..Value::default()
        });
    }
    if v.ty.float() && t.float() {
        return Ok(Value {
            ty: t,
            code: format!(
                "({} as {})",
                rust_text::operand(&concrete(g, v)?, Precedence::Cast, false),
                t.rust(g)
            ),
            inference: Inference::ResultLocation,
            ..Value::default()
        });
    }
    // `return null` from a component (React renders nothing): an empty node,
    // which takes no space in its parent's layout
    if v.ty == Type::Null && t == Type::Node {
        return Ok(Value::new(t, "ui::Node::default()"));
    }
    if (v.ty == Type::Node || v.ty == Type::Null) && t == Type::OptionalNode {
        if v.ty == Type::Null {
            return Ok(Value::new(t, "None"));
        }
        let mut result = v;
        result.ty = t;
        result.code = format!("Some({})", result.code);
        result.precedence = Precedence::Suffix;
        return Ok(result);
    }
    if (v.ty.float() && t.integer()) || (v.ty == Type::FloatLiteral && t.integer()) {
        return fail(
            g,
            loc,
            format!(
                "cannot convert {} to {}; float-to-integer conversion is not implicit; use an integer value",
                v.ty.rust(g),
                t.rust(g)
            ),
        );
    }
    // `Uint8Array` is a `&'static [u8]`: an unannotated byte literal lowered to
    // a `tsx::List<u8>` cannot become a static slice after the fact.
    if model::is_static_bytes(g, t) {
        return fail(
            g,
            loc,
            format!(
                "cannot convert {} to {}; annotate byte literals with `const name: Uint8Array = [..]` (or import the bytes)",
                v.ty.rust(g),
                t.rust(g)
            ),
        );
    }
    fail(
        g,
        loc,
        format!(
            "cannot convert {} to {}; use an explicit matching type",
            v.ty.rust(g),
            t.rust(g)
        ),
    )
}

/// Explicit assertions allow numeric conversions that implicit coercion
/// rejects.
pub fn explicit_cast(
    g: &mut Generator<'_>,
    value: Value,
    target: Type,
    loc: ast::Loc,
) -> Result<Value> {
    let mut source = value;
    if source.ty.literal() {
        let concrete_ty = source.ty.concrete();
        source = coerce(g, source, concrete_ty, loc)?;
    }
    if source.ty == target {
        return Ok(source);
    }
    if !source.ty.numeric() || !target.numeric() {
        return fail(
            g,
            loc,
            format!(
                "cannot cast {} to {}; only numeric conversions are supported",
                source.ty.rust(g),
                target.rust(g)
            ),
        );
    }
    let source = concrete(g, source)?;
    Ok(Value {
        ty: target,
        code: format!(
            "({} as {})",
            rust_text::operand(&source, Precedence::Cast, false),
            target.rust(g)
        ),
        inference: Inference::ResultLocation,
        ..Value::default()
    })
}

/// Materialize a contextual literal or conversion into a typed expression.
/// Conversions already carry their `as` cast, so only literals need the
/// typed spelling here.
pub fn concrete(g: &mut Generator<'_>, v: Value) -> Result<Value> {
    if v.inference == Inference::Known || (!v.ty.numeric() && !model::enum_type(g, v.ty)) {
        return Ok(v);
    }
    let mut result = v;
    if result.ty.literal() {
        result.ty = result.ty.concrete();
        result.code = literal_code(&result.code, result.ty);
    }
    result.inference = Inference::Known;
    result.precedence = Precedence::Primary;
    Ok(result)
}

/// Rust literals are polymorphic, so [`concrete`] decides the final type of a
/// literal before it is formatted.
pub fn format_value(g: &mut Generator<'_>, v: Value) -> Result<Value> {
    if v.inference == Inference::Literal || v.inference == Inference::ResultLocation {
        return concrete(g, v);
    }
    Ok(v)
}

/// The Rust spelling of a numeric literal for target type `t`.
///
/// Integer targets get a type suffix (`1u32`); float targets need a decimal
/// point (`1.0f64`, `0.5f32`). Negative literals keep their sign, and
/// hexadecimal/octal/binary integer literals become `as` casts for floats.
pub fn literal_code(raw: &str, t: Type) -> String {
    let (sign, rest) = if let Some(rest) = raw.strip_prefix('-') {
        ("-", rest)
    } else if let Some(rest) = raw.strip_prefix('+') {
        ("", rest)
    } else {
        ("", raw)
    };
    let suffix = match t {
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
        _ => return raw.to_owned(),
    };
    if t.float() {
        let prefixed = rest.starts_with("0x") || rest.starts_with("0X");
        if prefixed {
            return format!("({sign}{rest} as {suffix})");
        }
        if rest.contains('.') || rest.contains('e') || rest.contains('E') {
            return format!("{sign}{rest}{suffix}");
        }
        return format!("{sign}{rest}.0{suffix}");
    }
    format!("{sign}{rest}{suffix}")
}

/// Unify one operand type with the other.
pub fn unify(g: &mut Generator<'_>, a: Type, b: Type, loc: ast::Loc) -> Result<Type> {
    if a == b {
        return Ok(a);
    }
    if a.numeric() && b.numeric() {
        return match Type::numeric_peer(a, b) {
            Some(t) => Ok(t),
            None => fail(
                g,
                loc,
                format!(
                    "different integer types {} and {}; use the same integer type",
                    a.rust(g),
                    b.rust(g)
                ),
            ),
        };
    }
    if a == Type::Null {
        return model::optional(g, b);
    }
    if b == Type::Null {
        return model::optional(g, a);
    }
    if model::optional_child(g, a) == Some(b) {
        return Ok(a);
    }
    if model::optional_child(g, b) == Some(a) {
        return Ok(b);
    }
    let a_node = matches!(a, Type::Node | Type::OptionalNode | Type::Null);
    let b_node = matches!(b, Type::Node | Type::OptionalNode | Type::Null);
    if a_node && b_node {
        return Ok(Type::OptionalNode);
    }
    fail(
        g,
        loc,
        format!("incompatible types {} and {}", a.rust(g), b.rust(g)),
    )
}

/// Only literal-only numeric trees take peer context; typed integers must
/// still unify.
pub fn numeric_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<Option<Type>> {
    Ok(expression_hint(g, c, e)?.filter(|t| t.numeric() && !t.literal()))
}

/// The type an expression is expected to have, without emitting it.
pub fn expression_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<Option<Type>> {
    let hint: Option<Type> = match &e.data {
        ast::ExprData::Number(n) => Some(if float_literal(n) {
            Type::FloatLiteral
        } else {
            Type::IntLiteral
        }),
        ast::ExprData::Ident(n) => name_hint(g, c, n),
        ast::ExprData::NonNull(inner) => expression_hint(g, c, inner)?,
        ast::ExprData::As(cast) => cast_hint(g, &cast.ty),
        ast::ExprData::Call(call) => call_hint(g, call),
        ast::ExprData::Unary(u) => {
            if u.op == b"!" {
                None
            } else {
                expression_hint(g, c, &u.operand)?
            }
        }
        ast::ExprData::Binary(b) => binary_hint(g, c, b)?,
        ast::ExprData::Cond(v) => branch_hint(g, c, v)?,
        ast::ExprData::Member(_) | ast::ExprData::Index(_) => value_hint(g, c, e)?,
        _ => None,
    };
    Ok(hint.filter(|t| t.numeric()))
}

/// A decimal literal cannot adopt an integer operand's type; it makes the
/// arithmetic floating point, even when the other side is integer.
pub fn numeric_context(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
    candidate: Option<Type>,
) -> Result<Option<Type>> {
    let Some(t) = candidate else {
        return Ok(None);
    };
    let Some(hint) = expression_hint(g, c, e)? else {
        return Ok(Some(t));
    };
    if t.integer() && (hint.float() || hint == Type::FloatLiteral) {
        return Ok(None);
    }
    Ok(Some(t))
}

/// Reject an out-of-range integer literal at its TSX source position.
pub fn check_literal(g: &mut Generator<'_>, e: &ast::Expr, t: Type) -> Result<()> {
    if !t.integer() {
        return Ok(());
    }
    let raw: Vec<u8> = match &e.data {
        ast::ExprData::Number(n) => n.clone(),
        ast::ExprData::Unary(u) => match negative_literal(u) {
            Some(raw) => raw.into_bytes(),
            None => return Ok(()),
        },
        _ => return Ok(()),
    };
    if float_literal(&raw) {
        return Ok(());
    }
    let Some(n) = parse_integer(&raw) else {
        return fail(
            g,
            e.loc,
            format!("integer literal does not fit {}", t.rust(g)),
        );
    };
    let Some(info) = t.int_info() else {
        return Ok(());
    };
    let bound: i128 = 1i128 << (info.bits - if info.signed { 1 } else { 0 });
    let fits = if info.signed {
        n >= -bound && n < bound
    } else {
        n >= 0 && n < bound
    };
    if !fits {
        return fail(
            g,
            e.loc,
            format!(
                "integer literal does not fit {}; choose a wider or signed type",
                t.rust(g)
            ),
        );
    }
    Ok(())
}

/// Parse a TSX integer literal (decimal or `0x`/`0o`/`0b`, with `_`
/// separators) into an `i128`; `None` when it does not fit.
pub fn parse_integer(raw: &[u8]) -> Option<i128> {
    let text = std::str::from_utf8(raw).ok()?.replace('_', "");
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1i128, rest),
        None => (1i128, text.strip_prefix('+').unwrap_or(&text)),
    };
    let (radix, digits) = if let Some(rest) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16, rest)
    } else if let Some(rest) = digits
        .strip_prefix("0o")
        .or_else(|| digits.strip_prefix("0O"))
    {
        (8, rest)
    } else if let Some(rest) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
        (2, rest)
    } else {
        (10, digits)
    };
    i128::from_str_radix(digits, radix).ok().map(|n| sign * n)
}

pub fn numeric_name(n: &[u8]) -> Option<Type> {
    let ty = match n {
        b"u8" => Type::U8,
        b"u16" => Type::U16,
        b"u32" => Type::U32,
        b"u64" => Type::U64,
        b"i8" => Type::I8,
        b"i16" => Type::I16,
        b"i32" => Type::I32,
        b"i64" => Type::I64,
        b"usize" => Type::Usize,
        b"isize" => Type::Isize,
        b"f32" => Type::F32,
        b"f64" => Type::F64,
        _ => return None,
    };
    Some(ty)
}

pub fn float_literal(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(raw);
    let n = text.trim_start_matches(['+', '-']);
    let hexadecimal = n.starts_with("0x") || n.starts_with("0X");
    !hexadecimal && n.contains(['.', 'e', 'E'])
}

pub fn arithmetic(op: &[u8]) -> bool {
    matches!(op, b"+" | b"-" | b"*" | b"/" | b"%")
}

pub fn numeric_literals(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Number(_) => true,
        ast::ExprData::NonNull(inner) => numeric_literals(inner),
        ast::ExprData::Unary(u) => u.op != b"!" && numeric_literals(&u.operand),
        ast::ExprData::Binary(b) => {
            arithmetic(&b.op) && numeric_literals(&b.lhs) && numeric_literals(&b.rhs)
        }
        ast::ExprData::Cond(v) => numeric_literals(&v.then) && numeric_literals(&v.else_),
        _ => false,
    }
}

pub fn is_literal(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Number(_) | ast::ExprData::String(_) | ast::ExprData::Boolean(_) => true,
        ast::ExprData::Unary(u) => {
            u.op == b"-" && matches!(u.operand.data, ast::ExprData::Number(_))
        }
        _ => false,
    }
}

/// Resolve a local before consulting state, matching ordinary identifier reads.
fn name_hint(g: &mut Generator<'_>, c: &mut scope::Context, name: &[u8]) -> Option<Type> {
    if let Some(binding) = scope::local(c, name) {
        return Some(binding.ty);
    }
    if let Some(index) = scope::state_index(g, name) {
        return Some(g.states[index].ty);
    }
    g.constants
        .iter()
        .find(|constant| constant.name.as_bytes() == name)
        .map(|constant| constant.value.ty)
}

/// Only a named component function has a supported call result type.
fn call_hint(g: &mut Generator<'_>, call: &ast::Call) -> Option<Type> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return None;
    };
    let index = scope::source_method(g, name)?;
    Some(g.methods[index].ret)
}

/// Infer numeric peers only for supported arithmetic operators.
fn binary_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    b: &ast::Binary,
) -> Result<Option<Type>> {
    if !arithmetic(&b.op) {
        return Ok(None);
    }
    let Some(lhs) = expression_hint(g, c, &b.lhs)? else {
        return Ok(None);
    };
    let Some(rhs) = expression_hint(g, c, &b.rhs)? else {
        return Ok(None);
    };
    Ok(Type::numeric_peer(lhs, rhs))
}

/// Infer a conditional's numeric peer without evaluating its condition.
fn branch_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    value: &ast::Cond,
) -> Result<Option<Type>> {
    let Some(then) = expression_hint(g, c, &value.then)? else {
        return Ok(None);
    };
    let Some(otherwise) = expression_hint(g, c, &value.else_)? else {
        return Ok(None);
    };
    Ok(Type::numeric_peer(then, otherwise))
}

/// Recognize only the supported minus-number literal shape for range checks.
fn negative_literal(unary: &ast::Unary) -> Option<String> {
    if unary.op != b"-" {
        return None;
    }
    let ast::ExprData::Number(n) = &unary.operand.data else {
        return None;
    };
    Some(format!("-{}", String::from_utf8_lossy(n)))
}

/// A numeric cast supplies the peer type even when its operand was untyped.
fn cast_hint(g: &mut Generator<'_>, reference: &ast::TypeRef) -> Option<Type> {
    if reference.detail.is_some() {
        return None;
    }
    for alias in &g.type_aliases {
        if alias.name.as_bytes() == reference.name {
            return Some(alias.ty);
        }
    }
    if reference.name == b"number" {
        return Some(Type::F64);
    }
    numeric_name(&reference.name)
}

/// Inspect members and indexes without emitting expressions or registering
/// captures.
fn value_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    e: &ast::Expr,
) -> Result<Option<Type>> {
    match &e.data {
        ast::ExprData::Ident(name) => Ok(name_hint(g, c, name)),
        ast::ExprData::NonNull(inner) => {
            Ok(value_hint(g, c, inner)?.map(|ty| model::optional_child(g, ty).unwrap_or(ty)))
        }
        ast::ExprData::Index(ix) => {
            Ok(value_hint(g, c, &ix.object)?.and_then(|ty| model::element(g, ty)))
        }
        ast::ExprData::Member(member) => member_hint(g, c, member),
        ast::ExprData::Call(call) => Ok(call_hint(g, call)),
        _ => Ok(None),
    }
}

fn member_hint(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    member: &ast::Member,
) -> Result<Option<Type>> {
    let Some(ty) = value_hint(g, c, &member.object)? else {
        return Ok(None);
    };
    if member.name.name == b"length" && (ty == Type::String || model::element(g, ty).is_some()) {
        return Ok(Some(Type::Usize));
    }
    let Some(entry) = model::info(g, ty) else {
        return Ok(None);
    };
    if entry.kind == model::InfoKind::Object {
        for field in &entry.fields {
            if field.name == member.name.name {
                return Ok(Some(field.ty));
            }
        }
    }
    Ok(None)
}

/// Element type names accepted both in annotations and Zant / React named
/// imports.
pub fn node_name(name: &[u8]) -> bool {
    matches!(
        name,
        b"JSX.Element"
            | b"React.ReactNode"
            | b"ReactNode"
            | b"React.ReactElement"
            | b"ReactElement"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, ExprData};

    fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        f(&mut g);
    }

    #[test]
    fn integer_peers_prefer_the_wider_type() {
        assert_eq!(Some(Type::Usize), Type::numeric_peer(Type::U8, Type::Usize));
        assert_eq!(Some(Type::I32), Type::numeric_peer(Type::U16, Type::I32));
        // u64 and i32 hold different ranges: a TS number
        assert_eq!(Some(Type::F64), Type::numeric_peer(Type::U64, Type::I32));
        assert_eq!(
            Some(Type::F32),
            Type::numeric_peer(Type::F32, Type::IntLiteral)
        );
        assert_eq!(
            Some(Type::F64),
            Type::numeric_peer(Type::FloatLiteral, Type::I32)
        );
        assert_eq!(None, Type::numeric_peer(Type::Boolean, Type::F64));
    }

    #[test]
    fn literal_codes_are_typed_for_the_target() {
        assert_eq!("1u32", literal_code("1", Type::U32));
        assert_eq!("-2i64", literal_code("-2", Type::I64));
        assert_eq!("1.0f64", literal_code("1", Type::F64));
        assert_eq!("0.5f32", literal_code("0.5", Type::F32));
        assert_eq!("1e3f64", literal_code("1e3", Type::F64));
        assert_eq!("(0xff as f64)", literal_code("0xff", Type::F64));
    }

    #[test]
    fn integer_literals_parse_radix_and_separators() {
        assert_eq!(Some(255), parse_integer(b"0xff"));
        assert_eq!(Some(-2), parse_integer(b"-2"));
        assert_eq!(Some(1_000_000), parse_integer(b"1_000_000"));
        assert_eq!(Some(8), parse_integer(b"0o10"));
        assert_eq!(Some(5), parse_integer(b"0b101"));
        assert_eq!(None, parse_integer(b"1.5"));
    }

    #[test]
    fn float_literals_are_decimal_only() {
        assert!(float_literal(b"1.5"));
        assert!(float_literal(b"1e3"));
        assert!(!float_literal(b"15"));
        assert!(!float_literal(b"0xff"));
    }

    #[test]
    fn out_of_range_literals_report_the_source_location() {
        with_generator(|g| {
            let e = Expr {
                loc: 3,
                data: ExprData::Number(b"300".to_vec()),
            };
            assert!(check_literal(g, &e, Type::U8).is_err());
            let diag = g.diag.as_ref().expect("a diagnostic");
            assert!(diag.msg.contains("does not fit u8"), "{}", diag.msg);
            assert_eq!(3, diag.loc);
        });
    }

    #[test]
    fn integer_literals_inside_the_range_pass() {
        with_generator(|g| {
            let e = Expr {
                loc: 0,
                data: ExprData::Number(b"200".to_vec()),
            };
            assert!(check_literal(g, &e, Type::U8).is_ok());
            assert!(check_literal(g, &e, Type::I8).is_err());
        });
    }

    #[test]
    fn unify_null_with_a_concrete_type_is_optional() {
        with_generator(|g| {
            let ty = unify(g, Type::String, Type::Null, 0).expect("unifies");
            assert_eq!(Some(Type::String), model::optional_child(g, ty));
            assert!(ty.rust(g).starts_with("Option<"));
            let nodes = unify(g, Type::Node, Type::Null, 0).expect("unifies");
            assert_eq!(Type::OptionalNode, nodes);
        });
    }

    #[test]
    fn is_literal_recognizes_negative_number_literals() {
        let negative = Expr {
            loc: 0,
            data: ExprData::Unary(ast::Unary {
                op: b"-".to_vec(),
                operand: Box::new(Expr {
                    loc: 1,
                    data: ExprData::Number(b"1".to_vec()),
                }),
            }),
        };
        assert!(is_literal(&negative));
        assert!(numeric_literals(&negative));
        assert!(!is_literal(&Expr {
            loc: 0,
            data: ExprData::Ident(b"x".to_vec()),
        }));
    }
}
