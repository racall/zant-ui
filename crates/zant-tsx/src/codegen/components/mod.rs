//! Child components (`docs/tsx.md`, "子组件").
//!
//! Each non-default top-level function becomes a struct with a `build` function:
//!
//! ```text
//! pub struct TodoRow { props: Props, _id: u64 }
//! impl TodoRow {
//!     pub fn build(props: Props, id: u64) -> ui::Node { … }
//!     fn render(&mut self) -> ui::Node { … }
//! }
//! ```
//!
//! Stateful children grow a `Rc<RefCell<…>>` local state slot (see
//! [`super::generate`]); callback props take `ui::Action`-family types when
//! their signature matches a ui event and `std::rc::Rc<dyn Fn(..)>` otherwise.

use crate::ast;
use crate::tags;

use super::captures;
use super::expr;
use super::functions;
use super::model;
use super::nesting;
use super::rust_text;
use super::scope;
use super::spread;
use super::stateful;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod call;
mod callback_values;
mod props;
mod scan;

pub use self::call::*;
use self::callback_values::*;
pub use self::props::*;
pub use self::scan::*;

#[derive(Clone)]
pub struct Component {
    /// TSX and Rust struct name (`TodoRow`).
    pub name: String,
    /// Builder path (`TodoRow::build`).
    pub builder: String,
    pub func: Box<ast::Function>,
    /// The props struct type (an empty struct when there is no parameter).
    pub props: Option<Type>,
    /// The parameter's name (`props`) when not destructured.
    pub param: String,
    /// Destructured prop names (`{ todo, on_toggle }`) and their defaults.
    pub destructure: Vec<ast::DestructuredProp>,
    pub destructure_defaults: Vec<Option<Box<ast::Expr>>>,
    /// Imported from another file: `builder` is already Rust code.
    pub external: bool,
    /// Keeps local state (see `stateful`): the builder takes an instance id.
    pub stateful: bool,
}

/// An inline arrow passed to a child component's callback prop.
#[derive(Clone, Debug)]
pub struct CallbackEvent {
    pub expr: ast::Expr,
    pub method: usize,
}

// ── types ─────────────────────────────────────────────────────────────

/// The action type of a callback signature, if it matches a ui event.
pub fn action_kind(params: &[Type]) -> Option<tags::Kind> {
    if params.is_empty() {
        return Some(tags::Kind::Action);
    }
    if params.len() != 1 {
        return None;
    }
    match params[0] {
        Type::Boolean => Some(tags::Kind::BoolAction),
        Type::String => Some(tags::Kind::TextAction),
        Type::Usize => Some(tags::Kind::IndexAction),
        _ => None,
    }
}

/// The Rust type of an action kind (`ui::TextAction`).
pub fn action_type(kind: tags::Kind) -> &'static str {
    match kind {
        tags::Kind::Action => "ui::Action",
        tags::Kind::BoolAction => "ui::BoolAction",
        tags::Kind::TextAction => "ui::TextAction",
        tags::Kind::IndexAction => "ui::IndexAction",
        tags::Kind::ShortcutAction => "ui::HotkeyAction",
        _ => "ui::Action",
    }
}

/// The `ui::*` builder of an action kind (`ui::text_action`).
pub fn action_builder(kind: tags::Kind) -> &'static str {
    match kind {
        tags::Kind::Action => "ui::action",
        tags::Kind::BoolAction => "ui::bool_action",
        tags::Kind::TextAction => "ui::text_action",
        tags::Kind::IndexAction => "ui::index_action",
        tags::Kind::ShortcutAction => "ui::hotkey_action",
        _ => "ui::action",
    }
}

/// `()` / `bool` / `&str` / `usize`; action callbacks return nothing.
pub fn action_value_type(kind: tags::Kind) -> Type {
    match kind {
        tags::Kind::Action => Type::Void,
        tags::Kind::TextAction => Type::String,
        tags::Kind::BoolAction => Type::Boolean,
        tags::Kind::IndexAction => Type::Usize,
        _ => Type::Void,
    }
}

