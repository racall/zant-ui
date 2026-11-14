//! `#[derive(Table)]` code generation: table name, SQL text, parameter binding
//! and row reading. The SQL text is stable so existing databases keep opening.

use proc_macro2::{Literal, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use syn::{DeriveInput, Error, LitStr, spanned::Spanned};

use self::model::{FieldKind, ModelField, is_f32, model_fields, type_ident};

mod model;

pub(crate) fn expand_table(input: &DeriveInput) -> syn::Result<TokenStream2> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "`#[derive(Table)]` does not support generic models",
        ));
    }
    let model = &input.ident;
    let table_name = struct_table_name(input)?;
    let fields = model_fields(input)?;

    let span = model.span();
    let table_name_lit = LitStr::new(&table_name, span);
    let table_name_sql = format!("\"{table_name}\"");
    let table_name_sql_lit = LitStr::new(&table_name_sql, span);

    let column_names: Vec<String> = fields.iter().map(|f| f.ident.to_string()).collect();
    let columns: Vec<LitStr> = column_names
        .iter()
        .map(|name| LitStr::new(name, span))
        .collect();
    let columns_sql = column_names
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let columns_sql_lit = LitStr::new(&columns_sql, span);

    // CREATE TABLE IF NOT EXISTS "t" ("id" ...).
    let mut create_table_sql = format!("CREATE TABLE IF NOT EXISTS {table_name_sql} (");
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            create_table_sql.push_str(", ");
        }
        create_table_sql.push('"');
        create_table_sql.push_str(&field.ident.to_string());
        create_table_sql.push_str("\" ");
        if field.is_id && !field.optional {
            create_table_sql.push_str("INTEGER PRIMARY KEY AUTOINCREMENT");
        } else {
            create_table_sql.push_str(field.kind.sql_type());
            if !field.optional {
                create_table_sql.push_str(" NOT NULL");
            }
        }
    }
    create_table_sql.push(')');
    let create_table_sql = LitStr::new(&create_table_sql, span);

    // Columns are named in struct field order.
    let select_sql = LitStr::new(&format!("SELECT {columns_sql} FROM {table_name_sql}"), span);
    let select_by_id_sql = LitStr::new(
        &format!("SELECT {columns_sql} FROM {table_name_sql} WHERE id = ? LIMIT 1"),
        span,
    );
    let count_sql = LitStr::new(&format!("SELECT COUNT(*) FROM {table_name_sql}"), span);
    let delete_sql = LitStr::new(&format!("DELETE FROM {table_name_sql} WHERE id = ?"), span);
    let delete_all_sql = LitStr::new(&format!("DELETE FROM {table_name_sql}"), span);

    let insert_columns = quoted_columns(&fields, false);
    let insert_count = fields.iter().filter(|field| !field.is_id).count();
    let insert_placeholders = vec!["?"; insert_count].join(", ");
    let insert_sql = LitStr::new(
        &format!("INSERT INTO {table_name_sql} ({insert_columns}) VALUES ({insert_placeholders})"),
        span,
    );

    let mut update_sets: Vec<String> = Vec::new();
    for field in fields.iter().filter(|field| !field.is_id) {
        update_sets.push(format!("\"{}\" = ?", field.ident));
    }
    let update_sql = LitStr::new(
        &format!(
            "UPDATE {table_name_sql} SET {} WHERE id = ?",
            update_sets.join(", ")
        ),
        span,
    );

    let added_column_arms = added_column_arms(&fields, span);
    let insert_params = params_body(&fields, false);
    let update_params = update_params_body(&fields);
    let field_reads = field_reads(&fields);
    let has_id = fields.iter().any(|field| field.is_id);

    let update_params = if has_id {
        quote! {
            fn update_params(value: &Self) -> ::std::vec::Vec<::zant::db::sqlite::Value> {
                ::std::vec![#(#update_params),*]
            }
        }
    } else {
        quote! {
            // A model without an `id` field cannot be updated; the trait
            // method cannot be left out, so fail at the call site instead.
            fn update_params(_value: &Self) -> ::std::vec::Vec<::zant::db::sqlite::Value> {
                ::std::panic!("`#[derive(Table)]`: the model has no `id` field")
            }
        }
    };

    let expanded = quote! {
        impl ::zant::db::sqlite::TableModel for #model {
            const TABLE_NAME: &'static str = #table_name_lit;
            const TABLE_NAME_SQL: &'static str = #table_name_sql_lit;
            const COLUMNS: &'static [&'static str] = &[#(#columns),*];
            const COLUMNS_SQL: &'static str = #columns_sql_lit;
            const CREATE_TABLE_SQL: &'static str = #create_table_sql;
            const SELECT_SQL: &'static str = #select_sql;
            const SELECT_BY_ID_SQL: &'static str = #select_by_id_sql;
            const INSERT_SQL: &'static str = #insert_sql;
            const UPDATE_SQL: &'static str = #update_sql;
            const DELETE_SQL: &'static str = #delete_sql;
            const DELETE_ALL_SQL: &'static str = #delete_all_sql;
            const COUNT_SQL: &'static str = #count_sql;

            /// Column definition used by `ALTER TABLE ... ADD COLUMN` for a
            /// field added in a later app version.
            fn added_column(index: usize) -> ::std::option::Option<::std::string::String> {
                match index {
                    #(#added_column_arms)*
                    _ => ::std::option::Option::None,
                }
            }

            fn insert_params(value: &Self) -> ::std::vec::Vec<::zant::db::sqlite::Value> {
                ::std::vec![#(#insert_params),*]
            }

            #update_params

            fn from_row(
                row: &::zant::db::sqlite::SqliteRow<'_>,
            ) -> ::zant::db::sqlite::SqliteResult<Self> {
                ::std::result::Result::Ok(Self { #(#field_reads),* })
            }
        }
    };
    Ok(expanded)
}

fn struct_table_name(input: &DeriveInput) -> syn::Result<String> {
    let mut name = input.ident.to_string();
    for attr in &input.attrs {
        if !attr.path().is_ident("table") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value: LitStr = meta.value()?.parse()?;
                name = value.value();
                Ok(())
            } else {
                Err(meta.error("unknown `table` attribute, expected `name = \"..\"`"))
            }
        })?;
    }
    Ok(name)
}

