//! WP-5.5 golden tests: components, state, hooks, stores and array methods.
//!
//! The JSX / event emitters are WP-5.4's modules and have not landed in this
//! worktree (`crates/zant-tsx/src/codegen/jsx.rs` / `events.rs`). These tests
//! inject a small **test-only** stand-in through
//! `generate::generate_with(.., install_extra)` that
//!
//! - emits `ui::text("stub", …)` for every element (so the module parses and
//!   the render statements around it are generated), and
//! - registers inline event arrows / setters the way `events.zig` does, so the
//!   handler methods of `codegen.zig` are exercised.
//!
//! Everything this work package owns — the struct, `impl Default`, the setter
//! and handler methods, `impl ui::App`, `build` + `ui::local_state`, memo /
//! effect / ref / store lowering and the array adapters — is real generated
//! code. The expected handler bodies follow the design-doc snippet of
//! `docs/rust-port/tsx-codegen.md` §5/§6; the render body itself is covered by
//! WP-5.4's own golden tests once its emitter lands.

use zant_tsx::ast;
use zant_tsx::codegen::{self, components, generate, scope};
use zant_tsx::parser::Parser;
use zant_tsx::tags;

const STUB_NODE: &str = "ui::text(zant::Str::from(\"stub\"), ui::TextOptions::default())";

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

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The test-only JSX emitter: a stub node plus handler registration.
fn stub_jsx(
    g: &mut codegen::Generator<'_>,
    _c: &mut scope::Context,
    j: &ast::Jsx,
) -> codegen::Result<codegen::types::Value> {
    for attr in &j.attrs {
        let Some(value) = attr.value.as_deref().cloned() else {
            continue;
        };
        let kind = match text(&attr.name.name).as_str() {
            "onClick" => Some(tags::Kind::Action),
            "onChange" => Some(tags::Kind::TextAction),
            "onToggle" | "onChecked" => Some(tags::Kind::BoolAction),
            "onSelect" => Some(tags::Kind::IndexAction),
            _ => None,
        };
        let Some(kind) = kind else {
            // props are lowered by WP-5.4's `jsx.rs`; emitting their values
            // here keeps the expression codegen (reads, clones, array calls)
            // under test.
            let _ = codegen::expr::emit(g, _c, &value, None, false)?;
            continue;
        };
        register_handler(g, &value, kind)?;
    }
    for child in &j.children {
        match child {
            ast::Child::Element(element) => {
                stub_jsx(g, _c, element)?;
            }
            ast::Child::Expr(Some(value)) => {
                let _ = codegen::expr::emit(g, _c, value, None, false)?;
            }
            _ => {}
        }
    }
    Ok(codegen::types::Value::new(
        codegen::types::Type::Node,
        STUB_NODE,
    ))
}

