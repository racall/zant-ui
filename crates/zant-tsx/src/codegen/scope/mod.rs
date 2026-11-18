//! Function scopes, Rust name clashes, unused-value discards, and handler
//! state snapshots.
//!
//! Bindings live in a `Vec<Binding>` and are referred to by index.
//! Declaration placeholders are expanded by [`finish`]: the control-byte
//! markers are replaced after the body has been generated.

use crate::ast;

use super::model;
use super::narrowing;
use super::rust_text;
use super::types::{Type, Value};
use super::{Generator, Result, fail};

mod finish;
mod names;
mod reads;
#[cfg(test)]
mod tests;

pub use self::finish::*;
pub use self::names::*;
pub use self::reads::*;

pub const PLACEHOLDER_START: char = '\u{1}';

pub const PLACEHOLDER_END: char = '\u{2}';

pub const DISCARD_START: char = '\u{3}';

pub const DISCARD_END: char = '\u{4}';

#[derive(Clone, Debug, Default)]
pub struct Binding {
    pub name: String,
    /// The Rust identifier.
    pub rust: String,
    pub ty: Type,
    pub mutable: bool,
    pub assigned: bool,
    /// The initializer, materialized by [`finish`].
    pub init: Option<String>,
    pub serial: usize,
    /// Holds a value no other variable shares, so it may change in place
    /// (`place`): set from the initializer or the last assignment.
    pub fresh: bool,
    /// Used as a whole value (passed, stored, copied) since it became fresh.
    pub escaped: bool,
    /// Assignments emitted so far: a narrowing made before the latest one no
    /// longer holds (`narrowing`, TypeScript's flow analysis).
    pub assignments: u32,
}

#[derive(Clone, Debug)]
pub struct ReturnValue {
    pub value: Value,
    pub loc: ast::Loc,
}

#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub state: usize,
    pub binding: usize,
}

/// A function scope.
#[derive(Clone, Debug)]
pub struct Context {
    pub render: bool,
    pub ret: Type,
    pub active: Vec<usize>,
    pub bindings: Vec<Binding>,
    pub params: Vec<usize>,
    pub constant: bool,
    pub return_label: Option<String>,
    /// Nearest statement control targets; callbacks start a fresh control scope.
    pub break_label: Option<String>,
    pub continue_label: Option<String>,
    pub return_values: Option<Vec<ReturnValue>>,
    /// A callback's source context: the visible local names of the enclosing
    /// function.
    pub capture_source: Option<std::rc::Rc<CaptureSource>>,
    pub capture_method: Option<usize>,
    pub snapshots: Vec<Snapshot>,
    /// Inside an effect with a cleanup / inside a cleanup (`hooks`).
    pub effect_field: Option<String>,
    pub cleanup: bool,
    /// A plain top-level function (`functions`): no component, no `self`.
    pub no_self: bool,
    /// Optionals proven non-null here (`narrowing`).
    pub narrowed: Vec<narrowing::Narrow>,
    /// The runtime context parameter available in this handler (see
    /// [`super::action_cx`]): a local must not shadow it.
    pub cx_var: Option<String>,
    /// Functions declared in this handler so far: name → method index
    /// (`local_functions`).
    pub local_functions: Vec<(Vec<u8>, usize)>,
    /// The next local read only looks into the value (`place::project`).
    pub projection: bool,
    /// Where a throw outside every `try` of this body goes (`exceptions`).
    pub throw_mode: super::exceptions::ThrowMode,
    /// The enclosing `try` regions of this body, innermost last.
    pub try_frames: Vec<super::exceptions::Frame>,
    /// The labels of the enclosing loops / switches of this body, outermost
    /// first (a `finally` reroutes the jumps that leave it).
    pub jump_labels: Vec<String>,
}

impl Default for Context {
    fn default() -> Self {
        Context {
            render: false,
            ret: Type::Void,
            active: Vec::new(),
            bindings: Vec::new(),
            params: Vec::new(),
            constant: false,
            return_label: None,
            break_label: None,
            continue_label: None,
            return_values: None,
            capture_source: None,
            capture_method: None,
            snapshots: Vec::new(),
            effect_field: None,
            cleanup: false,
            no_self: false,
            narrowed: Vec::new(),
            cx_var: None,
            local_functions: Vec::new(),
            projection: false,
            throw_mode: super::exceptions::ThrowMode::Forbid,
            try_frames: Vec::new(),
            jump_labels: Vec::new(),
        }
    }
}

/// The visible names of an enclosing function, for `builtins.shadowed`.
#[derive(Clone, Debug, Default)]
pub struct CaptureSource {
    pub names: Vec<String>,
    pub parent: Option<std::rc::Rc<CaptureSource>>,
}

impl Context {
    /// A snapshot of the names visible to a callback created here.
    pub fn capture_source(&self) -> std::rc::Rc<CaptureSource> {
        std::rc::Rc::new(CaptureSource {
            names: self
                .active
                .iter()
                .map(|index| self.bindings[*index].name.clone())
                .collect(),
            parent: self.capture_source.clone(),
        })
    }
}
