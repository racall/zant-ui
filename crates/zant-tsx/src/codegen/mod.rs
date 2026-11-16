//! Rust code generation core: the shared state of the code generator (the
//! [`Generator`]), its diagnostic helper and the extension points through
//! which the JSX, component/state and application modules plug in.
//!
//! The entry point is [`generate`] (module validation, component collection
//! and emission), which builds the generator with [`Generator::new`]. The
//! emitted Rust follows these conventions:
//!
//! - `string` is `zant::Str`, `T[]` is `tsx::List<T>`, `T | null` is
//!   `Option<T>`, `ui::Node` is `ui::Node`.
//! - Every read of a non-`Copy` value appends `.clone()`; the generated file
//!   header allows `clippy::redundant_clone`.
//! - `ui::`/`tsx::`/`zant::Str` paths rely on the file preamble
//!   `use zant::ui; use zant::tsx;`.
//! - Locals are declared with explicit types; unused ones get `let _ = x;`.
//!
//! # Lowering choices
//!
//! - Identifiers that collide with Rust keywords become `keyword_` instead of
//!   raw identifiers, which keeps the usage scanner and golden output simple.
//! - `switch` is a labeled `if`/`else` chain: case tests are arbitrary
//!   expressions with side effects, so a Rust `match` cannot represent their
//!   evaluation order. Fallthrough is a diagnostic.
//! - A C-style `for` becomes `loop { update; if !(cond) { break; } body }`
//!   (update at the top after the first iteration) so `continue` still runs
//!   the update; `for…of` uses `for x in items.iter().cloned()`.
//! - State fields are plain Rust fields (no wrapper type), and
//!   `scope::rename_modules` is a no-op because Rust module paths live in
//!   another namespace.
//! - `interface` structs derive `Clone`/`PartialEq`/`Default`; structs with
//!   `Rc<dyn Fn>` callback fields derive `Clone` only and get a hand-written
//!   `Default` (no-op closure), since `Rc<dyn Fn>` is neither `Default` nor
//!   `PartialEq`.
//! - String literals remain `"...".into()`; where a generic `PartialEq`
//!   context would make that ambiguous (string comparison, `switch` case
//!   tests) the literal is spelled `zant::Str::Static("...")`.
//! - Enum names, numeric casts and `??` become `.as_str()`, typed literals
//!   and `Option` combinators (`unwrap`, `unwrap_or_else`, `map`).
//!
//! # Extension points
//!
//! The modules of the code generator depend on each other cyclically; the
//! borrow rules are simpler when the cross-module calls are function pointers
//! on [`Generator::extensions`]. [`generate`] installs them once before
//! generation:
//!
//! | Module | Hook field | Shape |
//! |---|---|---|
//! | `jsx` | `hooks.emit_jsx` | `fn(&mut Generator, &mut Context, &ast::Jsx) -> Result<Value>` |
//! | `captures` | `hooks.captures_read` | `fn(&mut Generator, &mut Context, &str) -> Result<Option<Value>>` |
//! | `components` (`read_prop`) | `hooks.components_read_prop` | `fn(&mut Generator, &str) -> Result<Option<Value>>` |
//! | `components` (`callback_type`) | `hooks.callback_type` | `fn(&mut Generator, Loc, &ast::FuncType) -> Result<Type>` |
//! | `hooks` | `hooks.ref_read` / `ref_write` / `bare_ref` / `hook_statement` / `hook_declaration` / `cleanup_return` | see [`Hooks`] |
//! | `events` | `hooks.method_param` | `fn(&mut Generator, &Method, usize, &ast::Param) -> Result<Type>` |
//! | `events` (`scan_body` / `emit_setter` / `forward_arrow` / `parameter_type`) | `hooks.scan_body` / `emit_setter` / `forward_arrow` / `parameter_type` | see [`Hooks`] |
//! | `captures` (`nested`) | `hooks.nested` | `fn(..) -> Result<NestedCall>` |
//! | `array_methods` | `extensions.array_methods` | `fn(&mut Generator, &mut Context, Loc, &ast::Call, Option<Type>) -> Result<Value>` |
//! | `components` / `stores` / `windows` (`try_call`) | `extensions.call_before_functions: Vec<TryCall>` | tried in registration order |
//! | `zant_api` (`try_call`) | `extensions.call_before_builtins: Vec<TryCall>` | tried in registration order |
//! | `imports` / `rust_imports` | `hooks.imported_type` / `hooks.rust_type` / `hooks.external_name` | see [`Hooks`] |
//! | `app_hooks` / `jsx` (`image_type`) | `hooks.key_event_type` / `hooks.image_type` | `fn(&mut Generator) -> Result<Type>` |
//! | hook predicates | `extensions.extra_hook` / `is_app_hook` / `is_desktop_hook` / `component_shadow` | `fn(..) -> bool` |
//!
//! `jsx::emit` handles `ui::*` tags, `<List>` and stateful widgets only.
//! Component tags are dispatched by `components`, so the installed `emit_jsx`
//! is a wrapper that checks components first and then calls `jsx::emit`; the
//! scan pass likewise dispatches component tags before calling
//! `events::scan_jsx`.

