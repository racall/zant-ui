//! State, refs, memos, effects, stores, custom hooks, stateful children and declarations.

use super::*;

pub(super) const STATE: &str = r#"
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

pub(super) const ARRAYS: &str = r#"
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

// ── custom hooks (already expanded by `hooks::expand`) and child state ─

#[test]
fn a_custom_hook_expanded_into_usestate_lowers_like_state() {
    // `hooks::expand` inlines `useCounter` in place; the expansion is plain
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
    // handlers read the state of the last render (`_render_count`, set by
    // `render`), like React's per-render state
    assert!(
        source.contains("let count: u32 = self._render_count;"),
        "{source}"
    );
    assert!(
        source.contains("self._render_count = self.count;"),
        "{source}"
    );
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
        type_args: Vec::new(),
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
        source.contains(
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]\npub enum Mode {"
        ),
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

// ── store diagnostics ────────────────────────────────────────────────

pub(super) fn diagnostic(src: &str) -> String {
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
