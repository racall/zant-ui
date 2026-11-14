//! Which child components keep local state (phase 3, `docs/tsx.md`).
//! Translation of `tools/tsx/codegen/stateful.zig`.
//!
//! A component that calls a hook (`useState`, `useRef`, …), renders a
//! `<List>` (its scroll state), or renders another stateful component needs an
//! instance id, so its builder takes one (`TodoRow::build(props, id)`) and its
//! call sites pass `tsx::child_id(parent, "TodoRow", key)` (or
//! `tsx::site_id`). Components without state keep the plain
//! `TodoRow::build(props)` and cost nothing extra.

use crate::ast;
use crate::tags;

use crate::codegen::hooks;

/// Nesting limit for components rendering components across files (cycles).
const MAX_DEPTH: u32 = 16;

/// `./components/TodoRow.tsx` → `TodoRow` (Zig `imports.stem`).
pub fn stem(source: &[u8]) -> Vec<u8> {
    let mut base = source;
    if let Some(index) = base.iter().rposition(|b| *b == b'/' || *b == b'\\') {
        base = &base[index + 1..];
    }
    if let Some(stripped) = base.strip_suffix(b".tsx") {
        base = stripped;
    }
    base.to_vec()
}

/// Whether the project file `source` is the one a relative import names.
fn module_matches(source: &ModuleSource<'_>, import: &[u8]) -> bool {
    source.name.as_bytes() == stem(import).as_slice()
}

/// A parsed file of the project (`imports.NamedModule` in the Zig original;
/// WP-5.6's `imports` module owns the full representation).
#[derive(Clone, Copy, Debug)]
pub struct ModuleSource<'a> {
    /// The file stem (`TodoRow`), the key a relative import names it by.
    pub name: &'a str,
    pub module: &'a ast::Module,
}

/// Whether `func` of `module` (a file of `project`) keeps local state.
pub fn stateful(project: &[ModuleSource<'_>], module: &ast::Module, func: &ast::Function) -> bool {
    check(project, module, func, 0)
}

fn check(
    project: &[ModuleSource<'_>],
    module: &ast::Module,
    func: &ast::Function,
    depth: u32,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let mut walker = Walker {
        project,
        module,
        depth,
        found: false,
    };
    walker.body(&func.body);
    walker.found
}

/// `ui` builders that keep per-instance state and take a stable id first
/// (`zant-tsx-tags` marks them `instance_id`).
pub fn needs_id(name: &[u8]) -> bool {
    tag_needs_id(&String::from_utf8_lossy(name))
}

/// The string form of [`needs_id`] (the tag table is keyed by `&str`).
pub fn tag_needs_id(name: &str) -> bool {
    tags::find(name).is_some_and(|tag| tag.instance_id)
}

pub(crate) struct Walker<'a> {
    pub project: &'a [ModuleSource<'a>],
    pub module: &'a ast::Module,
    pub depth: u32,
    pub found: bool,
}

impl Walker<'_> {
    fn body(&mut self, b: &ast::Body) {
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
                    self.expr(e);
                }
            }
            ast::StmtData::Func(f) => self.body(&f.body),
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
        for a in &j.attrs {
            if let Some(v) = a.value.as_deref() {
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

    /// `useRef`, `useMemo`, `useEffect`, `useState` or the local name it is
    /// imported under.
    fn is_hook(&self, name: &[u8]) -> bool {
        if hooks::extra_hook(&String::from_utf8_lossy(name)) || name == b"useHotkey" {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse_module(src: &str) -> ast::Module {
        let mut parser = Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    // Zig `stateful.zig` test "useState, List and stateful children make a
    // component stateful" (the same source and the same expectations).
    #[test]
    fn usestate_list_and_stateful_children_make_a_component_stateful() {
        let src = "import { useState as useS } from \"zant\";\n\
             function Plain() { return <Text>hi</Text>; }\n\
             function Counter() { const [n, setN] = useS(0); return <Button onClick={() => setN(n + 1)}>{n}</Button>; }\n\
             function Wrapper() { return <View>{true && <Counter />}</View>; }\n\
             function Feed() { return <List data={[1]} renderItem={(x) => <Text>{x}</Text>} />; }\n\
             export default function App() { return <View><Plain /></View>; }\n";
        let module = parse_module(src);
        let expected = [false, true, true, true, false];
        for (top, want) in module.functions.iter().zip(expected) {
            assert_eq!(
                want,
                stateful(&[], &module, &top.func),
                "{}",
                String::from_utf8_lossy(&top.func.name)
            );
        }
    }

    #[test]
    fn widgets_that_take_an_instance_id_are_stateful() {
        assert!(needs_id(b"Markdown"));
        assert!(!needs_id(b"View"));
    }
}
