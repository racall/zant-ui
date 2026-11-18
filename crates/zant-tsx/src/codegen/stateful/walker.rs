//! `Walker`: walks a component's body for state, hooks, actions and stateful children.

use super::*;

pub(crate) struct Walker<'a> {
    pub project: &'a [ModuleSource<'a>],
    pub module: &'a ast::Module,
    pub depth: u32,
    pub found: bool,
    /// The component's parameters (`props`, destructured props): a callback
    /// prop passed on as is needs no slot.
    pub forwarded: Vec<Vec<u8>>,
    /// Local `const f = () => …` seen so far in the body.
    pub local_functions: Vec<Vec<u8>>,
}

impl Walker<'_> {
    pub(super) fn body(&mut self, b: &ast::Body) {
        match b {
            ast::Body::Block(ss) => {
                for s in ss {
                    self.stmt(s);
                }
            }
            ast::Body::Expr(e) => self.expr(e),
        }
    }

    fn stmt(&mut self, s: &ast::Stmt) {
        if self.found {
            return;
        }
        match &s.data {
            ast::StmtData::For(v) => {
                if let Some(init) = v.init.as_deref() {
                    self.stmt(init);
                }
                if let Some(cond) = v.cond.as_deref() {
                    self.expr(cond);
                }
                if let Some(update) = v.update.as_deref() {
                    self.expr(update);
                }
                self.stmt(&v.body);
            }
            ast::StmtData::ForOf(v) => {
                self.expr(&v.iterable);
                self.stmt(&v.body);
            }
            ast::StmtData::While(v) => {
                self.expr(&v.cond);
                self.stmt(&v.body);
            }
            ast::StmtData::Switch(v) => {
                self.expr(&v.value);
                for case in &v.cases {
                    if let Some(value) = case.value.as_deref() {
                        self.expr(value);
                    }
                    for child in &case.body {
                        self.stmt(child);
                    }
                }
            }
            ast::StmtData::Break | ast::StmtData::Continue => {}
            ast::StmtData::Decl(d) => {
                if let Some(e) = d.init.as_deref() {
                    if let (ast::Pattern::Ident(name), ast::ExprData::Arrow(_)) =
                        (&d.pattern, &e.data)
                    {
                        self.local_functions.push(name.name.clone());
                    }
                    self.expr(e);
                }
            }
            // a local function is a method; handlers reach it through the slot
            ast::StmtData::Func(_) => self.found = true,
            ast::StmtData::Ret(e) => {
                if let Some(v) = e.as_deref() {
                    self.expr(v);
                }
            }
            ast::StmtData::If(v) => {
                self.expr(&v.cond);
                self.stmt(&v.then);
                if let Some(el) = v.else_.as_deref() {
                    self.stmt(el);
                }
            }
            ast::StmtData::Expr(e) => self.expr(e),
            ast::StmtData::Block(ss) => {
                for child in ss {
                    self.stmt(child);
                }
            }
            ast::StmtData::Throw(e) => self.expr(e),
            ast::StmtData::Try(t) => {
                for child in t
                    .body
                    .iter()
                    .chain(t.catch.iter().flat_map(|c| c.body.iter()))
                    .chain(t.finally.iter().flatten())
                {
                    self.stmt(child);
                }
            }
        }
    }

    fn expr(&mut self, e: &ast::Expr) {
        if self.found {
            return;
        }
        match &e.data {
            ast::ExprData::Number(_)
            | ast::ExprData::String(_)
            | ast::ExprData::Boolean(_)
            | ast::ExprData::NullLit
            | ast::ExprData::Ident(_) => {}
            ast::ExprData::Template(t) => {
                for x in &t.exprs {
                    self.expr(x);
                }
            }
            ast::ExprData::Unary(u) => self.expr(&u.operand),
            ast::ExprData::Binary(b) => {
                self.expr(&b.lhs);
                self.expr(&b.rhs);
            }
            ast::ExprData::Cond(c) => {
                self.expr(&c.cond);
                self.expr(&c.then);
                self.expr(&c.else_);
            }
            ast::ExprData::Call(c) => {
                if let ast::ExprData::Ident(name) = &c.callee.data
                    && self.is_hook(name)
                {
                    self.found = true;
                }
                self.expr(&c.callee);
                for a in &c.args {
                    self.expr(a);
                }
            }
            ast::ExprData::Member(m) => self.expr(&m.object),
            ast::ExprData::Arrow(f) => self.body(&f.body),
            ast::ExprData::Jsx(j) => self.jsx(j),
            ast::ExprData::Object(props) => {
                for p in props {
                    self.expr(&p.value);
                }
            }
            ast::ExprData::Array(items) => {
                for x in items {
                    self.expr(x);
                }
            }
            ast::ExprData::Spread(inner) | ast::ExprData::NonNull(inner) => self.expr(inner),
            ast::ExprData::Index(i) => {
                self.expr(&i.object);
                self.expr(&i.index);
            }
            ast::ExprData::As(a) => self.expr(&a.expr),
            ast::ExprData::Assign(a) => {
                self.expr(&a.target);
                self.expr(&a.value);
            }
            ast::ExprData::Update(u) => self.expr(&u.target),
        }
    }

    fn jsx(&mut self, j: &ast::Jsx) {
        if self.found {
            return;
        }
        let name = String::from_utf8_lossy(&j.name);
        if name == "List" || tag_needs_id(&name) || self.stateful_tag(&name) {
            self.found = true;
            return;
        }
        let tag = tags::find(&name);
        for a in &j.attrs {
            // `{...{ onClick: save }}`: each property is an attribute; a
            // spread of anything else may carry handlers too (`{...look}`)
            if a.spread
                && let Some(v) = a.value.as_deref()
            {
                if self.spread_binds_handler(tag, v) {
                    self.found = true;
                    return;
                }
                self.expr(v);
                continue;
            }
            if let Some(v) = a.value.as_deref() {
                // a handler becomes a method reached through the instance's
                // state slot: without a slot the event is lost
                if self.binds_handler(tag, &a.name.name, v) {
                    self.found = true;
                    return;
                }
                self.expr(v);
            }
        }
        for child in &j.children {
            match child {
                ast::Child::Text(_) => {}
                ast::Child::Expr(e) => {
                    if let Some(v) = e.as_deref() {
                        self.expr(v);
                    }
                }
                ast::Child::Element(el) => self.jsx(el),
            }
        }
    }

    /// Whether attribute `attr = value` of a JSX element binds an event
    /// handler that compiles to a method of this component.
    fn binds_handler(&self, tag: Option<&tags::Tag>, attr: &[u8], value: &ast::Expr) -> bool {
        match &value.data {
            ast::ExprData::Arrow(_) => true,
            ast::ExprData::Ident(name) => {
                if self.forwarded.iter().any(|param| param == name) {
                    return false;
                }
                event_name(attr) || action_prop(tag, attr) || self.is_function(name)
            }
            // `props.onSave` is forwarded; `store.save` and the like are
            // handlers when they sit where a handler goes
            ast::ExprData::Member(member) => {
                let forwarded = matches!(&member.object.data,
                    ast::ExprData::Ident(object) if self.forwarded.iter().any(|param| param == object));
                !forwarded && (event_name(attr) || action_prop(tag, attr))
            }
            // `ready ? save : reset`: either branch may bind one
            ast::ExprData::Cond(cond) => {
                self.binds_handler(tag, attr, &cond.then)
                    || self.binds_handler(tag, attr, &cond.else_)
                    || event_name(attr)
                    || action_prop(tag, attr)
            }
            // literals are no handlers
            ast::ExprData::Number(_)
            | ast::ExprData::String(_)
            | ast::ExprData::Boolean(_)
            | ast::ExprData::NullLit
            | ast::ExprData::Template(_) => false,
            // `cond ? save : undefined`, `makeHandler(id)`, … where a handler
            // goes: conservatively a handler (a slot too many costs little, a
            // missing one drops events)
            _ => event_name(attr) || action_prop(tag, attr),
        }
    }

    /// `{...value}` on an element: an object literal binds a handler when one
    /// of its properties does; any other value (a typed object, props) might
    /// carry a callback field, so it conservatively counts as one.
    fn spread_binds_handler(&self, tag: Option<&tags::Tag>, value: &ast::Expr) -> bool {
        match &value.data {
            ast::ExprData::Object(properties) => properties.iter().any(|property| {
                if property.spread {
                    self.spread_binds_handler(tag, &property.value)
                } else {
                    self.binds_handler(tag, &property.key.name, &property.value)
                }
            }),
            // the component's own props passed on: their callbacks are
            // forwarded values
            ast::ExprData::Ident(name) => !self.forwarded.iter().any(|param| param == name),
            _ => true,
        }
    }

    /// `name` is a function: of this file (top level or a local arrow), a
    /// project file or a native module it imports.
    fn is_function(&self, name: &[u8]) -> bool {
        if self.local_functions.iter().any(|local| local == name)
            || self
                .module
                .functions
                .iter()
                .any(|top| top.func.name == name)
            || self.module.declared.iter().any(|top| top.func.name == name)
            || self.module.consts.iter().any(|top| {
                top.name.name == name && matches!(top.value.data, ast::ExprData::Arrow(_))
            })
        {
            return true;
        }
        for import in &self.module.imports {
            if import.type_only {
                continue;
            }
            for n in &import.names {
                if n.type_only || n.local.name != name {
                    continue;
                }
                // native module specs (`native/capture`) export functions
                if import.source.starts_with(b"native/") {
                    return true;
                }
                if !import.source.starts_with(b".") {
                    return false;
                }
                let Some(target) = self
                    .project
                    .iter()
                    .find(|source| module_matches(source, &import.source))
                else {
                    // a `.ts` helper outside the parsed project: a value
                    // used as an attribute here is most likely a function
                    return true;
                };
                let module = target.module;
                return module
                    .functions
                    .iter()
                    .any(|top| top.func.name == n.imported.name)
                    || module
                        .declared
                        .iter()
                        .any(|top| top.func.name == n.imported.name)
                    || module.consts.iter().any(|top| {
                        top.name.name == n.imported.name
                            && matches!(top.value.data, ast::ExprData::Arrow(_))
                    });
            }
        }
        false
    }

    /// `useRef`, `useMemo`, `useEffect`, `useState` or the local name it is
    /// imported under.
    fn is_hook(&self, name: &[u8]) -> bool {
        if hooks::extra_hook(&String::from_utf8_lossy(name))
            || name == b"useHotkey"
            || name == b"useTray"
        {
            return true;
        }
        for import in &self.module.imports {
            for n in &import.names {
                if n.imported.name == b"useState" && n.local.name == name {
                    return true;
                }
            }
        }
        name == b"useState"
    }

    /// A tag naming a stateful component of this file or an imported one.
    fn stateful_tag(&self, name: &str) -> bool {
        for top in &self.module.functions {
            if top.func.name == name.as_bytes() {
                return check(self.project, self.module, &top.func, self.depth + 1);
            }
        }
        // Relative imports of the project: `module.source` names the file.
        for import in &self.module.imports {
            if !import.source.starts_with(b".") {
                continue;
            }
            for n in &import.names {
                if n.local.name != name.as_bytes() {
                    continue;
                }
                let target = self
                    .project
                    .iter()
                    .find(|source| module_matches(source, &import.source));
                let Some(target) = target else {
                    return false;
                };
                for top in &target.module.functions {
                    if top.exported && top.func.name == n.imported.name {
                        return check(self.project, target.module, &top.func, self.depth + 1);
                    }
                }
                return false;
            }
        }
        false
    }
}
