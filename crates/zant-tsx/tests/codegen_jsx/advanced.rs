//! Stateful widgets, fragments, spreads, icons, slots, spans and handlers.

use super::*;

#[test]
fn stateful_widgets_take_an_instance_id() {
    let generated = compiled(
        r#"function App() {
            return <Markdown key="doc" source="hello" />;
        }"#,
        vec![],
    );
    generated
        .assert_render("ui::markdown(")
        .assert_render(", ui::MarkdownOptions { ")
        .assert_render("key: Some(zant::Str::from(\"doc\")), ");
    // the id comes from the key: child_id(parent, "Markdown", key)
    generated.assert_render("tsx::child_id(0, \"Markdown\", (zant::Str::from(\"doc\")).as_str())");

    let generated = compiled(
        r#"function App() { return <Markdown source="x" />; }"#,
        vec![],
    );
    generated.assert_render("tsx::site_id(0, \"Markdown\", 1)");
}

#[test]
fn fragments_inside_conditionals_flatten_under_one_guard() {
    let generated = compiled(
        r#"function App() {
            return <Column>{show && <><Text>a</Text><Text>b</Text></>}</Column>;
        }"#,
        vec![state("show", "setShow", Type::Boolean)],
    );
    generated
        .assert_render("'_children: {")
        .assert_render("let _condition: bool = self.show;")
        .assert_render("if _condition { Some(ui::text(zant::Str::from(\"a\"), ui::TextOptions { ..Default::default() })) } else { None }")
        .assert_render("if _condition { Some(ui::text(zant::Str::from(\"b\"), ui::TextOptions { ..Default::default() })) } else { None }");
}

#[test]
fn a_fragment_as_the_whole_render_is_flattened_into_view_children() {
    let generated = compiled(
        r#"function App() {
            return <>{show && <Text>a</Text>}<Text>b</Text></>;
        }"#,
        vec![state("show", "setShow", Type::Boolean)],
    );
    generated
        .assert_render("ui::view(ui::ViewOptions { children: nodes![if self.show { Some(ui::text(zant::Str::from(\"a\"), ui::TextOptions { ..Default::default() })) } else { None }, ui::text(zant::Str::from(\"b\"), ui::TextOptions { ..Default::default() })]");
}

#[test]
fn nested_fragment_children_and_string_keys_are_supported() {
    let generated = compiled(
        r#"function App() {
            return <Column><Text key="a">a</Text><><Text key="b">b</Text></></Column>;
        }"#,
        vec![],
    );
    generated.assert_render(
        "children: nodes![ui::text(zant::Str::from(\"a\"), ui::TextOptions { key: Some(zant::Str::from(\"a\")), ..Default::default() }), ui::text(zant::Str::from(\"b\"), ui::TextOptions { key: Some(zant::Str::from(\"b\")), ..Default::default() })]",
    );
}

#[test]
fn spread_object_literals_merge_in_source_order() {
    let generated = compiled(
        r#"function App() {
            return <Text {...{ key: "a", size: 12 }} key="b">x</Text>;
        }"#,
        vec![],
    );
    generated
        .assert_render("key: Some(zant::Str::from(\"b\"))")
        .assert_render("size: 12.0f32");
}

#[test]
fn unknown_tags_and_dynamic_spreads_are_diagnostics() {
    let message = diagnostic(r#"function App() { return <Widget />; }"#, vec![]);
    assert!(
        message.contains("unknown tag <Widget>; valid tags:"),
        "{message}"
    );

    let message = diagnostic(
        r#"function App() {
            return <Column {...items[0]} />;
        }"#,
        vec![],
    );
    assert!(
        message.contains("spread a name or a member path"),
        "{message}"
    );
}

#[test]
fn a_second_icon_in_a_button_is_a_diagnostic() {
    let message = diagnostic(
        r#"function App() {
            return <Button><Icon name="check" /><Icon name="close" />Save</Button>;
        }"#,
        vec![],
    );
    assert!(message.contains("a <Button> has one icon"), "{message}");
}

#[test]
fn size_slots_require_exactly_one_child() {
    let message = diagnostic(r#"function App() { return <Tooltip></Tooltip>; }"#, vec![]);
    assert!(
        message.contains("<Tooltip> requires exactly one child element or the `child` prop"),
        "{message}"
    );
}

#[test]
fn explicit_children_props_use_the_nodes_macro() {
    let generated = compiled(
        r#"function App() {
            return <Column children={[<Text>a</Text>, show && <Text>b</Text>]} />;
        }"#,
        vec![state("show", "setShow", Type::Boolean)],
    );
    generated.assert_render(
        "children: nodes![ui::text(zant::Str::from(\"a\"), ui::TextOptions { ..Default::default() }), if self.show { Some(ui::text(zant::Str::from(\"b\"), ui::TextOptions { ..Default::default() })) } else { None }], ",
    );
}

