//! `throw`, `try` / `catch` / `finally`, `Error` values.
//!
//! Lowered the way Swift lowers `throws` (no unwinding; see
//! `zant::tsx::error`):
//!
//! - A function that can let a throw escape (`throws`, inferred) returns
//!   `Result<T, tsx::Thrown>`; its `return x` is `return Ok(x)` and a call of
//!   it is `f(..)?` — or, inside a `try`, a `match` whose `Err` arm jumps to
//!   the handler.
//! - A `try` region is a labeled block; `throw` inside it is
//!   `break 'try Err(value)`. With a `finally`, every way out of the region
//!   (normal end, throw, `return`, a `break` / `continue` to an outer loop)
//!   is a `tsx::Completion` value — the ECMAScript spec's completion record:
//!   the `finally` block runs, then the completion resumes. An abrupt
//!   `finally` (its own `return` / `throw`) overrides it.
//! - A throw nobody catches leaves an event handler / effect / timer as an
//!   uncaught error report (the app keeps running, like a browser); a
//!   component cannot throw while it renders (a diagnostic), and neither can
//!   an arrow passed as a value (catch inside it).
//!
//! `catch (e)` binds `e: unknown` (`tsx::Thrown`): `e instanceof Error`
//! narrows it to an `Error` (`e.message`, `e.name`), `typeof e === "string"`
//! to a string; `String(e)` / `` `${e}` `` show any thrown value.

use crate::ast;

use super::aggregate;
use super::expr;
use super::model::{self, Field, Info, InfoKind};
use super::narrowing::{self, Narrow};
use super::rust_text::{self, Precedence};
use super::scope;
use super::stmt;
use super::types::{Type, Value};
use super::{Generator, Result, fail};

const THROWN: &str = "zant::tsx::Thrown";
const ERROR: &str = "zant::tsx::Error";
const COMPLETION: &str = "zant::tsx::Completion";

/// The error constructors (`new TypeError("…")`); `instanceof` tests them.
pub const ERROR_NAMES: [&str; 7] = [
    "Error",
    "TypeError",
    "RangeError",
    "SyntaxError",
    "ReferenceError",
    "EvalError",
    "URIError",
];

/// Where a throw outside every `try` of a body goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThrowMode {
    /// Nowhere: a diagnostic (render, arrows passed as values).
    #[default]
    Forbid,
    /// Out of the function (`return Err(..)`): a throwing function.
    Propagate,
    /// Reported as uncaught, then the handler returns (event handlers).
    Report,
}

/// A `try` region being emitted.
#[derive(Clone, Debug)]
pub struct Frame {
    pub label: String,
    /// A `finally` follows: exits leave through a `Completion`.
    pub finally: bool,
    /// `c.jump_labels.len()` when the region opened: labels below it are
    /// outside the region.
    pub jump_depth: usize,
    /// The jumps leaving it, numbered (`Completion::Jump(n)`): `(break?,
    /// label)`.
    pub jumps: Vec<(bool, String)>,
}

// ── types ────────────────────────────────────────────────────────────

fn find(g: &Generator<'_>, rust: &str) -> Option<Type> {
    g.compounds
        .iter()
        .position(|entry| entry.rust == rust)
        .map(|index| Type::Compound(index as u32))
}

/// `Error` (any error kind): `{ name: string; message: string }`.
pub fn error_type(g: &mut Generator<'_>) -> Result<Type> {
    if let Some(ty) = find(g, ERROR) {
        return Ok(ty);
    }
    let field = |name: &str| Field {
        name: name.as_bytes().to_vec(),
        ty: Type::String,
        optional: false,
        rust: None,
    };
    model::add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: ERROR.to_owned(),
            fields: vec![field("name"), field("message")],
            ..Info::default()
        },
    )
}

/// What a `catch` binding holds (TypeScript's `unknown`).
pub fn thrown_type(g: &mut Generator<'_>) -> Result<Type> {
    if let Some(ty) = find(g, THROWN) {
        return Ok(ty);
    }
    model::add(
        g,
        Info {
            kind: InfoKind::Object,
            rust: THROWN.to_owned(),
            ..Info::default()
        },
    )
}

pub fn is_thrown(g: &Generator<'_>, ty: Type) -> bool {
    find(g, THROWN) == Some(ty)
}

pub fn is_error(g: &Generator<'_>, ty: Type) -> bool {
    find(g, ERROR) == Some(ty)
}

