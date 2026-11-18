//! Elements: actions, lists, conditionals, rich text, fragments, styles and text children.

use super::*;

#[test]
fn button_emits_options_action_and_optional_key() {
    // Hook: `emit_jsx` receives the render JSX and returns a `ui::Node` value.
    let generated = compiled(
        r#"function App() {
            return <Button key="reset" text="Reset" onClick={() => setCount(0)} disabled={count === 0} />;
        }"#,
        vec![number_state("count", "setCount", Type::U32)],
    );
    generated
        .assert_render("ui::button(ui::ButtonOptions { ")
        .assert_render("key: Some(zant::Str::from(\"reset\")), ")
        .assert_render("text: zant::Str::from(\"Reset\"), ")
        .assert_render("disabled: self.count == 0u32, ")
        .assert_render("on_click: Some(ui::action(|s: &mut App| s._on_click1())), ")
        .assert_render("..Default::default() })");
}

#[test]
fn input_emits_text_action_for_a_setter() {
    let generated = compiled(
        r#"function App() {
            return <Input key="name" value={name} placeholder="Your name" onChange={setName} />;
        }"#,
        vec![state("name", "setName", Type::String)],
    );
    generated
        .assert_render("ui::input(ui::InputOptions { ")
        .assert_render("key: Some(zant::Str::from(\"name\")), ")
        .assert_render("value: Some(self.name.clone()), ")
        .assert_render("placeholder: zant::Str::from(\"Your name\"), ")
        .assert_render("on_change: Some(ui::text_action(|s: &mut App, v: &str| s._set_name(v))), ")
        .assert_methods("fn _set_name(&mut self, value: &str) {")
        .assert_methods("self.name = zant::Str::copy(value);");
}

#[test]
fn list_emits_a_closure_over_the_data_copy() {
    let result = compile(
        r#"interface Todo { title: string; done: boolean }
        function App() {
            return <List key="todos" data={todos} renderItem={(todo) => <Text>{todo.title}</Text>} />;
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
        .assert_render("{\nlet _data = self.todos.clone();")
        .assert_render("let _rows = _data.clone();")
        .assert_render("let _list = self._list1.clone();")
        .assert_render("ui::list(&_list, ui::ListOptions { ")
        .assert_render("count: _data.len(), ")
        .assert_render("item: std::rc::Rc::new(move |_index: usize| -> ui::Node {\n")
        .assert_render("let todo: Todo = _rows[_index].clone();")
        .assert_render(
            "ui::text(todo.clone().title.clone(), ui::TextOptions { ..Default::default() })",
        )
        .assert_render("key: zant::Str::from(\"todos\"), ");
}

#[test]
fn conditional_children_are_optional_nodes() {
    let generated = compiled(
        r#"function App() {
            return <Column>{show && <Text>hi</Text>}</Column>;
        }"#,
        vec![state("show", "setShow", Type::Boolean)],
    );
    generated
        .assert_render("ui::column(ui::BoxOptions { children: nodes![if self.show { Some(ui::text(zant::Str::from(\"hi\"), ui::TextOptions { ..Default::default() })) } else { None }], ..Default::default() })");
}

#[test]
fn ternaries_with_a_fragment_split_into_guarded_children() {
    let generated = compiled(
        r#"function App() {
            return <Column>{show ? <><Text>on</Text><Text>now</Text></> : <Text>off</Text>}</Column>;
        }"#,
        vec![state("show", "setShow", Type::Boolean)],
    );
    generated
        .assert_render("'_children: {")
        .assert_render("let _condition: bool = self.show;")
        .assert_render("nodes![if _condition { Some(ui::text(zant::Str::from(\"on\"), ui::TextOptions { ..Default::default() })) } else { None }, ")
        .assert_render("if _condition { Some(ui::text(zant::Str::from(\"now\"), ui::TextOptions { ..Default::default() })) } else { None }, ")
        .assert_render("if !_condition { Some(ui::text(zant::Str::from(\"off\"), ui::TextOptions { ..Default::default() })) } else { None }]")
        .assert_render("break '_children ");
}

#[test]
fn rich_text_builds_spans_links_and_span_styles() {
    let generated = compiled(
        r#"function App() {
            return <Text size={16}>Read <Text weight="bold">this</Text> or <Link onClick={() => setOpen(true)}>the docs</Link></Text>;
        }"#,
        vec![state("open", "setOpen", Type::Boolean)],
    );
    generated
        .assert_render("ui::rich_text(ui::RichTextOptions { ")
        .assert_render("line_height: 0.0, ")
        .assert_render(
            "tsx::text::span((zant::Str::from(\"Read \")).as_str(), tsx::text::SpanStyle { size: Some(16.0f32), ..Default::default() })",
        )
        .assert_render(
            "tsx::text::span((zant::Str::from(\"this\")).as_str(), tsx::text::SpanStyle { size: Some(16.0f32), weight: Some(zant::renderer::text::Weight::Bold), ..Default::default() })",
        )
        .assert_render(
            "tsx::text::span((zant::Str::from(\"the docs\")).as_str(), tsx::text::SpanStyle { size: Some(16.0f32), link: Some(0), ..Default::default() })",
        )
        .assert_render("on_link: Some(tsx::text::links(vec![Some(ui::action(|s: &mut App| s._on_click1()))])), ");
}

#[test]
fn fragments_become_a_view_with_children() {
    let generated = compiled(
        r#"function App() {
            return <><Text>a</Text><Text>b</Text></>;
        }"#,
        vec![],
    );
    generated
        .assert_render("ui::view(ui::ViewOptions { children: nodes![ui::text(zant::Str::from(\"a\"), ui::TextOptions { ..Default::default() }), ui::text(zant::Str::from(\"b\"), ui::TextOptions { ..Default::default() })], ..Default::default() })");
}

#[test]
fn style_props_lower_to_options_with_inset_and_argb() {
    let generated = compiled(
        r##"function App() {
            return <Row style={{ padding: 8, background: "#ff0000", gap: 6 }} />;
        }"##,
        vec![],
    );
    generated
        .assert_render("ui::row(ui::BoxOptions { ")
        .assert_render("padding: ui::inset(8.0f32), ")
        .assert_render("background: Some(0xffff0000), ")
        .assert_render("gap: 6.0f32, ");
}

#[test]
fn text_children_join_into_the_text_argument() {
    let generated = compiled(
        r#"function App() {
            return <Text>Hello {name}!</Text>;
        }"#,
        vec![state("name", "setName", Type::String)],
    );
    generated
        .assert_render("ui::text(zant::Str::copy(&format!(\"Hello {}!\", self.name.clone())), ui::TextOptions { ..Default::default() })");
}

// ── diagnostics and smaller conversions ──────────────────────────────
