//! Shared state across components and files (Zustand style, `zant.store` in
//! `docs/tsx.md`).
//!
//! ```tsx
//! export const settings = createStore<Settings>({ dark: false, size: 14 });
//! const dark = useStore(settings, (s) => s.dark);   // in a component
//! settings.update({ dark: true });                  // in a handler
//! ```
//!
//! Rust has no mutable module `static` that can be built at run time, so a
//! store becomes a `thread_local!` value plus four `pub fn` accessors
//! (`settings_get` / `settings_read` / `settings_set` / `settings_update`).
//! Generated code calls the accessors, which keeps a store usable from another
//! file through its module alias. `tsx::Store<T>` only needs
//! `zant::store::Store` (the runtime store) inside a `RefCell`, plus
//! `get` / `read` / `set` / `update(f)` accessor functions.
//!
//! Changing a store rebuilds every window (the `tsx::Store` runtime does it).

use crate::ast;

use super::callbacks;
use super::expr;
use super::model;
use super::rust_text;
use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

#[derive(Clone, Debug)]
pub struct Store {
    /// The TSX name in this file.
    pub name: String,
    /// The name the declaring file's accessors use (differs from [`name`] when
    /// the store was imported under an alias, `import { prefs as shared }`).
    pub declared: String,
    /// The Rust accessor prefix: empty in the declaring file, `_store::` when
    /// imported from another file.
    pub module: String,
    pub ty: Type,
}

impl Store {
    fn accessor(&self, operation: &str) -> String {
        format!("{}()", self.accessor_fn(operation))
    }

    /// The accessor path without the call parenthesis.
    fn accessor_fn(&self, operation: &str) -> String {
        format!("{}{}_{operation}", self.module, self.declared)
    }

    /// The `thread_local!` static of a store declared in this file.
    fn static_name(&self) -> String {
        let mut name = String::new();
        for (index, ch) in self.name.chars().enumerate() {
            if ch.is_ascii_uppercase() && index > 0 {
                name.push('_');
            }
            name.push(ch.to_ascii_uppercase());
        }
        name
    }
}

fn is_create(e: &ast::Expr) -> bool {
    matches!(
        &e.data,
        ast::ExprData::Call(call)
            if matches!(&call.callee.data, ast::ExprData::Ident(name) if name == b"createStore")
    )
}

/// The file-level constants that are not stores (for `globals.collect`).
pub fn plain_constants<'a>(
    g: &mut Generator<'_>,
    constants: &'a [ast::TopConst],
    is_window: fn(&ast::Expr) -> bool,
) -> Result<Vec<&'a ast::TopConst>> {
    let mut plain: Vec<&'a ast::TopConst> = Vec::new();
    for constant in constants {
        if is_create(&constant.value) || is_window(&constant.value) {
            continue;
        }
        plain.push(constant);
    }
    let _ = g;
    Ok(plain)
}

/// `createStore<T>(initial)` constants → `thread_local!` declarations plus
/// their accessors.
pub fn collect(g: &mut Generator<'_>, constants: &[ast::TopConst]) -> Result<String> {
    let mut code = String::new();
    for constant in constants {
        if !is_create(&constant.value) {
            continue;
        }
        let ast::ExprData::Call(call) = &constant.value.data else {
            unreachable!("createStore call")
        };
        if call.type_args.len() != 1 || call.args.len() != 1 {
            return fail(
                g,
                constant.value.loc,
                "createStore takes the state type and its initial value: createStore<Settings>({ … })",
            );
        }
        let name = String::from_utf8_lossy(&constant.name.name).into_owned();
        if scope::global_name(g, constant.name.name.as_slice()) {
            return fail(
                g,
                constant.name.loc,
                format!("duplicate or reserved name `{name}`"),
            );
        }
        let ty = types::type_ref(g, &call.type_args[0], false)?;
        let mut c = scope::Context {
            render: true,
            ret: Type::Void,
            constant: true,
            ..scope::Context::default()
        };
        let mut initial = expr::emit(g, &mut c, &call.args[0], Some(ty), false)?;
        initial = types::coerce(g, initial, ty, call.args[0].loc)?;
        let store = Store {
            name: name.clone(),
            declared: name.clone(),
            module: String::new(),
            ty,
        };
        let rust = ty.rust(g).to_owned();
        code.push_str(&format!(
            "thread_local! {{\n    static {static_name}: std::cell::RefCell<zant::store::Store<{rust}>> = std::cell::RefCell::new(zant::store::Store::new({value}));\n}}\n\n\
             pub fn {name}_get() -> {rust} {{ {static_name}.with(|store| store.borrow().get().clone()) }}\n\
             pub fn {name}_read() -> {rust} {{ {static_name}.with(|store| store.borrow().get().clone()) }}\n\
             pub fn {name}_set(value: {rust}) {{ {static_name}.with(|store| store.borrow_mut().set(value)); }}\n\
             pub fn {name}_update(update: impl FnOnce(&mut {rust})) {{ {static_name}.with(|store| store.borrow_mut().update(update)); }}\n\n",
            static_name = store.static_name(),
            value = initial.code,
        ));
        g.stores.push(store);
    }
    Ok(code)
}

