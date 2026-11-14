//! Rust code generation core (WP-5.3), a translation of
//! `tools/tsx/codegen/codegen.zig`'s shared state: the [`Generator`], its
//! diagnostic helper and the extension points that the JSX, component/state
//! and application WPs fill in.
//!
//! The entry point `generate` (module validation, component collection and
//! emission) belongs to WP-5.5, which adds its module next to this one and
//! calls [`Generator::new`]. The code generator emits Rust with these
//! conventions:
//!
//! - `string` is `zant::Str`, `T[]` is `tsx::List<T>`, `T | null` is
//!   `Option<T>`, `ui::Node` is `ui::Node` (`docs/rust-port/tsx-codegen.md` §2).
//! - Every read of a non-`Copy` value appends `.clone()` (§3); the generated
//!   file header allows `clippy::redundant_clone`.
//! - `ui::`/`tsx::`/`zant::Str` paths need a preamble owned by WP-5.6:
//!   `use zant::ui; use zant::tsx;` (`docs/rust-port/tsx-codegen.md` §1).
//! - Locals are declared with explicit types; unused ones get `let _ = x;`
//!   in place of Zig's `_ = x;`.
//!
//! # Deviations from the Zig codegen (reported, not silent)
//!
//! - `@"keyword"` identifiers become `keyword_` instead of Rust raw
//!   identifiers, which keeps the usage scanner and golden output simple.
//! - `switch` stays a labeled `if`/`else` chain (the Zig lowering): case tests
//!   are arbitrary expressions with side effects, so a Rust `match` cannot
//!   represent their evaluation order. Fallthrough is still a diagnostic.
//! - A C-style `for` becomes `loop { update; if !(cond) { break; } body }`
//!   (update at the top after the first iteration) so `continue` still runs
//!   the update; `for…of` uses `for x in items.iter().cloned()`.
//! - Zig's `_ = x;` discards become `let _ = x;`; state fields are plain Rust
//!   fields (no `tsx.State` wrapper), and `scope.rename_modules` is a no-op
//!   because Rust module paths live in another namespace.
//! - `interface` structs derive `Clone`/`PartialEq`/`Default`; structs with
//!   `Rc<dyn Fn>` callback fields derive `Clone` only and get a hand-written
//!   `Default` (no-op closure), since `Rc<dyn Fn>` is neither `Default` nor
//!   `PartialEq`.
//! - String literals remain `"...".into()`; where a generic `PartialEq`
//!   context would make that ambiguous (string comparison, `switch` case
//!   tests) the literal is spelled `zant::Str::Static("...")`.
//! - Zig's `@tagName`/`@as`/`orelse` become `.as_str()`, typed literals and
//!   `Option` combinators (`unwrap`, `unwrap_or_else`, `map`).
//!
//! # Extension points for WP-5.4 – WP-5.6
//!
//! Zig's modules call each other directly; Rust's borrow rules are simpler if
//! the not-yet-ported pieces are function pointers on
//! [`Generator::extensions`]. The owning WP installs them once before
//! generation:
//!
//! | Zig module | Hook field | Shape |
//! |---|---|---|
//! | `jsx.zig` | `hooks.emit_jsx` | `fn(&mut Generator, &mut Context, &ast::Jsx) -> Result<Value>` |
//! | `captures.zig` | `hooks.captures_read` | `fn(&mut Generator, &mut Context, &str) -> Result<Option<Value>>` |
//! | `components.zig` (`readProp`) | `hooks.components_read_prop` | `fn(&mut Generator, &str) -> Result<Option<Value>>` |
//! | `components.zig` (`callbackType`) | `hooks.callback_type` | `fn(&mut Generator, Loc, &ast::FuncType) -> Result<Type>` |
//! | `hooks.zig` | `hooks.ref_read` / `ref_write` / `bare_ref` / `hook_statement` / `hook_declaration` / `cleanup_return` | see [`Hooks`] |
//! | `events.zig` | `hooks.method_param` | `fn(&mut Generator, &Method, usize, &ast::Param) -> Result<Type>` |
//! | `captures.zig` (`nested`) | `hooks.nested` | `fn(..) -> Result<NestedCall>` |
//! | `array_methods.zig` | `extensions.array_methods` | `fn(&mut Generator, &mut Context, Loc, &ast::Call, Option<Type>) -> Result<Value>` |
//! | `components/stores/windows.zig` (`tryCall`) | `extensions.call_before_functions: Vec<TryCall>` | tried in registration order |
//! | `zant_api/timers.zig` (`tryCall`) | `extensions.call_before_builtins: Vec<TryCall>` | tried in registration order |
//! | `imports.zig` / `zig_types.zig` | `hooks.imported_type` / `hooks.zig_type` / `hooks.external_name` | see [`Hooks`] |
//! | `app_hooks.zig` / `jsx.zig` (`imageType`) | `hooks.key_event_type` / `hooks.image_type` | `fn(&mut Generator) -> Result<Type>` |
//! | hook predicates | `extensions.extra_hook` / `is_app_hook` / `is_desktop_hook` / `component_shadow` | `fn(..) -> bool` |

