//! Interned compound types: arrays, optionals, unions / string-literal enums
//! and interface structs. Translation of `tools/tsx/codegen/model.zig`.
//!
//! Zig interns each compound as a tag above [`types::Type`]'s primitives; Rust
//! uses [`types::Type::Compound`], an index into [`Generator::compounds`].
//! Interface declarations are emitted as
//! `#[derive(Clone, PartialEq, Default)] pub struct` and string-literal unions
//! as `#[derive(Clone, Copy, Default, PartialEq, Eq)] pub enum` with the
//! `as_str()` accessor frozen in `docs/rust-port/tsx-codegen.md` §2.

use crate::ast;
use crate::tags;

use super::rust_text;
use super::types::{self, Type};
use super::{Generator, Result, fail};

#[derive(Clone, Debug)]
pub struct Field {
    pub name: Vec<u8>,
    pub ty: Type,
    pub optional: bool,
    /// The Rust field when it differs from the TSX name (`colorPrimary` →
    /// `color_primary` on the theme).
    pub rust: Option<String>,
}

/// The Rust spelling of field `name` of `ty` (its TSX name unless aliased).
pub fn field_rust(g: &Generator<'_>, ty: Type, name: &[u8]) -> Option<String> {
    let entry = info(g, ty)?;
    entry
        .fields
        .iter()
        .find(|field| field.name == name)
        .and_then(|field| field.rust.clone())
}

/// `zant::theme::Theme` as an object type with camelCase fields (`useTheme()`).
pub fn theme_type(g: &mut Generator<'_>) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.rust == "zant::theme::Theme" {
            return Ok(Type::Compound(index as u32));
        }
    }
    let fields = tags::theme_fields()
        .iter()
        .map(|prop| Field {
            name: prop.name.as_bytes().to_vec(),
            ty: if prop.kind == tags::Kind::Number {
                Type::F32
            } else {
                Type::U32
            },
            optional: false,
            rust: Some(prop.field.to_owned()),
        })
        .collect();
    add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: "zant::theme::Theme".to_owned(),
            copy: true,
            fields,
            ..Info::default()
        },
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InfoKind {
    #[default]
    Array,
    Object,
    Optional,
    Enumeration,
    Callback,
}

#[derive(Clone, Debug)]
pub struct Info {
    pub kind: InfoKind,
    /// The Rust type spelling.
    pub rust: String,
    pub child: Type,
    pub fields: Vec<Field>,
    pub members: Vec<Vec<u8>>,
    pub declaration: Option<String>,
    /// `.callback`: the parameter types (callbacks return nothing).
    pub params: Vec<Type>,
    /// Whether the type is `Copy` (string-literal enums, the theme).
    pub copy: bool,
}

impl Default for Info {
    fn default() -> Self {
        Info {
            kind: InfoKind::Array,
            rust: String::new(),
            child: Type::Void,
            fields: Vec::new(),
            members: Vec::new(),
            declaration: None,
            params: Vec::new(),
            copy: false,
        }
    }
}

/// The info of a compound type, cloned so callers may keep mutating the
/// generator (Zig returns a stable pointer).
pub fn info(g: &Generator<'_>, ty: Type) -> Option<Info> {
    match ty {
        Type::Compound(index) => g.compounds.get(index as usize).cloned(),
        _ => None,
    }
}

fn info_mut<'g>(g: &'g mut Generator<'_>, ty: Type) -> &'g mut Info {
    match ty {
        Type::Compound(index) => &mut g.compounds[index as usize],
        _ => unreachable!("compound type expected"),
    }
}

pub fn add(g: &mut Generator<'_>, value: Info) -> Result<Type> {
    let index = g.compounds.len() as u32;
    g.compounds.push(value);
    Ok(Type::Compound(index))
}

pub fn array(g: &mut Generator<'_>, child: Type) -> Result<Type> {
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == InfoKind::Array && entry.child == child {
            return Ok(Type::Compound(index as u32));
        }
    }
    let rust = format!("tsx::List<{}>", child.rust(g));
    add(
        g,
        Info {
            kind: InfoKind::Array,
            rust,
            child,
            ..Info::default()
        },
    )
}