/// `(id: number) => void` as a callback type (interned by signature): a ui
/// action when the signature matches an event, `Rc<dyn Fn(..)>` otherwise.
pub fn callback_type(g: &mut Generator<'_>, loc: ast::Loc, f: &ast::FuncType) -> Result<Type> {
    let ret = types::type_ref(g, &f.ret, true)?;
    if ret != Type::Void {
        return fail(g, loc, "callback props must return void");
    }
    let mut params: Vec<Type> = Vec::new();
    for param in &f.params {
        let Some(reference) = param.ty.as_ref() else {
            return fail(
                g,
                param.name.loc,
                format!(
                    "callback parameter `{}` needs a type",
                    String::from_utf8_lossy(&param.name.name)
                ),
            );
        };
        params.push(types::type_ref(g, reference, false)?);
    }
    for (index, entry) in g.compounds.iter().enumerate() {
        if entry.kind == model::InfoKind::Callback && entry.params == params {
            return Ok(Type::Compound(index as u32));
        }
    }
    let rust = match action_kind(&params) {
        Some(kind) if !matches!(kind, tags::Kind::ShortcutAction) => action_type(kind).to_owned(),
        // Not a ui event: the callback runs from a handler with the runtime
        // context, which is how it reaches the root state (a closure cannot
        // capture `&mut App`, so it takes the `ActionCx` the handler runs in).
        _ => format!(
            "std::rc::Rc<dyn Fn({})>",
            std::iter::once("&mut ui::ActionCx<'_>".to_owned())
                .chain(params.iter().map(|ty| ty.rust(g).to_owned()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    model::add(
        g,
        model::Info {
            kind: model::InfoKind::Callback,
            rust,
            params,
            ..model::Info::default()
        },
    )
}

/// The callback info of `ty` (or of its optional child).
pub fn callback_info(g: &Generator<'_>, ty: Type) -> Option<model::Info> {
    let base = model::optional_child(g, ty).unwrap_or(ty);
    let entry = model::info(g, base)?;
    if entry.kind == model::InfoKind::Callback {
        Some(entry)
    } else {
        None
    }
}

// ── collection ────────────────────────────────────────────────────────

/// Registers every non-default top-level function as a child component.
pub fn collect(
    g: &mut Generator<'_>,
    module: &ast::Module,
    root: Option<&ast::Function>,
    project: &[stateful::ModuleSource<'_>],
) -> Result<()> {
    for top in &module.functions {
        if let Some(root) = root
            && top.func.name == root.name
            && top.func.loc == root.loc
        {
            continue;
        }
        if functions::is_function(&top.func) {
            continue;
        }
        let f = &top.func;
        if let Some(rest) = f.params.iter().find(|param| param.rest) {
            return fail(
                g,
                rest.name.loc,
                "components take one props object, not rest parameters",
            );
        }
        if f.params.len() > 1 {
            return fail(g, f.params[1].name.loc, "components take one props object");
        }
        let name = String::from_utf8_lossy(&f.name).into_owned();
        let builder = builder_name(g, &f.name)?;
        let mut component = Component {
            name: name.clone(),
            builder,
            func: f.clone(),
            props: None,
            param: String::new(),
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            external: false,
            stateful: stateful::stateful(project, module, f),
        };
        if let Some(p) = f.params.first() {
            let Some(reference) = p.ty.as_ref() else {
                return fail(
                    g,
                    p.name.loc,
                    format!("props need a type: function {name}(props: {name}Props)"),
                );
            };
            let ty = types::type_ref(g, reference, false)?;
            let entry = model::info(g, ty);
            if entry
                .as_ref()
                .is_none_or(|entry| entry.kind != model::InfoKind::Object)
            {
                return fail(g, reference.loc, "props must be an object type");
            }
            component.props = Some(ty);
            component.param = String::from_utf8_lossy(&p.name.name).into_owned();
            component.destructure = p.destructure.clone();
            component.destructure_defaults = p.destructure_defaults.clone();
            for (index, n) in p.destructure.iter().enumerate() {
                let Some(field) = field_type(g, ty, &n.name) else {
                    return fail(
                        g,
                        n.loc,
                        format!(
                            "`{}` is not a prop of `{}`",
                            String::from_utf8_lossy(&n.name),
                            String::from_utf8_lossy(&reference.name)
                        ),
                    );
                };
                let default = p.destructure_defaults.get(index).cloned().flatten();
                if let Some(default) = default {
                    functions::prop(g, &default, field, "", f)?;
                }
            }
        }
        if g.components.iter().any(|other| other.name == name) {
            return fail(g, f.loc, format!("duplicate component `{name}`"));
        }
        g.extra_globals.push(name);
        g.components.push(component);
    }
    Ok(())
}

/// `TodoRow` → `TodoRow::build` (the builder inside the component's struct).
pub fn builder_name(g: &mut Generator<'_>, name: &[u8]) -> Result<String> {
    if name.is_empty() || !name[0].is_ascii_uppercase() {
        return fail(
            g,
            0,
            format!(
                "component `{}` must start with a capital letter",
                String::from_utf8_lossy(name)
            ),
        );
    }
    Ok(format!("{}::build", rust_text::id_at(g, name, 0)?))
}

pub fn find<'a>(g: &'a Generator<'_>, name: &str) -> Option<&'a Component> {
    g.components.iter().find(|component| component.name == name)
}

fn field_type(g: &Generator<'_>, props: Type, name: &[u8]) -> Option<Type> {
    let entry = model::info(g, props)?;
    entry
        .fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| field.ty)
}

fn fields(g: &Generator<'_>, component: &Component) -> Vec<model::Field> {
    match component.props {
        Some(props) => model::info(g, props)
            .map(|info| info.fields)
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

// ── inside a child component ──────────────────────────────────────────
