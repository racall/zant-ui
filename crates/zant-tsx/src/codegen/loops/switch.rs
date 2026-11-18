//! `switch` statements: cases, defaults, exhaustive string / enum switches.

use super::*;

pub(super) fn switch_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    v: &ast::Switch,
    label: &str,
) -> Result<String> {
    let value = expr::emit(g, c, &v.value, None, false)?;
    let concrete_ty = value.ty.concrete();
    let value = types::coerce(g, value, concrete_ty, v.value.loc)?;
    if !value.ty.numeric()
        && value.ty != Type::String
        && value.ty != Type::Boolean
        && !model::enum_type(g, value.ty)
    {
        return fail(
            g,
            v.value.loc,
            "switch requires a number, string, boolean or string-literal union",
        );
    }
    // every member of a string union handled and no default: exhaustive
    // (TypeScript's control-flow analysis knows nothing falls through)
    let exhaustive = model::enum_type(g, value.ty)
        && v.cases.iter().all(|case| case.value.is_some())
        && model::info(g, value.ty).is_some_and(|info| {
            info.members.iter().all(|member| {
                v.cases.iter().any(|case| {
                    matches!(case.value.as_deref().map(|e| &e.data), Some(ast::ExprData::String(text)) if text == member)
                        || case.value.as_deref().is_some_and(|e| enum_member_text(g, e).as_deref() == Some(member.as_slice()))
                })
            })
        });
    // `switch (s.kind)` / `switch (typeof x)` over a union's members
    let literals: Option<Vec<Vec<u8>>> = v
        .cases
        .iter()
        .map(|case| match case.value.as_deref().map(|e| &e.data) {
            Some(ast::ExprData::String(text)) => Some(text.clone()),
            _ => None,
        })
        .collect();
    let exhaustive = exhaustive
        || match &literals {
            Some(literals) => crate::codegen::unions::switch_covers(g, c, &v.value, literals)?,
            None => false,
        };
    if exhaustive {
        EXHAUSTIVE.with(|switches| switches.borrow_mut().push(v.value.loc));
    }
    let discriminant = aggregate::temporary(g, c, b"_value")?;
    let default_label = if grouped_default(v) {
        Some(aggregate::temporary(g, c, b"_default")?)
    } else {
        None
    };
    let mut branches = String::new();
    let mut fallback: Option<String> = None;
    let mut start = 0usize;
    for (index, case) in v.cases.iter().enumerate() {
        if !case.body.is_empty() && index + 1 < v.cases.len() && !ends_case(&case.body) {
            return fail(
                g,
                case.loc,
                "add `break`; falling through to the next case is not supported",
            );
        }
        if case.body.is_empty() && index + 1 < v.cases.len() {
            continue;
        }
        let mut conditions: Vec<String> = Vec::new();
        let mut is_default = false;
        for grouped in v.cases.iter().take(index + 1).skip(start) {
            match grouped.value.as_deref() {
                Some(test_expr) => {
                    let mut test_value = expr::emit(g, c, test_expr, Some(value.ty), false)?;
                    // `Str` has several `PartialEq` implementations, so a bare
                    // `"...".into()` pattern would be ambiguous; name the
                    // static variant explicitly.
                    test_value = explicit_string(g, test_value)?;
                    conditions.push(format!("({discriminant} == {})", test_value.code));
                }
                None => is_default = true,
            }
        }
        let depth = c.active.len();
        // a case of `switch (s.kind)` narrows `s` like `s.kind === "…"`
        let mark = match v.cases[start..=index].as_ref() {
            [only]
                if matches!(
                    only.value.as_deref().map(|e| &e.data),
                    Some(ast::ExprData::String(_))
                ) =>
            {
                let test = ast::Expr {
                    loc: only.loc,
                    data: ast::ExprData::Binary(ast::Binary {
                        op: b"===".to_vec(),
                        lhs: v.value.clone(),
                        rhs: only.value.clone().expect("case value"),
                    }),
                };
                Some(crate::codegen::narrowing::push(g, c, &test, true)?)
            }
            _ => None,
        };
        let body = stmt::statements(g, c, &case.body, false)?;
        if let Some(mark) = mark {
            crate::codegen::narrowing::restore(c, mark);
        }
        c.active.truncate(depth);
        let end = if default_label.is_some() && !is_default && !ends_case(&case.body) {
            format!("break '{label};\n")
        } else {
            String::new()
        };
        let block = format!("{{\n{body}{end}}}");
        if is_default {
            fallback = Some(block.clone());
        }
        if !conditions.is_empty() {
            let condition = conditions.join(" || ");
            let joined = if default_label.is_some() && is_default {
                let target = default_label.as_deref().unwrap_or("_default");
                format!("{{ break '{target}; }}")
            } else {
                block
            };
            if branches.is_empty() {
                branches.push_str(&format!("if {condition} {joined}"));
            } else {
                branches.push_str(&format!(" else if {condition} {joined}"));
            }
        }
        start = index + 1;
    }
    if let Some(target) = default_label.as_ref() {
        // A grouped default body must appear only once (locals and captures
        // have one lexical binding).
        branches = format!("'{target}: {{\n{branches}\n}}\n");
    }
    if let Some(block) = fallback {
        if !branches.is_empty() && default_label.is_none() {
            branches.push_str(" else ");
        }
        branches.push_str(&block);
    } else if exhaustive && !branches.is_empty() {
        branches.push_str(" else { unreachable!(\"every member is handled\") }");
    }
    // Even an empty switch evaluates its discriminant once.
    let discard = if v.cases.is_empty() || branches.is_empty() || !has_tests(v) {
        format!("let _ = {discriminant};\n")
    } else {
        String::new()
    };
    let prefix = if rust_text::uses_label(&branches, label) {
        format!("'{label}: ")
    } else {
        String::new()
    };
    Ok(format!(
        "{prefix}{{\nlet {discriminant}: {} = {};\n{discard}{branches}\n}}\n",
        value.ty.rust(g),
        value.code
    ))
}

