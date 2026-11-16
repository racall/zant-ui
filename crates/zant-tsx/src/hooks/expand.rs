//! Custom hooks (React's `function useTimers() { … }`): a call at the top
//! level of a component is replaced by the hook's body, before code is
//! generated, so the hooks inside it (state, effects, app and desktop hooks)
//! belong to that component as if written there:
//!
//! ```tsx
//! function useToast(ms: number) {            // src/hooks/useToast.tsx
//!   const [text, setText] = useState("");
//!   useEffect(() => { … }, [text]);
//!   return { text, show: setText };
//! }
//! const { text, show } = useToast(2000);     // in a component
//! ```
//!
//! The body's names get the hook's prefix (`useToast_text`), arguments
//! replace the parameters (or become constants), and the names the hook
//! returned stand for its values in the rest of the component. Names the
//! hook's file imports are imported where it is inlined (`resolve.rs`).
//! Hooks may call hooks; a call anywhere but the top level is an error.
//!
//! The component and the hooks are cloned out of the units, and the side
//! effects of `resolve.rs` on the units (added imports, names a file must
//! export) are collected and applied after each component.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::leftover;
use super::resolve::{self, Locals, Unit};
use super::rewrite::{self, Binding, Entry, Rewriter};
use crate::ast::{self, Body, Expr, ExprData, Loc, Stmt, StmtData};

/// A failure in file `unit` (inlined code is reported in the hook's file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub unit: usize,
    pub loc: Loc,
    pub msg: String,
}

type Error = rewrite::Failure;

/// Locations of code inlined from another file carry that file (`unit + 1`)
/// in their top bits; [`decode`] splits them again.
pub const LOC_BITS: u32 = 24;

pub fn tag(unit: usize) -> Loc {
    if unit >= 255 {
        return 0;
    }
    ((unit + 1) as Loc) << LOC_BITS
}

/// The file and offset of `loc` in a file that is `unit`.
pub fn decode(unit: usize, loc: Loc) -> (usize, Loc) {
    let t = loc >> LOC_BITS;
    if t == 0 {
        (unit, loc)
    } else {
        ((t - 1) as usize, loc & ((1 << LOC_BITS) - 1))
    }
}

/// Inlines every custom hook call of every unit's components, then removes
/// the hooks (they are not code of their own).
pub fn expand(units: &mut [Unit], failure: &mut Option<Failure>) -> bool {
    for i in 0..units.len() {
        let count = units[i].module.functions.len();
        for fi in 0..count {
            let top = &units[i].module.functions[fi];
            if resolve::is_hook_name(&top.func.name) || !is_component(&top.func) {
                continue;
            }
            let component = (*units[i].module.functions[fi].func).clone();
            let mut added = std::mem::take(&mut units[i].added);
            let mut pending: Vec<(usize, Vec<u8>)> = Vec::new();
            let result = {
                let expander = Expander {
                    units: &*units,
                    unit: i,
                    added: &mut added,
                    pending: &mut pending,
                    component,
                    locals: Rc::new(RefCell::new(HashSet::new())),
                    aliases: Rewriter::empty(),
                    counts: HashMap::new(),
                };
                expander.run()
            };
            units[i].added = added;
            for (u, name) in pending {
                units[u].exported.push(name);
            }
            let component = match result {
                Ok(component) => component,
                Err(why) => return report(failure, i, why),
            };
            *units[i].module.functions[fi].func = component;
        }
    }
    for i in 0..units.len() {
        if let Some(found) = leftover::find(units, i) {
            let why = Error {
                loc: found.loc,
                msg: format!(
                    "`{}` is a custom hook: call it at the top level of a component or of another hook",
                    text(&found.name)
                ),
            };
            return report(failure, i, why);
        }
    }
    // which imports name hooks, before the hooks go
    let mut imports: Vec<Vec<ast::Import>> = Vec::with_capacity(units.len());
    for u in units.iter() {
        imports.push(imports_without_hooks(units, u));
    }
    for (i, list) in imports.into_iter().enumerate() {
        finish(&mut units[i]);
        units[i].module.imports = list;
    }
    true
}

fn report(failure: &mut Option<Failure>, unit: usize, why: Error) -> bool {
    let (unit, loc) = decode(unit, why.loc);
    *failure = Some(Failure {
        unit,
        loc,
        msg: why.msg,
    });
    false
}