pub mod aggregate;
pub mod builtins;
pub mod expr;
pub mod functions;
pub mod loops;
pub mod model;
pub mod narrowing;
pub mod rust_text;
pub mod scope;
pub mod stmt;
pub mod types;

use crate::ast;

pub type Diag = crate::error::Diag;

/// A code generation failure (Zig `error{Tsx, OutOfMemory}`; allocation
/// failures are not recoverable in Rust). The message lives in [`Diag`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("tsx code generation error")]
    Tsx,
}

pub type Result<T> = std::result::Result<T, Error>;

/// The first diagnostic of this compilation (Zig `core.fail`).
pub fn fail<T>(g: &mut Generator<'_>, loc: ast::Loc, msg: impl Into<String>) -> Result<T> {
    if g.diag.is_none() {
        *g.diag = Some(Diag {
            loc,
            msg: msg.into(),
        });
    }
    Err(Error::Tsx)
}

/// Add schema context to the first diagnostic without moving its caret
/// (Zig `core.contextualize`).
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

/// A top-level `const` that is not a component (Zig `Generator.constants`).
#[derive(Clone, Debug)]
pub struct Constant {
    pub name: String,
    pub value: types::Value,
}

/// `interface` / `type` alias already resolved (Zig `Generator.type_aliases`).
#[derive(Clone, Debug)]
pub struct TypeAlias {
    pub name: String,
    pub ty: types::Type,
}

/// A resolved structured type, keyed by the address of its AST detail
/// (Zig compares the `*const ast.TypeDetail` pointers).
#[derive(Clone, Copy, Debug)]
pub struct ResolvedType {
    pub detail: usize,
    pub ty: types::Type,
}

/// The address of an AST type detail for [`Generator::resolved_types`].
///
/// `TypeRef::detail` owns its `Box`, so the address is stable for the lifetime
/// of the parsed module.
pub fn detail_address(detail: &ast::TypeDetail) -> usize {
    std::ptr::from_ref(detail) as usize
}

/// `useState` or `useRef` (read and written through `.current`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StateKind {
    #[default]
    State,
    Ref,
}

/// A `useState` field of the component being generated (Zig `core.State`).
#[derive(Clone, Debug)]
pub struct State {
    pub name: String,
    /// Empty for a ref.
    pub setter: String,
    pub ty: types::Type,
    pub initial: String,
    pub kind: StateKind,
}

/// An arrow function registered as a method (Zig `events.Method`). WP-5.5
/// adds the capture and hook bookkeeping; WP-5.3 only reads `name`, `func`,
/// `ret`, `user` and `inline_only`.
#[derive(Clone, Debug, Default)]
pub struct Capture {
    pub name: String,
    pub field: String,
    pub ty: types::Type,
    pub source: String,
}

/// An arrow function registered as a method (Zig `events.Method`).
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

/// `tags::Kind` lives in [`crate::tags`]; re-exported under a short name so the
/// struct above reads like the Zig original.
pub mod tags_kind {
    pub use crate::tags::Kind;
}

