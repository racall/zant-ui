//! Props of a tag: options struct fields classified into prop kinds and enum spellings.

use super::*;

/// One classified field type.
pub(super) enum KindData {
    Plain {
        kind: Kind,
        values: Vec<Value>,
        int_type: &'static str,
    },
    /// `Vec<T>` of a reflected struct (`RadioOption`, `TabItem`).
    StructList(&'static TsxStruct),
    /// A non-optional nested options struct, flattened into its parent
    /// (`ClickableOptions.box`).
    NestedOptions(&'static TsxStruct),
}

#[derive(Default)]
pub(super) struct PropsResult {
    pub(super) props: Vec<Prop>,
    /// An unsupported required leaf: the tag / array is unusable (it is skipped
    /// with "options have unsupported required fields").
    pub(super) invalid: bool,
    pub(super) children: bool,
    pub(super) child_count: usize,
    pub(super) child_field: &'static str,
}

pub(super) fn build_props(
    index: &Index,
    registry: &HashMap<&'static str, &'static TsxStruct>,
    tsx: &'static TsxStruct,
    parent: &str,
    top: &'static TsxStruct,
) -> PropsResult {
    let module = module_path(tsx.module_path);
    let mut result = PropsResult::default();
    for field in tsx.fields {
        let prop_name = zant_tsx::tags::camel(field.name);
        if !parent.is_empty()
            && top
                .fields
                .iter()
                .any(|top_field| zant_tsx::tags::camel(top_field.name) == prop_name)
        {
            // A flattened name that collides with a top-level prop loses.
            continue;
        }
        let required = field.required;
        match classify(index, registry, &module, field) {
            Some(KindData::Plain {
                kind,
                values,
                int_type,
            }) => {
                if kind == Kind::Children && field.name == "children" {
                    result.children = true;
                }
                if kind == Kind::Node && required {
                    result.child_count += 1;
                    result.child_field = intern(field.name.to_string());
                }
                result.props.push(Prop {
                    name: intern(prop_name),
                    field: intern(field.name.to_string()),
                    kind,
                    values,
                    sides: if kind == Kind::Edges {
                        Sides::ALL
                    } else {
                        Sides::NONE
                    },
                    int_type,
                    required,
                    parent: if parent.is_empty() {
                        ""
                    } else {
                        intern(parent.to_string())
                    },
                    ..Default::default()
                });
            }
            Some(KindData::StructList(item)) => {
                let item_props = build_props(index, registry, item, "", item);
                if item_props.invalid {
                    result.invalid |= required;
                    continue;
                }
                result.props.push(Prop {
                    name: intern(prop_name),
                    field: intern(field.name.to_string()),
                    kind: Kind::StructList,
                    required,
                    item_type: intern(format!("ui.{}", item.name)),
                    item_props: item_props.props,
                    parent: if parent.is_empty() {
                        ""
                    } else {
                        intern(parent.to_string())
                    },
                    ..Default::default()
                });
            }
            Some(KindData::NestedOptions(nested)) if parent.is_empty() => {
                let nested = build_props(index, registry, nested, field.name, tsx);
                result.invalid |= nested.invalid;
                result.children |= nested.children;
                result.child_count += nested.child_count;
                if nested.child_count > 0 {
                    result.child_field = nested.child_field;
                }
                result.props.extend(nested.props);
            }
            _ => result.invalid |= required,
        }
    }
    result
}

pub(super) fn classify(
    index: &Index,
    registry: &HashMap<&'static str, &'static TsxStruct>,
    module: &[String],
    field: &TsxField,
) -> Option<KindData> {
    let ty = syn::parse_str::<syn::Type>(field.ty).ok()?;
    classify_type(index, registry, module, &ty, false)
}

pub(super) fn classify_type(
    index: &Index,
    registry: &HashMap<&'static str, &'static TsxStruct>,
    module: &[String],
    ty: &syn::Type,
    optional: bool,
) -> Option<KindData> {
    let name = last_type_ident(ty)?;
    match name.as_str() {
        "Option" => {
            let inner = type_arguments(ty).first().copied()?;
            classify_type(index, registry, module, inner, true)
        }
        // `Option<Rc<Image>>` is an optional `Image`: look through the `Rc`.
        "Rc" => {
            let inner = type_arguments(ty).first().copied()?;
            classify_type(index, registry, module, inner, optional)
        }
        "u32" => Some(plain(Kind::Color)),
        "f32" => Some(plain(Kind::Number)),
        "bool" => Some(plain(Kind::Boolean)),
        "u8" | "u16" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize" => {
            Some(KindData::Plain {
                kind: Kind::Integer,
                values: Vec::new(),
                int_type: intern(name),
            })
        }
        "Str" => Some(plain(Kind::String)),
        "Node" => Some(plain(if optional {
            Kind::OptionalNode
        } else {
            Kind::Node
        })),
        "Edges" => Some(plain(Kind::Edges)),
        "Action" => Some(plain(Kind::Action)),
        "BoolAction" => Some(plain(Kind::BoolAction)),
        "TextAction" => Some(plain(Kind::TextAction)),
        "IndexAction" => Some(plain(Kind::IndexAction)),
        "Image" => Some(plain(Kind::Image)),
        "Shortcut" => Some(plain(Kind::Shortcut)),
        "HotkeyAction" => Some(plain(Kind::ShortcutAction)),
        "Vec" => classify_vec(index, registry, module, ty),
        _ => {
            // Rust enums used by the options (`flex::Wrap`, `text::Weight`,
            // `Cursor`, `Icon`, ...) and nested options structs: the path is
            // resolved in the source index.
            let segments = type_segments(ty)?;
            match index.resolve(module, &segments)? {
                Resolved::Enum(variants) => Some(KindData::Plain {
                    kind: Kind::Enumeration,
                    values: enum_values(&variants),
                    int_type: "",
                }),
                Resolved::Struct if !optional => registry
                    .get(name.as_str())
                    .copied()
                    .map(KindData::NestedOptions),
                _ => None,
            }
        }
    }
}

pub(super) fn classify_vec(
    index: &Index,
    registry: &HashMap<&'static str, &'static TsxStruct>,
    module: &[String],
    ty: &syn::Type,
) -> Option<KindData> {
    let item = type_arguments(ty).first().copied()?;
    let item_name = last_type_ident(item)?;
    match item_name.as_str() {
        "Node" => Some(plain(Kind::Children)),
        "Str" => Some(plain(Kind::StringList)),
        _ => {
            let segments = type_segments(item)?;
            match index.resolve(module, &segments)? {
                Resolved::Struct => registry
                    .get(item_name.as_str())
                    .copied()
                    .map(KindData::StructList),
                _ => None,
            }
        }
    }
}

pub(super) fn plain(kind: Kind) -> KindData {
    KindData::Plain {
        kind,
        values: Vec::new(),
        int_type: "",
    }
}

/// One `tags.Value` per TSX spelling of every enum variant, in declaration
/// order: the Rust `WrapReverse` becomes `wrap_reverse` / `wrap-reverse` /
/// `wrapReverse` (table value `wrap_reverse`).
pub(super) fn enum_values(variants: &[String]) -> Vec<Value> {
    let mut values = Vec::new();
    for variant in variants {
        let rust = intern(snake_case(variant));
        for spelling in zant_tsx::tags::spellings(rust) {
            values.push(Value {
                ts: intern(spelling),
                rust,
            });
        }
    }
    values
}

/// `WrapReverse` → `wrap_reverse`: the variant spelling stored in the table.
pub fn snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    for (index, char) in name.chars().enumerate() {
        if char.is_ascii_uppercase() {
            if index != 0 {
                result.push('_');
            }
            result.push(char.to_ascii_lowercase());
        } else {
            result.push(char);
        }
    }
    result
}
