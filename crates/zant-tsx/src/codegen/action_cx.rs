//! The runtime `ActionCx` a handler needs to call a callback prop.
//!
//! A child component calls a callback prop synchronously (see
//! `components::try_call`). Callbacks are `ui::Action`-family values /
//! `Rc<dyn Fn(&mut ui::ActionCx<'_>, …)>`, which can only run inside
//! an action with the runtime context. A generated handler therefore takes the
//! context as its first parameter when its body calls a callback prop:
//!
//! ```ignore
//! fn _on_click2(&mut self, cx: &mut ui::ActionCx<'_>) {
//!     (self.props.onDelete.clone())(cx, self.props.note.clone().id);
//! }
//! ```
//!
//! [`needs_cx`] decides whether a handler needs the parameter. It walks the
//! handler body once before it is emitted:
//!
//! - a call to one of the component's callback props (directly, or through a
//!   callback prop of a non-destructured `props` parameter) needs it;
//! - a call to another generated function that needs it needs it too
//!   (`function add(delta) { setN(n + delta); onChange(delta); }` called from
//!   an event arrow is the real-world shape);
//! - event / callback arrows are emitted as their own methods, so their bodies
//!   are skipped here (their own `needs_cx` is computed when they are
//!   registered); array callbacks (`items.map(…)`) are closures emitted inside
//!   the handler and are walked.
//!
//! The decision is **not** stored: the code generator asks the same question
//! from the same AST every time it needs it (handler signature, `ui::*Action`
//! wrapper, direct call), so the answers cannot disagree.

use crate::ast;

use super::Generator;
use super::components;
use super::model;
use super::scope;

/// The handler parameter carrying the runtime context (`_cx`).
pub const CX: &str = "_cx";

/// Guard against pathological call graphs; components are generated in one
/// pass and real handlers nest a handful of calls deep.
const MAX_DEPTH: u32 = 64;

/// The current component's callback props, as call sites name them.
#[derive(Default)]
struct CallbackProps {
    /// Destructured callback props: `onDelete(..)`.
    destructured: Vec<Vec<u8>>,
    /// The props parameter when it is not destructured: `props.onDelete(..)`.
    param: Option<Vec<u8>>,
    /// Callback fields reachable through that parameter.
    fields: Vec<Vec<u8>>,
}

fn callback_props(g: &Generator<'_>) -> CallbackProps {
    let mut props = CallbackProps::default();
    let Some(index) = g.current_component else {
        return props;
    };
    let component = &g.components[index];
    let Some(props_ty) = component.props else {
        return props;
    };
    let Some(info) = model::info(g, props_ty) else {
        return props;
    };
    let is_callback = |ty| components::callback_info(g, ty).is_some();
    if !component.destructure.is_empty() {
        for name in &component.destructure {
            let field = info.fields.iter().find(|field| field.name == name.name);
            if let Some(field) = field
                && is_callback(field.ty)
            {
                props.destructured.push(name.name.clone());
            }
        }
    } else if !component.param.is_empty() {
        props.param = Some(component.param.as_bytes().to_vec());
        for field in &info.fields {
            if is_callback(field.ty) {
                props.fields.push(field.name.clone());
            }
        }
    }
    props
}

/// Whether the handler whose body is `body` (its parameters already in scope)
/// calls a callback prop and so needs the runtime context parameter.
fn walk(g: &Generator<'_>, function: &ast::Function) -> bool {
    let mut walker = Walker {
        g,
        props: callback_props(g),
        scopes: Vec::new(),
        visiting: Vec::new(),
        depth: 0,
    };
    walker.function(function)
}

/// Whether the generated handler `function` needs the runtime context.
pub fn needs_cx(g: &Generator<'_>, function: &ast::Function) -> bool {
    walk(g, function)
}

/// Whether the handler at `index` needs the runtime context.
pub fn method_needs_cx(g: &Generator<'_>, index: usize) -> bool {
    match g.methods[index].func.as_deref() {
        Some(function) => needs_cx(g, function),
        None => false,
    }
}