pub mod action_cx;
pub mod aggregate;
pub mod app_hooks;
pub mod app_state;
pub mod array_methods;
pub mod array_mutations;
pub mod builtins;
pub mod callbacks;
pub mod captures;
pub mod components;
pub mod desktop_hooks;
pub mod events;
pub mod expr;
pub mod free_names;
pub mod functions;
pub mod generate;
pub mod generic_functions;
pub mod generic_types;
pub(crate) mod generics;
pub mod globals;
pub mod hooks;
pub mod i18n_calls;
pub mod imports;
pub mod jsx;
pub mod jsx_children;
pub mod jsx_values;
pub mod list;
pub mod loops;
pub mod model;
pub mod narrowing;
pub mod nesting;
pub mod place;
pub mod rich_text;
pub mod rust_imports;
pub mod rust_text;
pub mod scope;
pub mod spread;
pub mod stateful;
pub mod stmt;
pub mod stores;
pub mod string_methods;
pub mod types;
pub mod windows;
pub mod zant_api;

use crate::ast;

pub type Diag = crate::error::Diag;

/// A code generation failure. The message lives in [`Diag`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("tsx code generation error")]
    Tsx,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Record the first diagnostic of this compilation and fail.
pub fn fail<T>(g: &mut Generator<'_>, loc: ast::Loc, msg: impl Into<String>) -> Result<T> {
    if g.diag.is_none() {
        *g.diag = Some(Diag {
            loc,
            msg: msg.into(),
        });
    }
    Err(Error::Tsx)
}

/// Add schema context to the first diagnostic without moving its caret.
pub fn contextualize(g: &mut Generator<'_>, prefix: &str) -> Result<()> {
    let Some(diagnostic) = g.diag.as_ref() else {
        return Err(Error::Tsx);
    };
    *g.diag = Some(Diag {
        loc: diagnostic.loc,
        msg: format!("{prefix}: {}", diagnostic.msg),
    });
    Ok(())
}

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

/// `components::try_call`, `imports::try_call`, `stores::try_call`,
/// `windows::try_call` (the first dispatch chain of `expr::call_value`).
pub type TryCall = fn(
    &mut Generator<'_>,
    &mut scope::Context,
    ast::Loc,
    &ast::Call,
) -> Result<Option<types::Value>>;