pub(super) fn grouped_default(v: &ast::Switch) -> bool {
    let mut has_default = false;
    let mut has_value = false;
    for (index, case) in v.cases.iter().enumerate() {
        if case.value.is_none() {
            has_default = true;
        } else {
            has_value = true;
        }
        if case.body.is_empty() && index + 1 < v.cases.len() {
            continue;
        }
        if has_default && has_value {
            return true;
        }
        has_default = false;
        has_value = false;
    }
    false
}

pub(super) fn has_tests(v: &ast::Switch) -> bool {
    v.cases.iter().any(|case| case.value.is_some())
}

/// Materialize a string literal as `zant::Str::Static` where a generic
/// `PartialEq` / `match` context would leave `.into()` ambiguous.
pub fn explicit_string(
    g: &mut Generator<'_>,
    value: crate::codegen::types::Value,
) -> Result<crate::codegen::types::Value> {
    let (Some(text), true) = (value.text.as_ref(), value.ty == Type::String) else {
        return Ok(value);
    };
    let quoted = rust_text::quote_at(g, text, 0)?;
    Ok(types::Value {
        code: format!("zant::Str::Static({quoted})"),
        ..value
    })
}

pub(super) fn ends_case(ss: &[Box<ast::Stmt>]) -> bool {
    ss.iter().any(|s| exits(s))
}

/// A switch returns on every path only with a default (or every member of
/// its string union handled) and a returning body for each group.
pub fn returns(v: &ast::Switch) -> bool {
    let mut has_default = EXHAUSTIVE.with(|switches| switches.borrow().contains(&v.value.loc));
    for (index, case) in v.cases.iter().enumerate() {
        if case.value.is_none() {
            has_default = true;
        }
        if case.body.is_empty() && index + 1 < v.cases.len() {
            continue;
        }
        if !stmt::returns_all(&case.body) || sequence_jumps(&case.body, true) {
            return false;
        }
    }
    has_default
}

/// The text of `E.Member` when `e` reads a string enum member.
pub(super) fn enum_member_text(g: &Generator<'_>, e: &ast::Expr) -> Option<Vec<u8>> {
    let ast::ExprData::Member(member) = &e.data else {
        return None;
    };
    let ast::ExprData::Ident(name) = &member.object.data else {
        return None;
    };
    let decl = g.enums.iter().find(|decl| decl.name.name == *name)?;
    decl.members
        .iter()
        .find_map(|candidate| match &candidate.value {
            ast::EnumValue::Text(text) if candidate.name.name == member.name.name => {
                Some(text.clone())
            }
            _ => None,
        })
}