/// The store a file exports as `name`, if any.
pub fn exported(module: &ast::Module, name: &str) -> Option<ast::TopConst> {
    module
        .consts
        .iter()
        .find(|constant| {
            constant.exported
                && is_create(&constant.value)
                && String::from_utf8_lossy(&constant.name.name) == name
        })
        .cloned()
}

/// `import { settings } from "./store"`: registers the imported store under
/// its local name. The type is resolved by the importing module (`imports`).
pub fn imported(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    alias: &str,
    ty: Type,
) -> Result<()> {
    let _ = imported;
    // `alias` is already a Rust module path (`crate::store`, from
    // `imports`): escaping it would turn the `::` into `__`.
    let _ = g;
    let module = format!("{alias}::");
    g.stores.push(Store {
        name: local.to_owned(),
        declared: imported.to_owned(),
        module,
        ty,
    });
    Ok(())
}

fn find(g: &Generator<'_>, c: &scope::Context, e: &ast::Expr) -> Option<Store> {
    let ast::ExprData::Ident(name) = &e.data else {
        return None;
    };
    if scope::local(c, name).is_some() {
        return None;
    }
    g.stores
        .iter()
        .find(|store| store.name.as_bytes() == name.as_slice())
        .cloned()
}

/// `const dark = useStore(settings, (s) => s.dark)` as a render local: the
/// store's value (or the selected part) at render time, like a state value.
pub fn use_store(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    d: &ast::Decl,
    top: bool,
) -> Result<String> {
    let Some(init) = d.init.as_deref() else {
        return fail(g, loc, "useStore needs an initializer");
    };
    let ast::ExprData::Call(call) = &init.data else {
        unreachable!("useStore call")
    };
    if !c.render || !top {
        return fail(
            g,
            loc,
            "hooks must be called at the top level of the component",
        );
    }
    let ast::Pattern::Ident(pattern) = &d.pattern else {
        return fail(g, loc, "const value = useStore(store)");
    };
    if call.args.is_empty() || call.args.len() > 2 {
        return fail(
            g,
            loc,
            "useStore takes a store and an optional selector: useStore(store, (s) => s.x)",
        );
    }
    let Some(store) = find(g, c, &call.args[0]) else {
        return fail(
            g,
            call.args[0].loc,
            "useStore needs a store made with createStore",
        );
    };
    let value: Value = if call.args.len() == 2 {
        let ast::ExprData::Arrow(selector) = &call.args[1].data else {
            return fail(g, call.args[1].loc, "useStore takes a selector arrow");
        };
        let pattern_name = match selector.params.first() {
            Some(param) => String::from_utf8_lossy(&param.name.name).into_owned(),
            None => "_store".to_owned(),
        };
        let pattern = format!("{pattern_name}: {}", store.ty.rust(g));
        let (closure, ty) = callbacks::closure(g, c, &call.args[1], &[store.ty], &pattern, None)?;
        Value {
            ty,
            code: format!("({closure})({})", store.accessor("read")),
            ..Value::default()
        }
    } else {
        Value {
            ty: store.ty,
            code: store.accessor("read"),
            ..Value::default()
        }
    };
    let binding = scope::bind(g, c, &pattern.name, value.ty, false)?;
    c.bindings[binding].init = Some(value.code);
    Ok(format!(
        "{}{}{}",
        scope::PLACEHOLDER_START,
        binding,
        scope::PLACEHOLDER_END
    ))
}

