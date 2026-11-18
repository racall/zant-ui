//! Hook points between the code generation modules: the function types and the `Extensions` / `Hooks` tables that `generate::install` fills in.

use super::*;

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
