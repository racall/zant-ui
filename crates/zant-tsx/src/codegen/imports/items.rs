//! Importing a project file's exported constants, functions, components and stores.

use super::*;

/// A plain exported constant (not a store or a window).
pub(super) fn exported_constant(module: &ast::Module, name: &str) -> Option<ast::TopConst> {
    module
        .consts
        .iter()
        .find(|constant| {
            constant.exported
                && String::from_utf8_lossy(&constant.name.name) == name
                && !windows::is_create(&constant.value)
                && stores::exported(module, name).is_none()
        })
        .cloned()
}

/// `import { promptMs } from "./AppStore"`: `crate::app_store::promptMs`,
/// typed by its annotation (resolved in the declaring file).
pub(super) fn import_constant(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    constant: &ast::TopConst,
    scope: usize,
    loc: ast::Loc,
) -> Result<()> {
    let ty = match constant.ty.as_ref() {
        Some(reference) => in_scope(g, scope, |g| types::type_ref(g, reference, false))?,
        // an unannotated literal constant (`export const rowPrefix = "#"`)
        // takes the literal's type
        None => match &constant.value.data {
            ast::ExprData::String(_) => Type::String,
            ast::ExprData::Number(_) => Type::F64,
            ast::ExprData::Boolean(_) => Type::Boolean,
            _ => {
                return fail(
                    g,
                    loc,
                    format!("give `{imported}` a type annotation to import it"),
                );
            }
        },
    };
    // module constants are functions over a thread_local value (globals.rs)
    let code = format!(
        "{}::{}()",
        scope_alias(scope),
        rust_text::id_at(g, imported.as_bytes(), loc)?
    );
    g.constants.push(crate::codegen::Constant {
        name: local.to_owned(),
        value: types::Value {
            ty,
            code,
            ..types::Value::default()
        },
    });
    Ok(())
}

pub(super) fn exported_type(module: &ast::Module, name: &str) -> bool {
    module
        .types
        .iter()
        .any(|decl| decl.exported && String::from_utf8_lossy(&decl.name.name) == name)
}

pub(super) fn exported_function<'a>(
    module: &'a ast::Module,
    name: &str,
) -> Option<&'a ast::Function> {
    module
        .functions
        .iter()
        .find(|top| top.exported && String::from_utf8_lossy(&top.func.name) == name)
        .map(|top| &*top.func)
}

/// An imported plain function: its signature resolves in the declaring file.
pub(super) fn import_function(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    function: &ast::Function,
    scope: usize,
    module: &str,
) -> Result<()> {
    if !function.type_params.is_empty() {
        return fail(
            g,
            function.loc,
            format!(
                "`{imported}` is generic: generic functions are called in the file that declares them for now; move the call there or give `{imported}` concrete types"
            ),
        );
    }
    let rust = format!(
        "{}::{}",
        scope_alias(scope),
        rust_text::id_at(g, imported.as_bytes(), function.loc)?
    );
    let mut signature = in_scope(g, scope, |g| functions::signature(g, function, rust, true))?;
    signature.name = local.to_owned();
    signature.throws = g.throwing.throws(module, imported.as_bytes());
    g.functions.push(signature);
    Ok(())
}

/// An imported component: its props type resolves in the declaring file.
pub(super) fn import_component(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    function: &ast::Function,
    scope: usize,
    keeps_state: bool,
) -> Result<()> {
    let builder = format!(
        "{}::{}::build",
        scope_alias(scope),
        rust_text::id_at(g, imported.as_bytes(), function.loc)?
    );
    let mut component = components::Component {
        name: local.to_owned(),
        builder,
        func: Box::new(function.clone()),
        props: None,
        param: String::new(),
        destructure: Vec::new(),
        destructure_defaults: Vec::new(),
        external: true,
        stateful: keeps_state,
    };
    if let Some(parameter) = function.params.first() {
        let Some(reference) = parameter.ty.as_ref() else {
            return fail(
                g,
                parameter.name.loc,
                format!("`{imported}` has untyped props"),
            );
        };
        component.props = Some(in_scope(g, scope, |g| {
            types::type_ref(g, reference, false)
        })?);
    }
    if let Some(parameter) = function.params.first() {
        component.param = String::from_utf8_lossy(&parameter.name.name).into_owned();
        component.destructure = parameter.destructure.clone();
        component.destructure_defaults = parameter.destructure_defaults.clone();
    }
    g.components.push(component);
    Ok(())
}

/// `import { prefs } from "./store"`: the accessors live in the declaring
/// module (`crate::store::prefs_read()`).
pub(super) fn import_store(
    g: &mut Generator<'_>,
    local: &str,
    imported: &str,
    constant: &ast::TopConst,
    scope: usize,
    loc: ast::Loc,
) -> Result<()> {
    let ast::ExprData::Call(call) = &constant.value.data else {
        unreachable!("createStore call")
    };
    if call.type_args.len() != 1 {
        return fail(g, loc, format!("`{imported}` needs a type argument"));
    }
    let ty = in_scope(g, scope, |g| types::type_ref(g, &call.type_args[0], false))?;
    stores::imported(g, local, imported, &scope_alias(scope), ty)
}