/// `settings.get()`, `settings.set(v)`, `settings.update({ … })`; null when
/// the callee is not a store method.
pub fn try_call(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    loc: ast::Loc,
    call: &ast::Call,
) -> Result<Option<Value>> {
    let ast::ExprData::Member(member) = &call.callee.data else {
        return Ok(None);
    };
    let Some(store) = find(g, c, &member.object) else {
        return Ok(None);
    };
    let method = String::from_utf8_lossy(&member.name.name).into_owned();
    if method == "get" {
        if !call.args.is_empty() {
            return fail(g, loc, "get() takes no arguments");
        }
        return Ok(Some(Value {
            ty: store.ty,
            code: store.accessor("get"),
            ..Value::default()
        }));
    }
    let changes = method == "set" || method == "update";
    if !changes {
        return fail(
            g,
            member.name.loc,
            "a store has get(), set(value) and update({ … })",
        );
    }
    if c.render {
        return fail(
            g,
            loc,
            "stores change in event handlers and effects, not while rendering",
        );
    }
    if call.args.len() != 1 {
        return fail(g, loc, format!("{method}() takes one argument"));
    }
    if method == "set" {
        let value = expr::emit(g, c, &call.args[0], Some(store.ty), false)?;
        return Ok(Some(Value {
            ty: Type::Void,
            code: format!("{}({})", store.accessor_fn("set"), value.code),
            ..Value::default()
        }));
    }
    let code = update(g, c, &store, &call.args[0])?;
    Ok(Some(Value {
        ty: Type::Void,
        code,
        ..Value::default()
    }))
}

/// `update({ dark: true })`: a shallow merge of the listed fields.
fn update(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    store: &Store,
    arg: &ast::Expr,
) -> Result<String> {
    let ast::ExprData::Object(properties) = &arg.data else {
        return fail(
            g,
            arg.loc,
            "update takes an object literal with the fields to change",
        );
    };
    let mut fields = String::new();
    for property in properties {
        if property.spread {
            return fail(
                g,
                property.value.loc,
                "list the fields to change (no spread)",
            );
        }
        let ty = model::field_type(g, store.ty, &property.key.name, property.key.loc)?;
        let value = expr::emit(g, c, &property.value, Some(ty), false)?;
        let field = model::field_rust(g, store.ty, &property.key.name)
            .unwrap_or_else(|| rust_text::id(&property.key.name).unwrap_or_default());
        fields.push_str(&format!("        state.{field} = {};\n", value.code));
    }
    Ok(format!(
        "{}(\n    |state| {{\n{fields}    }},\n)",
        store.accessor_fn("update")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse_module(src: &str) -> ast::Module {
        let mut parser = Parser::init(src);
        parser.parse_module().expect("test source parses")
    }

    #[test]
    fn create_store_constants_become_a_thread_local_and_accessors() {
        let module = parse_module(
            "export const settings = createStore<{ dark: boolean, size: u32 }>({ dark: false, size: 14 });\n",
        );
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        g.resolving = vec![false; module.types.len()];
        let code = collect(&mut g, &module.consts).expect("stores lower");
        assert!(code.contains("thread_local! {"), "{code}");
        assert!(
            code.contains("static SETTINGS: std::cell::RefCell<zant::store::Store<"),
            "{code}"
        );
        assert!(code.contains("pub fn settings_get() ->"), "{code}");
        assert!(
            code.contains("pub fn settings_update(update: impl FnOnce(&mut "),
            "{code}"
        );
        assert_eq!(1, g.stores.len());
        assert_eq!("settings", g.stores[0].name);
    }

    #[test]
    fn exported_finds_a_store_of_the_module() {
        let module = parse_module(
            "export const settings = createStore<{ dark: boolean }>({ dark: false });\n",
        );
        assert!(exported(&module, "settings").is_some());
        assert!(exported(&module, "other").is_none());
    }
}
