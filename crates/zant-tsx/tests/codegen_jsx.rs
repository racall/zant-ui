//! WP-5.4 JSX code generation tests.
//!
//! The six Zig files of this work package (`jsx.zig`, `jsx_values.zig`,
//! `jsx_children.zig`, `rich_text.zig`, `list.zig`, `events.zig`) carry **no**
//! `test "…"` blocks (checked with `grep -c '^test '`: 0 each), so there are no
//! same-name Zig tests to port. Instead these tests compile small TSX
//! components end to end (lexer → parser → `events::scan` → `jsx::emit` →
//! `scope::finish`), assert hand-reviewed Rust snippets, and require every
//! generated module to pass `syn::parse_file` (the `zant-tsx` output
//! validation of docs/rust-port/tsx-codegen.md §1).
//!
//! The component layer (WP-5.5) is not part of this work package: the harness
//! installs the component state and scans the render expression itself, which
//! is exactly what the WP-5.5 entry point will do before calling the
//! `hooks.emit_jsx` hook.

use zant_tsx::ast;
use zant_tsx::codegen::types::Type;
use zant_tsx::codegen::{self, Generator, StateKind, events, jsx, model, scope};
use zant_tsx::parser::Parser;

struct Generated {
    /// The whole Rust module (header + component + handlers).
    module: String,
    /// The generated `render` body, before formatting.
    render: String,
    /// Registered setter adapters (`_set_name` …).
    methods: String,
}

impl Generated {
    fn assert_parses(&self) -> &Generated {
        let formatted = codegen::parse_and_format(&self.module);
        // The formatted output must still carry the same render body.
        assert!(
            formatted.contains("fn render"),
            "formatted module lost the render function:\n{formatted}"
        );
        self
    }

    fn assert_render(&self, expected: &str) -> &Generated {
        self.assert_parses();
        assert!(
            self.render.contains(expected),
            "missing `{expected}` in generated render:\n{}",
            self.render
        );
        self
    }

    fn assert_methods(&self, expected: &str) -> &Generated {
        self.assert_parses();
        assert!(
            self.methods.contains(expected),
            "missing `{expected}` in generated methods:\n{}",
            self.methods
        );
        self
    }
}

struct StateSpec {
    name: String,
    setter: String,
    ty: Type,
}

fn state(name: &str, setter: &str, ty: Type) -> StateSpec {
    StateSpec {
        name: name.to_owned(),
        setter: setter.to_owned(),
        ty,
    }
}

fn parse_module(src: &str) -> ast::Module {
    let mut parser = Parser::init(src);
    match parser.parse_module() {
        Ok(module) => module,
        Err(_) => match parser.diag {
            Some(diag) => panic!("parse error at {}: {}", diag.loc, diag.msg),
            None => panic!("parse error without a diagnostic"),
        },
    }
}

/// The JSX returned by the first function of the module.
fn render_jsx(module: &ast::Module) -> (ast::Jsx, ast::Loc) {
    for top in &module.functions {
        match &top.func.body {
            ast::Body::Expr(e) => {
                if let ast::ExprData::Jsx(element) = &e.data {
                    return (element.as_ref().clone(), e.loc);
                }
            }
            ast::Body::Block(statements) => {
                for statement in statements {
                    if let ast::StmtData::Ret(Some(e)) = &statement.data
                        && let ast::ExprData::Jsx(element) = &e.data
                    {
                        return (element.as_ref().clone(), e.loc);
                    }
                }
            }
        }
    }
    panic!("no function returning JSX in the test source");
}

