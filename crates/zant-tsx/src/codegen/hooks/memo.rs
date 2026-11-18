//! `useMemo` and dependency lists: the cached value and the fields that hold its dependencies.

use super::*;

/// `[a, b]` as the Rust tuple `(a, b)` and its type `(A, B)`.
#[derive(Clone, Debug)]
pub struct DepList {
    pub value: String,
    pub ty: String,
}

pub fn dep_list(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<DepList> {
    let ast::ExprData::Array(items) = &e.data else {
        return fail(
            g,
            e.loc,
            "the dependency list must be an array literal: [a, b]",
        );
    };
    let mut values: Vec<String> = Vec::new();
    let mut tys: Vec<String> = Vec::new();
    for item in items {
        if matches!(item.data, ast::ExprData::Spread(_)) {
            return fail(g, item.loc, "list each dependency");
        }
        let mut value = expr::emit(g, c, item, None, false)?;
        let concrete = value.ty.concrete();
        value = types::coerce(g, value, concrete, item.loc)?;
        if value.ty == Type::Node
            || value.ty == Type::OptionalNode
            || value.ty == Type::Void
            || value.ty == Type::Null
        {
            return fail(
                g,
                item.loc,
                "dependencies are values (numbers, strings, arrays, objects)",
            );
        }
        if let Some(entry) = model::info(g, value.ty)
            && entry.kind == model::InfoKind::Callback
        {
            return fail(g, item.loc, "functions can't be dependencies");
        }
        values.push(value.code);
        tys.push(value.ty.rust(g).to_owned());
    }
    let tuple = format!("({})", values.join(", "));
    let tuple = if values.len() == 1 {
        format!("({},)", values[0])
    } else {
        tuple
    };
    Ok(DepList {
        value: tuple,
        ty: if tys.is_empty() {
            "()".to_owned()
        } else if tys.len() == 1 {
            format!("({},)", tys[0])
        } else {
            format!("({})", tys.join(", "))
        },
    })
}

pub(super) fn add_field(g: &mut Generator<'_>, base: &str, rust_type: String) -> Result<String> {
    g.counter += 1;
    let name = scope::unique_method(g, &format!("{base}{}", g.counter))?;
    g.hook_fields.push(Field {
        name: name.clone(),
        rust_type,
    });
    Ok(name)
}

// ── useMemo ───────────────────────────────────────────────────────────

/// `const v = useMemo(() => …, [deps])` as a render local.
pub fn memo(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    top: bool,
) -> Result<String> {
    let Some(init) = d.init.as_deref() else {
        return fail(g, loc, "useMemo needs an initializer");
    };
    let ast::ExprData::Call(call) = &init.data else {
        unreachable!("hook call")
    };
    if !c.render || !top {
        return fail(
            g,
            loc,
            "hooks must be called at the top level of the component",
        );
    }
    let ast::Pattern::Ident(pattern) = &d.pattern else {
        return fail(g, loc, "const value = useMemo(() => …, [deps])");
    };
    let arrow = match call.args.first().map(|arg| &arg.data) {
        Some(ast::ExprData::Arrow(function)) if function.params.is_empty() => function,
        _ => {
            return fail(
                g,
                loc,
                "useMemo takes a function without parameters and a dependency list",
            );
        }
    };
    if call.args.len() != 2 {
        return fail(
            g,
            loc,
            "useMemo takes a function without parameters and a dependency list",
        );
    }
    if call.type_args.len() > 1 {
        return fail(g, loc, "useMemo takes at most one type argument");
    }
    let annotation: Option<Type> = match call.type_args.first() {
        Some(reference) => Some(types::type_ref(g, reference, false)?),
        None => match d.ty.as_ref() {
            Some(reference) => Some(types::type_ref(g, reference, false)?),
            None => None,
        },
    };
    let deps = dep_list(g, c, &call.args[1])?;
    let callback = ast::Expr {
        loc: call.args[0].loc,
        data: ast::ExprData::Arrow(arrow.clone()),
    };
    let computed = callbacks::inline(g, c, &callback, annotation)?;
    let ty = computed.ty;
    if ty == Type::Node || ty == Type::OptionalNode || ty == Type::Void {
        return fail(
            g,
            loc,
            "useMemo caches values; elements are rebuilt every frame",
        );
    }
    let deps_ty = deps.ty.clone();
    let field = add_field(g, "_memo", format!("Option<({deps_ty}, {})>", ty.rust(g)))?;
    let slot = scope::state_field(g, field.as_bytes())?;
    let binding = scope::bind(g, c, &pattern.name, ty, false)?;
    let rust_ty = ty.rust(g).to_owned();
    let init = format!(
        "'{label}: {{\n\
         let _deps: {deps_ty} = {deps_value};\n\
         if self.{slot}.as_ref().is_none_or(|(deps, _)| deps != &_deps) {{\n\
         let _value: {rust_ty} = {computed};\n\
         self.{slot} = Some((_deps, _value));\n\
         }}\n\
         self.{slot}.as_ref().expect(\"memo\").1.clone()\n\
         }}",
        label = aggregate::temporary(g, c, b"_memo")?,
        deps_value = deps.value,
        computed = computed.code,
    );
    c.bindings[binding].init = Some(init);
    Ok(format!(
        "{}{}{}",
        scope::PLACEHOLDER_START,
        binding,
        scope::PLACEHOLDER_END
    ))
}

// ── useEffect ─────────────────────────────────────────────────────────