/// `jsx::emit`.
pub type EmitJsx = fn(&mut Generator<'_>, &mut scope::Context, &ast::Jsx) -> Result<types::Value>;

/// `captures::read` / `zant_api::read` / `components::read_prop`.
pub type ReadName =
    fn(&mut Generator<'_>, &mut scope::Context, &str) -> Result<Option<types::Value>>;
pub type ReadNameAt =
    fn(&mut Generator<'_>, &mut scope::Context, &str, ast::Loc) -> Result<Option<types::Value>>;
pub type ReadProp = fn(&mut Generator<'_>, &str) -> Result<Option<types::Value>>;

/// `hooks::ref_read` / `hooks::ref_write`.
pub type ReadRef =
    fn(&mut Generator<'_>, &mut scope::Context, &ast::Member) -> Result<Option<types::Value>>;
pub type WriteRef =
    fn(&mut Generator<'_>, &mut scope::Context, &ast::Expr) -> Result<Option<String>>;

/// `hooks::bare_ref`.
pub type BareRef = fn(&mut Generator<'_>, usize, ast::Loc) -> Result<types::Value>;

/// A hook call in statement position (`app_hooks`, `desktop_hooks`,
/// `hooks::effect`), or a hook declaration (`useMemo`, `useStore`, desktop
/// hooks).
pub type HookStatement =
    fn(&mut Generator<'_>, &mut scope::Context, &ast::Expr, bool) -> Result<String>;
pub type HookDeclaration =
    fn(&mut Generator<'_>, &mut scope::Context, ast::Loc, &ast::Decl, bool) -> Result<String>;

/// `hooks::cleanup_return` (a `return () => …` inside an effect).
pub type CleanupReturn = fn(&mut Generator<'_>, &mut scope::Context, &ast::Expr) -> Result<String>;

/// `events::method_param_type`.
pub type MethodParam = fn(&mut Generator<'_>, &Method, usize, &ast::Param) -> Result<types::Type>;

/// `captures::nested`.
pub type Nested =
    fn(&mut Generator<'_>, &mut scope::Context, ast::Loc, usize) -> Result<NestedCall>;

/// `app_hooks::key_event_type` / `jsx::image_type`.
pub type TypeHook = fn(&mut Generator<'_>) -> Result<types::Type>;

/// `events::scan_body` (register the handlers of a body before emitting it).
pub type ScanBody = fn(&mut Generator<'_>, &ast::Body) -> Result<()>;

/// `events::scan_expr` (register the handlers of an expression before it is
/// emitted).
pub type ScanExpr = fn(&mut Generator<'_>, &ast::Expr) -> Result<()>;

/// `events::emit_setter` (the adapter method reused by every setter action).
pub type EmitSetter = fn(&mut Generator<'_>, &Method) -> Result<String>;

/// `events::forward_arrow` (`fn` passed as a handler, wrapped as an arrow).
pub type ForwardArrow = fn(&mut Generator<'_>, &ast::Expr, usize) -> Result<Box<ast::Expr>>;

/// `events::parameter_type` (an event / callback parameter's type).
pub type ParameterType =
    fn(&mut Generator<'_>, &ast::Param, Option<tags_kind::Kind>) -> Result<types::Type>;

/// `components::callback_type` (the default [`model`] implementation returns
/// `Rc<dyn Fn(..)>`; `components` maps UI event signatures to `ui::Action`).
pub type CallbackType = fn(&mut Generator<'_>, ast::Loc, &ast::FuncType) -> Result<types::Type>;

/// `imports::named` / `rust_imports::named` / `imports::external_name`.
pub type NamedType = fn(&mut Generator<'_>, &str) -> Result<Option<types::Type>>;
pub type ExternalName = fn(&mut Generator<'_>, &str) -> Result<Option<String>>;

/// `array_methods::emit`.
pub type ArrayMethod = fn(
    &mut Generator<'_>,
    &mut scope::Context,
    ast::Loc,
    &ast::Call,
    Option<types::Type>,
) -> Result<types::Value>;

/// `imports::process`: resolves relative imports.
pub type ProcessImports = fn(&mut Generator<'_>, &ast::Module) -> Result<()>;

/// `windows::collect`: the `createWindow` declarations of the file.
pub type CollectWindows = fn(&mut Generator<'_>, &ast::Module) -> Result<String>;

/// `app_hooks::lifecycle`: the `impl ui::App` lifecycle methods.
pub type AppLifecycle = fn(&mut Generator<'_>) -> Result<String>;

/// `windows::is_create`: a constant built by `createWindow`.
pub type IsWindowConst = fn(&ast::Expr) -> bool;

/// Hook points filled in by the other code generation modules. Everything
/// here has an inert default, so the core is testable on its own.
#[derive(Clone)]
pub struct Extensions {
    /// `hooks::extra_hook` (custom hooks from `hooks/`).
    pub extra_hook: fn(&str) -> bool,
    /// `app_hooks::is_app_hook`.
    pub is_app_hook: fn(&str) -> bool,
    /// `desktop_hooks::is_hook`.
    pub is_desktop_hook: fn(&str) -> bool,
    /// Whether a child component's prop shadows a global (`components`).
    pub component_shadow: fn(&Generator<'_>, &str) -> bool,
    /// `imports::process`.
    pub process_imports: Option<ProcessImports>,
    /// `windows::collect`.
    pub collect_windows: Option<CollectWindows>,
    /// `app_hooks::lifecycle`.
    pub app_lifecycle: Option<AppLifecycle>,
    /// `windows::is_create`.
    pub is_window_const: Option<IsWindowConst>,
    pub call_before_functions: Vec<TryCall>,
    pub call_before_builtins: Vec<TryCall>,
    pub array_methods: Option<ArrayMethod>,
    pub hooks: Hooks,
}

impl Default for Extensions {
    fn default() -> Self {
        Extensions {
            extra_hook: |_| false,
            is_app_hook: |_| false,
            is_desktop_hook: |_| false,
            component_shadow: |_, _| false,
            process_imports: None,
            collect_windows: None,
            app_lifecycle: None,
            is_window_const: None,
            call_before_functions: Vec::new(),
            call_before_builtins: Vec::new(),
            array_methods: None,
            hooks: Hooks::default(),
        }
    }
}

/// Optional hooks; `None` until the owning module installs it.
#[derive(Clone, Default)]
pub struct Hooks {
    pub emit_jsx: Option<EmitJsx>,
    pub captures_read: Option<ReadName>,
    pub components_read_prop: Option<ReadProp>,
    pub zant_api_read: Option<ReadNameAt>,
    pub ref_read: Option<ReadRef>,
    pub ref_write: Option<WriteRef>,
    pub bare_ref: Option<BareRef>,
    pub hook_statement: Option<HookStatement>,
    pub hook_declaration: Option<HookDeclaration>,
    pub cleanup_return: Option<CleanupReturn>,
    pub method_param: Option<MethodParam>,
    /// Handler scanning of a render body / event arrow (`events`).
    pub scan_body: Option<ScanBody>,
    /// Handler scanning of a single expression (`events`).
    pub scan_expr: Option<ScanExpr>,
    /// Setter adapter methods (`events::emit_setter`).
    pub emit_setter: Option<EmitSetter>,
    /// `fn` handlers wrapped as arrows (`events::forward_arrow`).
    pub forward_arrow: Option<ForwardArrow>,
    /// An event / callback parameter's type (`events::parameter_type`).
    pub parameter_type: Option<ParameterType>,
    pub nested: Option<Nested>,
    pub key_event_type: Option<TypeHook>,
    pub image_type: Option<TypeHook>,
    pub callback_type: Option<CallbackType>,
    pub imported_type: Option<NamedType>,
    pub rust_type: Option<NamedType>,
    pub external_name: Option<ExternalName>,
}

/// State shared by the whole module: type declarations, constants and import
/// aliases, plus per-component state.
pub struct Generator<'a> {
    pub diag: &'a mut Option<Diag>,
    pub name: String,
    pub states: Vec<State>,
    pub methods: Vec<Method>,
    pub render_names: Vec<String>,
    pub hook_names: Vec<String>,
    pub type_aliases: Vec<TypeAlias>,
    pub module_names: [String; 4],
    pub counter: usize,
    pub managed_strings: bool,
    pub compounds: Vec<model::Info>,
    pub type_decls: Vec<ast::TypeDecl>,
    /// React bindings used for qualified element type annotations.
    pub react_imports: Vec<ast::Import>,
    pub resolving: Vec<bool>,
    pub resolved_types: Vec<ResolvedType>,
    /// Type parameters bound while a generic declaration is instantiated
    /// (`generic_types`): innermost last.
    pub type_env: Vec<(Vec<u8>, types::Type)>,
    /// Instantiated generic types (`Box<number>`), by declaration and arguments.
    pub generic_instances: Vec<generic_types::Instance>,
    /// Generic function templates of this file (`generic_functions`).
    pub generic_functions: Vec<generic_functions::Template>,
    /// How many of `functions` were emitted (`functions::emit_all`).
    pub emitted_functions: usize,
    pub constants: Vec<Constant>,
    /// Plain functions of this file and imported ones (`functions`).
    pub functions: Vec<functions::Function>,
    /// Handler expressions registered before emission (`events`).
    pub events: Vec<events::Event>,
    /// `<List>` elements of the component being generated (`list`).
    pub lists: Vec<list::ListInfo>,
    /// Key-less instance ids handed out to stateful child tags (`stateful`).
    pub child_sites: usize,
    /// How many scrolling containers enclose the element being generated.
    pub scroll_depth: usize,
    /// How many loops enclose the element being generated (keys, callbacks).
    pub loop_depth: usize,
    /// Extra lines of the file header.
    pub header: String,
    /// Child components of the module, and the one being generated
    /// (`components`; `None` while generating the root component).
    pub components: Vec<components::Component>,
    pub current_component: Option<usize>,
    /// Whether emission of the current component has read its props.
    pub component_props_read: bool,
    /// Inline arrows passed to a child component's callback props.
    pub callback_events: Vec<components::CallbackEvent>,
    /// `useMemo` caches and `useEffect` dependency lists of the current
    /// component (fields next to its state), and its effects.
    pub hook_fields: Vec<hooks::Field>,
    pub effects: Vec<hooks::Effect>,
    pub cleanups: Vec<hooks::Cleanup>,
    /// Stores declared in or imported into this file (`stores`).
    pub stores: Vec<stores::Store>,
    /// The enclosing render's typed bindings while a captured method is
    /// emitted (`captures`).
    pub capture_scope: Option<Vec<ScopeEntry>>,
    /// Generating a child component: its state lives outside `self`.
    pub child_component: bool,
    /// Whether imported type declarations are being resolved.
    pub type_scope: bool,
    /// Names of stores, windows and child components: they shadow locals
    /// like the module-level declarations of `scope::global_name`.
    pub extra_globals: Vec<String>,
    /// A local `Weak<RefCell<Component>>` that generated actions must capture
    /// instead of `self._state`: `<List>` row closures cannot borrow `self`
    /// (the closures outlive the `&mut self` build). Set by `list.rs`.
    pub handler_state: Option<String>,
    pub extensions: Extensions,
}

impl<'a> Generator<'a> {
    pub fn new(diag: &'a mut Option<Diag>) -> Self {
        *diag = None;
        Generator {
            diag,
            name: String::new(),
            states: Vec::new(),
            methods: Vec::new(),
            render_names: Vec::new(),
            hook_names: Vec::new(),
            type_aliases: Vec::new(),
            module_names: [
                "std".to_owned(),
                "zant".to_owned(),
                "ui".to_owned(),
                "tsx".to_owned(),
            ],
            counter: 0,
            managed_strings: false,
            compounds: Vec::new(),
            type_decls: Vec::new(),
            react_imports: Vec::new(),
            resolving: Vec::new(),
            resolved_types: Vec::new(),
            type_env: Vec::new(),
            generic_instances: Vec::new(),
            generic_functions: Vec::new(),
            emitted_functions: 0,
            constants: Vec::new(),
            functions: Vec::new(),
            events: Vec::new(),
            lists: Vec::new(),
            child_sites: 0,
            scroll_depth: 0,
            loop_depth: 0,
            header: String::new(),
            components: Vec::new(),
            current_component: None,
            component_props_read: false,
            callback_events: Vec::new(),
            hook_fields: Vec::new(),
            effects: Vec::new(),
            cleanups: Vec::new(),
            stores: Vec::new(),
            capture_scope: None,
            child_component: false,
            type_scope: false,
            extra_globals: Vec::new(),
            handler_state: None,
            extensions: Extensions::default(),
        }
    }
}

/// `useState`, a custom hook, a store hook or an app / desktop hook.
pub fn hook_name(g: &Generator<'_>, n: &str) -> bool {
    if n == "useState" || (g.extensions.extra_hook)(n) || n == "useStore" {
        return true;
    }
    if (g.extensions.is_app_hook)(n) || (g.extensions.is_desktop_hook)(n) {
        return true;
    }
    g.hook_names.iter().any(|name| name == n)
}

/// Whether this expression is a hook call.
pub fn hook(g: &Generator<'_>, e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Call(call) => match &call.callee.data {
            ast::ExprData::Ident(n) => hook_name(g, &String::from_utf8_lossy(n)),
            _ => false,
        },
        _ => false,
    }
}

/// Validate generated Rust with `syn::parse_file` and format it with
/// `prettyplease`. A parse failure panics with the generated source: it is a
/// compiler bug, never a user error. [`generate`] calls this once on the
/// complete module.
///
/// The caller supplies the whole file, including the `// @generated` marker
/// and the `#![allow(..)]` header; only the body is generated here.
pub fn parse_and_format(source: &str) -> String {
    match syn::parse_file(source) {
        Ok(file) => prettyplease::unparse(&file),
        Err(error) => panic!(
            "zant-tsx: generated Rust does not parse: {error}\n--- generated source ---\n{source}"
        ),
    }
}