/// `events.zig`'s `registerArrow` / setter reuse, reduced to what the stub
/// emitter needs to exercise the component methods.
fn register_handler(
    g: &mut codegen::Generator<'_>,
    value: &ast::Expr,
    kind: tags::Kind,
) -> codegen::Result<()> {
    match &value.data {
        ast::ExprData::Arrow(function) => {
            g.counter += 1;
            let name = scope::unique_method(g, &format!("_on_click{}", g.counter))?;
            let capture = scope::unique_method(g, &format!("_Capture{}", g.counter))?;
            g.methods.push(codegen::Method {
                name,
                func: Some(function.clone()),
                action: Some(kind),
                capture_name: Some(capture),
                ..codegen::Method::default()
            });
            Ok(())
        }
        ast::ExprData::Ident(name) => {
            if let Some(index) = scope::setter_index(g, name) {
                let already = g
                    .methods
                    .iter()
                    .any(|method| method.setter == Some(index) && method.action == Some(kind));
                if !already {
                    let setter = scope::unique_method(g, &format!("_{}", text(name)))?;
                    g.methods.push(codegen::Method {
                        name: setter,
                        setter: Some(index),
                        action: Some(kind),
                        ..codegen::Method::default()
                    });
                }
            } else if let Some(index) = scope::source_method(g, name) {
                g.methods[index].action = Some(kind);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// `events.emitSetter` for the stub harness (design-doc §5: the setter adapter
/// becomes a private method).
fn stub_setter(g: &mut codegen::Generator<'_>, m: &codegen::Method) -> codegen::Result<String> {
    let state = g.states[m.setter.expect("setter")].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    let value = match m.action {
        Some(tags::Kind::IndexAction) if state.ty.integer() => "value as _".to_owned(),
        Some(tags::Kind::IndexAction) => "value as _".to_owned(),
        _ => "value".to_owned(),
    };
    let ty = match m.action {
        Some(tags::Kind::TextAction) => "&str",
        Some(tags::Kind::BoolAction) => "bool",
        Some(tags::Kind::IndexAction) => "usize",
        _ => "()",
    };
    let value = if ty == "&str" {
        format!("Str::copy({value})")
    } else if state.ty.integer() && ty == "usize" {
        format!("{value} as {}", state.ty.rust(g))
    } else {
        value
    };
    let name = zant_tsx::codegen::rust_text::id_at(g, m.name.as_bytes(), 0)?;
    Ok(format!(
        "fn {name}(&mut self, value: {ty}) {{\nlet _ = value;\nself.{field} = {value};\n}}\n"
    ))
}

fn install_stubs(g: &mut codegen::Generator<'_>) {
    g.extensions.hooks.emit_jsx = Some(stub_jsx);
    g.extensions.hooks.emit_setter = Some(stub_setter);
}

fn generated(src: &str) -> generate::GeneratedFile {
    // The real JSX / event emitters are wired since WP-5.4 landed; no stub.
    let module = parse_module(src);
    match generate::generate("src/App.tsx", &module, &generate::Options::default()) {
        Ok(file) => file,
        Err(_) => match generate::last_diag() {
            Some(diag) => panic!("codegen error at {}: {}", diag.loc, diag.msg),
            None => panic!("codegen error without a diagnostic"),
        },
    }
}

/// The fields of the generated root struct, in order.
fn struct_fields(source: &str, name: &str) -> Vec<String> {
    let start = source
        .find(&format!("pub struct {name} {{\n"))
        .unwrap_or_else(|| panic!("struct {name} in\n{source}"));
    let rest = &source[start..];
    let end = rest.find("\n}\n").expect("struct end");
    rest[..end]
        .lines()
        .skip(1)
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

/// The body of `impl Default for Name`, one field per line.
fn default_fields(source: &str, name: &str) -> Vec<String> {
    let start = source
        .find(&format!("impl Default for {name} {{\n"))
        .unwrap_or_else(|| panic!("impl Default for {name} in\n{source}"));
    let rest = &source[start..];
    let end = rest.find("\n    }\n}\n").expect("default end");
    rest[..end]
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.contains(": ") && !line.starts_with("Self") && !line.starts_with("fn ") {
                Some(line.trim_end_matches(',').to_owned())
            } else {
                None
            }
        })
        .collect()
}

// ── the counter golden (docs/rust-port/tsx-codegen.md §5.1 / §6) ──────

const COUNTER: &str = r#"
import { useState, View, Text, Button, Input } from "zant";
import type { u32 } from "zant";

export default function Counter() {
  const [count, setCount] = useState<u32>(0);
  const [name, setName] = useState("Zant");
  const [notify, setNotify] = useState(false);
  return (
    <View padding={24}>
      <Input key="name" value={name} onChange={setName} />
      <Button key="reset" text="Reset" disabled={count === 0} onClick={() => setCount(0)} />
      <Text key="note">Notifications are on</Text>
    </View>
  );
}
"#;

#[test]
fn counter_matches_the_design_doc_snippet() {
    let file = generated(COUNTER);
    let source = &file.source;
    assert!(
        source.starts_with("// @generated by zant-tsx from src/App.tsx — do not edit."),
        "{source}"
    );
    assert!(source.contains("#![allow(clippy::redundant_clone)]"));
    assert!(source.contains("#![allow(clippy::too_many_arguments)]"));
    assert!(source.contains("use zant::tsx;"));
    assert!(source.contains("use zant::ui;"));

    assert!(!source.contains("#[derive(Default)]\npub struct Counter"));
    // §6.1: plain fields, `impl Default` from the useState initial values.
    assert_eq!(
        vec!["count: u32,", "name: zant::Str,", "notify: bool,"],
        struct_fields(source, "Counter"),
        "{source}"
    );
    assert_eq!(
        vec![
            "count: 0u32".to_owned(),
            "name: zant::Str::from(\"Zant\")".to_owned(),
            "notify: false".to_owned(),
        ],
        default_fields(source, "Counter"),
        "{source}"
    );

    // `impl ui::App for Counter` holds `render`; the handlers are inherent.
    assert!(
        source.contains("impl ui::App for Counter {\n    fn render(&mut self) -> ui::Node {"),
        "{source}"
    );

    // §5: `onChange={setName}` becomes a setter adapter method, and
    // `onClick={() => setCount(0)}` a numbered private handler.
    assert!(
        source.contains("fn _set_name(&mut self, value: &str) {"),
        "{source}"
    );
    assert!(source.contains("fn _on_click"), "{source}");
    assert!(
        source.contains("self.count = 0u32;"),
        "the reset handler body is generated\n{source}"
    );
    // the whole module was validated with `syn` inside `generate`.
    assert!(syn::parse_file(source).is_ok());
}

// ── state, refs, memo, effects, stores ────────────────────────────────

const STATE: &str = r#"
import { useState, useRef, useMemo, useEffect, createStore, useStore, View, Text, Button } from "zant";
import type { u32 } from "zant";

const settings = createStore<{ dark: boolean }>({ dark: false });

export default function App() {
  const [count, setCount] = useState<u32>(0);
  const [label, setLabel] = useState("hi");
  const greeting = `${label}!`;
  const ref = useRef<u32>(0);
  const doubled = useMemo(() => count * 2, [count]);
  useEffect(() => { setCount(count + 1); }, [count]);
  const dark = useStore(settings, (s) => s.dark);
  return <View><Text>{`${count} ${doubled}`}</Text><Button onClick={() => { ref.current = 1; settings.update({ dark: true }); }}>go</Button></View>;
}
"#;

#[test]
fn state_refs_memo_effects_and_stores_lower() {
    let file = generated(STATE);
    let source = &file.source;

    // `createStore<T>(initial)` → a thread_local plus accessors.
    assert!(source.contains("thread_local! {"), "{source}");
    assert!(source.contains("pub fn settings_read() ->"), "{source}");
    assert!(
        source.contains("pub fn settings_update(update: impl FnOnce(&mut "),
        "{source}"
    );

    // `useState` field + `useRef` field carry their initial values.
    assert_eq!(
        vec![
            "count: u32,".to_owned(),
            "label: zant::Str,".to_owned(),
            "ref_: u32,".to_owned(),
            "_memo3: Option<((u32,), u32)>,".to_owned(),
            "_deps4: Option<(u32,)>,".to_owned(),
        ],
        struct_fields(source, "App"),
        "{source}"
    );
    assert!(default_fields(source, "App").contains(&"ref_: 0u32".to_owned()));

    // §3: every non-`Copy` state read clones (the template `{label}` read).
    assert!(source.contains("self.label.clone()"), "{source}");

    // `useMemo` compares the dependency tuple in its cache field.
    assert!(source.contains("_memo"), "{source}");
    assert!(
        source.contains("let _deps: (u32,) = (self.count,);"),
        "{source}"
    );

    // `useEffect` queues the method when its dependency tuple changed.
    assert!(source.contains("_deps"), "{source}");
    assert!(source.contains("ui::effect(ui::action("), "{source}");

    // `useStore(settings, (s) => s.dark)` becomes a render local read.
    assert!(source.contains("settings_read()"), "{source}");

    // handler: `ref.current = 1` writes the live field, `settings.update(…)`
    // calls the store accessor (both inside the same handler method).
    assert!(source.contains("self.ref_ = 1u32;"), "{source}");
    assert!(source.contains("settings_update("), "{source}");
}

// ── array methods ─────────────────────────────────────────────────────

const ARRAYS: &str = r#"
import { useState, View, Text } from "zant";
import type { u32 } from "zant";

export default function App() {
  const [items, setItems] = useState<u32[]>([1, 2, 3]);
  const doubled = items.map((item) => item * 2);
  const even = items.filter((item) => item % 2 === 0);
  const total = items.reduce((sum, item) => sum + item, 0);
  return <View><Text>{`${doubled.length} ${even.length} ${total}`}</Text></View>;
}
"#;

#[test]
fn array_map_filter_reduce_lower_to_iterator_adapters() {
    let file = generated(ARRAYS);
    let source = &file.source;
    assert!(source.contains("tsx::List::from_vec("), "{source}");
    assert!(source.contains(".map(|item: u32|"), "{source}");
    assert!(source.contains(".filter_map(|item|"), "{source}");
    assert!(source.contains(".fold("), "{source}");
    // the receiver is evaluated once, in source order
    assert!(source.contains("let _items:"), "{source}");
}

// ── custom hooks (already expanded by WP-4.3) and child local state ───

#[test]
fn a_custom_hook_expanded_into_usestate_lowers_like_state() {
    // WP-4.3 expands `useCounter` in place; the expansion is plain
    // `useState` + a method, which is what this test feeds the generator.
    let file = generated(
        r#"
import { useState, View, Text, Button } from "zant";
import type { u32 } from "zant";

export default function App() {
  const [count, setCount] = useState<u32>(0);
  return <View><Button onClick={() => setCount(count + 1)}>{`${count}`}</Button></View>;
}
"#,
    );
    let source = &file.source;
    assert!(source.contains("count: u32,"), "{source}");
    assert!(source.contains("self.count = "), "{source}");
    assert!(source.contains("let count: u32 = self.count;"), "{source}");
}

#[test]
fn a_child_component_keeps_its_state_in_a_local_state_slot() {
    let module = parse_module(
        r#"
import { useState, View, Text, Button } from "zant";
import type { u32 } from "zant";

function Row(props: { label: string }) {
  const [n, setN] = useState<u32>(0);
  return <View><Text>{props.label}</Text><Button onClick={() => setN(n + 1)}>+</Button></View>;
}

export default function App() {
  return <View><Row label="row" /></View>;
}
"#,
    );
    let mut diag = None;
    let mut g = codegen::Generator::new(&mut diag);
    g.type_decls = module.types.clone();
    g.react_imports = module.imports.clone();
    g.resolving = vec![false; module.types.len()];
    generate::install(&mut g);
    install_stubs(&mut g);
    components::collect(&mut g, &module, None, &[]).expect("components collect");
    let index = g
        .components
        .iter()
        .position(|component| component.name == "Row")
        .expect("Row");
    let component = g.components[index].clone();
    assert!(component.stateful, "useState makes Row stateful");
    assert_eq!("Row::build", component.builder);

    let app = module
        .functions
        .iter()
        .find(|top| top.func.name == b"App")
        .expect("App");
    let jsx = match &app.func.body {
        ast::Body::Block(statements) => match &statements[statements.len() - 1].data {
            ast::StmtData::Ret(Some(value)) => match &value.data {
                ast::ExprData::Jsx(view) => match &view.children[0] {
                    ast::Child::Element(row) => (**row).clone(),
                    _ => panic!("a <Row> child"),
                },
                _ => panic!("a JSX element"),
            },
            _ => panic!("a return statement"),
        },
        ast::Body::Expr(_) => panic!("block body"),
    };
    let mut c = scope::Context {
        render: true,
        ret: codegen::types::Type::Node,
        ..scope::Context::default()
    };
    let call = match components::emit_call(&mut g, &mut c, &jsx, &component) {
        Ok(call) => call,
        Err(_) => match &diag {
            Some(diag) => panic!("call site error at {}: {}", diag.loc, diag.msg),
            None => panic!("call site error without a diagnostic"),
        },
    };
    assert!(
        call.code.contains("ui::local_state") || call.code.contains("Row::build("),
        "{}",
        call.code
    );
    assert!(
        call.code.contains("tsx::site_id(0, \"Row\", 1)"),
        "{}",
        call.code
    );
}

#[test]
fn a_stateful_child_in_a_list_needs_a_key() {
    let module = parse_module(
        r#"
function Row() { return null; }
function App() { return null; }
"#,
    );
    let mut diag = None;
    let mut g = codegen::Generator::new(&mut diag);
    g.type_decls = module.types.clone();
    g.resolving = vec![false; module.types.len()];
    generate::install(&mut g);
    let mut c = scope::Context::default();
    g.loop_depth = 1;
    let jsx = ast::Jsx {
        loc: 0,
        name: b"Row".to_vec(),
        attrs: Vec::new(),
        children: Vec::new(),
    };
    let component = components::Component {
        name: "Row".to_owned(),
        builder: "Row::build".to_owned(),
        func: module.functions[0].func.clone(),
        props: None,
        param: String::new(),
        destructure: Vec::new(),
        destructure_defaults: Vec::new(),
        external: false,
        stateful: true,
    };
    let error = components::emit_call(&mut g, &mut c, &jsx, &component).unwrap_err();
    let diag = diag.expect("a diagnostic");
    assert_eq!(codegen::Error::Tsx, error);
    assert!(
        diag.msg.contains("give each one in a list a key"),
        "{}",
        diag.msg
    );
}

// ── declarations ──────────────────────────────────────────────────────

#[test]
fn interface_and_enum_declarations_are_emitted() {
    let module = parse_module(
        "type Mode = \"a\" | \"in-progress\";\ninterface Row { title: string; done: boolean }\n",
    );
    let options = generate::Options {
        source_name: "types.tsx",
        require_root: false,
        ..generate::Options::default()
    };
    let file = generate::generate("types.tsx", &module, &options).expect("types generate");
    let source = &file.source;
    assert!(
        source.contains("#[derive(Clone, Copy, Default, PartialEq, Eq)]\npub enum Mode {"),
        "{source}"
    );
    assert!(source.contains("InProgress,"), "{source}");
    assert!(
        source.contains("Mode::InProgress => \"in-progress\","),
        "{source}"
    );
    assert!(
        source.contains("#[derive(Clone, PartialEq, Default)]\npub struct Row {"),
        "{source}"
    );
    assert!(source.contains("pub title: zant::Str,"), "{source}");
    assert!(source.contains("pub done: bool,"), "{source}");
}

// ── store diagnostics (ported from `tools/tsx/codegen/stores_test.zig`) ──

fn diagnostic(src: &str) -> String {
    let module = parse_module(src);
    if generate::generate("src/App.tsx", &module, &generate::Options::default()).is_err() {
        return generate::last_diag()
            .map(|diag| diag.msg)
            .unwrap_or_else(|| "codegen failed without a diagnostic".to_owned());
    }
    panic!("expected a diagnostic");
}

#[test]
fn store_diagnostics() {
    let decl = "const settings = createStore<{ n: number }>({ n: 0 });\n";
    let app = "export default function App() {\n";
    let cases = [
        (
            format!("const s = createStore({{ n: 0 }});\n{app}  return <View/>;\n}}"),
            "createStore takes the state type and its initial value",
        ),
        (
            format!("{decl}{app}  const v = useStore(42);\n  return <View/>;\n}}"),
            "useStore needs a store",
        ),
        (
            format!("{decl}{app}  settings.set({{ n: 1 }});\n  return <View/>;\n}}"),
            "stores change in event handlers and effects",
        ),
        (
            format!("{decl}{app}  return <Button onClick={{() => settings.reset()}}/>;\n}}"),
            "a store has get(), set(value) and update",
        ),
        (
            format!(
                "{decl}{app}  return <Button onClick={{() => settings.update({{ m: 1 }})}}/>;\n}}"
            ),
            "has no field `m`",
        ),
        (
            format!(
                "{decl}{app}  return <Button onClick={{() => settings.update({{ ...settings.get() }})}}/>;\n}}"
            ),
            "list the fields to change",
        ),
        (
            format!(
                "{decl}{app}  if (true) {{\n    const v = useStore(settings);\n  }}\n  return <View/>;\n}}"
            ),
            "hooks must be called at the top level",
        ),
    ];
    for (source, expected) in cases {
        let message = diagnostic(&source);
        assert!(
            message.contains(expected),
            "missing `{expected}` in `{message}` for:\n{source}"
        );
    }
}