/// The generated module for one component: state first, then JSX scanning and
/// emission with the same order the WP-5.5 entry point uses.
fn compile(
    src: &str,
    setup: impl FnOnce(&mut Generator<'_>) -> codegen::Result<()>,
) -> Result<Generated, String> {
    let module = parse_module(src);
    let (element, loc) = render_jsx(&module);
    let mut diag = None;
    let result = {
        let mut g = Generator::new(&mut diag);
        g.name = "App".to_owned();
        g.type_decls = module.types.clone();
        g.resolving = vec![false; module.types.len()];
        g.react_imports = module.imports.clone();
        (|| -> codegen::Result<Generated> {
            // The hook the WP-5.5 entry point installs (docs/rust-port/tsx-codegen.md §1).
            g.extensions.hooks.emit_jsx = Some(jsx::emit);
            setup(&mut g)?;
            let expr = ast::Expr {
                loc,
                data: ast::ExprData::Jsx(Box::new(element)),
            };
            events::scan_expr(&mut g, &expr)?;
            let ast::ExprData::Jsx(element) = &expr.data else {
                unreachable!("the harness wraps JSX")
            };
            let mut c = scope::Context {
                render: true,
                ret: Type::Node,
                ..scope::Context::default()
            };
            let value = jsx::emit(&mut g, &mut c, element)?;
            let render = scope::finish(&mut g, &c, value.code)?;
            // Arrow handler bodies are emitted by WP-5.5's method pass; the
            // setter adapters are part of events.zig and generated here.
            let mut methods = String::new();
            let registered = g.methods.clone();
            for method in &registered {
                if method.setter.is_some() {
                    methods.push_str(&events::emit_setter(&mut g, method)?);
                }
            }
            let declarations = model::declarations(&g);
            let header = "#![allow(clippy::redundant_clone)]\n\
                          #![allow(clippy::too_many_arguments)]\n\
                          use zant::nodes;\nuse zant::tsx;\nuse zant::ui;\n\n";
            let body = format!(
                "{declarations}pub struct App {{}}\n\nimpl App {{\n    pub fn render(&mut self) -> ui::Node {{\n{render}\n    }}\n{methods}}}\n"
            );
            Ok(Generated {
                module: format!("{header}{body}"),
                render,
                methods,
            })
        })()
    };
    result.map_err(|_| match diag {
        Some(diag) => format!("{} @{}", diag.msg, diag.loc),
        None => "codegen failed without a diagnostic".to_owned(),
    })
}

fn compiled(src: &str, states: Vec<StateSpec>) -> Generated {
    match compile(src, |g| {
        for state in states {
            g.states.push(codegen::State {
                name: state.name,
                setter: state.setter,
                ty: state.ty,
                initial: String::new(),
                kind: StateKind::State,
            });
        }
        Ok(())
    }) {
        Ok(generated) => {
            generated.assert_parses();
            generated
        }
        Err(message) => panic!("unexpected diagnostic: {message}"),
    }
}

fn diagnostic(src: &str, states: Vec<StateSpec>) -> String {
    match compile(src, |g| {
        for state in states {
            g.states.push(codegen::State {
                name: state.name,
                setter: state.setter,
                ty: state.ty,
                initial: String::new(),
                kind: StateKind::State,
            });
        }
        Ok(())
    }) {
        Ok(generated) => panic!("expected a diagnostic, generated:\n{}", generated.render),
        Err(message) => message,
    }
}

fn number_state(name: &str, setter: &str, ty: Type) -> StateSpec {
    state(name, setter, ty)
}

// ── golden components ────────────────────────────────────────────────

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
        .assert_render("key: Some(\"reset\".into()), ")
        .assert_render("text: \"Reset\".into(), ")
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
        .assert_render("key: Some(\"name\".into()), ")
        .assert_render("value: Some(self.name.clone()), ")
        .assert_render("placeholder: \"Your name\".into(), ")
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
        .assert_render("ui::list(&self._list1, ui::ListOptions { ")
        .assert_render("count: _data.len(), ")
        .assert_render("item: std::rc::Rc::new(move |_index: usize| -> ui::Node {\n")
        .assert_render("let todo: Todo = _rows[_index].clone();")
        .assert_render(
            "ui::text(todo.clone().title.clone(), ui::TextOptions { ..Default::default() })",
        )
        .assert_render("key: \"todos\".into(), ");
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
        .assert_render("ui::column(ui::BoxOptions { children: nodes![if self.show { Some(ui::text(zant::Str::copy(\"hi\"), ui::TextOptions { ..Default::default() })) } else { None }], ..Default::default() })");
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
        .assert_render("nodes![if _condition { ui::text(zant::Str::copy(\"on\"), ui::TextOptions { ..Default::default() }) } else { None }, ")
        .assert_render("if _condition { ui::text(zant::Str::copy(\"now\"), ui::TextOptions { ..Default::default() }) } else { None }, ")
        .assert_render("if !_condition { ui::text(zant::Str::copy(\"off\"), ui::TextOptions { ..Default::default() }) } else { None }]")
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
            "tsx::text::span(zant::Str::copy(\"Read \"), tsx::text::SpanStyle { size: Some(16.0f32), ..Default::default() })",
        )
        .assert_render(
            "tsx::text::span(zant::Str::copy(\"this\"), tsx::text::SpanStyle { size: Some(16.0f32), weight: Some(zant::renderer::text::Weight::Bold), ..Default::default() })",
        )
        .assert_render(
            "tsx::text::span(zant::Str::copy(\"the docs\"), tsx::text::SpanStyle { size: Some(16.0f32), link: Some(0), ..Default::default() })",
        )
        .assert_render("on_link: Some(tsx::text::links(&[Some(ui::action(|s: &mut App| s._on_click1()))])), ");
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
        .assert_render("ui::view(ui::ViewOptions { children: nodes![ui::text(zant::Str::copy(\"a\"), ui::TextOptions { ..Default::default() }), ui::text(zant::Str::copy(\"b\"), ui::TextOptions { ..Default::default() })], ..Default::default() })");
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
    generated.assert_render("options: vec![\"a\".into(), \"b\".into()], ");
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
        "items: vec![ui::TabItem { label: \"One\".into(), content: ui::text(zant::Str::copy(\"a\"), ui::TextOptions { ..Default::default() }), ..Default::default() }], ",
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
        .assert_render("text: zant::Str::copy(\"Save\"), ");
}