/// React's rule: components are capitalized.
fn is_component(f: &ast::Function) -> bool {
    !f.name.is_empty() && f.name[0].is_ascii_uppercase() && matches!(f.body, Body::Block(_))
}

/// The unit's imports and the ones inlining added, without the hooks.
fn imports_without_hooks(units: &[Unit], u: &Unit) -> Vec<ast::Import> {
    let mut imports: Vec<ast::Import> = Vec::new();
    for imp in u.module.imports.iter().chain(u.added.iter()) {
        let mut names = Vec::new();
        let target = resolve::unit_of(units, &imp.source);
        for n in &imp.names {
            if let Some(t) = target
                && resolve::hook_in(&units[t].module, &n.imported.name).is_some()
            {
                continue;
            }
            names.push(n.clone());
        }
        let bare = imp.default_name.is_none() && imp.namespace_name.is_none();
        if names.is_empty() && !imp.names.is_empty() && bare {
            continue;
        }
        let mut copy = imp.clone();
        copy.names = names;
        imports.push(copy);
    }
    imports
}

/// The hooks go; the declarations other files now import are exported.
fn finish(u: &mut Unit) {
    let exported = u.exported.clone();
    let m = &mut u.module;
    let mut functions = Vec::new();
    for top in m.functions.drain(..) {
        if resolve::is_hook_name(&top.func.name) {
            continue;
        }
        let mut copy = top;
        if contains(&exported, &copy.func.name) {
            copy.exported = true;
        }
        functions.push(copy);
    }
    let mut consts = Vec::new();
    for c in m.consts.drain(..) {
        if resolve::is_hook_name(&c.name.name) && matches!(c.value.data, ExprData::Arrow(_)) {
            continue;
        }
        let mut copy = c;
        if contains(&exported, &copy.name.name) {
            copy.exported = true;
        }
        consts.push(copy);
    }
    let mut types = std::mem::take(&mut m.types);
    for t in types.iter_mut() {
        if contains(&exported, &t.name.name) {
            t.exported = true;
        }
    }
    m.functions = functions;
    m.consts = consts;
    m.types = types;
}

fn contains(list: &[Vec<u8>], name: &[u8]) -> bool {
    list.iter().any(|n| n.as_slice() == name)
}

/// The hook call a top-level statement makes, if any.
struct Site {
    call: ast::Call,
    loc: Loc,
    hook: resolve::Hook,
    target: Target,
    ty: Option<ast::TypeRef>,
}

/// Where the hook call's result goes.
enum Target {
    None,
    Ident(ast::Name),
    Array(Vec<Option<ast::Name>>),
    Object(Vec<ast::ObjectBinding>),
}

struct Expander<'a, 'b> {
    units: &'a [Unit],
    unit: usize,
    added: &'b mut Vec<ast::Import>,
    pending: &'b mut Vec<(usize, Vec<u8>)>,
    component: ast::Function,
    locals: Locals,
    /// Names the component's later statements read as a hook's values.
    aliases: Rewriter<'static>,
    counts: HashMap<Vec<u8>, usize>,
}

impl Expander<'_, '_> {
    fn run(mut self) -> Result<ast::Function, Error> {
        let params = self.component.params.clone();
        for p in &params {
            self.add_local(&p.name.name);
            for n in &p.destructure {
                self.add_local(&n.local.name);
            }
        }
        let mut list: Vec<Box<Stmt>> = match &self.component.body {
            Body::Block(ss) => ss.clone(),
            Body::Expr(_) => panic!("a component's body is a block"),
        };
        for s in &list {
            self.declare(s);
        }
        let mut depth: Vec<u8> = vec![0; list.len()];
        let mut changed = false;
        let mut i: usize = 0;
        while i < list.len() {
            let s = if !self.aliases.env.is_empty() {
                self.aliases.stmt(&list[i])?
            } else {
                list[i].clone()
            };
            list[i] = s;
            let call = match self.site(&list[i])? {
                Some(call) => call,
                None => {
                    i += 1;
                    continue;
                }
            };
            if depth[i] >= 32 {
                return Err(Error {
                    loc: list[i].loc,
                    msg: "custom hooks call each other without end".to_string(),
                });
            }
            let body = self.inline_(&call)?;
            for b in &body {
                self.declare(b);
            }
            let next_depth = depth[i] + 1;
            let len = body.len();
            list.splice(i..i + 1, body);
            depth.splice(i..i + 1, vec![next_depth; len]);
            changed = true;
        }
        if changed {
            self.component.body = Body::Block(list);
        }
        Ok(self.component)
    }