/// The enclosing render context of an inline method (`captures.nested`).
#[derive(Clone, Debug, Default)]
pub struct NestedCall {
    pub receiver: String,
    pub target: String,
}

// ── extension points (WP-5.4 – WP-5.6) ────────────────────────────────

/// `components::try_call`, `imports::try_call`, `stores::try_call`,
/// `windows::try_call` (Zig `expr.callValue`'s first dispatch chain).
pub type TryCall = fn(
    &mut Generator<'_>,
    &mut scope::Context,
    ast::Loc,
    &ast::Call,
) -> Result<Option<types::Value>>;

/// `jsx::emit` (WP-5.4).
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

/// `components::callback_type` (the default [`model`] implementation returns
/// `Rc<dyn Fn(..)>`; WP-5.5 maps UI event signatures to `ui::Action`).
pub type CallbackType = fn(&mut Generator<'_>, ast::Loc, &ast::FuncType) -> Result<types::Type>;

/// `imports::named` / `zig_types::named` / `imports::external_name`.
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

/// Hook points owned by later work packages. Everything here has a
/// default that reports which WP implements it, so WP-5.3's core compiles and
/// is testable on its own.
#[derive(Clone)]
pub struct Extensions {
    /// `hooks::extra_hook` (custom hooks from `hooks/`).
    pub extra_hook: fn(&str) -> bool,
    /// `app_hooks::is_app_hook`.
    pub is_app_hook: fn(&str) -> bool,
    /// `desktop_hooks::is_hook`.
    pub is_desktop_hook: fn(&str) -> bool,
    /// Whether a child component's prop shadows a global (`components.zig`).
    pub component_shadow: fn(&Generator<'_>, &str) -> bool,
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
            call_before_functions: Vec::new(),
            call_before_builtins: Vec::new(),
            array_methods: None,
            hooks: Hooks::default(),
        }
    }
}

/// Optional hooks; `None` means the owning work package has not landed yet.
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
    pub nested: Option<Nested>,
    pub key_event_type: Option<TypeHook>,
    pub image_type: Option<TypeHook>,
    pub callback_type: Option<CallbackType>,
    pub imported_type: Option<NamedType>,
    pub zig_type: Option<NamedType>,
    pub external_name: Option<ExternalName>,
}

/// State shared by the whole module: type declarations, constants and import
/// aliases, plus per-component state (Zig `core.Generator`).
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
    pub constants: Vec<Constant>,
    /// Plain functions of this file and imported ones (`functions.zig`).
    pub functions: Vec<functions::Function>,
    /// `const _X = @import("X.zig");` lines of the file header (WP-5.6).
    pub header: String,
    /// Generating a child component: its state lives outside `self` (WP-5.5).
    pub child_component: bool,
    /// Whether imported type declarations are being resolved (WP-5.6).
    pub type_scope: bool,
    /// Names of stores, windows and child components (WP-5.5/5.6): they shadow
    /// locals like the module-level declarations of `scope.globalName`.
    pub extra_globals: Vec<String>,
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
            constants: Vec::new(),
            functions: Vec::new(),
            header: String::new(),
            child_component: false,
            type_scope: false,
            extra_globals: Vec::new(),
            extensions: Extensions::default(),
        }
    }
}

/// `useState`, a custom hook, a store hook or an app / desktop hook
/// (Zig `core.hookName`).
pub fn hook_name(g: &Generator<'_>, n: &str) -> bool {
    if n == "useState" || (g.extensions.extra_hook)(n) || n == "useStore" {
        return true;
    }
    if (g.extensions.is_app_hook)(n) || (g.extensions.is_desktop_hook)(n) {
        return true;
    }
    g.hook_names.iter().any(|name| name == n)
}

/// Whether this expression is a hook call (Zig `core.hook`).
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
/// `prettyplease` (`docs/rust-port/tsx-codegen.md` §1). A parse failure panics
/// with the generated source, exactly like the Zig compiler's `std.zig.Ast`
/// re-parse. The WP-5.5 entry point calls this once on the complete module.
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
