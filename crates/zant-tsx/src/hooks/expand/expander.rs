//! The expansion of the hook calls of one component: finding call sites and inlining the hook bodies.

use super::*;

impl Expander<'_, '_> {
    pub(super) fn run(mut self) -> Result<ast::Function, Error> {
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

    pub(super) fn add_local(&self, name: &[u8]) {
        self.locals.borrow_mut().insert(name.to_vec());
    }

    pub(super) fn declare(&self, s: &Stmt) {
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

    pub(super) fn site(&self, s: &Stmt) -> Result<Option<Site>, Error> {
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
    pub(super) fn inline_(&mut self, at: &Site) -> Result<Vec<Box<Stmt>>, Error> {
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
                type_names: Vec::new(),
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