    fn add_local(&self, name: &[u8]) {
        self.locals.borrow_mut().insert(name.to_vec());
    }

    fn declare(&self, s: &Stmt) {
        match &s.data {
            StmtData::Decl(d) => match &d.pattern {
                ast::Pattern::Ident(n) => self.add_local(&n.name),
                ast::Pattern::Array(names) => {
                    for n in names.iter().flatten() {
                        self.add_local(&n.name);
                    }
                }
                ast::Pattern::Object(bindings) => {
                    for b in bindings {
                        self.add_local(&b.local.name);
                    }
                }
            },
            StmtData::Func(f) => self.add_local(&f.name),
            _ => {}
        }
    }

    fn site(&self, s: &Stmt) -> Result<Option<Site>, Error> {
        let (e, target, ty): (&Expr, Target, Option<ast::TypeRef>) = match &s.data {
            StmtData::Expr(e) => (e, Target::None, None),
            StmtData::Decl(d) => {
                let init = match &d.init {
                    Some(init) => init,
                    None => return Ok(None),
                };
                let target = match &d.pattern {
                    ast::Pattern::Ident(n) => Target::Ident(n.clone()),
                    ast::Pattern::Array(names) => Target::Array(names.clone()),
                    ast::Pattern::Object(bindings) => Target::Object(bindings.clone()),
                };
                (init, target, d.ty.clone())
            }
            _ => return Ok(None),
        };
        let ExprData::Call(call) = &e.data else {
            return Ok(None);
        };
        let ExprData::Ident(name) = &call.callee.data else {
            return Ok(None);
        };
        let hook = match resolve::find_hook_in(self.units, self.unit, name) {
            Some(hook) => hook,
            None => return Ok(None),
        };
        if !name.contains(&b'@') && hook.unit != self.unit && !hook.exported {
            return Err(Error {
                loc: e.loc,
                msg: format!("`{}` is not exported by its file", text(name)),
            });
        }
        Ok(Some(Site {
            call: call.clone(),
            loc: e.loc,
            hook,
            target,
            ty,
        }))
    }

    /// The statements that replace the call at `site`.
    // AST statement lists hold boxed statements (see `ast.rs`).
    #[allow(clippy::vec_box)]
    fn inline_(&mut self, at: &Site) -> Result<Vec<Box<Stmt>>, Error> {
        let h = at.hook.func.clone();
        let count = self.counts.get(&h.name).copied().unwrap_or(0) + 1;
        self.counts.insert(h.name.clone(), count);
        let prefix = if count == 1 {
            let mut prefix = h.name.clone();
            prefix.push(b'_');
            prefix
        } else {
            let mut prefix = h.name.clone();
            prefix.extend_from_slice(count.to_string().as_bytes());
            prefix.push(b'_');
            prefix
        };
        let mut out: Vec<Box<Stmt>> = Vec::new();
        let body: Vec<Box<Stmt>> = match &h.body {
            Body::Block(ss) => ss.clone(),
            Body::Expr(e) => vec![new_stmt(e.loc, StmtData::Ret(Some(e.clone())))],
        };
        let loc_tag = if at.hook.unit == self.unit {
            0
        } else {
            tag(at.hook.unit)
        };
        let locals = self.locals.clone();
        let mut returned: Option<Box<Expr>> = None;
        {
            let mut names = resolve::Context {
                units: self.units,
                caller: self.unit,
                from: at.hook.unit,
                hook: h.name.clone(),
                component: self.component.name.clone(),
                locals: locals.clone(),
                caller_added: &mut *self.added,
                pending_exports: &mut *self.pending,
                cache: HashMap::new(),
            };
            let mut rw = Rewriter {
                env: Vec::new(),
                prefix: Some(prefix),
                loc_tag,
                free: Some(&mut names),
            };
            arguments(&mut rw, at, &mut out, &locals, &h)?;
            rw.predeclare(&body)?;
            for (i, s) in body.iter().enumerate() {
                if let StmtData::Ret(e) = &s.data {
                    if i + 1 != body.len() {
                        return Err(Error {
                            loc: s.loc | rw.loc_tag,
                            msg: "a custom hook returns once, at its end".to_string(),
                        });
                    }
                    returned = match e {
                        Some(v) => Some(rw.expr(v)?),
                        None => None,
                    };
                    continue;
                }
                if let Some(loc) = leftover::returns(s) {
                    return Err(Error {
                        loc: loc | rw.loc_tag,
                        msg: "a custom hook returns once, at its end (not inside `if` or loops)"
                            .to_string(),
                    });
                }
                let rewritten = rw.stmt(s)?;
                out.push(rewritten);
            }
        }
        result(&mut self.aliases, at, returned, &mut out)?;
        Ok(out)
    }
}