#[test]
fn richer_text_span_props_and_link_actions() {
    let generated = compiled(
        r##"function App() {
            return <Text lineHeight={1.5} color="#123456">
                <Text italic underline strike background="#ff0000">styled</Text>
            </Text>;
        }"##,
        vec![],
    );
    generated
        .assert_render("line_height: 1.5f32, ")
        .assert_render("color: Some(0xff123456)")
        .assert_render(
            "tsx::text::SpanStyle { color: Some(0xff123456), italic: true, underline: true, strike: true, background: 0xffff0000, ..Default::default() }",
        );
}

#[test]
fn clickable_flattens_its_box_options_and_raw_identifiers() {
    let generated = compiled(
        r#"function App() {
            return <Clickable style={{ padding: 6, gap: 4 }} onClick={() => setCount(1)}>
                <Text>x</Text>
            </Clickable>;
        }"#,
        vec![number_state("count", "setCount", Type::U32)],
    );
    generated
        .assert_render("ui::clickable(ui::ClickableOptions { r#box: ui::BoxOptions { ")
        .assert_render("padding: ui::inset(6.0f32), ")
        .assert_render("gap: 4.0f32, ")
        .assert_render(
            "children: nodes![ui::text(zant::Str::from(\"x\"), ui::TextOptions { ..Default::default() })], ",
        )
        .assert_render("on_click: Some(ui::action(|s: &mut App| s._on_click1()))");
}

#[test]
fn alert_type_field_is_a_rust_raw_identifier() {
    let generated = compiled(
        r#"function App() {
            return <Alert type="success" title="Saved" />;
        }"#,
        vec![],
    );
    generated
        .assert_render("ui::alert(ui::AlertOptions { ")
        .assert_render("r#type: zant::ui::AlertType::Success, ")
        .assert_render("title: zant::Str::from(\"Saved\"), ");
}

#[test]
fn a_component_function_handler_keeps_its_method_reference() {
    let module = parse_module(
        r#"function App() {
            function increment() { setCount(count + 1); }
            return <Button onClick={increment}>+</Button>;
        }"#,
    );
    let (element, loc) = render_jsx(&module);
    let mut diag = None;
    let generated = {
        let mut g = Generator::new(&mut diag);
        g.name = "App".to_owned();
        g.type_decls = module.types.clone();
        g.extensions.hooks.emit_jsx = Some(jsx::emit);
        g.states.push(codegen::State {
            name: "count".to_owned(),
            setter: "setCount".to_owned(),
            ty: Type::U32,
            initial: String::new(),
            kind: StateKind::State,
        });
        // The component layer collects component functions; the harness registers one so the
        // source-method branch is exercised.
        let function = ast::Function {
            type_params: Vec::new(),
            loc: 0,
            name: b"increment".to_vec(),
            params: Vec::new(),
            ret: None,
            body: ast::Body::Block(Vec::new()),
        };
        g.methods.push(codegen::Method {
            name: "increment".to_owned(),
            func: Some(Box::new(function)),
            user: true,
            ret: Type::Void,
            ..codegen::Method::default()
        });
        let expr = ast::Expr {
            loc,
            data: ast::ExprData::Jsx(Box::new(element)),
        };
        events::scan_expr(&mut g, &expr).expect("scan");
        let ast::ExprData::Jsx(element) = &expr.data else {
            unreachable!()
        };
        let mut c = scope::Context {
            render: true,
            ret: Type::Node,
            ..scope::Context::default()
        };
        let value = jsx::emit(&mut g, &mut c, element).expect("emit");
        let render = scope::finish(&mut g, &c, value.code).expect("finish");
        Generated {
            module: String::new(),
            render,
            methods: String::new(),
        }
    };
    assert!(
        generated
            .render
            .contains("on_click: Some(ui::action(|s: &mut App| s.increment())), "),
        "generated render:\n{}",
        generated.render
    );
}

#[test]
fn dimension_props_set_numbers_or_percent_fields() {
    let generated = compiled(
        r#"function App() {
            return <Column width={120} height="50%" />;
        }"#,
        vec![],
    );
    generated
        .assert_render("width: Some(120.0f32), ")
        .assert_render("height_percent: Some(50.0), ");
}

#[test]
fn alias_flag_and_flex_props_lower_to_container_fields() {
    let generated = compiled(
        r#"function App() {
            return <Column position="absolute" flex={2} />;
        }"#,
        vec![],
    );
    generated
        .assert_render("absolute: true, ")
        .assert_render("grow: 2.0f32, shrink: 1.0f32, basis: Some(0.0f32), ");
}
