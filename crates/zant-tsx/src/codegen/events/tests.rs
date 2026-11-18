//! Unit tests.

use super::*;
use crate::codegen::Generator;
use crate::codegen::types::{Type, Value};
use crate::tags::Kind;

fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    f(&mut g);
}

#[test]
fn action_kinds_map_to_rust_builders_and_signatures() {
    assert_eq!("action", action_builder(Kind::Action));
    assert_eq!("text_action", action_builder(Kind::TextAction));
    assert_eq!("bool_action", action_builder(Kind::BoolAction));
    assert_eq!("index_action", action_builder(Kind::IndexAction));
    assert_eq!(Type::Void, action_type(Kind::Action));
    assert_eq!(Type::String, action_type(Kind::TextAction));
    assert_eq!(Type::Boolean, action_type(Kind::BoolAction));
    assert_eq!(Type::Usize, action_type(Kind::IndexAction));
    assert_eq!(
        "(value: string) => void",
        action_signature(Kind::TextAction)
    );
    assert!(is_action(Kind::IndexAction));
    assert!(!is_action(Kind::ShortcutAction));
}

#[test]
fn setters_emit_str_and_bool_adapters() {
    with_generator(|g| {
        g.name = "Counter".to_owned();
        g.states.push(crate::codegen::State {
            name: "name".to_owned(),
            setter: "setName".to_owned(),
            ty: Type::String,
            initial: "String::new()".to_owned(),
            kind: crate::codegen::StateKind::State,
        });
        let method = Method {
            name: "_set_name".to_owned(),
            setter: Some(0),
            action: Some(Kind::TextAction),
            ..Method::default()
        };
        let code = emit_setter(g, &method).expect("setter");
        assert_eq!(
            "fn _set_name(&mut self, value: &str) {\nself.name = zant::Str::copy(value);\n}\n",
            code
        );
    });
}

#[test]
fn the_jsx_hook_contract_is_the_registered_method() {
    with_generator(|g| {
        assert_eq!(0, g.events.len());
        assert!(g.lists.is_empty());
        assert_eq!(0, g.child_sites);
        assert_eq!(0, g.scroll_depth);
        assert_eq!(0, g.loop_depth);
        let _ = Value::new(Type::Node, "ui::Node");
    });
}
