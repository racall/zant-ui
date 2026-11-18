//! Names an inlined hook reads from its own file, made available in the
//! file of the component it is inlined into (`expand.rs`): the same import
//! (its relative path recomputed), an import of the hook file's top-level
//! declaration, or the name itself when both are one file. A name the
//! component's file already uses for something else gets an alias.
//!
//! The byte-slice path helpers at the end of this file (`is_asset`,
//! `asset_path`) repeat the `&str` ones of `crate::assets`.
//! TODO: share them with `crate::assets`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::ast::{self, Loc};
use crate::hooks::rewrite::{self, Failure, Kind};

mod lookup;
mod paths;
#[cfg(test)]
mod tests;

pub use self::lookup::*;
pub use self::paths::*;

/// A parsed .tsx file of the project.
pub struct Unit {
    /// Its stem: relative imports name files by it.
    pub name: Vec<u8>,
    pub path: Vec<u8>,
    pub module: ast::Module,
    /// Imports added while inlining hooks here.
    pub added: Vec<ast::Import>,
    /// Top-level declarations other files now import.
    pub exported: Vec<Vec<u8>>,
}

pub fn is_hook_name(name: &[u8]) -> bool {
    name.len() > 3 && name.starts_with(b"use") && name[3].is_ascii_uppercase()
}

/// The names a component declares at its top level: shared by `expand.rs`,
/// which adds the names it inlines, and this module, which reads them.
pub type Locals = Rc<RefCell<HashSet<Vec<u8>>>>;

/// Inlining one hook call: from the hook's file into the caller's.
pub struct Context<'a> {
    pub units: &'a [Unit],
    pub caller: usize,
    pub from: usize,
    pub hook: Vec<u8>,
    pub component: Vec<u8>,
    /// Names the component declares at its top level.
    pub locals: Locals,
    /// The caller's added imports (its unit's `added`), written here.
    pub caller_added: &'a mut Vec<ast::Import>,
    /// Names the hook's file must export (its unit's `exported`), applied by
    /// `expand.rs` after the component is done so expansion can read units.
    pub pending_exports: &'a mut Vec<(usize, Vec<u8>)>,
    pub cache: HashMap<Vec<u8>, Vec<u8>>,
}

impl rewrite::Free for Context<'_> {
    fn resolve(&mut self, name: &[u8], loc: Loc, kind: Kind) -> Result<Vec<u8>, Failure> {
        resolve(self, name, loc, kind)
    }
}

fn text(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}

fn bytes(value: impl Into<String>) -> Vec<u8> {
    value.into().into_bytes()
}

/// What a name of the hook's file refers to.
enum Origin {
    /// `import { imported as name } from source`
    Named {
        source: Vec<u8>,
        imported: Vec<u8>,
        type_only: bool,
    },
    /// `import name from source` / `import * as name from source`
    Default(Vec<u8>),
    Namespace(Vec<u8>),
    /// A top-level declaration of the hook's file.
    Top,
}

fn resolve(ctx: &mut Context<'_>, name: &[u8], loc: Loc, kind: Kind) -> Result<Vec<u8>, Failure> {
    if let Some(known) = ctx.cache.get(name) {
        return Ok(known.clone());
    }
    let result = resolve_new(ctx, name, loc, kind)?;
    ctx.cache.insert(name.to_vec(), result.clone());
    Ok(result)
}

fn resolve_new(
    ctx: &mut Context<'_>,
    name: &[u8],
    loc: Loc,
    kind: Kind,
) -> Result<Vec<u8>, Failure> {
    let from = &ctx.units[ctx.from];
    // another hook: called (and inlined) later, from wherever it was found
    if kind == Kind::Value
        && is_hook_name(name)
        && find_hook_in(ctx.units, ctx.from, name).is_some()
    {
        return Ok(bytes(format!("{}@{}", text(name), ctx.from)));
    }
    let origin = match origin_of(&from.module, name, kind) {
        Some(origin) => origin,
        None => return Ok(name.to_vec()),
    };
    if ctx.from == ctx.caller {
        if kind != Kind::TypeName && ctx.locals.borrow().contains(name) {
            return Err(ctx.fail(
                loc,
                format!(
                    "`{}` reads `{}`, which {} also declares; rename one of them",
                    text(&ctx.hook),
                    text(name),
                    text(&ctx.component)
                ),
            ));
        }
        return Ok(name.to_vec());
    }
    let key = origin_key(ctx.units, ctx.from, &origin, name);
    // the caller's file already has it
    if let Some(local) = ctx.existing(&key) {
        return Ok(local);
    }
    let mut local = name.to_vec();
    let mut n: usize = 1;
    while ctx.taken(&local) {
        local = bytes(format!("{}_{}{}", text(name), text(&ctx.hook), n));
        n += 1;
    }
    ctx.add_import(&origin, name, &local, kind);
    Ok(local)
}

fn origin_of(module: &ast::Module, name: &[u8], kind: Kind) -> Option<Origin> {
    for imp in &module.imports {
        for n in &imp.names {
            if n.local.name.as_slice() == name {
                return Some(Origin::Named {
                    source: imp.source.clone(),
                    imported: n.imported.name.clone(),
                    type_only: n.type_only || imp.type_only,
                });
            }
        }
        if let Some(d) = &imp.default_name
            && d.name.as_slice() == name
        {
            return Some(Origin::Default(imp.source.clone()));
        }
        if let Some(d) = &imp.namespace_name
            && d.name.as_slice() == name
        {
            return Some(Origin::Namespace(imp.source.clone()));
        }
    }
    if kind == Kind::TypeName {
        for t in &module.types {
            if t.name.name.as_slice() == name {
                return Some(Origin::Top);
            }
        }
        return None;
    }
    for f in &module.functions {
        if f.func.name.as_slice() == name {
            return Some(Origin::Top);
        }
    }
    for c in &module.consts {
        if c.name.name.as_slice() == name {
            return Some(Origin::Top);
        }
    }
    None
}

