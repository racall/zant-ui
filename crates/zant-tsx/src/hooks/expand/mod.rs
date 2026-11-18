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

mod expander;
mod results;

use self::results::*;

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

impl Expander<'_, '_> {}
