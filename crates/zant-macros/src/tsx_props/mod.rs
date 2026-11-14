//! `#[derive(TsxProps)]` code generation.
//!
//! Rust cannot reflect on a type from inside a `proc-macro`, so the derive
//! emits the field list as a `const` and `zant-tsx-tags` reads it back at run
//! time (the generator links the `zant` crate). The trait and the metadata
//! types live in `zant::ui::options::tsx_props` because a `proc-macro` crate
//! cannot export runtime items.
//!
//! Field types are emitted as tokens (`stringify!`), not classified: the
//! generator resolves them against the `zant` sources (enums, nested options
//! structs) and decides the TSX prop kind.

mod emit;
mod model;

use proc_macro2::TokenStream;

/// Expand the derive: one `impl TsxProps for <struct>` with its reflection
/// metadata (`crates/zant/src/ui/options.rs::tsx_props`).
pub(crate) fn expand(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let model = model::StructProps::from_input(input)?;
    Ok(emit::expand(&model))
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::model::StructProps;

    fn model(input: syn::DeriveInput) -> StructProps {
        StructProps::from_input(&input).expect("the test struct is a valid model")
    }

    #[test]
    fn fields_keep_declaration_order_and_types() {
        let model = model(parse_quote! {
            /// Layout and look of a container.
            pub struct BoxOptions {
                pub key: Option<Str>,
                pub gap: f32,
                pub children: Vec<Node>,
            }
        });
        assert_eq!(model.ident.to_string(), "BoxOptions");
        assert_eq!(model.doc, "Layout and look of a container.");
        let names: Vec<String> = model
            .fields
            .iter()
            .map(|field| field.ident.to_string())
            .collect();
        assert_eq!(names, ["key", "gap", "children"]);
        assert_eq!(model.fields[0].ty_string(), "Option < Str >");
        assert_eq!(model.fields[1].ty_string(), "f32");
        assert_eq!(model.fields[2].ty_string(), "Vec < Node >");
    }

    #[test]
    fn raw_identifiers_lose_their_prefix() {
        let model = model(parse_quote! {
            pub struct ClickableOptions {
                pub r#box: BoxOptions,
                pub r#type: AlertType,
            }
        });
        assert_eq!(model.fields[0].name(), "box");
        assert_eq!(model.fields[1].name(), "type");
    }

    #[test]
    fn doc_lines_are_joined_without_the_leading_space() {
        let model = model(parse_quote! {
            /// First line.
            ///
            /// Second line.
            pub struct TextOptions {}
        });
        assert_eq!(model.doc, "First line.\n\nSecond line.");
    }

    #[test]
    fn tsx_attributes_mark_required_fields_and_defaults() {
        let model = model(parse_quote! {
            pub struct TooltipOptions {
                #[tsx(required)]
                pub text: Str,
                #[tsx(default = "Str::Static(\"?\")")]
                pub avatar: Str,
                pub child: Node,
            }
        });
        assert!(model.fields[0].required);
        assert_eq!(
            model.fields[1].default.as_deref(),
            Some("Str::Static(\"?\")")
        );
        assert!(!model.fields[2].required);
        assert_eq!(model.fields[2].default, None);
    }

    #[test]
    fn unknown_tsx_attributes_are_rejected() {
        let input: syn::DeriveInput = parse_quote! {
            pub struct ToggleOptions {
                #[tsx(require = "yes")]
                pub label: Str,
            }
        };
        let error = StructProps::from_input(&input).expect_err("unknown attribute");
        assert!(error.to_string().contains("tsx"));
    }

    #[test]
    fn enums_and_unnamed_structs_are_rejected() {
        let enum_input: syn::DeriveInput = parse_quote! {
            pub enum AlertType {
                Info,
            }
        };
        let error = StructProps::from_input(&enum_input).expect_err("enums are not options");
        assert!(error.to_string().contains("structs"));

        let tuple_input: syn::DeriveInput = parse_quote! {
            pub struct Options(pub f32);
        };
        let error = StructProps::from_input(&tuple_input).expect_err("named fields only");
        assert!(error.to_string().contains("named fields"));
    }

    #[test]
    fn generic_structs_are_rejected() {
        let input: syn::DeriveInput = parse_quote! {
            pub struct Options<T> {
                pub value: T,
            }
        };
        let error = StructProps::from_input(&input).expect_err("no generics");
        assert!(error.to_string().contains("generic"));
    }

    #[test]
    fn the_expansion_parses_as_an_impl() {
        let input: syn::DeriveInput = parse_quote! {
            /// Text.
            pub struct TextOptions {
                #[tsx(required)]
                pub size: f32,
            }
        };
        let tokens = super::expand(&input).expect("the derive expands");
        let item: syn::ItemImpl = syn::parse2(tokens).expect("the output is a valid impl");
        assert!(item.trait_.is_some(), "the expansion implements a trait");
    }
}