fn new_stmt(loc: Loc, data: StmtData) -> Box<Stmt> {
    Box::new(Stmt { loc, data })
}

fn decl_stmt(name: &[u8], loc: Loc, ty: Option<ast::TypeRef>, value: Box<Expr>) -> Box<Stmt> {
    new_stmt(
        loc,
        StmtData::Decl(ast::Decl {
            is_const: true,
            pattern: ast::Pattern::Ident(ast::Name {
                name: name.to_vec(),
                loc,
            }),
            ty,
            init: Some(value),
        }),
    )
}

fn undefined_expr(loc: Loc) -> Box<Expr> {
    Box::new(Expr {
        loc,
        data: ExprData::Ident(b"undefined".to_vec()),
    })
}

/// Parameters become the arguments: names and literals are used as
/// they are, functions and other values become constants.
#[allow(clippy::vec_box)] // AST statement lists hold boxed statements
fn arguments(
    rw: &mut Rewriter<'_>,
    at: &Site,
    out: &mut Vec<Box<Stmt>>,
    locals: &Locals,
    h: &ast::Function,
) -> Result<(), Error> {
    if at.call.args.len() > h.params.len() {
        return Err(Error {
            loc: at.loc,
            msg: format!("`{}` takes {} arguments", text(&h.name), h.params.len()),
        });
    }
    if !at.call.type_args.is_empty() {
        return Err(Error {
            loc: at.loc,
            msg: format!("`{}` takes no type arguments", text(&h.name)),
        });
    }
    let mut bindings: Vec<Entry> = Vec::new();
    for (i, p) in h.params.iter().enumerate() {
        if !p.destructure.is_empty() {
            return Err(Error {
                loc: p.name.loc | rw.loc_tag,
                msg: "a custom hook's parameters are plain names".to_string(),
            });
        }
        let given = i < at.call.args.len();
        if given && matches!(at.call.args[i].data, ExprData::Spread(_)) {
            return Err(Error {
                loc: at.call.args[i].loc,
                msg: "spread arguments are not supported".to_string(),
            });
        }
        // defaults are the hook's code (its file's names)
        let value: Box<Expr> = if given {
            at.call.args[i].clone()
        } else if let Some(d) = &p.default {
            rw.expr(d)?
        } else if p.optional {
            undefined_expr(at.loc)
        } else {
            return Err(Error {
                loc: at.loc,
                msg: format!("`{}` takes {} arguments", text(&h.name), h.params.len()),
            });
        };
        if simple(&value) {
            bindings.push(Entry {
                name: p.name.name.clone(),
                binding: Binding::Expr(value),
            });
            continue;
        }
        let mut name = rw
            .prefix
            .clone()
            .expect("a rewriter inlining a hook has a prefix");
        name.extend_from_slice(&p.name.name);
        let ty = match &p.ty {
            Some(t) => Some(rw.type_ref(t)?),
            None => None,
        };
        let ty = if matches!(value.data, ExprData::Arrow(_)) {
            None
        } else {
            ty
        };
        out.push(decl_stmt(&name, value.loc, ty, value));
        locals.borrow_mut().insert(name.clone());
        bindings.push(Entry {
            name: p.name.name.clone(),
            binding: Binding::Rename(name),
        });
    }
    rw.env.extend(bindings);
    Ok(())
}

