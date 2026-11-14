//! Model reflection for `#[derive(Table)]`: the Zig comptime part that turned
//! struct fields into columns, SQL types and defaults.

use syn::{Data, DeriveInput, Error, Fields, LitStr, Type};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldKind {
    Integer,
    Float,
    Boolean,
    Text,
    Enum,
}

impl FieldKind {
    /// Zig `Table(T).sqlType`.
    pub(super) fn sql_type(self) -> &'static str {
        match self {
            FieldKind::Integer | FieldKind::Boolean => "INTEGER",
            FieldKind::Float => "REAL",
            FieldKind::Text | FieldKind::Enum => "TEXT",
        }
    }
}

pub(super) struct ModelField {
    pub(super) ident: syn::Ident,
    /// The field type with any `Option<..>` stripped.
    pub(super) inner: Type,
    pub(super) kind: FieldKind,
    pub(super) optional: bool,
    /// Field named `id` (Zig `isId`): skipped by INSERT/UPDATE and used by the
    /// `WHERE id = ?` clauses. May itself be optional.
    pub(super) is_id: bool,
    /// `#[table(default = "tag")]`, only meaningful for enum fields.
    pub(super) default_tag: Option<String>,
}

pub(super) fn model_fields(input: &DeriveInput) -> syn::Result<Vec<ModelField>> {
    let data = match &input.data {
        Data::Struct(data) => data,
        _ => {
            return Err(Error::new_spanned(
                input,
                "`#[derive(Table)]` can only be derived for structs",
            ));
        }
    };
    let Fields::Named(named) = &data.fields else {
        return Err(Error::new_spanned(
            &data.fields,
            "`#[derive(Table)]` needs a struct with named fields",
        ));
    };
    named.named.iter().map(model_field).collect()
}

fn model_field(field: &syn::Field) -> syn::Result<ModelField> {
    let ident = field
        .ident
        .clone()
        .ok_or_else(|| Error::new_spanned(field, "`#[derive(Table)]` needs named fields"))?;
    let default_tag = field_default_tag(field)?;
    let (inner, optional) = unwrap_option(&field.ty)?;
    let kind = classify(&inner)?;
    if default_tag.is_some() && kind != FieldKind::Enum {
        return Err(Error::new_spanned(
            &ident,
            "`#[table(default = \"..\")]` is only supported on enum fields",
        ));
    }
    let is_id = ident == "id";
    Ok(ModelField {
        ident,
        inner,
        kind,
        optional,
        is_id,
        default_tag,
    })
}

fn field_default_tag(field: &syn::Field) -> syn::Result<Option<String>> {
    let mut tag = None;
    for attr in &field.attrs {
        if !attr.path().is_ident("table") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("default") {
                let value: LitStr = meta.value()?.parse()?;
                tag = Some(value.value());
                Ok(())
            } else {
                Err(meta.error("unknown field `table` attribute, expected `default = \"..\"`"))
            }
        })?;
    }
    Ok(tag)
}

/// Strip one `Option<..>` layer (Zig `?T` → `Option<T>`).
fn unwrap_option(ty: &Type) -> syn::Result<(Type, bool)> {
    let Type::Path(path) = ty else {
        return Err(Error::new_spanned(
            ty,
            "unsupported Table model field type; use `String` for TEXT and a Rust enum for an enum column",
        ));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(Error::new_spanned(ty, "unsupported Table model field type"));
    };
    if segment.ident != "Option" {
        return Ok((ty.clone(), false));
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(Error::new_spanned(ty, "`Option` needs a type argument"));
    };
    let Some(syn::GenericArgument::Type(inner)) = arguments.args.first() else {
        return Err(Error::new_spanned(ty, "`Option` needs a type argument"));
    };
    if matches!(inner, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Option"))
    {
        return Err(Error::new_spanned(ty, "nested `Option` is not supported"));
    }
    Ok((inner.clone(), true))
}

fn classify(ty: &Type) -> syn::Result<FieldKind> {
    let Type::Path(path) = ty else {
        return Err(Error::new_spanned(
            ty,
            "unsupported Table model field type; use `String` for TEXT, not a slice or reference",
        ));
    };
    if path.qself.is_some() {
        return Err(Error::new_spanned(ty, "unsupported Table model field type"));
    }
    let Some(segment) = path.path.segments.last() else {
        return Err(Error::new_spanned(ty, "unsupported Table model field type"));
    };
    let name = segment.ident.to_string();
    let kind = match name.as_str() {
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
            FieldKind::Integer
        }
        "f32" | "f64" => FieldKind::Float,
        "bool" => FieldKind::Boolean,
        "String" => FieldKind::Text,
        _ => {
            if !matches!(segment.arguments, syn::PathArguments::None) {
                return Err(Error::new_spanned(
                    ty,
                    "unsupported Table model field type; only `Option<..>` is generic",
                ));
            }
            FieldKind::Enum
        }
    };
    Ok(kind)
}

pub(super) fn is_f32(ty: &Type) -> bool {
    type_ident(ty).as_deref() == Some("f32")
}

/// Bare identifier of a `/`-separated path type (`i32`, `f64`, `Kind`).
pub(super) fn type_ident(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}