/// Identifies what an import names, whichever file it is written in.
fn import_key(importer: &[u8], source: &[u8], what: &[u8]) -> Vec<u8> {
    if !source.starts_with(b"./") && !source.starts_with(b"../") {
        return bytes(format!("{}|{}", text(source), text(what)));
    }
    if is_asset(source) {
        return bytes(format!(
            "asset:{}|{}",
            text(&normalized(&asset_path(importer, source))),
            text(what)
        ));
    }
    // project files are named by their stems
    let base = basename(source);
    let kind = if base.ends_with(b".rs") { "rs" } else { "tsx" };
    let stem = if base.ends_with(b".tsx") {
        &base[..base.len() - 4]
    } else {
        base
    };
    bytes(format!("{}:{}|{}", kind, text(stem), text(what)))
}

fn origin_key(units: &[Unit], from: usize, origin: &Origin, name: &[u8]) -> Vec<u8> {
    let path = &units[from].path;
    match origin {
        Origin::Named {
            source, imported, ..
        } => import_key(path, source, imported),
        Origin::Default(source) => import_key(path, source, b"default"),
        Origin::Namespace(source) => import_key(path, source, b"*"),
        Origin::Top => bytes(format!("tsx:{}|{}", text(&units[from].name), text(name))),
    }
}

impl Context<'_> {
    fn fail(&self, loc: Loc, msg: String) -> Failure {
        Failure { loc, msg }
    }

    /// The caller's local name for `key`, if its file has it.
    fn existing(&self, key: &[u8]) -> Option<Vec<u8>> {
        let caller = &self.units[self.caller];
        // its own declaration
        let own = bytes(format!("tsx:{}|", text(&caller.name)));
        if key.starts_with(&own) {
            let name = &key[own.len()..];
            if !self.locals.borrow().contains(name) {
                return Some(name.to_vec());
            }
        }
        for imp in caller.module.imports.iter().chain(self.caller_added.iter()) {
            for n in &imp.names {
                let n_key = import_key(&caller.path, &imp.source, &n.imported.name);
                if n_key == key && !self.locals.borrow().contains(&n.local.name) {
                    return Some(n.local.name.clone());
                }
            }
            let default_key = import_key(&caller.path, &imp.source, b"default");
            if let Some(d) = &imp.default_name
                && default_key == key
                && !self.locals.borrow().contains(&d.name)
            {
                return Some(d.name.clone());
            }
            if let Some(d) = &imp.namespace_name {
                let namespace_key = import_key(&caller.path, &imp.source, b"*");
                if namespace_key == key && !self.locals.borrow().contains(&d.name) {
                    return Some(d.name.clone());
                }
            }
        }
        None
    }

    /// Is `name` used in the caller's file (or its component) already?
    fn taken(&self, name: &[u8]) -> bool {
        if self.locals.borrow().contains(name) {
            return true;
        }
        let caller = &self.units[self.caller];
        if origin_of(&caller.module, name, Kind::Value).is_some()
            || origin_of(&caller.module, name, Kind::TypeName).is_some()
        {
            return true;
        }
        for imp in self.caller_added.iter() {
            for n in &imp.names {
                if n.local.name.as_slice() == name {
                    return true;
                }
            }
            if let Some(d) = &imp.default_name
                && d.name.as_slice() == name
            {
                return true;
            }
            if let Some(d) = &imp.namespace_name
                && d.name.as_slice() == name
            {
                return true;
            }
        }
        false
    }

    fn add_import(&mut self, origin: &Origin, name: &[u8], local: &[u8], kind: Kind) {
        let from_path = self.units[self.from].path.clone();
        let from_name = self.units[self.from].name.clone();
        let caller_path = self.units[self.caller].path.clone();
        let at = ast::Name {
            name: local.to_vec(),
            loc: 0,
        };
        let mut imp = ast::Import {
            loc: 0,
            source: Vec::new(),
            source_loc: 0,
            type_only: false,
            names: Vec::new(),
            default_name: None,
            namespace_name: None,
        };
        match origin {
            Origin::Named {
                source,
                imported,
                type_only,
            } => {
                imp.source = source_for(&from_path, &caller_path, source);
                imp.names = vec![ast::ImportName {
                    type_only: *type_only || kind == Kind::TypeName,
                    imported: ast::Name {
                        name: imported.clone(),
                        loc: 0,
                    },
                    local: at,
                }];
            }
            Origin::Default(source) => {
                imp.source = source_for(&from_path, &caller_path, source);
                imp.default_name = Some(at);
            }
            Origin::Namespace(source) => {
                imp.source = source_for(&from_path, &caller_path, source);
                imp.namespace_name = Some(at);
            }
            Origin::Top => {
                imp.source = bytes(format!("./{}", text(&from_name)));
                imp.names = vec![ast::ImportName {
                    type_only: kind == Kind::TypeName,
                    imported: ast::Name {
                        name: name.to_vec(),
                        loc: 0,
                    },
                    local: at,
                }];
                self.pending_exports.push((self.from, name.to_vec()));
            }
        }
        self.caller_added.push(imp);
    }
}
