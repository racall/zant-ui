//! Flatten fragments and preserve conditional-child evaluation with hidden
//! bindings. Translation of `tools/tsx/codegen/jsx_children.zig`.
//!
//! Rust has the `nodes!` macro, which accepts a node, an optional node or a
//! node list, so the Zig slice concatenation (`tsx.concat`) disappears: a
//! child list is simply `nodes![…]`. Conditions are still hoisted into hidden
//! `boolean` locals so each branch is evaluated once, exactly like the Zig
//! `_condition` bindings.

use crate::ast;

use super::expr;
use super::jsx;
use super::model;
use super::narrowing;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

/// The children of one JSX element, in source order.
#[derive(Clone, Debug, Default)]
pub struct ChildList {
    pub values: Vec<Value>,
    /// Hoisted condition declarations (`\x01serial\x02` placeholders).
    pub prefix: String,
}

impl ChildList {
    pub fn new() -> ChildList {
        ChildList::default()
    }

    /// A compiler temporary binding: its name never resolves in TSX source.
    fn hidden(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        name: &[u8],
        ty: Type,
    ) -> Result<(usize, String)> {
        let index = scope::bind(g, c, name, ty, false)?;
        let zig = c.bindings[index].zig.clone();
        c.bindings[index].name.clear();
        Ok((index, zig))
    }

    pub fn append(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        children: &[ast::Child],
        guard: Option<&str>,
    ) -> Result<()> {
        for child in children {
            match child {
                ast::Child::Text(text) => {
                    return fail(g, text.loc, "text must be inside <Text>");
                }
                ast::Child::Element(element) => self.element(g, c, element, guard)?,
                ast::Child::Expr(Some(e)) => self.expression(g, c, e, guard)?,
                ast::Child::Expr(None) => {}
            }
        }
        Ok(())
    }

    /// Flatten a fragment or append its element under the inherited guard.
    fn element(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        element: &ast::Jsx,
        guard: Option<&str>,
    ) -> Result<()> {
        if element.name.is_empty() {
            return self.append(g, c, &element.children, guard);
        }
        // nested component tags must route through the component-aware
        // dispatcher (Zig `jsx.zig:251` emits components inside `emit`)
        let value = super::generate::emit_jsx(g, c, element)?;
        self.node(g, c, value, guard, element.loc)
    }

    pub fn node(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        value: Value,
        guard: Option<&str>,
        loc: ast::Loc,
    ) -> Result<()> {
        // `Option<Vec<Node>>`: a list-valued optional child
        if let Some(child) = model::optional_child(g, value.ty)
            && model::element(g, child).is_some()
        {
            let (_, name) = self.hidden(g, c, b"_nodes", child)?;
            let unwrapped = Value::new(
                child,
                format!(
                    "if let Some({name}) = {} {{ {name} }} else {{ Vec::new() }}",
                    value.code
                ),
            );
            return self.node(g, c, unwrapped, guard, loc);
        }
        if let Some(element_type) = model::element(g, value.ty) {
            if element_type == Type::Null {
                let empty = Value::new(model::array(g, Type::Node)?, "nodes![]".to_owned());
                return self.node(g, c, empty, guard, loc);
            }
            if element_type != Type::Node && element_type != Type::OptionalNode {
                return fail(g, loc, "children arrays must contain nodes");
            }
            let code = match guard {
                Some(cond) => format!("if {cond} {{ {} }} else {{ Vec::new() }}", value.code),
                None => value.code,
            };
            self.values.push(Value::new(value.ty, code));
            return Ok(());
        }
        if value.ty != Type::Node && value.ty != Type::OptionalNode && value.ty != Type::Null {
            return fail(g, loc, "text must be inside <Text>");
        }
        if value.ty == Type::Null && value.code != "None" {
            let coerced = types::coerce(g, value, Type::OptionalNode, loc)?;
            return self.node(g, c, coerced, guard, loc);
        }
        match guard {
            Some(cond) => {
                if value.ty == Type::Null {
                    return Ok(());
                }
                self.values.push(Value::new(
                    Type::OptionalNode,
                    format!("if {cond} {{ {} }} else {{ None }}", value.code),
                ));
            }
            None => self.values.push(value),
        }
        Ok(())
    }

