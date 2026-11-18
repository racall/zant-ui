//! Props: diagnostics, edges, enums, integers, lists, children slots and overlays.

use super::*;

#[test]
fn unknown_props_list_the_valid_ones() {
    let message = diagnostic(r#"function App() { return <Button txt="x" />; }"#, vec![]);
    assert!(
        message.contains("unknown prop `txt` on <Button>"),
        "{message}"
    );
    assert!(message.contains("valid props: key, text"), "{message}");
}

#[test]
fn text_inside_a_container_is_a_diagnostic() {
    let message = diagnostic(
        r#"function App() { return <Column>plain</Column>; }"#,
        vec![],
    );
    assert!(message.contains("text must be inside <Text>"), "{message}");
}

#[test]
fn two_equal_literal_sibling_keys_are_a_diagnostic() {
    let message = diagnostic(
        r#"function App() {
            return <Column><Text key="a">x</Text><Text key="a">y</Text></Column>;
        }"#,
        vec![],
    );
    assert!(
        message.contains("two siblings have the key \"a\""),
        "{message}"
    );
}

#[test]
fn a_missing_required_prop_is_a_diagnostic() {
    let message = diagnostic(r#"function App() { return <Icon />; }"#, vec![]);
    assert!(
        message.contains("<Icon> is missing required prop `name`"),
        "{message}"
    );
}

#[test]
fn object_spread_in_style_is_not_supported() {
    let message = diagnostic(
        r#"function App() {
            return <Column style={{ ...extra }} />;
        }"#,
        vec![],
    );
    assert!(
        message.contains("object spread is not supported yet"),
        "{message}"
    );
}

#[test]
fn edge_objects_keep_per_side_values() {
    let generated = compiled(
        r#"function App() {
            return <Row style={{ padding: { top: 4, horizontal: 8 } }} />;
        }"#,
        vec![],
    );
    generated.assert_render(
        "padding: zant::core::geometry::Edges { top: 4.0f32, right: 8.0f32, bottom: 0.0, left: 8.0f32 }",
    );
}

#[test]
fn enum_props_use_the_rust_enum_and_flags_set_their_field() {
    let generated = compiled(
        r#"function App() {
            return <Button variant="primary" size="small" icon="check" />;
        }"#,
        vec![],
    );
    generated
        .assert_render("variant: zant::core::node::ButtonType::Primary, ")
        .assert_render("size: zant::core::node::ButtonSize::Small, ")
        .assert_render("icon: ui::Icon::Check, ");
}

#[test]
fn a_conditional_enum_and_a_union_value_are_matched() {
    let generated = compiled(
        r#"function App() {
            return <Button variant={primary ? "primary" : "default"} />;
        }"#,
        vec![state("primary", "setPrimary", Type::Boolean)],
    );
    generated.assert_render(
        "variant: if self.primary { zant::core::node::ButtonType::Primary } else { zant::core::node::ButtonType::Default }, ",
    );
}

#[test]
fn integer_props_are_coerced_to_their_field_type() {
    let generated = compiled(
        r#"function App() {
            return <Column order={2} />;
        }"#,
        vec![],
    );
    generated.assert_render("order: 2i32, ");
}

#[test]
fn string_lists_are_vectors_of_str() {
    let generated = compiled(
        r#"function App() {
            return <Select options={["a", "b"]} />;
        }"#,
        vec![],
    );
    generated.assert_render("options: vec![zant::Str::from(\"a\"), zant::Str::from(\"b\")], ");
}

#[test]
fn struct_lists_emit_typed_rust_structs() {
    let generated = compiled(
        r#"function App() {
            return <Tabs items={[{ label: "One", content: <Text>a</Text> }]} />;
        }"#,
        vec![],
    );
    generated.assert_render(
        "items: vec![ui::TabItem { label: zant::Str::from(\"One\"), content: ui::text(zant::Str::from(\"a\"), ui::TextOptions { ..Default::default() }), }], ",
    );
}

#[test]
fn button_text_and_icon_children_are_split() {
    let generated = compiled(
        r#"function App() {
            return <Button><Icon name="check" /> Save</Button>;
        }"#,
        vec![],
    );
    generated
        .assert_render("icon: ui::Icon::Check, ")
        .assert_render("text: zant::Str::from(\"Save\"), ");
}

#[test]
fn overlays_are_portals() {
    let generated = compiled(
        r#"function App() {
            return <Modal content={<Text>body</Text>} title="Hi" onClose={() => setOpen(false)} />;
        }"#,
        vec![state("open", "setOpen", Type::Boolean)],
    );
    generated
        .assert_render("ui::portal(ui::modal(ui::ModalOptions { ")
        .assert_render(
            "content: ui::text(zant::Str::from(\"body\"), ui::TextOptions { ..Default::default() }), ",
        );
}

#[test]
fn bool_and_index_actions_use_their_closure_parameters() {
    let generated = compiled(
        r#"function App() {
            return <Switch checked={on} onChange={(value) => setOn(value)} />;
        }"#,
        vec![state("on", "setOn", Type::Boolean)],
    );
    generated.assert_render(
        "on_change: Some(ui::bool_action(|s: &mut App, v: bool| s._on_change1(v))), ",
    );

    let generated = compiled(
        r#"function App() {
            return <Segmented options={[{ label: "a" }, { label: "b" }]} onChange={(index) => setSelected(index)} />;
        }"#,
        vec![number_state("selected", "setSelected", Type::Usize)],
    );
    generated.assert_render(
        "on_change: Some(ui::index_action(|s: &mut App, v: usize| s._on_change1(v))), ",
    )
    .assert_render("options: vec![ui::SegmentedOption { label: zant::Str::from(\"a\"), ..Default::default() }, ui::SegmentedOption { label: zant::Str::from(\"b\"), ..Default::default() }], ");
}

#[test]
fn list_key_extractor_hashes_strings_and_follow_scrolls() {
    let result = compile(
        r#"interface Todo { id: string; title: string }
        function App() {
            return <List data={todos} scrollToIndex={2} scrollAlign="end"
                renderItem={(todo) => <Text>{todo.title}</Text>}
                keyExtractor={(todo) => todo.id} />;
        }"#,
        |g| {
            let todo = model::named(g, b"Todo")?.expect("Todo resolves");
            let list = model::array(g, todo)?;
            g.states.push(codegen::State {
                name: "todos".to_owned(),
                setter: "setTodos".to_owned(),
                ty: list,
                initial: String::new(),
                kind: StateKind::State,
            });
            Ok(())
        },
    );
    let generated = match result {
        Ok(generated) => generated,
        Err(message) => panic!("unexpected diagnostic: {message}"),
    };
    generated
        .assert_parses()
        .assert_render("let _ids = _data.clone();")
        .assert_render(
            "item_id: Some(std::rc::Rc::new(move |_index: usize| -> u64 {",
        )
        .assert_render("return zant::core::hash::hash(0, (todo.clone().id.clone()).as_str());")
        .assert_render("self._list1.borrow_mut().follow(0, tsx::list_index(2.0f64), ui::ListScrollPosition::End);");
}
