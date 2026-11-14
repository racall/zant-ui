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

/// `source` as the caller's file writes it: relative paths are relative to
/// the importing file's directory.
fn source_for(from_path: &[u8], caller_path: &[u8], source: &[u8]) -> Vec<u8> {
    if !source.starts_with(b"./") && !source.starts_with(b"../") {
        return source.to_vec();
    }
    let target = normalized(&asset_path(from_path, source));
    let dir = dirname(caller_path)
        .map(normalized)
        .unwrap_or_else(|| normalized(b"."));
    relative(&dir, &target)
}

fn normalized(path: &[u8]) -> Vec<u8> {
    let slashes: Vec<u8> = path
        .iter()
        .map(|&b| if b == b'\\' { b'/' } else { b })
        .collect();
    resolve_posix(&slashes)
}

/// A path with `.` and `..` resolved lexically (POSIX separators only).
fn resolve_posix(path: &[u8]) -> Vec<u8> {
    let is_abs = path.starts_with(b"/");
    let mut negative: usize = 0;
    let mut parts: Vec<&[u8]> = Vec::new();
    for component in path.split(|&b| b == b'/') {
        if component.is_empty() || component == b"." {
            continue;
        }
        if component == b".." {
            if parts.is_empty() {
                if !is_abs {
                    negative += 1;
                }
            } else {
                parts.pop();
            }
        } else {
            parts.push(component);
        }
    }
    let mut joined: Vec<u8> = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            joined.push(b'/');
        }
        joined.extend_from_slice(part);
    }
    if is_abs {
        let mut out = vec![b'/'];
        out.extend_from_slice(&joined);
        out
    } else if negative > 0 {
        let mut out = Vec::new();
        for _ in 0..negative {
            out.extend_from_slice(b"../");
        }
        if joined.is_empty() {
            // "../" n times minus the trailing slash
            out.truncate(out.len() - 1);
        } else {
            out.extend_from_slice(&joined);
        }
        out
    } else if joined.is_empty() {
        b".".to_vec()
    } else {
        joined
    }
}

/// `dir` and `target` are both relative to the same directory (or both
/// absolute).
pub fn relative(dir: &[u8], target: &[u8]) -> Vec<u8> {
    let dir_parts: Vec<&[u8]> = dir
        .split(|&b| b == b'/')
        .filter(|p| !p.is_empty() && *p != b".")
        .collect();
    let target_parts: Vec<&[u8]> = target
        .split(|&b| b == b'/')
        .filter(|p| !p.is_empty() && *p != b".")
        .collect();
    let mut common: usize = 0;
    while common < dir_parts.len()
        && common + 1 < target_parts.len()
        && dir_parts[common] == target_parts[common]
    {
        common += 1;
    }
    let mut out: Vec<u8> = Vec::new();
    if common == dir_parts.len() {
        out.extend_from_slice(b"./");
    }
    for _ in &dir_parts[common..] {
        out.extend_from_slice(b"../");
    }
    for (i, p) in target_parts[common..].iter().enumerate() {
        if i > 0 {
            out.push(b'/');
        }
        out.extend_from_slice(p);
    }
    out
}

/// A custom hook: a top-level `function useX` (or `const useX = () => …`).
pub struct Hook {
    pub unit: usize,
    /// The hook's function (owned: expansion copies it out of the unit so
    /// the units can stay immutable while a component is rewritten).
    pub func: ast::Function,
    pub exported: bool,
}

pub fn hook_in(module: &ast::Module, name: &[u8]) -> Option<Hook> {
    for top in &module.functions {
        if top.func.name.as_slice() == name && is_hook_name(name) {
            return Some(Hook {
                unit: 0,
                func: (*top.func).clone(),
                exported: top.exported,
            });
        }
    }
    for c in &module.consts {
        if c.name.name.as_slice() == name
            && is_hook_name(name)
            && let ast::ExprData::Arrow(f) = &c.value.data
        {
            return Some(Hook {
                unit: 0,
                func: (**f).clone(),
                exported: c.exported,
            });
        }
    }
    None
}

/// The hook `name` means in file `unit`: its own, or one it imports from
/// another file. `name@<unit>` names the hook as the file `unit` sees it.
pub fn find_hook_in(units: &[Unit], unit: usize, name: &[u8]) -> Option<Hook> {
    if let Some(at) = name.iter().position(|&b| b == b'@') {
        let from = text(&name[at + 1..]).parse::<usize>().ok()?;
        if from >= units.len() {
            return None;
        }
        return find_hook_in(units, from, &name[..at]);
    }
    if !is_hook_name(name) {
        return None;
    }
    if let Some(hook) = hook_in(&units[unit].module, name) {
        let mut hook = hook;
        hook.unit = unit;
        return Some(hook);
    }
    for imp in &units[unit].module.imports {
        for n in &imp.names {
            if n.local.name.as_slice() != name {
                continue;
            }
            let target = unit_of(units, &imp.source)?;
            let mut hook = hook_in(&units[target].module, &n.imported.name)?;
            hook.unit = target;
            return Some(hook);
        }
    }
    None
}

