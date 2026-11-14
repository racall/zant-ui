//! Array operations lower to Rust iterator adapters over `tsx::List<T>`.
//! Translation of `tools/tsx/codegen/array_methods.zig`, adapted to Rust:
//! where the Zig original opens a labeled value block around an explicit
//! `for` loop (Zig has no closures), the Rust port emits closures inside
//! `.map` / `.filter_map` / `.any` / `.all` / `.fold`, and the write-on-copy
//! list (`Rc<Vec<T>>`) is rebuilt with `tsx::List::from_vec`
//! (`docs/rust-port/tsx-codegen.md` §5/§7/§8).

use crate::ast;

use super::aggregate;
use super::callbacks;
use super::expr;
use super::model;
use super::scope;
use super::string_methods;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Map,
    Filter,
    Find,
    FindIndex,
    Some,
    Every,
    Reduce,
    IndexOf,
    Includes,
    Slice,
    Join,
    Concat,
}

fn lookup(name: &[u8]) -> Option<Kind> {
    Some(match name {
        b"map" => Kind::Map,
        b"filter" => Kind::Filter,
        b"find" => Kind::Find,
        b"findIndex" => Kind::FindIndex,
        b"some" => Kind::Some,
        b"every" => Kind::Every,
        b"reduce" => Kind::Reduce,
        b"indexOf" => Kind::IndexOf,
        b"includes" => Kind::Includes,
        b"slice" => Kind::Slice,
        b"join" => Kind::Join,
        b"concat" => Kind::Concat,
        _ => return None,
    })
}

pub fn emit(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    expected: Option<Type>,
) -> Result<Value> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        unreachable!("array method call")
    };
    let value = expr::emit(g, c, &member.object, None, false)?;
    if value.ty == Type::String {
        return string_methods::emit(g, c, loc, call, value);
    }
    if let Some(optional) = model::optional_child(g, value.ty) {
        let message = if optional == Type::String {
            "optional value needs ?? or !"
        } else {
            "optional array needs ?? or !"
        };
        return fail(g, loc, message);
    }
    let Some(child) = model::element(g, value.ty) else {
        return fail(g, loc, "array method needs an array");
    };
    let Some(kind) = lookup(&member.name.name) else {
        return fail(
            g,
            member.name.loc,
            format!(
                "unknown array method `{}`",
                String::from_utf8_lossy(&member.name.name)
            ),
        );
    };
    if c.constant {
        return fail(g, loc, "array methods cannot run in a top-level constant");
    }
    // Evaluate the receiver once, in source order (`_items` in the Zig code).
    let source = aggregate::temporary(g, c, b"_items")?;
    let setup = format!("let {source}: {} = {};", value.ty.rust(g), value.code);
    let (ty, code) = match kind {
        Kind::Map
        | Kind::Filter
        | Kind::Find
        | Kind::FindIndex
        | Kind::Some
        | Kind::Every
        | Kind::Reduce => loop_method(g, c, loc, kind, call, &source, child, expected)?,
        Kind::IndexOf | Kind::Includes => search(g, c, loc, kind, call, &source, child)?,
        Kind::Slice => slice(g, c, loc, call, &source, value.ty)?,
        Kind::Join => join(g, c, loc, call, &source, child)?,
        Kind::Concat => concat(g, c, call, &source, value.ty, child)?,
    };
    Ok(Value {
        ty,
        code: format!("{{\n{setup}\n{code}\n}}"),
        ..Value::default()
    })
}

fn arity(g: &mut Generator<'_>, loc: ast::Loc, count: usize, min: usize, max: usize) -> Result<()> {
    if count < min || count > max {
        return fail(
            g,
            loc,
            format!("array method expects {min}..{max} arguments"),
        );
    }
    Ok(())
}

/// The arrow of an array callback (`None` for a non-arrow, reported later).
fn arrow(call: &ast::Call) -> Option<&ast::Function> {
    match &call.args[0].data {
        ast::ExprData::Arrow(function) => Some(function),
        _ => None,
    }
}

