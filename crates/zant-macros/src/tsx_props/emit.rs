//! Token emission for `#[derive(TsxProps)]`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::LitStr;

use super::model::StructProps;

pub(super) fn expand(model: &StructProps) -> TokenStream {
    let ident = &model.ident;
    let doc = LitStr::new(&model.doc, ident.span());
    let fields = model.fields.iter().map(|field| {
        let name = LitStr::new(&field.name(), field.ident.span());
        let ty = &field.ty;
        let required = field.required;
        let default = match &field.default {
            Some(default) => {
                let default = LitStr::new(default, field.ident.span());
                quote!(Some(#default))
            }
            None => quote!(None),
        };
        quote! {
            ::zant::ui::options::tsx_props::TsxField {
                name: #name,
                ty: stringify!(#ty),
                required: #required,
                default: #default,
            }
        }
    });
    quote! {
        impl ::zant::ui::options::tsx_props::TsxProps for #ident {
            const TSX: ::zant::ui::options::tsx_props::TsxStruct =
                ::zant::ui::options::tsx_props::TsxStruct {
                    name: stringify!(#ident),
                    doc: #doc,
                    module_path: module_path!(),
                    fields: &[#(#fields),*],
                };
        }
    }
}
