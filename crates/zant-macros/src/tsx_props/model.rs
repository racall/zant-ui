//! Model extraction for `#[derive(TsxProps)]`: the struct name, doc comment and
//! the fields in declaration order.

use syn::{Data, DeriveInput, Error, Fields, Ident, Type};

/// One `pub struct` deriving `TsxProps`.
#[derive(Debug)]
pub(super) struct StructProps {
    pub(super) ident: Ident,
    /// The struct's `///` doc comment, lines joined with `\n` and the leading
    /// space of each doc line removed. Empty when undocumented.
    pub(super) doc: String,
    pub(super) fields: Vec<FieldProps>,
}

/// One field of a reflected struct.
#[derive(Debug)]
pub(super) struct FieldProps {
    pub(super) ident: Ident,
    pub(super) ty: Type,
    /// `#[tsx(required)]`: the TSX prop must be supplied.
    pub(super) required: bool,
    /// `#[tsx(default = "..")]`: the Rust default expression as written, for
    /// structs that do not use `Default::default()` for this field.
    pub(super) default: Option<String>,
}

impl FieldProps {
    /// The Rust field name with the raw-identifier prefix stripped (`type` for
    /// `r#type`).
    pub(super) fn name(&self) -> String {
        let name = self.ident.to_string();
        name.strip_prefix("r#").unwrap_or(&name).to_string()
    }

    /// The field type as token text (`Option < Str >`); the generator parses it
    /// again to classify it.
    #[cfg(test)]
    pub(super) fn ty_string(&self) -> String {
        let ty = &self.ty;
        quote::quote!(#ty).to_string()
    }
}

impl StructProps {
    pub(super) fn from_input(input: &DeriveInput) -> syn::Result<StructProps> {
        if !input.generics.params.is_empty() {
            return Err(Error::new_spanned(
                &input.generics,
                "`#[derive(TsxProps)]` does not support generic structs",
            ));
        }
        let data = match &input.data {
            Data::Struct(data) => data,
            _ => {
                return Err(Error::new_spanned(
                    input,
                    "`#[derive(TsxProps)]` can only be derived for structs",
                ));
            }
        };
        let Fields::Named(named) = &data.fields else {
            return Err(Error::new_spanned(
                &data.fields,
                "`#[derive(TsxProps)]` needs a struct with named fields",
            ));
        };
        let fields = named
            .named
            .iter()
            .map(FieldProps::from_field)
            .collect::<syn::Result<Vec<_>>>()?;
        Ok(StructProps {
            ident: input.ident.clone(),
            doc: doc_comment(&input.attrs),
            fields,
        })
    }
}

impl FieldProps {
    fn from_field(field: &syn::Field) -> syn::Result<FieldProps> {
        let ident = field
            .ident
            .clone()
            .ok_or_else(|| Error::new_spanned(field, "`#[derive(TsxProps)]` needs named fields"))?;
        let mut required = false;
        let mut default = None;
        for attr in &field.attrs {
            if !attr.path().is_ident("tsx") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("required") {
                    required = true;
                    Ok(())
                } else if meta.path.is_ident("default") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    default = Some(value.value());
                    Ok(())
                } else {
                    Err(meta.error(
                        "unknown field `tsx` attribute, expected `required` or `default = \"..\"`",
                    ))
                }
            })?;
        }
        Ok(FieldProps {
            ident,
            ty: field.ty.clone(),
            required,
            default,
        })
    }
}

/// The `///` lines of an item, with the single leading space `syn` keeps.
fn doc_comment(attrs: &[syn::Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(name_value) = &attr.meta else {
            continue;
        };
        let syn::Expr::Lit(expr) = &name_value.value else {
            continue;
        };
        let syn::Lit::Str(lit) = &expr.lit else {
            continue;
        };
        let value = lit.value();
        lines.push(value.strip_prefix(' ').unwrap_or(&value).to_string());
    }
    lines.join("\n")
}