pub fn optional(g: &mut Generator<'_>, child: Type) -> Result<Type> {
    if child == Type::Null {
        return Ok(child);
    }
    if child == Type::Node {
        return Ok(Type::OptionalNode);
    }
    if optional_child(g, child).is_some() {
        return Ok(child);
    }
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == InfoKind::Optional && entry.child == child {
            return Ok(Type::Compound(index as u32));
        }
    }
    let rust = format!("Option<{}>", child.rust(g));
    add(
        g,
        Info {
            kind: InfoKind::Optional,
            rust,
            child,
            copy: child.copy(g),
            ..Info::default()
        },
    )
}

pub fn optional_child(g: &Generator<'_>, ty: Type) -> Option<Type> {
    if ty == Type::OptionalNode {
        return Some(Type::Node);
    }
    let entry = info(g, ty)?;
    if entry.kind == InfoKind::Optional {
        Some(entry.child)
    } else {
        None
    }
}

pub fn element(g: &Generator<'_>, ty: Type) -> Option<Type> {
    let entry = info(g, ty)?;
    if entry.kind == InfoKind::Array {
        Some(entry.child)
    } else {
        None
    }
}

/// The parameter types when `ty` is (an optional) callback.
fn callback_params(g: &Generator<'_>, ty: Type) -> Option<Vec<Type>> {
    let entry = info(g, ty)?;
    if entry.kind == InfoKind::Callback {
        return Some(entry.params.clone());
    }
    if entry.kind == InfoKind::Optional {
        let child = info(g, entry.child)?;
        if child.kind == InfoKind::Callback {
            return Some(child.params.clone());
        }
    }
    None
}

/// Whether `ty` is an optional callback (default `None`) rather than a bare
/// callback (default no-op closure).
fn model_optional_child_is_none(g: &Generator<'_>, ty: Type) -> bool {
    info(g, ty).is_some_and(|entry| entry.kind == InfoKind::Optional)
}

pub fn owned(g: &Generator<'_>, ty: Type) -> bool {
    let Some(entry) = info(g, ty) else {
        return false;
    };
    match entry.kind {
        InfoKind::Array | InfoKind::Object => true,
        InfoKind::Optional => owned(g, entry.child) || entry.child == Type::String,
        InfoKind::Enumeration | InfoKind::Callback => false,
    }
}

pub fn enum_type(g: &Generator<'_>, ty: Type) -> bool {
    info(g, ty).is_some_and(|entry| entry.kind == InfoKind::Enumeration)
}

/// Resolve nullable unions, immutable arrays, nested objects, and literal
/// enums (Zig `model.resolve`).
pub fn resolve(
    g: &mut Generator<'_>,
    reference: &ast::TypeRef,
    name: Option<&[u8]>,
) -> Result<Type> {
    let Some(detail) = reference.detail.as_deref() else {
        return types::type_ref(g, reference, false);
    };
    match detail {
        ast::TypeDetail::Array(child) => {
            let child = types::type_ref(g, child, false)?;
            array(g, child)
        }
        ast::TypeDetail::Object(fields) => object(g, fields, name),
        ast::TypeDetail::Union(members) => union_type(g, reference.loc, members, name),
        ast::TypeDetail::StringLit(member) => enumeration(g, std::slice::from_ref(member), name),
        ast::TypeDetail::Null => Ok(Type::Null),
        ast::TypeDetail::Func(f) => callback_type(g, reference.loc, f),
    }
}

