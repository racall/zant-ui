//! Callback defaults, destructuring, renamed props and the child / list regressions.

use super::*;

pub(super) fn imported_callback_defaults() -> zant_tsx::compile::GeneratedProject {
    use zant_tsx::compile::{SourceFile, compile_project};
    compile_project(&[
        SourceFile::tsx("callbacks.tsx", "export function save(): void {} export const toggle = (checked: boolean) => {}; export function pair(id: number, text: string): void {}"),
        SourceFile::tsx("imported_defaults.tsx", r#"
import { save, toggle, pair } from "./callbacks";
import { Button, Checkbox, View } from "zant";
function Child({ onClose = save, onToggle = toggle, onPair = pair }: {
    onClose?: () => void; onToggle?: (checked: boolean) => void; onPair?: (id: number, text: string) => void;
}) {
    return <View><Button onClick={onClose}>Close</Button>
      <Checkbox checked={false} onChange={onToggle} />
      <Button onClick={() => { onClose(); onPair(1, "x"); }}>Pair</Button></View>;
}
export default function App() { return <Child />; }
"#),
    ]).expect("imported callback defaults compile")
}

#[test]
fn callback_defaults_accept_imported_functions_and_module_arrows() {
    let generated = imported_callback_defaults();
    contains(&generated.outputs[1], "crate::callbacks::save()");
    contains(&generated.outputs[1], "crate::callbacks::toggle(");
    contains(&generated.outputs[1], "crate::callbacks::pair(");
}

#[test]
fn callback_defaults_are_required_values_with_the_props_exact_callback_type() {
    let generated = compile(
        "callback_defaults",
        include_str!("../../testdata/tsx/callback_defaults.tsx"),
    );
    contains(
        &generated,
        "self.props.onClose).clone().unwrap_or_else(|| ui::Action::new(",
    );
    contains(&generated, "ui::BoolAction::new(");
    contains(&generated, "ui::TextAction::new(");
    contains(&generated, "ui::IndexAction::new(");
    contains(&generated, "std::rc::Rc::new(");
    contains(&generated, ".call(_cx)");
}

#[test]
fn callback_defaults_reject_render_captures_and_incompatible_signatures() {
    for (default, signature, expected) in [
        (
            "() => onOther()",
            "() => void",
            "callback defaults cannot read props, state or render locals",
        ),
        (
            "() => count",
            "() => void",
            "callback defaults cannot read props, state or render locals",
        ),
        (
            "() => label",
            "() => void",
            "callback defaults cannot read props, state or render locals",
        ),
        (
            "(value: string) => {}",
            "(value: number) => void",
            "different signature",
        ),
        (
            "(a, b) => {}",
            "(value: number) => void",
            "too many parameters",
        ),
        ("() : number => 1", "() => void", "must return void"),
        ("wrong", "() => void", "different signature"),
        (
            "() => { const n = count; }",
            "() => void",
            "cannot read props, state or render locals",
        ),
        (
            "() => { const values = [1]; values.map(n => count + n); }",
            "() => void",
            "cannot read props, state or render locals",
        ),
    ] {
        let source = format!(
            "function wrong(n: number): void {{}} function Child({{ onClose = {default}, onOther }}: {{ onClose?: {signature}; onOther: () => void }}) {{ const [count, setCount] = useState(0); const label = \"x\"; return <Button onClick={{() => onClose()}}>x</Button>; }} export default function App() {{ return <View />; }}"
        );
        let error = zant_tsx::compile::compile(&source, "invalid.tsx")
            .expect_err("invalid callback default");
        assert!(error.msg.contains(expected), "{}", error.msg);
    }
}

#[test]
fn local_object_destructuring_preserves_fields_and_evaluates_once() {
    let generated = compile("local_destructure_rename", LOCAL_DESTRUCTURE_RENAME);
    contains(&generated, "_destructure.title.clone()");
    contains(&generated, "_destructure.id");
    contains(&generated, "color_text");
    contains(&generated, "let mut mutable: zant::Str");
    assert_eq!(3, generated.matches("= makeItem();").count(), "{generated}");
}

#[test]
fn local_object_destructuring_rejects_missing_fields_and_non_objects() {
    for (body, expected) in [
        (
            "const item: { title: string } = { title: \"a\" }; const { missing: label } = item;",
            "has no field `missing`",
        ),
        (
            "const { title: label } = 1;",
            "object destructuring needs an object with a known type",
        ),
    ] {
        let source = format!("export default function App() {{ {body} return <View />; }}");
        let error =
            zant_tsx::compile::compile(&source, "invalid.tsx").expect_err("invalid pattern");
        assert!(error.msg.contains(expected), "{}", error.msg);
    }
}

#[test]
fn renamed_props_resolve_only_the_local_binding_and_validate_the_property() {
    for (params, value, expected) in [
        ("title: label", "title", "unknown name `title`"),
        ("missing: label", "label", "`missing` is not a prop"),
        (
            "title: label = 1",
            "label",
            "cannot convert f64 to zant::Str",
        ),
    ] {
        let source = format!(
            "function Label({{ {params} }}: {{ title: string }}) {{ return <Text>{{{value}}}</Text>; }} \
             export default function App() {{ return <Label title=\"Hello\" />; }}"
        );
        let error = zant_tsx::compile::compile(&source, "invalid.tsx").expect_err("invalid prop");
        assert!(error.msg.contains(expected), "{}", error.msg);
    }
}

#[test]
fn child_callback_props_run_synchronously() {
    let generated = compile("child_callbacks", CHILD_CALLBACKS);
    // no deferred queue: `ui::begin_frame` clears effects before they run
    assert!(
        !generated.contains("ui::effect("),
        "a callback prop must not be deferred through ui::effect:\n{generated}"
    );
    // the parent callback classes: `() => void` is an Action, a typed
    // parameter is an `Rc<dyn Fn(&mut ActionCx, …)>`, `string` is a TextAction
    contains(
        &generated,
        "fn _on_click1(&mut self, _cx: &mut ui::ActionCx<'_>)",
    );
    contains(&generated, "(self.props.onPick.clone())(_cx, 7u32)");
    contains(&generated, "(self.props.onRename.clone()).call(_cx,");
    contains(
        &generated,
        "(self.props.onToggle.clone()).call(_cx, checked)",
    );
    // the handler that calls a callback through a helper passes the context on
    contains(
        &generated,
        "fn add(&mut self, _cx: &mut ui::ActionCx<'_>, delta: u32)",
    );
    contains(&generated, "self.add(_cx, 1u32)");
    // the `boolean` callback prop keeps its BoolAction shape
    contains(
        &generated,
        "fn _on_change4(&mut self, _cx: &mut ui::ActionCx<'_>, checked: bool)",
    );
}

#[test]
fn a_props_object_callback_prop_also_runs_synchronously() {
    let generated = compile("props_object_callbacks", PROPS_OBJECT_CALLBACKS);
    contains(
        &generated,
        "fn _on_click1(&mut self, _cx: &mut ui::ActionCx<'_>)",
    );
    contains(&generated, "(self.props.clone().onSave.clone()).call(_cx)");
    contains(&generated, "(self.props.clone().onPick.clone())(_cx, 3u32)");
}

#[test]
fn optional_node_prop_accepts_a_bare_element() {
    let generated = compile("optional_node", OPTIONAL_NODE);
    contains(&generated, "footer: Some(ui::row(ui::BoxOptions {");
    assert!(
        !squeeze(&generated).contains("footer:ui::row("),
        "an Option<Node> prop must wrap the element in Some(..):\n{generated}"
    );
}

#[test]
fn rich_text_keeps_a_literal_after_an_expression() {
    let generated = compile("rich_text_tail", RICH_TEXT_TAIL);
    // the final `.` after `{…}` becomes part of the same span's format pattern
    contains(&generated, "format!(\"{}.\", if");
}

#[test]
fn list_in_a_child_component_clones_its_state_slot() {
    let generated = compile("list_in_child", LIST_IN_CHILD);
    // the row closures outlive the `&mut self` build: the list and the
    // component's local state slot are cloned into locals first (E0505)
    contains(&generated, "let _list = self._list1.clone();");
    contains(&generated, "let _state = self._state.clone();");
    contains(&generated, "ui::list(&_list, ui::ListOptions {");
    assert!(
        !squeeze(&generated).contains("ui::list(&self._list"),
        "the list state must not be borrowed from self:\n{generated}"
    );
    // the row handler upgrades the hoisted slot and calls the callback prop
    contains(&generated, "let _state = _state.clone();");
    contains(&generated, "(cap.onPick.clone())(_cx,");
}

#[test]
fn optional_children_are_wrapped_in_some() {
    let generated = compile("optional_children", OPTIONAL_CHILDREN);
    contains(&generated, "children: Some(tsx::List::from_vec(nodes![");
    contains(&generated, "children: Some(ui::text(");
}