/// `Error` / `TypeError` / … in a type position (unless the file declares
/// its own type of that name).
pub fn named_type(g: &mut Generator<'_>, name: &[u8]) -> Result<Option<Type>> {
    if g.type_decls.iter().any(|decl| decl.name.name == name) {
        return Ok(None);
    }
    if ERROR_NAMES.iter().any(|known| known.as_bytes() == name) {
        return error_type(g).map(Some);
    }
    Ok(None)
}

// ── values ───────────────────────────────────────────────────────────

/// `new Error(message?)` / `TypeError(message?)`; `None` for other calls.
pub fn construct(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Ident(name) = &call.callee.data else {
        return Ok(None);
    };
    let Some(kind) = ERROR_NAMES.iter().find(|known| known.as_bytes() == name) else {
        return Ok(None);
    };
    if scope::local(c, name).is_some() || super::functions::find_index(g, name).is_some() {
        return Ok(None);
    }
    if call.args.len() > 2 {
        return fail(g, loc, format!("`{kind}` takes a message"));
    }
    let message = match call.args.first() {
        Some(arg) => {
            let value = expr::emit(g, c, arg, Some(Type::String), false)?;
            super::types::coerce(g, value, Type::String, arg.loc)?.code
        }
        None => "zant::Str::Static(\"\")".to_owned(),
    };
    let ty = error_type(g)?;
    Ok(Some(Value::new(
        ty,
        format!("{ERROR}::new({kind:?}, {message})"),
    )))
}

/// The value of `throw e` as a `Thrown`.
fn thrown(g: &mut Generator<'_>, value: Value, loc: ast::Loc) -> Result<String> {
    let operand = rust_text::operand(&value, Precedence::Suffix, false);
    if is_thrown(g, value.ty) {
        return Ok(value.code);
    }
    if is_error(g, value.ty) {
        return Ok(format!("{THROWN}::error({})", value.code));
    }
    match value.ty {
        Type::String => Ok(format!("{THROWN}::String({})", value.code)),
        Type::Boolean => Ok(format!("{THROWN}::Boolean({})", value.code)),
        Type::Null => Ok(format!("{THROWN}::Null")),
        ty if ty.numeric() => Ok(format!("{THROWN}::Number(({operand}) as f64)")),
        _ => fail(
            g,
            loc,
            "throw an Error (`throw new Error(\"…\")`), a string or a number",
        ),
    }
}

/// `e instanceof Error` (the right side names an error constructor).
pub fn instance_of(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    b: &ast::Binary,
) -> Result<Value> {
    let Some(kind) = constructor(&b.rhs) else {
        return fail(
            g,
            b.rhs.loc,
            "`instanceof` tests an error kind (`e instanceof Error`); test objects by a field instead",
        );
    };
    // only looked into: a plain local is read in place (no copy)
    super::place::project(c, &b.lhs);
    let value = expr::emit(g, c, &b.lhs, None, false)?;
    let operand = match &b.lhs.data {
        ast::ExprData::Ident(name) if narrowing::find(c, name).is_none() => {
            match scope::local_index(c, name) {
                Some(index) => c.bindings[index].rust.clone(),
                None => rust_text::operand(&value, Precedence::Suffix, false),
            }
        }
        _ => rust_text::operand(&value, Precedence::Suffix, false),
    };
    if is_error(g, value.ty) {
        // an `Error` value: every error is an `Error`; a kind tests its name
        let code = if kind == "Error" {
            "true".to_owned()
        } else {
            format!("&*{operand}.name == {kind:?}")
        };
        return Ok(Value::new(Type::Boolean, code));
    }
    if !is_thrown(g, value.ty) {
        return fail(g, loc, "only caught errors can be tested with `instanceof`");
    }
    let code = if kind == "Error" {
        format!("{operand}.is_error()")
    } else {
        format!("{operand}.is_error_named({kind:?})")
    };
    Ok(Value::new(Type::Boolean, code))
}

fn constructor(e: &ast::Expr) -> Option<&'static str> {
    let ast::ExprData::Ident(name) = &e.data else {
        return None;
    };
    ERROR_NAMES
        .iter()
        .copied()
        .find(|known| known.as_bytes() == name)
}