fn object(g: &mut Generator<'_>, fields: &[ast::TypeField], name: Option<&[u8]>) -> Result<Type> {
    let external = match name {
        Some(name) => match g.extensions.hooks.external_name {
            Some(hook) => hook(g, &String::from_utf8_lossy(name))?,
            None => None,
        },
        None => None,
    };
    if name.is_none() && g.type_scope {
        return anonymous_external(g);
    }
    let is_external = external.is_some();
    let rust = match external {
        Some(external) => external,
        None => match name {
            Some(name) => rust_text::id_at(g, name, 0)?,
            None => format!("_Object{}", g.compounds.len()),
        },
    };
    let ty = add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: rust.clone(),
            ..Info::default()
        },
    )?;
    // another file's type keeps its alias key so it never shadows a type of
    // this file
    if let Some(name) = name {
        let alias = if rust != String::from_utf8_lossy(name) {
            rust.clone()
        } else {
            String::from_utf8_lossy(name).into_owned()
        };
        g.type_aliases.push(super::TypeAlias { name: alias, ty });
    }
    let mut result: Vec<Field> = Vec::new();
    let mut code = String::new();
    let mut has_callback = false;
    for field in fields {
        if result
            .iter()
            .any(|previous| previous.name == field.name.name)
        {
            return fail(
                g,
                field.name.loc,
                format!(
                    "duplicate field `{}`",
                    String::from_utf8_lossy(&field.name.name)
                ),
            );
        }
        let mut field_ty = types::type_ref(g, &field.ty, false)?;
        if field.optional {
            field_ty = optional(g, field_ty)?;
        }
        has_callback = has_callback || callback_params(g, field_ty).is_some();
        result.push(Field {
            name: field.name.name.clone(),
            ty: field_ty,
            optional: field.optional,
            rust: None,
        });
        code.push_str(&format!(
            "    pub {}: {},\n",
            rust_text::id_at(g, &field.name.name, field.name.loc)?,
            field_ty.rust(g)
        ));
    }
    // `Rc<dyn Fn(..)>` implements neither `Default` nor `PartialEq`, so structs
    // that hold callback props derive `Clone` only and get a hand-written
    // `Default` with a no-op closure.
    let mut declaration = if has_callback {
        format!("#[derive(Clone)]\npub struct {rust} {{\n{code}}}\n\n")
    } else {
        format!("#[derive(Clone, PartialEq, Default)]\npub struct {rust} {{\n{code}}}\n\n")
    };
    if has_callback {
        let mut defaults = String::new();
        for field in &result {
            let field_name = rust_text::id_at(g, &field.name, 0)?;
            let default = match callback_params(g, field.ty) {
                Some(params) if model_optional_child_is_none(g, field.ty) => {
                    let _ = params;
                    "None".to_owned()
                }
                Some(params) => {
                    let args = params
                        .iter()
                        .map(|ty| format!("_: {}", ty.rust(g)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("std::rc::Rc::new(|{args}| ())")
                }
                None => "Default::default()".to_owned(),
            };
            defaults.push_str(&format!("            {field_name}: {default},\n"));
        }
        declaration.push_str(&format!(
            "impl Default for {rust} {{\n    fn default() -> Self {{\n        Self {{\n{defaults}        }}\n    }}\n}}\n\n"
        ));
    }
    let info = info_mut(g, ty);
    info.fields = result;
    // declared by the other file
    info.declaration = if is_external { None } else { Some(declaration) };
    Ok(ty)
}

/// Inline object / union types inside another file's declarations would be
/// separate Rust types in each file.
fn anonymous_external<T>(g: &mut Generator<'_>) -> Result<T> {
    fail(
        g,
        0,
        "types shared between files must be named: declare nested object and union types as interfaces or type aliases",
    )
}

fn union_type(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    members: &[ast::TypeRef],
    name: Option<&[u8]>,
) -> Result<Type> {
    let mut nullable = false;
    let mut nonnull: Vec<ast::TypeRef> = Vec::new();
    for member in members {
        if matches!(member.detail.as_deref(), Some(ast::TypeDetail::Null)) {
            nullable = true;
        } else {
            nonnull.push(member.clone());
        }
    }
    if nonnull.is_empty() {
        return Ok(Type::Null);
    }
    if let Some(list) = node_or_nodes(g, &nonnull)? {
        return if nullable {
            optional(g, list)
        } else {
            Ok(list)
        };
    }
    let ty = if nonnull.len() == 1 {
        if nonnull[0].detail.is_some() {
            resolve(g, &nonnull[0], name)?
        } else {
            types::type_ref(g, &nonnull[0], false)?
        }
    } else {
        let mut names: Vec<Vec<u8>> = Vec::new();
        for member in &nonnull {
            let Some(ast::TypeDetail::StringLit(member)) = member.detail.as_deref() else {
                return fail(
                    g,
                    loc,
                    "unions must contain string literals or one type and null",
                );
            };
            names.push(member.clone());
        }
        enumeration(g, &names, name)?
    };
    if nullable { optional(g, ty) } else { Ok(ty) }
}

/// `JSX.Element | JSX.Element[]` (React's `children` that is one element or
/// several): a list of elements, which one child also fills.
fn node_or_nodes(g: &mut Generator<'_>, members: &[ast::TypeRef]) -> Result<Option<Type>> {
    if members.len() != 2 {
        return Ok(None);
    }
    for (index, member) in members.iter().enumerate() {
        let other = &members[1 - index];
        let Some(other_detail) = other.detail.as_deref() else {
            continue;
        };
        if member.detail.is_some() || !matches!(other_detail, ast::TypeDetail::Array(_)) {
            continue;
        }
        if types::type_ref(g, member, false)? != Type::Node {
            return Ok(None);
        }
        let list = types::type_ref(g, other, false)?;
        return Ok(if element(g, list) == Some(Type::Node) {
            Some(list)
        } else {
            None
        });
    }
    Ok(None)
}

fn enumeration(g: &mut Generator<'_>, members: &[Vec<u8>], name: Option<&[u8]>) -> Result<Type> {
    let external = match name {
        Some(name) => match g.extensions.hooks.external_name {
            Some(hook) => hook(g, &String::from_utf8_lossy(name))?,
            None => None,
        },
        None => None,
    };
    if name.is_none() && g.type_scope {
        return anonymous_external(g);
    }
    let is_external = external.is_some();
    let rust = match external {
        Some(external) => external,
        None => match name {
            Some(name) => rust_text::id_at(g, name, 0)?,
            None => format!("_Enum{}", g.compounds.len()),
        },
    };
    let mut names: Vec<Vec<u8>> = Vec::new();
    for member in members {
        if !names.iter().any(|previous| previous == member) {
            names.push(member.clone());
        }
    }
    let mut variants: Vec<String> = Vec::new();
    let mut declaration = String::new();
    let mut as_strs = String::new();
    for member in &names {
        let Some(variant) = rust_text::enum_variant(member) else {
            return fail(g, 0, "enum member is not valid UTF-8");
        };
        let text = match rust_text::quote(member) {
            Some(text) => text,
            None => return fail(g, 0, "enum member is not valid UTF-8"),
        };
        variants.push(format!("    {variant},"));
        as_strs.push_str(&format!("            {rust}::{variant} => {text},\n"));
    }
    if !names.is_empty() {
        variants[0] = format!("    #[default]\n{}", variants[0]);
        declaration.push_str(&format!(
            "#[derive(Clone, Copy, Default, PartialEq, Eq)]\npub enum {rust} {{\n{}\n}}\n\n",
            variants.join("\n")
        ));
        declaration.push_str(&format!(
            "impl {rust} {{\n    pub fn as_str(self) -> &'static str {{\n        match self {{\n{as_strs}        }}\n    }}\n}}\n\n"
        ));
    }
    add(
        g,
        Info {
            kind: InfoKind::Enumeration,
            rust,
            declaration: if is_external || names.is_empty() {
                None
            } else {
                Some(declaration)
            },
            members: names,
            copy: true,
            ..Info::default()
        },
    )
}

/// Lazily resolve named declarations, permitting references to later
/// declarations (Zig `model.named`).
pub fn named(g: &mut Generator<'_>, name: &[u8]) -> Result<Option<Type>> {
    if g.type_scope {
        return match g.extensions.hooks.imported_type {
            Some(hook) => hook(g, &String::from_utf8_lossy(name)),
            None => Ok(None),
        };
    }
    if let Some(alias) = g
        .type_aliases
        .iter()
        .find(|alias| alias.name.as_bytes() == name)
    {
        return Ok(Some(alias.ty));
    }
    if let Some(hook) = g.extensions.hooks.imported_type
        && let Some(ty) = hook(g, &String::from_utf8_lossy(name))?
    {
        return Ok(Some(ty));
    }
    if let Some(hook) = g.extensions.hooks.zig_type
        && let Some(ty) = hook(g, &String::from_utf8_lossy(name))?
    {
        return Ok(Some(ty));
    }
    let decl = g.type_decls.iter().position(|decl| decl.name.name == name);
    let Some(index) = decl else {
        return Ok(None);
    };
    if g.resolving[index] {
        return fail(
            g,
            g.type_decls[index].name.loc,
            "recursive types are not supported yet",
        );
    }
    g.resolving[index] = true;
    let declaration = g.type_decls[index].clone();
    let resolved = resolve(g, &declaration.ty, Some(name));
    g.resolving[index] = false;
    let ty = resolved?;
    if let Some(detail) = declaration.ty.detail.as_deref() {
        g.resolved_types.push(super::ResolvedType {
            detail: super::detail_address(detail),
            ty,
        });
    }
    if let Some(alias) = g
        .type_aliases
        .iter()
        .find(|alias| alias.name.as_bytes() == name)
    {
        return Ok(Some(alias.ty));
    }
    g.type_aliases.push(super::TypeAlias {
        name: String::from_utf8_lossy(name).into_owned(),
        ty,
    });
    Ok(Some(ty))
}

pub fn field_type(g: &mut Generator<'_>, ty: Type, name: &[u8], loc: ast::Loc) -> Result<Type> {
    let spelling = ty.rust(g).to_owned();
    let Some(entry) = info(g, ty) else {
        return fail(
            g,
            loc,
            format!(
                "{spelling} has no field `{}`",
                String::from_utf8_lossy(name)
            ),
        );
    };
    if entry.kind == InfoKind::Object {
        for field in &entry.fields {
            if field.name == name {
                return Ok(field.ty);
            }
        }
    }
    if let Some(near) = closest_field(&entry.fields, name) {
        return fail(
            g,
            loc,
            format!(
                "{spelling} has no field `{}`; did you mean `{}`?",
                String::from_utf8_lossy(name),
                String::from_utf8_lossy(&near)
            ),
        );
    }
    fail(
        g,
        loc,
        format!(
            "{spelling} has no field `{}`",
            String::from_utf8_lossy(name)
        ),
    )
}

/// The field whose name is within a couple of edits of `name` (typos).
fn closest_field(fields: &[Field], name: &[u8]) -> Option<Vec<u8>> {
    let mut best: Option<Vec<u8>> = None;
    let mut best_distance = 3usize;
    for field in fields {
        let distance = edit_distance(&field.name, name);
        if distance < best_distance {
            best = Some(field.name.clone());
            best_distance = distance;
        }
    }
    best
}

/// Levenshtein distance for short identifiers (longer ones count as far).
pub fn edit_distance(a: &[u8], b: &[u8]) -> usize {
    if a.len() > 64 || b.len() > 64 {
        return usize::MAX;
    }
    let mut row = [0usize; 65];
    for (index, slot) in row.iter_mut().enumerate().take(b.len() + 1) {
        *slot = index;
    }
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

/// `(id: number) => void` as a callback type (interned by signature): the
/// default `Rc<dyn Fn(..)>` mapping of `docs/rust-port/tsx-codegen.md` §2.
/// WP-5.5 maps UI event signatures to `ui::Action` through
/// [`Generator::extensions`].
pub fn callback_type(g: &mut Generator<'_>, loc: ast::Loc, f: &ast::FuncType) -> Result<Type> {
    if let Some(hook) = g.extensions.hooks.callback_type {
        return hook(g, loc, f);
    }
    default_callback_type(g, loc, f)
}

/// The generic closure shape; used when WP-5.5 has not installed its
/// action-aware mapping.
pub fn default_callback_type(
    g: &mut Generator<'_>,
    loc: ast::Loc,
    f: &ast::FuncType,
) -> Result<Type> {
    let ret = types::type_ref(g, &f.ret, true)?;
    if ret != Type::Void {
        return fail(g, loc, "callback props must return void");
    }
    let mut params: Vec<Type> = Vec::new();
    for param in &f.params {
        let Some(reference) = param.ty.as_ref() else {
            return fail(
                g,
                param.name.loc,
                format!(
                    "callback parameter `{}` needs a type",
                    String::from_utf8_lossy(&param.name.name)
                ),
            );
        };
        params.push(types::type_ref(g, reference, false)?);
    }
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == InfoKind::Callback && entry.params == params {
            return Ok(Type::Compound(index as u32));
        }
    }
    let list = params
        .iter()
        .map(|ty| ty.rust(g).to_owned())
        .collect::<Vec<_>>()
        .join(", ");
    add(
        g,
        Info {
            kind: InfoKind::Callback,
            rust: format!("std::rc::Rc<dyn Fn({list})>"),
            params,
            ..Info::default()
        },
    )
}

/// The declarations of every compound type that declares one, in interning
/// order (Zig `globals.declarations`). The WP-5.5 entry point prepends these
/// to the generated module.
pub fn declarations(g: &Generator<'_>) -> String {
    let mut out = String::new();
    for entry in &g.compounds {
        if let Some(declaration) = entry.declaration.as_ref() {
            out.push_str(declaration);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{Generator, types};

    fn parse(src: &str) -> ast::Module {
        let mut parser = crate::parser::Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    fn with_module(src: &str, f: impl FnOnce(&mut Generator<'_>, &ast::Module)) {
        let module = parse(src);
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        g.type_decls = module.types.clone();
        g.resolving = vec![false; module.types.len()];
        f(&mut g, &module);
    }

    #[test]
    fn edit_distance_is_levenshtein() {
        assert_eq!(3, edit_distance(b"kitten", b"sitting"));
        assert_eq!(0, edit_distance(b"same", b"same"));
        assert_eq!(3, edit_distance(b"abc", b""));
        assert_eq!(usize::MAX, edit_distance(&[b'a'; 65], b"a"));
    }

    #[test]
    fn arrays_and_optionals_are_interned() {
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        let list = array(&mut g, Type::F64).unwrap();
        assert_eq!(list, array(&mut g, Type::F64).unwrap());
        assert_eq!(Some(Type::F64), element(&g, list));
        assert_eq!("tsx::List<f64>", list.rust(&g));
        let maybe = optional(&mut g, list).unwrap();
        assert_eq!(maybe, optional(&mut g, list).unwrap());
        assert_eq!(Some(list), optional_child(&g, maybe));
        assert!(maybe.rust(&g).starts_with("Option<"));
        // `T | null` around a node uses the dedicated primitive
        assert_eq!(Type::OptionalNode, optional(&mut g, Type::Node).unwrap());
        assert_eq!(Type::Null, optional(&mut g, Type::Null).unwrap());
    }

    #[test]
    fn string_unions_declare_copy_enums_with_as_str() {
        with_module(r#"type Mode = "a" | "in-progress";"#, |g, _| {
            let ty = named(g, b"Mode").unwrap().expect("resolved");
            assert!(enum_type(g, ty));
            assert!(ty.copy(g));
            let declaration = declarations(g);
            assert!(
                declaration.contains("#[derive(Clone, Copy, Default, PartialEq, Eq)]"),
                "{declaration}"
            );
            assert!(declaration.contains("InProgress,"), "{declaration}");
            assert!(
                declaration.contains("Mode::InProgress => \"in-progress\","),
                "{declaration}"
            );
        });
    }

    #[test]
    fn interfaces_declare_structs_in_field_order() {
        with_module(
            r#"interface Point { x: number; y: number; label?: string }"#,
            |g, _| {
                let ty = named(g, b"Point").unwrap().expect("resolved");
                let declaration = declarations(g);
                assert!(
                    declaration.contains("#[derive(Clone, PartialEq, Default)]"),
                    "{declaration}"
                );
                let x = declaration.find("pub x: f64").unwrap();
                let y = declaration.find("pub y: f64").unwrap();
                assert!(x < y);
                assert!(declaration.contains("pub label: Option<zant::Str>,"));
                assert_eq!(Some(Type::F64), field_type(g, ty, b"x", 0).ok());
                let _ = types::Type::F64;
            },
        );
    }

    #[test]
    fn unknown_fields_suggest_the_closest_name() {
        with_module(
            r#"interface Row { title: string; done: boolean }"#,
            |g, _| {
                let ty = named(g, b"Row").unwrap().expect("resolved");
                assert!(field_type(g, ty, b"titel", 0).is_err());
                let diag = g.diag.as_ref().expect("a diagnostic");
                assert!(diag.msg.contains("did you mean `title`"), "{}", diag.msg);
            },
        );
    }

    #[test]
    fn recursive_types_are_a_diagnostic() {
        with_module(r#"type A = B; type B = A;"#, |g, _| {
            assert!(named(g, b"A").is_err());
            let diag = g.diag.as_ref().expect("a diagnostic");
            assert!(
                diag.msg.contains("recursive types are not supported"),
                "{}",
                diag.msg
            );
        });
    }
}