fn quoted_columns(fields: &[ModelField], include_id: bool) -> String {
    fields
        .iter()
        .filter(|field| include_id || !field.is_id)
        .map(|field| format!("\"{}\"", field.ident))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `ALTER TABLE` column definitions, one `added_column` match arm per field.
fn added_column_arms(fields: &[ModelField], span: proc_macro2::Span) -> Vec<TokenStream2> {
    let mut arms = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let index = Literal::usize_unsuffixed(index);
        if field.is_id {
            arms.push(quote! { #index => ::std::option::Option::None, });
            continue;
        }
        if field.optional {
            let definition = LitStr::new(field.kind.sql_type(), span);
            arms.push(quote! {
                #index => ::std::option::Option::Some(::std::string::String::from(#definition)),
            });
            continue;
        }
        match field.kind {
            FieldKind::Enum => match &field.default_tag {
                Some(tag) => {
                    let definition = LitStr::new(
                        &format!("{} NOT NULL DEFAULT '{}'", field.kind.sql_type(), tag),
                        span,
                    );
                    arms.push(quote! {
                        #index => ::std::option::Option::Some(::std::string::String::from(#definition)),
                    });
                }
                None => {
                    let sql_type = LitStr::new(field.kind.sql_type(), span);
                    let inner = &field.inner;
                    arms.push(quote! {
                        #index => ::std::option::Option::Some(::std::format!(
                            "{} NOT NULL DEFAULT '{}'",
                            #sql_type,
                            ::zant::db::sqlite::SqlEnum::as_str(
                                &<#inner as ::zant::db::sqlite::SqlEnum>::FIRST
                            )
                        )),
                    });
                }
            },
            kind => {
                let default = if kind == FieldKind::Text { "''" } else { "0" };
                let definition = LitStr::new(
                    &format!("{} NOT NULL DEFAULT {}", kind.sql_type(), default),
                    span,
                );
                arms.push(quote! {
                    #index => ::std::option::Option::Some(::std::string::String::from(#definition)),
                });
            }
        }
    }
    arms
}

/// One `rusqlite::types::Value` expression for a field. `value` is a reference
/// to the field (`&T`).
fn bind_value(field: &ModelField, value: TokenStream2) -> TokenStream2 {
    if field.optional {
        let bound = bind_scalar(field, quote!(inner));
        return quote! {
            match #value {
                ::std::option::Option::Some(inner) => #bound,
                ::std::option::Option::None => ::zant::db::sqlite::Value::Null,
            }
        };
    }
    bind_scalar(field, value)
}