/// What `e instanceof Error` / `typeof e === "string"` proves about a caught
/// `e` (pushed onto `c.narrowed`); false when `b` is no such test.
pub fn fact(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    b: &ast::Binary,
    positive: bool,
) -> Result<bool> {
    let (subject, narrowed) = if b.op == b"instanceof" {
        if constructor(&b.rhs).is_none() {
            return Ok(false);
        }
        (&*b.lhs, None)
    } else if (b.op == b"===" || b.op == b"==") && positive {
        let (test, literal) = match (&b.lhs.data, &b.rhs.data) {
            (ast::ExprData::Unary(u), ast::ExprData::String(s)) if u.op == b"typeof" => (u, s),
            (ast::ExprData::String(s), ast::ExprData::Unary(u)) if u.op == b"typeof" => (u, s),
            _ => return Ok(false),
        };
        let ty = match literal.as_slice() {
            b"string" => (Type::String, "as_string"),
            b"number" => (Type::F64, "as_number"),
            _ => return Ok(false),
        };
        (&*test.operand, Some(ty))
    } else {
        return Ok(false);
    };
    if !positive {
        return Ok(b.op == b"instanceof");
    }
    let Some(path) = narrowing::path(subject) else {
        return Ok(false);
    };
    let binding = scope::local_index(c, &narrowing::base(subject));
    let Some(index) = binding else {
        return Ok(false);
    };
    if !is_thrown(g, c.bindings[index].ty) {
        return Ok(false);
    }
    let rust = c.bindings[index].rust.clone();
    let (ty, method) = match narrowed {
        Some(found) => found,
        None => (error_type(g)?, "as_error"),
    };
    c.narrowed.push(Narrow {
        name: String::from_utf8_lossy(&path).into_owned(),
        binding,
        generation: narrowing::generation(c, binding),
        ty,
        code: format!("{rust}.{method}()"),
    });
    Ok(true)
}

/// `typeof e` / `String(e)` of a caught value; `None` for other types.
pub fn type_of(g: &Generator<'_>, value: &Value) -> Option<Value> {
    is_thrown(g, value.ty).then(|| {
        Value::new(
            Type::String,
            format!(
                "{}.type_of()",
                rust_text::operand(value, Precedence::Suffix, false)
            ),
        )
    })
}

/// A caught value or an `Error` shown as text.
pub fn text(g: &Generator<'_>, value: &Value) -> Option<Value> {
    let operand = rust_text::operand(value, Precedence::Suffix, false);
    if is_thrown(g, value.ty) {
        return Some(Value::new(Type::String, format!("{operand}.to_str()")));
    }
    if is_error(g, value.ty) {
        return Some(Value::new(
            Type::String,
            format!("zant::Str::from({operand}.to_string())"),
        ));
    }
    None
}

// ── control flow ─────────────────────────────────────────────────────

/// Code (diverging, `!`) that throws the `Thrown` expression `value` from
/// here.
pub fn throw_code(
    g: &mut Generator<'_>,
    c: &scope::Context,
    value: &str,
    loc: ast::Loc,
) -> Result<String> {
    if let Some(frame) = c.try_frames.last() {
        return Ok(if frame.finally {
            format!("break '{} {COMPLETION}::Throw({value})", frame.label)
        } else {
            format!("break '{} Err({value})", frame.label)
        });
    }
    match c.throw_mode {
        ThrowMode::Propagate => Ok(format!("return Err({value})")),
        ThrowMode::Report => Ok(format!(
            "{{ zant::tsx::error::uncaught(&({value})); return; }}"
        )),
        ThrowMode::Forbid if c.render => fail(
            g,
            loc,
            "a component cannot throw while it renders; catch the error here with try / catch",
        ),
        ThrowMode::Forbid => fail(
            g,
            loc,
            "a throw cannot leave this function yet; catch it inside with try / catch",
        ),
    }
}

/// `throw value;`
pub fn throw_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    value: &ast::Expr,
) -> Result<String> {
    let emitted = expr::emit(g, c, value, None, false)?;
    let thrown = thrown(g, emitted, value.loc)?;
    Ok(format!("{};\n", throw_code(g, c, &thrown, loc)?))
}

/// The call `code` of a throwing function, as a value: propagated, caught
/// by the enclosing `try`, or reported.
pub fn checked_call(
    g: &mut Generator<'_>,
    c: &scope::Context,
    value: Value,
    loc: ast::Loc,
) -> Result<Value> {
    if c.try_frames.is_empty() && c.throw_mode == ThrowMode::Propagate {
        return Ok(Value {
            code: format!("{}?", rust_text::operand(&value, Precedence::Suffix, false)),
            ..value
        });
    }
    let throw = throw_code(g, c, "_thrown", loc)?;
    Ok(Value {
        code: format!(
            "(match {} {{ Ok(_value) => _value, Err(_thrown) => {throw} }})",
            value.code
        ),
        precedence: Precedence::default(),
        ..value
    })
}