/// The name a callback parameter is bound to (`_item` when the arrow leaves it
/// out), so the emitted closure pattern and the body agree.
fn parameter_name(function: Option<&ast::Function>, index: usize) -> String {
    match function.and_then(|f| f.params.get(index)) {
        Some(param) => String::from_utf8_lossy(&param.name.name).into_owned(),
        None => format!("_arg{index}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn loop_method(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    kind: Kind,
    call: &ast::Call,
    source: &str,
    child: Type,
    expected: Option<Type>,
) -> Result<(Type, String)> {
    let reduce = kind == Kind::Reduce;
    arity(
        g,
        loc,
        call.args.len(),
        if reduce { 2 } else { 1 },
        if reduce { 2 } else { 1 },
    )?;
    let mut initial: Option<Value> = None;
    if reduce {
        let value = expr::emit(g, c, &call.args[1], None, false)?;
        initial = Some(types::coerce(
            g,
            value.clone(),
            value.ty.concrete(),
            call.args[1].loc,
        )?);
    }
    let target = match kind {
        Kind::Map => match expected {
            Some(ty) => model::element(g, ty),
            None => None,
        },
        Kind::Reduce => initial.as_ref().map(|value| value.ty),
        _ => Some(Type::Boolean),
    };
    let function = arrow(call);
    let params = function.map(|f| f.params.len()).unwrap_or(0);
    let needs_index = if reduce {
        params > 2
    } else {
        kind == Kind::FindIndex || params > 1
    };
    let item = parameter_name(function, if reduce { 1 } else { 0 });
    let index = parameter_name(function, if reduce { 2 } else { 1 });
    let result = parameter_name(function, 0);
    let initial_ty = initial.as_ref().map(|value| value.ty);
    let typed = |index: Option<Type>| -> Vec<Type> {
        let mut arguments = Vec::new();
        if reduce {
            arguments.push(initial_ty.expect("reduce initial"));
        }
        if let Some(ty) = index {
            arguments.push(ty);
        }
        arguments.push(child);
        arguments
    };
    // The iterator adapter's tuple order is `(index, item)`, the TSX arrow's is
    // `(item, index)`.
    let pattern = if needs_index {
        format!("({index}, {item})")
    } else {
        item.clone()
    };
    let fold_pattern = if needs_index {
        format!("({result}, ({index}, {item}))")
    } else {
        format!("{result}, {item}")
    };
    let _ = &pattern;
    let pattern = if reduce {
        fold_pattern.clone()
    } else if needs_index {
        format!("({index}, {item})")
    } else {
        item.clone()
    };
    let adapted = if needs_index {
        format!("{source}.as_slice().iter().cloned().enumerate()")
    } else {
        format!("{source}.as_slice().iter().cloned()")
    };
    Ok(match kind {
        Kind::Map => {
            let (closure, ty) =
                callbacks::closure(g, c, &call.args[0], &typed(None), &pattern, target)?;
            (
                model::array(g, ty)?,
                format!("tsx::List::from_vec({adapted}.map({closure}).collect::<Vec<_>>())"),
            )
        }
        Kind::Filter | Kind::Find | Kind::FindIndex => {
            // the predicate closure consumes its arguments: clone the item we
            // keep and bind the predicate's parameters by value
            let (closure, _) = callbacks::closure(
                g,
                c,
                &call.args[0],
                &typed(if needs_index { Some(Type::Usize) } else { None }),
                &pattern,
                Some(Type::Boolean),
            )?;
            let argument = if needs_index {
                format!("({index}, {item}.clone())")
            } else {
                format!("{item}.clone()")
            };
            let wrapper = if needs_index {
                format!("({index}, {item})")
            } else {
                item.clone()
            };
            match kind {
                Kind::Filter => {
                    let ty = model::array(g, child)?;
                    (
                        ty,
                        format!(
                            "tsx::List::from_vec({adapted}.filter_map(|{wrapper}| {{\n\
                             let _keep: bool = ({closure})({argument});\n\
                             if _keep {{ Some({item}) }} else {{ None }}\n\
                             }}).collect::<Vec<_>>())"
                        ),
                    )
                }
                Kind::Find => {
                    let ty = model::optional(g, child)?;
                    (
                        ty,
                        format!(
                            "{adapted}.find_map(|{wrapper}| {{\n\
                             let _hit: bool = ({closure})({argument});\n\
                             if _hit {{ Some({item}) }} else {{ None }}\n\
                             }})"
                        ),
                    )
                }
                _ => (
                    Type::Isize,
                    format!(
                        "{adapted}.find_map(|{wrapper}| {{\n\
                         let _hit: bool = ({closure})({argument});\n\
                         if _hit {{ Some({index}) }} else {{ None }}\n\
                         }}).map_or(-1, |index| index as isize)"
                    ),
                ),
            }
        }
        Kind::Some | Kind::Every => {
            let (closure, _) = callbacks::closure(
                g,
                c,
                &call.args[0],
                &typed(None),
                &pattern,
                Some(Type::Boolean),
            )?;
            let adapter = if kind == Kind::Some { "any" } else { "all" };
            (Type::Boolean, format!("{adapted}.{adapter}({closure})"))
        }
        Kind::Reduce => {
            let initial = initial.expect("reduce initial");
            let ty = initial.ty;
            let (closure, _) = callbacks::closure(
                g,
                c,
                &call.args[0],
                &typed(if needs_index { Some(Type::Usize) } else { None }),
                &pattern,
                Some(ty),
            )?;
            (ty, format!("{adapted}.fold({}, {closure})", initial.code))
        }
        _ => unreachable!("non-loop array method"),
    })
}

fn search(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    kind: Kind,
    call: &ast::Call,
    source: &str,
    child: Type,
) -> Result<(Type, String)> {
    arity(g, loc, call.args.len(), 1, 1)?;
    if !child.numeric()
        && child != Type::String
        && child != Type::Boolean
        && !model::enum_type(g, child)
    {
        return fail(g, loc, "includes/indexOf need primitive or string elements");
    }
    let value = expr::emit(g, c, &call.args[0], Some(child), false)?;
    let needle = aggregate::temporary(g, c, b"_needle")?;
    let setup = format!("let {needle}: {} = {};", child.rust(g), value.code);
    let comparison = if kind == Kind::Includes && child.float() {
        format!("item == {needle} || (item.is_nan() && {needle}.is_nan())")
    } else {
        format!("item == {needle}")
    };
    let iterator = format!("{source}.as_slice().iter().cloned()");
    let code = if kind == Kind::Includes {
        format!("{iterator}.any(|item| {comparison})")
    } else {
        format!("{iterator}.position(|item| {comparison}).map_or(-1, |index| index as isize)")
    };
    Ok((
        if kind == Kind::Includes {
            Type::Boolean
        } else {
            Type::Isize
        },
        format!("{setup}\n{code}"),
    ))
}

fn slice(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    source: &str,
    ty: Type,
) -> Result<(Type, String)> {
    arity(g, loc, call.args.len(), 0, 2)?;
    let mut bounds = ["0".to_owned(), format!("{source}.as_slice().len()")];
    for (index, arg) in call.args.iter().enumerate() {
        let value = expr::emit(g, c, arg, None, false)?;
        if model::optional_child(g, value.ty).is_some() {
            return fail(g, arg.loc, "optional slice endpoint needs ?? or !");
        }
        if !value.ty.numeric() {
            return fail(g, arg.loc, "slice endpoint must be numeric");
        }
        let concrete = types::concrete(g, value)?;
        bounds[index] = concrete.code;
    }
    let start = aggregate::temporary(g, c, b"_start")?;
    let end = aggregate::temporary(g, c, b"_end")?;
    Ok((
        ty,
        format!(
            "let {start}: usize = tsx::slice_index({}, {source}.as_slice().len());\n\
             let {end}: usize = tsx::slice_index({}, {source}.as_slice().len());\n\
             tsx::List::from_vec({source}.as_slice()[{start}..{end}.max({start})].to_vec())",
            bounds[0], bounds[1]
        ),
    ))
}

fn join(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    source: &str,
    child: Type,
) -> Result<(Type, String)> {
    arity(g, loc, call.args.len(), 0, 1)?;
    if child != Type::String {
        return fail(g, loc, "join needs a string array");
    }
    let separator = if call.args.len() == 1 {
        expr::emit(g, c, &call.args[0], Some(Type::String), false)?.code
    } else {
        "\",\".into()".to_owned()
    };
    Ok((
        Type::String,
        format!("tsx::join({source}.as_slice(), &{separator})"),
    ))
}

fn concat(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    call: &ast::Call,
    source: &str,
    ty: Type,
    child: Type,
) -> Result<(Type, String)> {
    let mut setup = format!(
        "let mut _out: Vec<{}> = {source}.as_slice().to_vec();",
        child.rust(g)
    );
    for arg in &call.args {
        let context = match &arg.data {
            ast::ExprData::Array(_) => Some(ty),
            ast::ExprData::Object(_) => Some(child),
            _ => None,
        };
        let value = expr::emit(g, c, arg, context, false)?;
        if value.ty == ty {
            let part = aggregate::temporary(g, c, b"_part")?;
            setup.push_str(&format!(
                "\nlet {part}: {} = {};\n_out.extend({part}.as_slice().iter().cloned());",
                ty.rust(g),
                value.code
            ));
        } else {
            let value = types::coerce(g, value, child, arg.loc)?;
            setup.push_str(&format!("\n_out.push({});", value.code));
        }
    }
    Ok((ty, format!("{setup}\ntsx::List::from_vec(_out)")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Zig `array_methods.zig` has no `test` block; the method table is pinned.
    #[test]
    fn the_zig_method_table_is_complete() {
        for name in [
            "map",
            "filter",
            "find",
            "findIndex",
            "some",
            "every",
            "reduce",
            "indexOf",
            "includes",
            "slice",
            "join",
            "concat",
        ] {
            assert!(lookup(name.as_bytes()).is_some(), "{name}");
        }
        assert!(lookup(b"forEach").is_none());
    }
}