fn bind_scalar(field: &ModelField, reference: TokenStream2) -> TokenStream2 {
    match field.kind {
        FieldKind::Integer => {
            // Conversion to i64: infallible for the narrow types,
            // checked for the ones that can exceed i64.
            let conversion = match type_ident(&field.inner).as_deref() {
                Some("i64") => quote!(*#reference),
                Some("isize") | Some("usize") | Some("u64") => {
                    quote!(::std::convert::TryFrom::try_from(*#reference).expect("int cast"))
                }
                _ => quote!(::std::convert::From::from(*#reference)),
            };
            quote_spanned! {field.inner.span()=>
                ::zant::db::sqlite::Value::Integer(#conversion)
            }
        }
        FieldKind::Float => {
            let conversion = if is_f32(&field.inner) {
                quote!(::std::convert::From::from(*#reference))
            } else {
                quote!(*#reference)
            };
            quote_spanned! {field.inner.span()=>
                ::zant::db::sqlite::Value::Real(#conversion)
            }
        }
        FieldKind::Boolean => quote_spanned! {field.inner.span()=>
            ::zant::db::sqlite::Value::Integer(::std::convert::From::from(*#reference))
        },
        FieldKind::Text => quote_spanned! {field.inner.span()=>
            ::zant::db::sqlite::Value::Text((#reference).to_owned())
        },
        FieldKind::Enum => quote_spanned! {field.inner.span()=>
            ::zant::db::sqlite::Value::Text(
                ::zant::db::sqlite::SqlEnum::as_str(#reference).to_owned()
            )
        },
    }
}

fn params_body(fields: &[ModelField], include_id: bool) -> Vec<TokenStream2> {
    fields
        .iter()
        .filter(|field| include_id || !field.is_id)
        .map(|field| {
            let ident = &field.ident;
            bind_value(field, quote!(&value.#ident))
        })
        .collect()
}

fn update_params_body(fields: &[ModelField]) -> Vec<TokenStream2> {
    let mut params = params_body(fields, false);
    if let Some(id) = fields.iter().find(|field| field.is_id) {
        let ident = &id.ident;
        params.push(bind_value(id, quote!(&value.#ident)));
    }
    params
}

/// `field: <read expression>` pairs for `from_row`.
fn field_reads(fields: &[ModelField]) -> Vec<TokenStream2> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let ident = &field.ident;
            let index = Literal::usize_unsuffixed(index);
            let read = read_value(field, index);
            quote! { #ident: #read }
        })
        .collect()
}

fn read_value(field: &ModelField, index: Literal) -> TokenStream2 {
    let inner = &field.inner;
    match (field.kind, field.optional) {
        (FieldKind::Integer, false) => quote! {
            ::zant::db::sqlite::column_int::<#inner>(row, #index)?
        },
        (FieldKind::Integer, true) => quote! {
            ::zant::db::sqlite::column_opt_int::<#inner>(row, #index)?
        },
        (FieldKind::Float, false) => {
            if is_f32(inner) {
                quote! { (::zant::db::sqlite::column_float(row, #index)? as f32) }
            } else {
                quote! { ::zant::db::sqlite::column_float(row, #index)? }
            }
        }
        (FieldKind::Float, true) => {
            if is_f32(inner) {
                quote! {
                    ::zant::db::sqlite::column_opt_float(row, #index)?.map(|value| value as f32)
                }
            } else {
                quote! { ::zant::db::sqlite::column_opt_float(row, #index)? }
            }
        }
        (FieldKind::Boolean, false) => quote! {
            ::zant::db::sqlite::column_bool(row, #index)?
        },
        (FieldKind::Boolean, true) => quote! {
            ::zant::db::sqlite::column_opt_bool(row, #index)?
        },
        (FieldKind::Text, false) => quote! {
            ::zant::db::sqlite::column_text(row, #index)?
        },
        (FieldKind::Text, true) => quote! {
            ::zant::db::sqlite::column_opt_text(row, #index)?
        },
        (FieldKind::Enum, true) => quote! {
            ::zant::db::sqlite::column_enum::<#inner>(row, #index)?
        },
        (FieldKind::Enum, false) => {
            let fallback = match &field.default_tag {
                Some(tag) => {
                    let tag = LitStr::new(tag, inner.span());
                    quote! {
                        <#inner as ::zant::db::sqlite::SqlEnum>::from_str(#tag)
                            .expect("`#[table(default = \"..\")]` names an unknown enum variant")
                    }
                }
                None => quote! { <#inner as ::zant::db::sqlite::SqlEnum>::FIRST },
            };
            quote! {
                ::zant::db::sqlite::column_enum::<#inner>(row, #index)?
                    .unwrap_or_else(|| #fallback)
            }
        }
    }
}
