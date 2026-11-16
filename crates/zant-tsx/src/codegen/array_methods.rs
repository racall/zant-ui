//! Array operations lower to Rust iterator adapters over `tsx::List<T>`.
//! Callbacks are emitted as closures inside `.map` / `.filter_map` / `.any` /
//! `.all` / `.fold`, and the write-on-copy list (`Rc<Vec<T>>`) is rebuilt with
//! `tsx::List::from_vec`.

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
    if let Some(value) = super::dicts::mutating_method(g, c, loc, call, member)? {
        return Ok(value);
    }
    if super::array_mutations::is_mutating(&member.name.name) {
        return super::array_mutations::emit(g, c, loc, call, member);
    }
    super::place::project(c, &member.object);
    let value = expr::emit(g, c, &member.object, None, false)?;
    // `title?.trim()` / `items?.map(…)`: absent stays absent, like
    // TypeScript's optional chaining (the result is optional; use `??`)
    if member.optional
        && let Some(child) = model::optional_child(g, value.ty)
    {
        let inner = emit_on(g, c, loc, call, expected, Value::new(child, "_some"))?;
        if inner.ty == Type::Void {
            return Ok(Value {
                ty: Type::Void,
                code: format!("if let Some(_some) = {} {{ {}; }}", value.code, inner.code),
                ..Value::default()
            });
        }
        let ty = if model::optional_child(g, inner.ty).is_some() {
            inner.ty
        } else {
            model::optional(g, inner.ty)?
        };
        let map = if ty == inner.ty { "and_then" } else { "map" };
        return Ok(Value {
            ty,
            code: format!("({}).{map}(|_some| {})", value.code, inner.code),
            ..Value::default()
        });
    }
    emit_on(g, c, loc, call, expected, value)
}

/// A string or array method on the evaluated receiver `value`.
fn emit_on(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
    expected: Option<Type>,
    value: Value,
) -> Result<Value> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        unreachable!("array method call")
    };
    if value.ty == Type::String {
        return string_methods::emit(g, c, loc, call, value);
    }
    if let Some(result) = super::dicts::read_method(g, c, loc, call, member, &value)? {
        return Ok(result);
    }
    // `op.run(x)`: a function stored in a field
    if let Some(info) = model::info(g, value.ty).filter(|info| info.kind == model::InfoKind::Object)
        && let Some(field) = info
            .fields
            .iter()
            .find(|field| field.name == member.name.name)
        && super::closures::signature(g, field.ty).is_some()
    {
        let rust = match &field.rust {
            Some(rust) => rust.clone(),
            None => super::rust_text::id_at(g, &field.name, member.name.loc)?,
        };
        let callee = Value::new(
            field.ty,
            format!(
                "{}.{rust}.clone()",
                super::rust_text::operand(&value, super::rust_text::Precedence::Suffix, false)
            ),
        );
        return super::closures::call_value(g, c, loc, callee, &call.args);
    }
    if let Some(optional) = model::optional_child(g, value.ty) {
        let message = if optional == Type::String {
            "optional value: call the method with `?.` (`value?.trim()`), or use ?? / !"
        } else {
            "optional array: call the method with `?.` (`items?.map(…)`), or use ?? / !"
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
    // Evaluate the receiver once, in source order (the `_items` temporary).
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
    // The arrow's parameter order: `(item[, index])` or, for `reduce`,
    // `(accumulator, item[, index])`.
    let typed = |index: Option<Type>| -> Vec<Type> {
        let mut arguments = Vec::new();
        if reduce {
            arguments.push(initial_ty.expect("reduce initial"));
        }
        arguments.push(child);
        if let Some(ty) = index {
            arguments.push(ty);
        }
        arguments
    };
    let callback_types = typed(if needs_index { Some(Type::Usize) } else { None });
    // The iterator adapter's tuple order is `(index, item)`, the TSX arrow's is
    // `(item, index)`.
    // Closure parameters need explicit types: the call's argument types are
    // inferred from the wrapper call, not from a closure used immediately.
    let item_ty = child.rust(g).to_owned();
    let reduce_initial_ty = initial_ty.map(|ty| ty.rust(g).to_owned());
    let pattern = if reduce {
        match &reduce_initial_ty {
            Some(initial) if needs_index => {
                format!("{result}: {initial}, ({index}, {item}): (usize, {item_ty})")
            }
            Some(initial) => format!("{result}: {initial}, {item}: {item_ty}"),
            None => format!("{result}, {item}"),
        }
    } else if needs_index {
        format!("({index}, {item}): (usize, {item_ty})")
    } else {
        format!("{item}: {item_ty}")
    };
    let _ = &pattern;
    let adapted = if needs_index {
        format!("{source}.as_slice().iter().cloned().enumerate()")
    } else {
        format!("{source}.as_slice().iter().cloned()")
    };
    Ok(match kind {
        Kind::Map => {
            let (closure, ty) =
                callbacks::closure(g, c, &call.args[0], &callback_types, &pattern, target)?;
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
                &callback_types,
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
                &callback_types,
                &pattern,
                Some(Type::Boolean),
            )?;
            let adapter = if kind == Kind::Some { "any" } else { "all" };
            (Type::Boolean, format!("{adapted}.{adapter}({closure})"))
        }
        Kind::Reduce => {
            let initial = initial.expect("reduce initial");
            let ty = initial.ty;
            let (closure, _) =
                callbacks::closure(g, c, &call.args[0], &callback_types, &pattern, Some(ty))?;
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
    let separator = if call.args.len() == 1 {
        expr::emit(g, c, &call.args[0], Some(Type::String), false)?.code
    } else {
        "\",\".into()".to_owned()
    };
    if child == Type::String {
        return Ok((
            Type::String,
            format!("tsx::join({source}.as_slice(), ({separator}).as_str())"),
        ));
    }
    // numbers, booleans and string unions join their `String(…)` spellings,
    // as Array.prototype.join does
    if !(child.numeric() || child == Type::Boolean || model::enum_type(g, child)) {
        return fail(
            g,
            loc,
            "join needs strings, numbers or booleans; map the items to text first (`items.map(i => i.title).join(\", \")`)",
        );
    }
    let mut format = super::rust_text::Format::default();
    let read = if child.copy(g) {
        "*_item"
    } else {
        "_item.clone()"
    };
    format.value(g, Value::new(child, read), loc)?;
    let text = format.finish(g, false)?;
    Ok((
        Type::String,
        format!(
            "{{ let _texts: Vec<zant::Str> = {source}.as_slice().iter().map(|_item| {text}).collect(); tsx::join(_texts.as_slice(), ({separator}).as_str()) }}"
        ),
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

    // The method table is pinned.
    #[test]
    fn the_method_table_is_complete() {
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
