//! Array methods without a callback loop: search, `slice`, `join`, `concat`.

use super::*;

pub(super) fn search(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    kind: Kind,
    call: &ast::Call,
    source: &str,
    child: Type,
) -> Result<(Type, String)> {
    arity(g, loc, call.args.len(), 1, 2)?;
    if !child.numeric()
        && child != Type::String
        && child != Type::Boolean
        && !model::enum_type(g, child)
    {
        return fail(g, loc, "includes/indexOf need primitive or string elements");
    }
    let value = expr::emit(g, c, &call.args[0], Some(child), false)?;
    let needle = aggregate::temporary(g, c, b"_needle")?;
    let mut setup = format!("let {needle}: {} = {};", child.rust(g), value.code);
    // `indexOf(x, from)`: a negative start counts from the end
    let start = match call.args.get(1) {
        Some(from) => {
            let from = expr::emit(g, c, from, Some(Type::F64), false)?;
            let from = types::coerce(g, from, Type::F64, call.args[1].loc)?;
            setup.push_str(&format!(
                "\nlet _start: usize = tsx::builtins::relative_index({}, {source}.len());",
                from.code
            ));
            Some("_start")
        }
        None => None,
    };
    let comparison = if kind == Kind::Includes && child.float() {
        format!("item == {needle} || (item.is_nan() && {needle}.is_nan())")
    } else {
        format!("item == {needle}")
    };
    let iterator = match start {
        Some(start) => format!("{source}.as_slice().iter().cloned().skip({start})"),
        None => format!("{source}.as_slice().iter().cloned()"),
    };
    let index = start.map_or("index".to_owned(), |start| format!("(index + {start})"));
    let code = if kind == Kind::Includes {
        format!("{iterator}.any(|item| {comparison})")
    } else {
        format!("{iterator}.position(|item| {comparison}).map_or(-1, |index| {index} as isize)")
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

pub(super) fn slice(
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

pub(super) fn join(
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
        "zant::Str::Static(\",\")".to_owned()
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
    let mut format = crate::codegen::rust_text::Format::default();
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

pub(super) fn concat(
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
