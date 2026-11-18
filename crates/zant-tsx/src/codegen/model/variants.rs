//! Union types: payload enums, node-or-nodes unions and string-literal enumerations.

use super::*;

pub(super) fn union_type(
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
    let numbers = nonnull.iter().all(|member| {
        matches!(
            member.detail.as_deref(),
            Some(ast::TypeDetail::NumberLit(_))
        )
    });
    let ty = if numbers {
        // `1 | 2 | 3`: checked as `number`
        Type::F64
    } else if nonnull.len() == 1 {
        if nonnull[0].detail.is_some() {
            resolve(g, &nonnull[0], name)?
        } else {
            types::type_ref(g, &nonnull[0], false)?
        }
    } else {
        let literals = nonnull.iter().all(|member| {
            matches!(
                member.detail.as_deref(),
                Some(ast::TypeDetail::StringLit(_))
            )
        });
        if literals {
            let names: Vec<Vec<u8>> = nonnull
                .iter()
                .map(|member| match member.detail.as_deref() {
                    Some(ast::TypeDetail::StringLit(text)) => text.clone(),
                    _ => unreachable!("string literal"),
                })
                .collect();
            enumeration(g, &names, name)?
        } else {
            // `string | number`, discriminated object unions
            crate::codegen::unions::declare(g, loc, &nonnull, name)?
        }
    };
    if nullable { optional(g, ty) } else { Ok(ty) }
}

/// `JSX.Element | JSX.Element[]` (React's `children` that is one element or
/// several): a list of elements, which one child also fills.
pub(super) fn node_or_nodes(
    g: &mut Generator<'_>,
    members: &[ast::TypeRef],
) -> Result<Option<Type>> {
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

pub(in crate::codegen) fn enumeration(
    g: &mut Generator<'_>,
    members: &[Vec<u8>],
    name: Option<&[u8]>,
) -> Result<Type> {
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
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]\npub enum {rust} {{\n{}\n}}\nimpl tsx::DictKey for {rust} {{\n    type Stored = Self;\n    fn stored(&self) -> Self {{\n        *self\n    }}\n}}\n\n",
            variants.join("\n")
        ));
        declaration.push_str(&format!(
            "impl {rust} {{\n    pub fn as_str(self) -> &'static str {{\n        match self {{\n{as_strs}        }}\n    }}\n}}\n\n"
        ));
    }
    let mut cases = String::new();
    for member in &names {
        let variant = rust_text::enum_variant(member).unwrap();
        let text = rust_text::quote(member).unwrap();
        cases.push_str(&format!("{text} => Some(Self::{variant}),"));
    }
    let json_members: Vec<(String, String)> = names
        .iter()
        .filter_map(|member| {
            Some((
                String::from_utf8(member.clone()).ok()?,
                rust_text::enum_variant(member)?,
            ))
        })
        .collect();
    declaration.push_str(&crate::codegen::json::enum_impl(&rust, &json_members));
    declaration.push_str(&crate::codegen::parse_and_format(&format!("impl tsx::hot::Hot for {rust} {{
        fn save(&self) -> tsx::hot::Value {{ tsx::hot::Value::from(self.as_str()) }}
        fn load(value: &tsx::hot::Value) -> Option<Self> {{ match value.as_str()? {{ {cases} _ => None }} }}
    }}\n")));
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