/// The project file a relative import names (by stem).
pub fn unit_of(units: &[Unit], source: &[u8]) -> Option<usize> {
    if !source.starts_with(b"./") && !source.starts_with(b"../") {
        return None;
    }
    if is_asset(source) || source.ends_with(b".rs") {
        return None;
    }
    let base = basename(source);
    let stem = if base.ends_with(b".tsx") {
        &base[..base.len() - 4]
    } else {
        base
    };
    units.iter().position(|u| u.name.as_slice() == stem)
}

// ── path helpers (POSIX semantics, on byte slices) ──────────────────────

fn basename(path: &[u8]) -> &[u8] {
    for part in path.rsplit(|&b| b == b'/') {
        if !part.is_empty() {
            return part;
        }
    }
    b""
}

fn trim_end_slashes(path: &[u8]) -> &[u8] {
    let mut end = path.len();
    while end > 0 && path[end - 1] == b'/' {
        end -= 1;
    }
    &path[..end]
}

fn dirname(path: &[u8]) -> Option<&[u8]> {
    let trimmed = trim_end_slashes(path);
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.iter().rposition(|&b| b == b'/') {
        None => None,
        Some(0) => Some(b"/"),
        Some(i) => Some(&trimmed[..i]),
    }
}

fn extension(path: &[u8]) -> &[u8] {
    let filename = basename(path);
    match filename.iter().rposition(|&b| b == b'.') {
        None | Some(0) => b"",
        Some(i) => &filename[i..],
    }
}

fn join_paths(dir: &[u8], source: &[u8]) -> Vec<u8> {
    if source.starts_with(b"/") {
        return source.to_vec();
    }
    if dir.is_empty() {
        return source.to_vec();
    }
    let mut out = dir.to_vec();
    if !out.ends_with(b"/") {
        out.push(b'/');
    }
    out.extend_from_slice(source);
    out
}

/// A relative import with an extension other than a source file's names a
/// file whose bytes are embedded.
fn is_asset(source: &[u8]) -> bool {
    if !source.starts_with(b"./") && !source.starts_with(b"../") {
        return false;
    }
    let ext = extension(source);
    if ext.is_empty() {
        return false;
    }
    ![
        b".tsx".as_slice(),
        b".ts".as_slice(),
        b".rs".as_slice(),
        b".jsx".as_slice(),
        b".js".as_slice(),
    ]
    .contains(&ext)
}

/// The asset's path on disk, relative to the importing file.
fn asset_path(importer: &[u8], source: &[u8]) -> Vec<u8> {
    let dir = dirname(importer).unwrap_or(b".");
    join_paths(dir, source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_between_directories() {
        assert_eq!("../assets/x.png", text(&relative(b"src", b"assets/x.png")));
        assert_eq!("./AppStore", text(&relative(b"src", b"src/AppStore")));
        assert_eq!("./hooks/useX", text(&relative(b"src", b"src/hooks/useX")));
        assert_eq!(
            "../AppStore",
            text(&relative(b"src/hooks", b"src/AppStore"))
        );
        assert_eq!(
            "../../assets/x.png",
            text(&source_for(
                b"src/hooks/useTray.tsx",
                b"src/a/App.tsx",
                b"../../assets/x.png"
            ))
        );
        assert_eq!(
            "../assets/x.png",
            text(&source_for(
                b"src/hooks/useTray.tsx",
                b"src/App.tsx",
                b"../../assets/x.png"
            ))
        );
    }

    // `basename` / `dirname` follow POSIX semantics (trailing slashes, root).
    #[test]
    fn path_helpers_follow_posix_semantics() {
        assert_eq!(b"basename.ext", basename(b"/dir/basename.ext"));
        assert_eq!(b"aaa", basename(b"/aaa/"));
        assert_eq!(b"", basename(b"/"));
        assert_eq!(None, dirname(b"a"));
        assert_eq!(b"/", dirname(b"/a").unwrap());
        assert_eq!(b"src/hooks", dirname(b"src/hooks/useX.tsx").unwrap());
        assert_eq!(None, dirname(b"/"));
        assert_eq!(b".png", extension(b".image.png"));
        assert_eq!(b"", extension(b".gitignore"));
        assert!(is_asset(b"../assets/tray.png"));
        assert!(!is_asset(b"./Row.tsx"));
        assert!(!is_asset(b"zant"));
        assert!(!is_asset(b"./.hidden"));
        assert_eq!(
            b"assets/x.png".as_slice(),
            normalized(b"src/hooks/../../assets/x.png").as_slice()
        );
        assert_eq!(b".".as_slice(), normalized(b"src/..").as_slice());
        assert_eq!(b"..".as_slice(), normalized(b"..").as_slice());
        assert_eq!(b"../a".as_slice(), normalized(b"../a").as_slice());
    }
}