#[test]
fn overlays_are_portals() {
    let generated = compiled(
        r#"function App() {
            return <Modal content={<Text>body</Text>} title="Hi" onClose={() => setOpen(false)} />;
        }"#,
        vec![],
    );
    generated
        .assert_render("ui::portal(ui::modal(ui::ModalOptions { ")
        .assert_render(
            "content: ui::text(zant::Str::copy(\"body\"), ui::TextOptions { ..Default::default() }), ",
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
    .assert_render("options: vec![ui::SegmentedOption { label: \"a\".into(), ..Default::default() }, ui::SegmentedOption { label: \"b\".into(), ..Default::default() }], ");
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
        .assert_render("self._list1.borrow_mut().follow(0, Some(tsx::list_index(2.0f64)), ui::ListScrollPosition::End);");
}

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
        .assert_render("key: Some(\"doc\".into()), ");
    // the id comes from the key: child_id(parent, "Markdown", key)
    generated.assert_render("tsx::child_id(0, \"Markdown\", (\"doc\".into()).as_str())");

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
        .assert_render("if _condition { ui::text(zant::Str::copy(\"a\"), ui::TextOptions { ..Default::default() }) } else { None }")
        .assert_render("if _condition { ui::text(zant::Str::copy(\"b\"), ui::TextOptions { ..Default::default() }) } else { None }");
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
        .assert_render("ui::view(ui::ViewOptions { children: nodes![if self.show { Some(ui::text(zant::Str::copy(\"a\"), ui::TextOptions { ..Default::default() })) } else { None }, ui::text(zant::Str::copy(\"b\"), ui::TextOptions { ..Default::default() })]");
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
        "children: nodes![ui::text(zant::Str::copy(\"a\"), ui::TextOptions { key: Some(\"a\".into()), ..Default::default() }), ui::text(zant::Str::copy(\"b\"), ui::TextOptions { key: Some(\"b\".into()), ..Default::default() })]",
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
        .assert_render("key: Some(\"b\".into())")
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
        "children: nodes![ui::text(zant::Str::copy(\"a\"), ui::TextOptions { ..Default::default() }), if self.show { Some(ui::text(zant::Str::copy(\"b\"), ui::TextOptions { ..Default::default() })) } else { None }], ",
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
            "children: nodes![ui::text(zant::Str::copy(\"x\"), ui::TextOptions { ..Default::default() })], ",
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
        .assert_render("title: \"Saved\".into(), ");
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
        // WP-5.5 collects component functions; the harness registers one so the
        // source-method branch is exercised.
        let function = ast::Function {
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
