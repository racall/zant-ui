//! Copies statements and expressions while renaming the bindings they
//! declare and replacing the names they read: how a custom hook's body is
//! inlined into a component (`expand.rs`), and how names a hook returned
//! are replaced in the rest of the component.
//!
//! The only error is [`Failure`]; copies own their nodes.

use crate::ast::{self, Loc};

mod exprs;
mod statements;
mod types;

/// Why inlining failed, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub loc: Loc,
    pub msg: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Binding {
    /// Another name (a renamed local, an import alias).
    Rename(Vec<u8>),
    /// An expression (a hook's argument, a value it returned).
    Expr(Box<ast::Expr>),
    /// An object a hook returned: `v.key` is the property's value.
    Object(Vec<ast::Property>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: Vec<u8>,
    pub binding: Binding,
}

/// What a name the copied code reads without declaring it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Value,
    TypeName,
    Tag,
}

/// How a name the copied code reads without declaring it is resolved in the
/// copy (callers pass a trait object).
pub trait Free {
    fn resolve(&mut self, name: &[u8], loc: Loc, kind: Kind) -> Result<Vec<u8>, Failure>;
}

pub struct Rewriter<'a> {
    pub env: Vec<Entry>,
    /// Lexically bound type names, separate from value bindings.
    pub type_names: Vec<Vec<u8>>,
    /// Bindings declared in the copy get this prefix (null: they keep their names).
    pub prefix: Option<Vec<u8>>,
    /// Or-ed into every location (the file of inlined code, `expand.rs`).
    pub loc_tag: Loc,
    pub free: Option<&'a mut dyn Free>,
}

impl Rewriter<'static> {
    /// A rewriter that only replaces names from its environment (no inlining).
    pub fn empty() -> Self {
        Rewriter {
            env: Vec::new(),
            type_names: Vec::new(),
            prefix: None,
            loc_tag: 0,
            free: None,
        }
    }
}

fn fail(at: Loc, msg: impl Into<String>) -> Failure {
    Failure {
        loc: at,
        msg: msg.into(),
    }
}

fn text(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}

impl Rewriter<'_> {
    fn lookup(&self, name: &[u8]) -> Option<Binding> {
        self.env
            .iter()
            .rev()
            .find(|entry| entry.name == name)
            .map(|entry| entry.binding.clone())
    }

    /// Declares `name` in the copy; returns its new name.
    pub fn bind(&mut self, name: &[u8]) -> Vec<u8> {
        let new = match &self.prefix {
            Some(prefix) => {
                let mut new = prefix.clone();
                new.extend_from_slice(name);
                new
            }
            None => name.to_vec(),
        };
        self.env.push(Entry {
            name: name.to_vec(),
            binding: Binding::Rename(new.clone()),
        });
        new
    }

    fn renamed(&mut self, name: &ast::Name) -> Result<ast::Name, Failure> {
        let new = match self.lookup(&name.name) {
            Some(Binding::Rename(new)) => new,
            _ => self.bind(&name.name),
        };
        Ok(ast::Name {
            name: new,
            loc: self.loc(name.loc),
        })
    }

    fn loc(&self, l: Loc) -> Loc {
        l | self.loc_tag
    }

    fn opt(&mut self, e: &Option<Box<ast::Expr>>) -> Result<Option<Box<ast::Expr>>, Failure> {
        match e {
            Some(v) => Ok(Some(self.expr(v)?)),
            None => Ok(None),
        }
    }
}