/// The caller's names for what the hook returned.
#[allow(clippy::vec_box)] // AST statement lists hold boxed statements
fn result(
    aliases: &mut Rewriter<'_>,
    at: &Site,
    value: Option<Box<Expr>>,
    out: &mut Vec<Box<Stmt>>,
) -> Result<(), Error> {
    let h = &at.hook.func;
    match &at.target {
        Target::None => {
            if let Some(v) = value
                && matches!(v.data, ExprData::Call(_))
            {
                out.push(new_stmt(at.loc, StmtData::Expr(v)));
            }
        }
        Target::Ident(n) => {
            let Some(v) = value else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!("`{}` returns nothing", text(&h.name)),
                });
            };
            if let ExprData::Object(props) = &v.data {
                alias(aliases, &n.name, Binding::Object(props.clone()))?;
            } else {
                give(aliases, n, v, at.ty.clone(), out)?;
            }
        }
        Target::Array(names) => {
            let Some(v) = value else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!("`{}` returns nothing", text(&h.name)),
                });
            };
            let ExprData::Array(items) = &v.data else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!(
                        "`{}` must return an array literal (`return [a, b]`) to destructure it",
                        text(&h.name)
                    ),
                });
            };
            for (i, n) in names.iter().enumerate() {
                if let Some(name) = n {
                    if i >= items.len() {
                        return Err(Error {
                            loc: name.loc,
                            msg: format!("`{}` returns {} values", text(&h.name), items.len()),
                        });
                    }
                    give(aliases, name, items[i].clone(), None, out)?;
                }
            }
        }
        Target::Object(bindings) => {
            let Some(v) = value else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!("`{}` returns nothing", text(&h.name)),
                });
            };
            let ExprData::Object(props) = &v.data else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!(
                        "`{}` must return an object literal (`return {{ a, b }}`) to destructure it",
                        text(&h.name)
                    ),
                });
            };
            for b in bindings {
                let field = props
                    .iter()
                    .find(|p| !p.spread && p.key.name == b.key.name)
                    .map(|p| p.value.clone());
                let Some(field) = field else {
                    return Err(Error {
                        loc: b.key.loc,
                        msg: format!("`{}` returns no `{}`", text(&h.name), text(&b.key.name)),
                    });
                };
                give(aliases, &b.local, field, None, out)?;
            }
        }
    }
    Ok(())
}

/// `name` stands for `value` in the rest of the component: replaced
/// when it is a name or a literal, else a constant.
#[allow(clippy::vec_box)] // AST statement lists hold boxed statements
fn give(
    aliases: &mut Rewriter<'_>,
    name: &ast::Name,
    value: Box<Expr>,
    ty: Option<ast::TypeRef>,
    out: &mut Vec<Box<Stmt>>,
) -> Result<(), Error> {
    if simple(&value) && ty.is_none() {
        return alias(aliases, &name.name, Binding::Expr(value));
    }
    out.push(decl_stmt(&name.name, name.loc, ty, value));
    Ok(())
}

/// The rest of the component reads `name` as `binding`. Earlier aliases
/// may name it (a hook returned a value of a hook it called): they are
/// updated, so every alias is final.
fn alias(aliases: &mut Rewriter<'_>, name: &[u8], binding: Binding) -> Result<(), Error> {
    let mut one = Rewriter::empty();
    one.env.push(Entry {
        name: name.to_vec(),
        binding: binding.clone(),
    });
    for entry in aliases.env.iter_mut() {
        match &mut entry.binding {
            Binding::Rename(_) => {}
            Binding::Expr(e) => {
                *e = one.expr(e)?;
            }
            Binding::Object(props) => {
                let mut copy = Vec::with_capacity(props.len());
                for p in props.iter() {
                    copy.push(ast::Property {
                        key: p.key.clone(),
                        value: one.expr(&p.value)?,
                        spread: p.spread,
                    });
                }
                *props = copy;
            }
        }
    }
    aliases.env.push(Entry {
        name: name.to_vec(),
        binding,
    });
    Ok(())
}

/// Names, literals and member paths: substituting them changes nothing.
fn simple(e: &Expr) -> bool {
    match &e.data {
        ExprData::Ident(_)
        | ExprData::Number(_)
        | ExprData::String(_)
        | ExprData::Boolean(_)
        | ExprData::NullLit => true,
        ExprData::Member(m) => simple(&m.object),
        ExprData::NonNull(inner) => simple(inner),
        ExprData::Template(t) => t.exprs.is_empty(),
        _ => false,
    }
}

fn text(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}