    /// Hoist a condition into a hidden local and return its Rust name.
    pub fn condition(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        e: &ast::Expr,
        guard: Option<&str>,
    ) -> Result<String> {
        let value = expr::condition(g, c, e)?;
        let (index, zig) = self.hidden(g, c, b"_condition", Type::Boolean)?;
        let init = match guard {
            Some(parent) => {
                expr::infix(
                    g,
                    Value::new(Type::Boolean, parent),
                    "&&",
                    value,
                    Type::Boolean,
                )?
                .code
            }
            None => value.code,
        };
        let serial = c.bindings[index].serial;
        c.bindings[index].init = Some(init);
        self.prefix.push_str(&format!(
            "{}{}{}",
            scope::PLACEHOLDER_START,
            serial,
            scope::PLACEHOLDER_END
        ));
        Ok(zig)
    }

    pub fn expression(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        e: &ast::Expr,
        guard: Option<&str>,
    ) -> Result<()> {
        if let ast::ExprData::NonNull(inner) = &e.data
            && jsx::has_fragment(inner)
        {
            return self.expression(g, c, inner, guard);
        }
        if let ast::ExprData::Jsx(element) = &e.data
            && element.name.is_empty()
        {
            return self.append(g, c, &element.children, guard);
        }
        if let ast::ExprData::Binary(binary) = &e.data
            && binary.op == b"&&"
            && jsx::has_fragment(&binary.rhs)
        {
            let cond = self.condition(g, c, &binary.lhs, guard)?;
            let mark = narrowing::push(g, c, &binary.lhs, true)?;
            let result = self.expression(g, c, &binary.rhs, Some(&cond));
            narrowing::restore(c, mark);
            return result;
        }
        if let ast::ExprData::Cond(value) = &e.data {
            let splits = jsx::has_fragment(&value.then)
                || jsx::has_fragment(&value.else_)
                || (is_list(&value.then) && matches!(value.else_.data, ast::ExprData::Jsx(_)))
                || (is_list(&value.else_) && matches!(value.then.data, ast::ExprData::Jsx(_)));
            if splits {
                let cond = self.condition(g, c, &value.cond, guard)?;
                let mut mark = narrowing::push(g, c, &value.cond, true)?;
                let then_result = self.expression(g, c, &value.then, Some(&cond));
                narrowing::restore(c, mark);
                then_result?;
                mark = narrowing::push(g, c, &value.cond, false)?;
                let otherwise = match guard {
                    Some(parent) => format!("{parent} && !{cond}"),
                    None => format!("!{cond}"),
                };
                let result = self.expression(g, c, &value.else_, Some(&otherwise));
                narrowing::restore(c, mark);
                return result;
            }
        }
        let value = expr::emit(g, c, e, None, true)?;
        self.node(g, c, value, guard, e.loc)
    }

    /// Wrap the finished node in a labeled block when conditions were hoisted.
    pub fn wrap(
        &mut self,
        g: &mut Generator<'_>,
        c: &mut scope::Context,
        code: &str,
    ) -> Result<String> {
        if self.prefix.is_empty() {
            return Ok(code.to_owned());
        }
        let (_, label) = self.hidden(g, c, b"_children", Type::Void)?;
        Ok(format!(
            "'{label}: {{\n{prefix}break '{label} {code};\n}}",
            prefix = self.prefix
        ))
    }
}

/// An expression that gives several children (`items.map(…)`, `[a, b]`): a
/// ternary with one in a branch and an element in the other becomes two
/// guarded children (`ok ? <Hint/> : peers.map(…)`).
fn is_list(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Array(_) => true,
        ast::ExprData::Call(call) => match &call.callee.data {
            ast::ExprData::Member(member) => {
                member.name.name == b"map" || member.name.name == b"filter"
            }
            _ => false,
        },
        ast::ExprData::Cond(value) => is_list(&value.then) || is_list(&value.else_),
        ast::ExprData::NonNull(inner) => is_list(inner),
        _ => false,
    }
}