/// `return value` from here: through the enclosing `finally`, if any;
/// `Ok(..)` out of a throwing function. `value` is `None` for a void return.
/// `None` when no rewrite is needed.
pub fn return_code(c: &scope::Context, value: Option<&str>) -> Option<String> {
    if let Some(frame) = c.try_frames.iter().rev().find(|frame| frame.finally) {
        return Some(format!(
            "break '{} {COMPLETION}::Return({});\n",
            frame.label,
            value.unwrap_or("()")
        ));
    }
    (c.throw_mode == ThrowMode::Propagate && c.return_label.is_none())
        .then(|| format!("return Ok({});\n", value.unwrap_or("()")))
}

/// `break` / `continue` to `label`: through the enclosing `finally` when the
/// loop is outside it; `None` for a plain jump.
pub fn jump_code(c: &mut scope::Context, is_break: bool, label: &str) -> Option<String> {
    let position = c.jump_labels.iter().position(|known| known == label)?;
    let frame = c.try_frames.iter_mut().rev().find(|frame| frame.finally)?;
    if position >= frame.jump_depth {
        return None;
    }
    let id = frame.jumps.len();
    frame.jumps.push((is_break, label.to_owned()));
    Some(format!(
        "break '{} {COMPLETION}::Jump({id});\n",
        frame.label
    ))
}

/// The plain form of a jump (after the `finally` ran).
fn resume_jump(c: &mut scope::Context, is_break: bool, label: &str) -> String {
    jump_code(c, is_break, label).unwrap_or_else(|| {
        format!(
            "{} '{label};\n",
            if is_break { "break" } else { "continue" }
        )
    })
}

/// The statements of a region, in their own scope.
fn region(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    statements: &[Box<ast::Stmt>],
) -> Result<String> {
    let depth = c.active.len();
    let narrowed = c.narrowed.len();
    let out = stmt::statements(g, c, statements, false);
    c.active.truncate(depth);
    narrowing::restore(c, narrowed);
    out
}

fn open(g: &mut Generator<'_>, c: &mut scope::Context, finally: bool) -> Result<String> {
    let label = aggregate::temporary(g, c, b"_try")?;
    c.try_frames.push(Frame {
        label: label.clone(),
        finally,
        jump_depth: c.jump_labels.len(),
        jumps: Vec::new(),
    });
    Ok(label)
}

/// The catch block, `e` bound to `thrown` (an expression).
fn catch_body(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    catch: &ast::Catch,
    thrown: &str,
) -> Result<String> {
    let depth = c.active.len();
    let mut out = String::new();
    if let Some(name) = &catch.name {
        let ty = thrown_type(g)?;
        let binding = scope::bind(g, c, &name.name, ty, false)?;
        let rust = c.bindings[binding].rust.clone();
        out.push_str(&format!(
            "let {rust}: {THROWN} = {thrown};\nlet _ = &{rust};\n"
        ));
    }
    out.push_str(&region(g, c, &catch.body)?);
    c.active.truncate(depth);
    Ok(out)
}

