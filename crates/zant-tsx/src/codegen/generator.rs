//! The generator state shared by every code generation module.

use super::*;

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
    /// `enum` declarations of this file (`enums`).
    pub enums: Vec<ast::EnumDecl>,
    /// Top-level constants of this file, for `typeof name` in types.
    pub const_decls: Vec<ast::TopConst>,
    /// Generic function templates of this file (`generic_functions`).
    pub generic_functions: Vec<generic_functions::Template>,
    /// How many of `functions` were emitted (`functions::emit_all`).
    pub emitted_functions: usize,
    /// The project's throwing functions (`throws`) and this file's stem,
    /// the key it is analyzed under.
    pub throwing: throws::Throwing,
    pub module_stem: String,
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
            enums: Vec::new(),
            const_decls: Vec::new(),
            generic_functions: Vec::new(),
            emitted_functions: 0,
            throwing: throws::Throwing::default(),
            module_stem: String::new(),
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
