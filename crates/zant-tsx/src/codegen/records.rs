//! The records the generator keeps: constants, type aliases, states, scope entries, captures, methods and nested calls.

use super::*;

/// A top-level `const` that is not a component.
#[derive(Clone, Debug)]
pub struct Constant {
    pub name: String,
    pub value: types::Value,
}

/// `interface` / `type` alias already resolved.
#[derive(Clone, Debug)]
pub struct TypeAlias {
    pub name: String,
    pub ty: types::Type,
}

/// A resolved structured type, keyed by the AST detail of its `TypeRef`.
///
/// The key is the detail value, not its address: the AST owns its nodes and
/// the code generator clones functions (`functions::signature`,
/// `expr::call_value`, `components::collect`), which allocates fresh detail
/// boxes with the same source location; an address cache would then either
/// miss (duplicate anonymous structs with different `Type`s) or, worse, hit a
/// recycled address (a wrong type).
#[derive(Clone, Debug)]
pub struct ResolvedType {
    pub detail: ast::TypeDetail,
    pub ty: types::Type,
}

/// `useState` or `useRef` (read and written through `.current`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StateKind {
    #[default]
    State,
    Ref,
}

/// A `useState` field of the component being generated.
#[derive(Clone, Debug)]
pub struct State {
    pub name: String,
    /// Empty for a ref.
    pub setter: String,
    pub ty: types::Type,
    pub initial: String,
    pub kind: StateKind,
}

/// A typed binding of the enclosing render, kept while a captured handler
/// method is emitted (see `captures`); a snapshot of the enclosing `Context`.
#[derive(Clone, Debug)]
pub struct ScopeEntry {
    pub name: String,
    pub ty: types::Type,
    pub code: String,
}

/// A render local captured by a handler method: its name, the field that
/// carries it, its type and the expression it is read from.
#[derive(Clone, Debug, Default)]
pub struct Capture {
    pub name: String,
    pub field: String,
    pub ty: types::Type,
    pub source: String,
}

/// An arrow function registered as a method.
#[derive(Clone, Debug)]
pub struct Method {
    pub name: String,
    pub func: Option<Box<ast::Function>>,
    pub action: Option<tags_kind::Kind>,
    pub setter: Option<usize>,
    pub ret: types::Type,
    pub user: bool,
    pub capture_name: Option<String>,
    pub captures: Vec<Capture>,
    pub emitted: Option<String>,
    /// Passed as a child component's callback prop: the parameter types of
    /// that prop (unannotated arrow params take them; missing ones become `_`).
    pub callback_params: Option<Vec<types::Type>>,
    /// An effect with a cleanup (`return () => …` stores the cleanup there).
    pub effect_field: Option<String>,
    /// An effect's cleanup function (runs later; can't read props).
    pub cleanup: bool,
    /// A component function that reads render locals: not a method of its
    /// own, emitted like an inline arrow wherever it is passed.
    pub inline_only: bool,
    /// A promise reaction (`.then(v => …)`): returns `Result<ret, Thrown>`
    /// (a throw rejects the next promise); an expression body's type is
    /// inferred into `ret` (`promises`).
    pub promise_callback: bool,
    /// A function declared inside a handler or effect (`async function
    /// load() { … }` in `useEffect`): a method with captures, called in
    /// place (`local_functions`).
    pub local_function: bool,
}

impl Default for Method {
    fn default() -> Self {
        Method {
            name: String::new(),
            func: None,
            action: None,
            setter: None,
            ret: types::Type::Void,
            user: false,
            capture_name: None,
            captures: Vec::new(),
            emitted: None,
            callback_params: None,
            effect_field: None,
            cleanup: false,
            inline_only: false,
            promise_callback: false,
            local_function: false,
        }
    }
}

/// `tags::Kind` lives in [`crate::tags`]; re-exported under a short name for
/// the signatures in this module.
pub mod tags_kind {
    pub use crate::tags::Kind;
}

/// The enclosing render context of an inline method (`captures.nested`).
#[derive(Clone, Debug, Default)]
pub struct NestedCall {
    pub receiver: String,
    pub target: String,
}

// ── extension points ──────────────────────────────────────────────────