/// `try { … } catch (e) { … } finally { … }`.
pub fn try_statement(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    t: &ast::Try,
) -> Result<String> {
    let Some(finally) = &t.finally else {
        let Some(catch) = &t.catch else {
            return fail(g, loc, "`try` needs a `catch` or `finally`");
        };
        // try / catch: the region's value is `Result<(), Thrown>`
        let label = open(g, c, false)?;
        let body = region(g, c, &t.body);
        c.try_frames.pop();
        let body = body?;
        let handler = catch_body(g, c, catch, "_thrown")?;
        return Ok(format!(
            "if let Err(_thrown) = '{label}: {{\n{body}Ok::<(), {THROWN}>(())\n}} {{\n{handler}}}\n{}",
            diverges(t)
        ));
    };
    if c.return_values.is_some() {
        return fail(
            g,
            loc,
            "`finally` needs the callback's return type written out (`(x): number => …`)",
        );
    }
    let ret = match c.ret {
        Type::Void => "()".to_owned(),
        ty => ty.rust(g).to_owned(),
    };
    let completion = aggregate::temporary(g, c, b"_completion")?;
    let label = open(g, c, true)?;
    let body = region(g, c, &t.body);
    let mut frame = c.try_frames.pop().expect("try frame");
    let body = body?;
    let mut out = format!(
        "let {completion}: {COMPLETION}<{ret}> = '{label}: {{\n{body}{COMPLETION}::Normal\n}};\n"
    );
    if let Some(catch) = &t.catch {
        // the catch block is a region of its own: its exits also run the
        // finally block; jumps keep their numbers
        let catch_label = aggregate::temporary(g, c, b"_catch")?;
        c.try_frames.push(Frame {
            label: catch_label.clone(),
            finally: true,
            jump_depth: frame.jump_depth,
            jumps: std::mem::take(&mut frame.jumps),
        });
        let handler = catch_body(g, c, catch, "_thrown");
        frame.jumps = c.try_frames.pop().expect("catch frame").jumps;
        let handler = handler?;
        out.push_str(&format!(
            "let {completion}: {COMPLETION}<{ret}> = match {completion} {{\n{COMPLETION}::Throw(_thrown) => '{catch_label}: {{\n{handler}{COMPLETION}::Normal\n}},\n_other => _other,\n}};\n"
        ));
    }
    // the finally block runs where the try statement is
    out.push_str(&format!("{{\n{}}}\n", region(g, c, finally)?));
    // then the completion resumes
    let mut arms = format!("{COMPLETION}::Normal => {{}}\n");
    let returned = match return_code(c, Some("_value")) {
        Some(code) => code,
        None if c.return_label.is_some() => {
            let target = c.return_label.clone().unwrap_or_default();
            format!("break '{target} (_value);\n")
        }
        None if c.ret == Type::Void => "return;\n".to_owned(),
        None => "return _value;\n".to_owned(),
    };
    arms.push_str(&format!(
        "{COMPLETION}::Return(_value) => {{\nlet _ = &_value;\n{returned}}}\n"
    ));
    let escapes = match &t.catch {
        Some(catch) => may_throw(g, &catch.body),
        None => may_throw(g, &t.body),
    };
    let rethrow = if escapes {
        throw_code(g, c, "_thrown", loc)?
    } else {
        "unreachable!(\"nothing throws here\")".to_owned()
    };
    arms.push_str(&format!("{COMPLETION}::Throw(_thrown) => {rethrow},\n"));
    for (id, (is_break, target)) in frame.jumps.iter().enumerate() {
        arms.push_str(&format!(
            "{COMPLETION}::Jump({id}) => {{\n{}}}\n",
            resume_jump(c, *is_break, target)
        ));
    }
    arms.push_str(&format!(
        "{COMPLETION}::Jump(_) => unreachable!(\"jump\"),\n"
    ));
    out.push_str(&format!("match {completion} {{\n{arms}}}\n{}", diverges(t)));
    Ok(out)
}

/// After a `try` that leaves on every path (TypeScript checked it), tell
/// Rust: it cannot see that the region's normal end is never reached.
fn diverges(t: &ast::Try) -> &'static str {
    if stmt::try_returns(t) {
        "unreachable!(\"every path of the try statement left\");\n"
    } else {
        ""
    }
}

/// Whether running `statements` may throw: a `throw`, or a call of a
/// throwing function or builtin (conservative: a nested `try` is not
/// subtracted).
fn may_throw(g: &Generator<'_>, statements: &[Box<ast::Stmt>]) -> bool {
    use crate::ast_walk::{self, StmtChild};
    fn child(g: &Generator<'_>, item: StmtChild<'_>) -> bool {
        match item {
            StmtChild::Stmt(s) => {
                if matches!(s.data, ast::StmtData::Throw(_)) {
                    return true;
                }
                let mut found = false;
                ast_walk::stmt_children(s, &mut |inner| found = found || child(g, inner));
                found
            }
            StmtChild::Expr(e) => expression(g, e),
        }
    }
    fn expression(g: &Generator<'_>, e: &ast::Expr) -> bool {
        if let ast::ExprData::Call(call) = &e.data {
            if super::throws::builtin_throws(call) {
                return true;
            }
            if let ast::ExprData::Ident(name) = &call.callee.data
                && g.functions
                    .iter()
                    .any(|function| function.throws && function.name.as_bytes() == name)
            {
                return true;
            }
        }
        let mut found = false;
        ast_walk::expr_children(e, &mut |inner| found = found || expression(g, inner));
        found
    }
    statements.iter().any(|s| child(g, StmtChild::Stmt(s)))
}