struct Walker<'a, 'g> {
    g: &'a Generator<'g>,
    props: CallbackProps,
    /// Declared local names, innermost scope last.
    scopes: Vec<Vec<Vec<u8>>>,
    /// Methods currently being walked (direct-call cycles end there).
    visiting: Vec<usize>,
    depth: u32,
}

impl Walker<'_, '_> {
    fn declared(&self, name: &[u8]) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|scope| scope.iter().any(|local| local == name))
    }

    fn function(&mut self, function: &ast::Function) -> bool {
        let mut names: Vec<Vec<u8>> = Vec::new();
        for param in &function.params {
            if !param.name.name.is_empty() {
                names.push(param.name.name.clone());
            }
            for name in &param.destructure {
                names.push(name.name.clone());
            }
        }
        self.scopes.push(names);
        let found = self.body(&function.body);
        self.scopes.pop();
        found
    }

    fn body(&mut self, body: &ast::Body) -> bool {
        match body {
            ast::Body::Expr(e) => self.expr(e),
            ast::Body::Block(statements) => self.statements(statements, false),
        }
    }

    fn statements(&mut self, statements: &[Box<ast::Stmt>], scoped: bool) -> bool {
        if scoped {
            self.scopes.push(Vec::new());
        }
        let found = statements.iter().any(|statement| self.statement(statement));
        if scoped {
            self.scopes.pop();
        }
        found
    }

    fn statement(&mut self, statement: &ast::Stmt) -> bool {
        match &statement.data {
            ast::StmtData::Decl(declaration) => {
                if let Some(init) = declaration.init.as_deref()
                    && self.expr(init)
                {
                    return true;
                }
                // the declaration joins its enclosing block's scope
                let mut names = Vec::new();
                pattern_names(&declaration.pattern, &mut names);
                match self.scopes.last_mut() {
                    Some(scope) => scope.extend(names),
                    None => self.scopes.push(names),
                }
                false
            }
            ast::StmtData::Func(function) => self.function(function),
            ast::StmtData::Ret(value) => value.as_deref().is_some_and(|e| self.expr(e)),
            ast::StmtData::If(value) => {
                self.expr(&value.cond) || self.statement(&value.then) || {
                    match value.else_.as_deref() {
                        Some(otherwise) => self.statement(otherwise),
                        None => false,
                    }
                }
            }
            ast::StmtData::For(value) => {
                self.scopes.push(Vec::new());
                let mut found = value
                    .init
                    .as_deref()
                    .is_some_and(|init| self.statement(init));
                if !found && let Some(cond) = value.cond.as_deref() {
                    found = self.expr(cond);
                }
                if !found && let Some(update) = value.update.as_deref() {
                    found = self.expr(update);
                }
                if !found {
                    found = self.statement(&value.body);
                }
                self.scopes.pop();
                found
            }
            ast::StmtData::ForOf(value) => {
                if self.expr(&value.iterable) {
                    return true;
                }
                self.scopes.push(vec![value.name.name.clone()]);
                let found = self.statement(&value.body);
                self.scopes.pop();
                found
            }
            ast::StmtData::While(value) => self.expr(&value.cond) || self.statement(&value.body),
            ast::StmtData::Switch(value) => {
                if self.expr(&value.value) {
                    return true;
                }
                value.cases.iter().any(|case| {
                    case.value.as_deref().is_some_and(|test| self.expr(test))
                        || self.statements(&case.body, true)
                })
            }
            ast::StmtData::Break | ast::StmtData::Continue => false,
            ast::StmtData::Expr(e) => self.expr(e),
            ast::StmtData::Block(statements) => self.statements(statements, true),
        }
    }

    fn expr(&mut self, expr: &ast::Expr) -> bool {
        match &expr.data {
            ast::ExprData::Number(_)
            | ast::ExprData::String(_)
            | ast::ExprData::Boolean(_)
            | ast::ExprData::NullLit
            | ast::ExprData::Ident(_) => false,
            ast::ExprData::Template(template) => template.exprs.iter().any(|e| self.expr(e)),
            ast::ExprData::Unary(unary) => self.expr(&unary.operand),
            ast::ExprData::Binary(binary) => self.expr(&binary.lhs) || self.expr(&binary.rhs),
            ast::ExprData::Cond(value) => {
                self.expr(&value.cond) || self.expr(&value.then) || self.expr(&value.else_)
            }
            ast::ExprData::Call(call) => {
                if self.call(call) {
                    return true;
                }
                call.args.iter().any(|arg| self.expr(arg))
            }
            ast::ExprData::Member(member) => self.expr(&member.object),
            ast::ExprData::Arrow(function) => {
                // Event / callback-prop arrows are emitted as their own
                // handlers: their bodies are not part of this handler.
                if self.registered(function) {
                    return false;
                }
                self.function(function)
            }
            ast::ExprData::Jsx(jsx) => self.jsx(jsx),
            ast::ExprData::Object(properties) => {
                properties.iter().any(|property| self.expr(&property.value))
            }
            ast::ExprData::Array(items) => items.iter().any(|item| self.expr(item)),
            ast::ExprData::Spread(inner) | ast::ExprData::NonNull(inner) => self.expr(inner),
            ast::ExprData::Index(index) => self.expr(&index.object) || self.expr(&index.index),
            ast::ExprData::As(cast) => self.expr(&cast.expr),
            ast::ExprData::Assign(assign) => self.expr(&assign.target) || self.expr(&assign.value),
            ast::ExprData::Update(update) => self.expr(&update.target),
        }
    }

    /// The callee: a callback prop call or a direct call to another handler.
    fn call(&mut self, call: &ast::Call) -> bool {
        match &call.callee.data {
            ast::ExprData::Ident(name) => {
                if self.declared(name) {
                    return false;
                }
                if self.props.destructured.iter().any(|prop| prop == name) {
                    return true;
                }
                match scope::source_method(self.g, name) {
                    Some(index) => self.method(index),
                    None => false,
                }
            }
            ast::ExprData::Member(member) => {
                let ast::ExprData::Ident(object) = &member.object.data else {
                    return self.expr(&member.object);
                };
                if !self.declared(object)
                    && self
                        .props
                        .param
                        .as_ref()
                        .is_some_and(|param| param == object)
                    && self
                        .props
                        .fields
                        .iter()
                        .any(|field| field.as_slice() == member.name.name.as_slice())
                {
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn method(&mut self, index: usize) -> bool {
        if self.depth >= MAX_DEPTH || self.visiting.contains(&index) {
            return false;
        }
        let Some(function) = self.g.methods[index].func.clone() else {
            return false;
        };
        self.visiting.push(index);
        self.depth += 1;
        let found = self.function(&function);
        self.depth -= 1;
        self.visiting.pop();
        found
    }

    /// Is this arrow one of the handler methods registered for the component?
    fn registered(&self, function: &ast::Function) -> bool {
        self.g
            .methods
            .iter()
            .any(|method| method.func.as_deref() == Some(function))
    }

    fn jsx(&mut self, jsx: &ast::Jsx) -> bool {
        for attr in &jsx.attrs {
            if let Some(value) = attr.value.as_deref()
                && self.expr(value)
            {
                return true;
            }
        }
        jsx.children.iter().any(|child| match child {
            ast::Child::Text(_) | ast::Child::Expr(None) => false,
            ast::Child::Expr(Some(value)) => self.expr(value),
            ast::Child::Element(element) => self.jsx(element),
        })
    }
}

fn pattern_names(pattern: &ast::Pattern, out: &mut Vec<Vec<u8>>) {
    match pattern {
        ast::Pattern::Ident(name) => out.push(name.name.clone()),
        ast::Pattern::Array(names) => {
            out.extend(names.iter().flatten().map(|name| name.name.clone()));
        }
        ast::Pattern::Object(bindings) => {
            out.extend(bindings.iter().map(|binding| binding.local.name.clone()));
        }
    }
}
