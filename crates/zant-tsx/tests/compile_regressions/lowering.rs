//! Lowering checks: effect cleanup, struct props, window setters, platform constants, captures.

use super::*;

/// A child's effect cleanup also runs after the child unmounted (React):
/// the cleanup field is a `tsx::Cleanup` (dropping it queues the cleanup)
/// and the cleanup action falls back to a default instance.
pub(super) const EFFECT_CLEANUP_UNMOUNT: &str = r#"
import { useState, useEffect, View, Text, Button } from "zant";
function Ticker() {
  const [ticks, setTicks] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTicks((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, []);
  return <Text>{ticks}</Text>;
}
export default function App() {
  const [shown, setShown] = useState(true);
  return <View>{shown && <Ticker />}<Button onClick={() => setShown(!shown)}>toggle</Button></View>;
}
"#;

#[test]
fn an_effect_cleanup_runs_after_its_component_unmounts() {
    let generated = compile("effect_cleanup_unmount", EFFECT_CLEANUP_UNMOUNT);
    contains(&generated, ": tsx::Cleanup,");
    contains(&generated, ".set(ui::Action::new(");
    contains(&generated, "let mut _unmounted = Ticker::default();");
}

/// Struct props: `labels={{ … }}` on `Markdown` / `PdfViewer` lowers to the
/// Rust `Labels` with the omitted fields left to the (locale) defaults.
pub(super) const STRUCT_LABELS: &str = r#"
import { useState, View, Markdown, PdfViewer } from "zant";

export default function App() {
  const [copy, setCopy] = useState("Kopieren");
  return <View>
    <Markdown source="Hello" labels={{ copy: copy, download: "Herunterladen" }} />
    <PdfViewer src="" labels={{ search: `Suche ${copy.length}` }} />
  </View>;
}
"#;

#[test]
fn struct_props_lower_to_the_labels_struct() {
    let generated = compile("struct_labels", STRUCT_LABELS);
    contains(
        &generated,
        "labels: ui::MarkdownLabels { copy: self.copy.clone(), download: zant::Str::from(\"Herunterladen\"), ..Default::default() }",
    );
    contains(&generated, "labels: ui::PdfLabels { search:");
    for (source, expected) in [
        (
            r#"<Markdown source="x" labels="Copy" />"#,
            "labels must be an object literal",
        ),
        (
            r#"<Markdown source="x" labels={{ kopieren: "x" }} />"#,
            "labels: unknown key `kopieren`; valid keys: copy, copyMarkdown",
        ),
        (
            r#"<PdfViewer src="" labels={{ search: 1 }} />"#,
            "in labels.search",
        ),
    ] {
        let module = format!(
            "import {{ Markdown, PdfViewer }} from \"zant\";\nexport default function App() {{ return {source}; }}\n"
        );
        let error = zant_tsx::compile::compile(&module, "labels.tsx").unwrap_err();
        assert!(error.msg.contains(expected), "{source}: {}", error.msg);
    }
}

pub(super) const MODULES: &[(&str, &str)] = &[
    ("struct_labels", STRUCT_LABELS),
    ("effect_cleanup_unmount", EFFECT_CLEANUP_UNMOUNT),
    (
        "callback_defaults",
        include_str!("../../testdata/tsx/callback_defaults.tsx"),
    ),
    ("local_destructure_rename", LOCAL_DESTRUCTURE_RENAME),
    ("child_callbacks", CHILD_CALLBACKS),
    ("props_object_callbacks", PROPS_OBJECT_CALLBACKS),
    ("optional_node", OPTIONAL_NODE),
    ("rich_text_tail", RICH_TEXT_TAIL),
    ("list_in_child", LIST_IN_CHILD),
    ("optional_children", OPTIONAL_CHILDREN),
    ("destructured_capture", DESTRUCTURED_CAPTURE),
    ("generic_object_literals", GENERIC_OBJECT_LITERALS),
    ("math_spread", MATH_SPREAD),
    ("platform_and_clocks", PLATFORM_AND_CLOCKS),
    ("window_setters", WINDOW_SETTERS),
];

/// The window setters of `mainWindow` and of a `createWindow` handle.
pub(super) const WINDOW_SETTERS: &str = r#"import { useState, createWindow, mainWindow, View, Text, Button } from "zant";
function Panel() {
  return <Text>panel</Text>;
}
const panel = createWindow(Panel, { title: "Panel", width: 320, height: 240 });
export default function App() {
  const [name, setName] = useState("untitled.md");
  const open = () => {
    mainWindow.setTitle(`${name} — Notes`);
    mainWindow.setMinSize(480, 320);
    mainWindow.setMaxSize(0, 0);
    mainWindow.setSize(900, 640);
    mainWindow.setFullscreen(!mainWindow.fullscreen());
    panel.setTitle(name);
    panel.setSize(400, 300);
    panel.setMinSize(200, 150);
    panel.setMaxSize(800, 0);
    panel.setAlwaysOnTop(true);
    panel.setFullscreen(panel.fullscreen());
    setName("b.md");
  };
  return <View><Button onClick={open}>open</Button></View>;
}
"#;

#[test]
fn window_setters_lower_to_the_runtime() {
    let generated = compile("window_setters", WINDOW_SETTERS);
    contains(&generated, "tsx::platform::main_window::set_title(");
    contains(&generated, "tsx::platform::main_window::set_fullscreen(");
    contains(&generated, ".set_max_size(800.0f64, 0.0f64)");
    contains(&generated, ".set_always_on_top(true)");
    let error = zant_tsx::compile::compile(
        &WINDOW_SETTERS.replace("panel.setSize(400, 300)", "panel.setSize(400)"),
        "window_setters.tsx",
    )
    .unwrap_err();
    assert!(
        error
            .msg
            .contains("setSize(width, height) takes two numbers"),
        "{}",
        error.msg
    );
}

/// `isMacos` / `isWindows` and the two clocks, read where they are used.
pub(super) const PLATFORM_AND_CLOCKS: &str = r#"import { useState, View, Text, Button, isMacos, isWindows } from "zant";
export default function App() {
  const [started, setStarted] = useState(0);
  const shortcut = isMacos ? "⌘" : isWindows ? "Ctrl" : "Ctrl";
  const elapsed = performance.now() - started;
  return (
    <View>
      <Text>{`${shortcut} ${elapsed}`}</Text>
      <Button onClick={() => setStarted(Date.now())}>start</Button>
    </View>
  );
}
"#;

#[test]
fn platform_constants_and_clocks_lower_to_the_runtime() {
    let generated = compile("platform_and_clocks", PLATFORM_AND_CLOCKS);
    contains(&generated, "zant::tsx::platform::IS_MACOS");
    contains(&generated, "zant::tsx::platform::IS_WINDOWS");
    contains(&generated, "tsx::builtins::date_now()");
    contains(&generated, "tsx::builtins::performance_now()");
    // a clock in a top-level constant would be read once, at startup
    let error = zant_tsx::compile::compile(
        "import { View } from \"zant\";\nconst T = Date.now();\nexport default function App() { return <View />; }\n",
        "clock_constant.tsx",
    )
    .unwrap_err();
    assert!(
        error
            .msg
            .contains("clocks cannot run in a top-level constant"),
        "{}",
        error.msg
    );
}

/// `Math.max(...xs)` over integer and float arrays, mixed with plain
/// arguments (the values are compared to Node by `node_parity`).
pub(super) const MATH_SPREAD: &str = r#"import { View, Text } from "zant";
import type { u32, i64 } from "zant";
export default function App() {
  const counts: u32[] = [3, 1, 2];
  const deltas: i64[] = [];
  const widths = [1.5, 2.5];
  const top = Math.max(...counts) + Math.min(0, ...deltas) + Math.hypot(...widths, 2);
  return <Text>{`${top}`}</Text>;
}
"#;

/// A generic function's type parameter inferred from an object literal
/// argument (alone, in an array, nested), as `const p = { a: 1 }` infers.
pub(super) const GENERIC_OBJECT_LITERALS: &str = r#"import { View, Text } from "zant";
function id<T>(x: T): T { return x; }
function first<T>(xs: T[]): T | null { return xs.length > 0 ? xs[0] : null; }
function pair<A, B>(a: A, b: B): [A, B] { return [a, b]; }
export default function App() {
  const a = id({ a: 1, tag: "x" });
  const b = first([{ n: 1 }, { n: 2 }]);
  const c = id({ inner: { deep: true } });
  const d = pair(1, { label: "y" });
  return <Text>{`${a.a}${a.tag}${b?.n ?? 0}${c.inner.deep}${d[1].label}`}</Text>;
}
"#;

/// Render locals bound by destructuring (`const { name } = user`,
/// `const [first, second] = pair`) are captured by named handlers like
/// `const name = user.name` is: a `function`, a `const` arrow, a named
/// function calling another, and a handler passed to a child component.
pub(super) const DESTRUCTURED_CAPTURE: &str = r#"import { useState, View, Text, Button } from "zant";
interface User { name: string; age: number }
function Child({ onGo }: { onGo: () => void }) {
  return <Button onClick={onGo}>go</Button>;
}
export default function App() {
  const [user, setUser] = useState<User>({ name: "a", age: 1 });
  const [pair, setPair] = useState<[string, number]>(["b", 2]);
  const [text, setText] = useState("");
  const { name, age: years } = user;
  const [first, second] = pair;
  function byName() { setText(name); }
  const byTuple = () => { setText(`${first}${second}`); };
  function both() { byName(); setText(`${text}${years}`); }
  return (
    <View>
      <Button onClick={byName}>name</Button>
      <Button onClick={byTuple}>tuple</Button>
      <Button onClick={both}>both</Button>
      <Button onClick={() => setText(first)}>inline</Button>
      <Child onGo={byTuple} />
      <Text>{text}</Text>
    </View>
  );
}
"#;

#[test]
fn named_handlers_capture_destructured_render_locals() {
    compile("destructured_capture", DESTRUCTURED_CAPTURE);
    // a hook's destructured setter stays a method, not a render capture: the
    // handler reads the current state (`self.draft`), not this frame's copy
    let generated = compile(
        "setter_method",
        r#"import { useState, View, Button } from "zant";
export default function App() {
  const [draft, setDraft] = useState("");
  function clear() { setDraft(""); }
  return <View><Button onClick={clear}>{draft}</Button></View>;
}
"#,
    );
    contains(&generated, "fn clear(&mut self)");
}
