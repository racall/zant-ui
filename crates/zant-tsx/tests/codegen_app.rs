//! WP-5.6 end-to-end tests: the application layer of the TSX → Rust compiler
//! (imports, windows, desktop hooks, app hooks, `zant` API, assets, entry,
//! project compilation) and the compiler CLI's project pipeline.
//!
//! The JSX / event emitters are WP-5.4's modules and have not landed in this
//! worktree (`crates/zant-tsx/src/codegen/jsx.rs` / `events.rs`). Like WP-5.5's
//! tests, these tests inject a small **test-only** stand-in through
//! `generate::generate_with(.., install_extra)`:
//!
//! - it emits `ui::text("stub", …)` for every element, so a module's render
//!   body generates and the app-level lowering around it is exercised, and
//! - it registers inline event arrows / setters the way `events.zig` does.
//!
//! Everything this work package owns — project imports and their Rust paths,
//! `createWindow` handles, tray / hotkey lowering, app hooks as `impl ui::App`
//! methods, `zant` platform calls, asset `include_bytes!` constants and the
//! generated entry — is real generated code. The render bodies themselves are
//! covered by WP-5.4's own golden tests once its emitter lands.

use zant_tsx::ast;
use zant_tsx::codegen::{self, scope};
use zant_tsx::compile::{self, SourceFile};
use zant_tsx::entry;
use zant_tsx::tags;

// ── the test-only JSX emitter ─────────────────────────────────────────

const STUB_NODE: &str = "ui::text(\"stub\", ui::TextOptions::default())";

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn stub_jsx(
    g: &mut codegen::Generator<'_>,
    c: &mut scope::Context,
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
            // callback props (`onPick`, `onSelect`, …) are WP-5.4's prop
            // lowering; don't try to evaluate their arrow functions here
            if text(&attr.name.name).starts_with("on") {
                continue;
            }
            let _ = codegen::expr::emit(g, c, &value, None, false)?;
            continue;
        };
        register_handler(g, &value, kind)?;
    }
    for child in &j.children {
        match child {
            ast::Child::Element(element) => {
                stub_jsx(g, c, element)?;
            }
            ast::Child::Expr(Some(value)) => {
                let _ = codegen::expr::emit(g, c, value, None, false)?;
            }
            _ => {}
        }
    }
    Ok(codegen::types::Value::new(
        codegen::types::Type::Node,
        STUB_NODE,
    ))
}

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

fn stub_setter(g: &mut codegen::Generator<'_>, m: &codegen::Method) -> codegen::Result<String> {
    let state = g.states[m.setter.expect("setter")].clone();
    let field = scope::state_field(g, state.name.as_bytes())?;
    let value = "value".to_owned();
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

// ── helpers ───────────────────────────────────────────────────────────

/// Compiles one file of a project with the JSX stub (`src/App.tsx`).
fn generated_in_project(
    files: &[SourceFile],
    which: usize,
) -> Result<String, compile::ProjectError> {
    let project = compile::compile_project_with(files, Some(install_stubs))?;
    Ok(project.outputs[which].clone())
}

fn generated(src: &str) -> String {
    generated_in_project(&[SourceFile::tsx("src/App.tsx", src)], 0)
        .unwrap_or_else(|error| panic!("codegen error at {}: {}", error.diag.loc, error.diag.msg))
}

/// The sections of a generated source, for readable assertions.
fn contains(source: &str, expected: &str) {
    assert!(
        source.contains(expected),
        "missing `{expected}` in:\n{source}"
    );
}

/// Compiles `src` (with a `~` marking the expected diagnostic byte) and
/// asserts the diagnostic message and position, like the Zig error tests.
#[track_caller]
fn expect_diagnostic(marked: &str, message: &str) {
    let at = marked.find('~').expect("`~` marker");
    let src = format!("{}{}", &marked[..at], &marked[at + 1..]);
    // the JSX stub lets the render body generate, so diagnostics in handler
    // bodies (which WP-5.4's `scan_body` emits) are reached too
    let files = [SourceFile::tsx("error.tsx", &src)];
    let diag = match compile::compile_project_with(&files, Some(install_stubs)) {
        Ok(_) => panic!("expected TSX error containing: {message}\nsource: {src}"),
        Err(error) => error.diag,
    };
    assert!(
        diag.msg.contains(message),
        "expected diagnostic: {message}\nactual diagnostic: {}\nsource: {src}",
        diag.msg
    );
    let expected = zant_tsx::error::position(&src, at as u32);
    let actual = zant_tsx::error::position(&src, diag.loc);
    assert_eq!(
        (expected.line, expected.column),
        (actual.line, actual.column),
        "diagnostic position: {}\nsource: {src}",
        diag.msg
    );
}

const APP: &str = "export default function App() {\n";
const END: &str = "  return <View/>;\n}\n";

// ── imports across project files ──────────────────────────────────────

const PROJECT_TESTDATA: &str = "\
export interface Todo { id: u32; title: string }
export interface RowProps { todo: Todo; onPick: (id: u32) => void }
export function Row({ todo, onPick }: RowProps) {
  return <Button onClick={() => onPick(todo.id)}>{todo.title}</Button>;
}
";

#[test]
fn a_project_resolves_components_and_types_across_files() {
    let files = [
        SourceFile::tsx("src/Row.tsx", PROJECT_TESTDATA),
        SourceFile::tsx(
            "src/App.tsx",
            "import { useState } from \"zant\";\n\
             import { Row, type Todo } from \"./Row\";\n\
             export default function App() {\n\
             const [todo, setTodo] = useState<Todo>({ id: 1, title: \"a\" });\n\
             const [picked, setPicked] = useState<u32>(0);\n\
             return <Row todo={todo} onPick={(id) => setPicked(id)} />;\n\
             }\n",
        ),
    ];
    let result = generated_in_project(&files, 1).expect("project compiles");
    // Row.rs declares its types and the component, App.rs imports them by path
    let row = generated_in_project(&files, 0).expect("row compiles");
    contains(&row, "pub struct Todo");
    contains(&row, "pub fn build");
    // the imported type resolves to the declaring module (`imports.named`)
    contains(&result, "crate::row::Todo");
}

#[test]
fn imports_name_exported_things_of_project_files() {
    let files = [
        SourceFile::tsx(
            "src/Row.tsx",
            "function Hidden(p: { a: string }) { return <View/>; }",
        ),
        SourceFile::tsx(
            "src/App.tsx",
            "import { Hidden } from \"./Row\"; export default function App() { return <View/>; }",
        ),
    ];
    let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
    assert_eq!(1, error.file);
    assert_eq!("`Hidden` is not exported by `./Row`", error.diag.msg);
}

#[test]
fn an_import_of_a_missing_project_file_is_a_diagnostic() {
    let files = [SourceFile::tsx(
        "src/App.tsx",
        "import { Row } from \"./Missing\"; export default function App() { return <View/>; }",
    )];
    let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
    assert!(
        error
            .diag
            .msg
            .contains("cannot find `./Missing` among the project's .tsx files"),
        "{}",
        error.diag.msg
    );
}

// ── windows ──────────────────────────────────────────────────────────

const WINDOWS: &str = r##"
import { useState, createWindow, useKeyDown, useWindowFocus, useCloseRequest, View, Text, Button } from "zant";
import type { KeyEvent } from "zant";

function Panel() {
  const [shown, setShown] = useState(0);
  useWindowFocus((focused: boolean) => {
    if (!focused) panel.hide();
  });
  useCloseRequest(() => {
    panel.hide();
    return false;
  });
  useKeyDown((e: KeyEvent) => {
    if (e.key === "Escape") {
      panel.hide();
      return true;
    }
    return false;
  });
  return (
    <View padding={12}>
      <Button key="count" onClick={() => setShown(shown + 1)}>
        Clicked {shown}
      </Button>
    </View>
  );
}

export const panel = createWindow(Panel, {
  title: "Panel",
  style: "panel",
  width: 380,
  height: 480,
  minWidth: 380,
  resizable: false,
  background: "#ffffff",
});

export default function App() {
  return (
    <View padding={16} gap={8}>
      <Text key="visible">{panel.visible() ? "open" : "closed"}</Text>
      <Button key="toggle" onClick={() => panel.toggle()}>Toggle</Button>
      <Button key="corner" onClick={() => { panel.place("trayCorner", 12); panel.activate(); panel.focus("count"); }}>
        By the tray
      </Button>
    </View>
  );
}
"##;

#[test]
fn a_window_becomes_a_thread_local_handle_and_a_window_component() {
    let source = generated(WINDOWS);
    // the `createWindow` constant: a lazy slot plus accessor
    contains(&source, "static PANEL");
    contains(&source, "tsx::window::Window < PanelWindow");
    contains(&source, "pub fn panel_with<R>(");
    contains(&source, "zant::desktop::OpenOptions {");
    contains(&source, "title: \"Panel\".into()");
    contains(&source, "style: zant::desktop::WindowStyle::Panel");
    contains(&source, "width: 380.0f32");
    contains(&source, "resizable: false");
    contains(&source, "background: 0xffffffff");
    // the component renders as the window's component, hooks included
    contains(&source, "pub struct PanelWindow");
    contains(&source, "impl tsx::window::Component for PanelWindow");
    contains(&source, "self._s.borrow_mut().render()");
    contains(
        &source,
        "fn on_key(&mut self, event: &tsx::KeyEvent) -> bool",
    );
    contains(&source, "fn on_focus(&mut self, focused: bool)");
    contains(&source, "fn on_close_request(&mut self) -> bool");
    // call sites go through the accessor
    contains(&source, "panel_with(|window| window.toggle())");
    contains(
        &source,
        "panel_with(|window| window.place(zant::desktop::Place::TrayCorner, 12.0f32))",
    );
    contains(&source, "panel_with(|window| window.activate())");
    contains(&source, "window.focus(\"count\".into().as_str())");
}

#[test]
fn window_diagnostics() {
    let panel = "function Panel() { return <View/>; }\n";
    let app = "export default function App() { return <View/>; }\n";
    let cases = [
        (
            format!(
                "function Row({{ n }}: {{ n: number }}) {{ return <View/>; }}\nconst w = createWindow(~Row, {{}});\n{app}"
            ),
            "a window's component takes no props (`Row` needs `n`)",
        ),
        (
            format!("const w = createWindow(~Missing, {{}});\n{app}"),
            "`Missing` is not a component of this file or imported",
        ),
        (
            format!("{panel}const w = createWindow(Panel, {{ ~size: 3 }});\n{app}"),
            "unknown window option `size`",
        ),
        (
            format!("{panel}const w = createWindow(Panel, {{ style: ~\"floating\" }});\n{app}"),
            "style is one of: native",
        ),
        (
            format!("{panel}const w = ~createWindow();\n{app}"),
            "createWindow takes a component and options",
        ),
        (
            format!(
                "{panel}const w = createWindow(Panel, {{}});\nexport default function App() {{\n  return <Button onClick={{() => w.place(~\"middle\")}}/>;\n}}\n"
            ),
            "the place is one of: center",
        ),
        (
            format!(
                "{panel}const w = createWindow(Panel, {{}});\nexport default function App() {{\n  return <Button onClick={{() => w.~close()}}/>;\n}}\n"
            ),
            "a window has show, hide, activate, toggle, visible, active, minimized, place and focus",
        ),
    ];
    for (marked, message) in cases {
        expect_diagnostic(&marked, message);
    }
}

// ── desktop hooks ────────────────────────────────────────────────────

const DESKTOP: &str = r#"
import { useState, useTray, useHotkey, useClipboardChange, useReopen, View, Text } from "zant";
import trayIcon from "./assets/tray.png";

export default function App() {
  const [sync, setSync] = useState(true);
  const [hotkey, setHotkey] = useState("ctrl+cmd+v");
  const [clips, setClips] = useState(0);
  const label = sync ? "Pause sync" : "Resume sync";
  const shown = useTray({
    icon: trayIcon,
    template: true,
    tooltip: "Clips",
    items: [
      { title: "Open", onSelect: () => setClips(0) },
      { separator: true },
      { title: label, checked: sync, onSelect: () => setSync(!sync) },
    ],
    onClick: () => setClips(clips + 100),
  });
  const ok = useHotkey(hotkey, () => setClips(clips + 1));
  useClipboardChange(() => setClips(clips + 1));
  useReopen(() => setHotkey(""));
  return (
    <View padding={16}>
      <Text key="t">{clips} {ok ? "hotkey" : "none"} {shown ? "tray" : "no tray"}</Text>
    </View>
  );
}
"#;

#[test]
fn desktop_hooks_lower_to_tsx_desktop() {
    let source = generated(DESKTOP);
    // `useTray` → `tsx::desktop::update(TrayOptions { … })`
    contains(&source, "tsx::desktop::update(tsx::desktop::TrayOptions {");
    contains(&source, "template: true");
    contains(&source, "tooltip: \"Clips\".into()");
    contains(&source, "tsx::desktop::TrayItem {");
    contains(&source, "on_select");
    contains(&source, "on_click: Some(ui::action(");
    // `useHotkey` → a hidden `Hotkey` field updated while rendering
    contains(&source, "_hotkey");
    contains(&source, ": tsx::desktop::Hotkey,");
    contains(&source, ".update(");
    // the callbacks are registered handlers with captures
    contains(&source, "_desktop");
}

#[test]
fn desktop_hook_diagnostics() {
    let icon = "import icon from \"./tray.png\";\n";
    let cases = [
        (
            format!("{APP}  useTray(~{{ tooltip: \"x\" }});\n{END}"),
            "the tray needs an `icon`",
        ),
        (
            format!("{icon}{APP}  useTray({{ icon: icon, ~menu: [] }});\n{END}"),
            "unknown tray option `menu`",
        ),
        (
            format!("{icon}{APP}  useTray({{ icon: icon, items: [{{ ~label: \"x\" }}] }});\n{END}"),
            "unknown tray item field `label`",
        ),
        (
            format!("{APP}  ~useHotkey(\"cmd+k\");\n{END}"),
            "useHotkey(shortcut, () => …)",
        ),
        (
            format!("{APP}  useHotkey(\"cmd+k\", ~(x: number) => {{}});\n{END}"),
            "this callback takes no parameters",
        ),
        (
            format!(
                "function Row({{ n }}: {{ n: number }}) {{\n  ~useClipboardChange(() => {{}});\n  return <View/>;\n}}\n{APP}  return <Row n={{1}}/>;\n}}\n"
            ),
            "useClipboardChange belongs to the root component",
        ),
        (
            format!("{APP}  if (true) {{\n    ~useReopen(() => {{}});\n  }}\n{END}"),
            "hooks must be called at the top level",
        ),
    ];
    for (marked, message) in cases {
        expect_diagnostic(&marked, message);
    }
}

// ── app hooks ────────────────────────────────────────────────────────

const APP_HOOKS: &str = r#"
import { useState, useCloseRequest, useKeyDown, useWake, View, Text } from "zant";
import type { KeyEvent } from "zant";

export default function App() {
  const [dirty, setDirty] = useState(true);
  const [asked, setAsked] = useState(0);
  const [last, setLast] = useState("");
  useCloseRequest(() => {
    if (dirty) {
      setAsked(asked + 1);
      return false;
    }
    return true;
  });
  useKeyDown((e: KeyEvent) => {
    setLast(e.key);
    if (e.key === "Escape") {
      setDirty(false);
      return true;
    }
    return e.mod && e.key === "s";
  });
  return (
    <View padding={16}>
      <Text key="state">{dirty ? "dirty" : "clean"} {asked} {last}</Text>
    </View>
  );
}
"#;

#[test]
fn app_hooks_become_impl_ui_app_methods() {
    let source = generated(APP_HOOKS);
    contains(&source, "impl ui::App for App {");
    contains(&source, "fn on_close_request(&mut self) -> bool {");
    contains(
        &source,
        "fn on_key(&mut self, event: &zant::core::events::KeyEvent) -> bool {",
    );
    contains(&source, "let e = tsx::app::key_event(event);");
    // the hook bodies are `_root_*` methods that the trait methods forward to
    contains(&source, "fn _root_on_close_request(&mut self) -> bool {");
    contains(
        &source,
        "fn _root_on_key(&mut self, e: tsx::KeyEvent) -> bool {",
    );
    // the KeyEvent fields read like the DOM's (`mod` maps to the runtime's r#mod)
    contains(&source, "r#mod");
}

#[test]
fn app_hook_diagnostics() {
    let cases = [
        (
            format!(
                "function Child({{ n }}: {{ n: number }}) {{\n  ~useCloseRequest(() => true);\n  return <View/>;\n}}\n{APP}  return <Child n={{1}}/>;\n}}\n"
            ),
            "useCloseRequest belongs to the root component or a window's component",
        ),
        (
            format!("{APP}  ~useKeyDown(1);\n{END}"),
            "useKeyDown takes one function",
        ),
        (
            format!("{APP}  useCloseRequest(~(x: number) => true);\n{END}"),
            "the useCloseRequest callback takes no parameters",
        ),
        (
            format!("{APP}  useQuitRequest(() => true);\n  ~useQuitRequest(() => false);\n{END}"),
            "useQuitRequest is used twice",
        ),
        (
            format!("{APP}  if (true) {{\n    ~useKeyDown((e: KeyEvent) => false);\n  }}\n{END}"),
            "hooks must be called at the top level",
        ),
        (
            format!("{APP}  useCloseRequest(~() => {{ if (true) return false; }});\n{END}"),
            "function must return a value on every path",
        ),
    ];
    for (marked, message) in cases {
        expect_diagnostic(&marked, message);
    }
}

// ── the `zant` API ───────────────────────────────────────────────────

const PLATFORM: &str = r#"
import { useState, clipboard, app, mainWindow, wakeAfter, keyLabel, isMacos, titlebarInsets, View, Row, Text, Button, Input } from "zant";

export default function App() {
  const [draft, setDraft] = useState("");
  const [copied, setCopied] = useState(0);
  const [until, setUntil] = useState(0);
  const insets = titlebarInsets();
  const pad = insets.left + insets.right;
  const now = performance.now();
  const _pad = pad;
  const toast = now < until;
  if (toast) wakeAfter(until - now);
  const hint = `Search ${keyLabel("mod+f")}`;
  const copy = () => {
    if (clipboard.setText(draft)) {
      setCopied(clipboard.changeCount());
      setUntil(performance.now() + 1800);
    }
  };
  return (
    <View padding={16} gap={8}>
      <Row key="bar">
        <Text key="os">{app.deviceName()}</Text>
      </Row>
      <Input key="draft" value={draft} placeholder={hint} onChange={setDraft} />
      <Row key="actions" gap={8}>
        <Button key="copy" onClick={() => { clipboard.setText(draft); setCopied(clipboard.changeCount()); }}>Copy</Button>
        <Button key="hide" onClick={() => mainWindow.hide()}>Hide</Button>
      </Row>
      <Text key="toast">Copied #{copied} · {app.dataDir() ?? "no data dir"}</Text>
    </View>
  );
}
"#;

#[test]
fn zant_api_names_map_to_tsx_platform() {
    let source = generated(PLATFORM);
    contains(&source, "tsx::platform::clipboard::set_text(");
    contains(&source, "tsx::platform::clipboard::change_count()");
    contains(&source, "tsx::platform::main_window::hide()");
    contains(&source, "tsx::platform::wake_after(");
    contains(&source, "tsx::platform::key_label(\"mod+f\")");
    contains(&source, "ui::titlebar_insets()");
}

#[test]
fn zant_api_diagnostics() {
    let import = "import { clipboard, app, mainWindow, keyLabel, wakeAfter } from \"zant\";\n";
    let cases = [
        (
            format!("{import}{APP}  const p = clipboard.~paste();\n{END}"),
            "unknown function `clipboard.paste`",
        ),
        (
            format!("{import}{APP}  const on = ~app.setLaunchAtLogin(true);\n{END}"),
            "`setLaunchAtLogin` changes the desktop",
        ),
        (
            format!("{import}{APP}  const c = ~clipboard;\n{END}"),
            "`clipboard` is a namespace",
        ),
        (
            format!("{import}{APP}  const t = ~clipboard.text(1);\n{END}"),
            "`text` takes 0 argument(s), got 1",
        ),
        (
            format!("{import}{APP}  const k = \"f\";\n  const l = ~keyLabel(k);\n{END}"),
            "keyLabel takes a string literal",
        ),
        (
            format!("{import}const t = ~clipboard.text();\n{APP}{END}"),
            "cannot run in a top-level constant",
        ),
    ];
    for (marked, message) in cases {
        expect_diagnostic(&marked, message);
    }
}

// ── assets ───────────────────────────────────────────────────────────

#[test]
fn an_asset_import_embeds_the_bytes() {
    let files = [SourceFile::tsx(
        "src/App.tsx",
        "import logo from \"./assets/logo.png\";\n\
         import { assetImage } from \"zant\";\n\
         export default function App() { return <Image image={assetImage(logo)} />; }\n",
    )];
    let project = compile::compile_project_with(&files, Some(install_stubs));
    // `assetImage` needs WP-5.4's `Image` type; only the constant is checked here
    match project {
        Ok(project) => {
            contains(&project.outputs[0], "const logo: &'static [u8]");
            contains(&project.outputs[0], "include_bytes!(\"_assets/");
            assert_eq!(1, project.assets.len());
            assert!(project.assets[0].key.ends_with("-logo.png"));
        }
        Err(error) => {
            assert!(
                error.diag.msg.contains("Image"),
                "unexpected error: {}",
                error.diag.msg
            );
        }
    }
}

#[test]
fn asset_imports_without_a_project_build_are_a_diagnostic() {
    let error = compile::compile(
        "import logo from \"./assets/logo.png\";\nexport default function App() { return <View/>; }",
        "App.tsx",
    )
    .unwrap_err();
    assert!(
        error.msg.contains("asset imports need a project build"),
        "{}",
        error.msg
    );
}

#[test]
fn asset_imports_are_listed_in_the_dep_file() {
    let files = [SourceFile::tsx(
        "src/App.tsx",
        "import logo from \"./assets/logo.png\";\nexport default function App() { return <View/>; }",
    )];
    let project = compile::compile_project_with(&files, Some(install_stubs)).expect("compiles");
    assert_eq!(1, project.assets.len());
    let dep = zant_tsx::assets::dep_file("out", &["src/App.tsx".to_owned()], &project.assets);
    assert!(dep.starts_with("out: \\\n  src/App.tsx \\\n"), "{dep}");
    assert!(dep.contains("logo.png\n"), "{dep}");
}

// ── hand-written Rust files (`zig_imports` → `rust_imports`) ─────────

const RUST_SOURCE: &str = "\
// Keep this file byte for byte.
pub fn plural(n: usize, word: &str) -> zant::Str { word.into() }
pub fn clamp(n: f32) -> f32 { n }
pub fn record(n: usize) {}
pub fn values(xs: Vec<Option<i32>>) -> Vec<Option<i32>> { xs }
pub fn words(xs: Vec<zant::Str>) -> Vec<zant::Str> { xs }
pub fn maybe(n: Option<usize>) -> Option<usize> { n }
pub fn bad(x: Foo) {}
fn hidden() {}
pub const value: u8 = 1;
";

#[test]
fn rust_functions_are_imported_with_their_signatures() {
    let files = [
        SourceFile::rust("src/format.rs", RUST_SOURCE),
        SourceFile::tsx(
            "src/App.tsx",
            "import { plural as label, clamp } from \"./format.rs\";\n\
             import { record } from \"./format.rs\";\n\
             export default function App() {\n\
             const title = label(3, \"note\");\n\
             function save() { record(1); }\n\
             return <View><Text>{title}</Text><Button width={clamp(24)} onClick={save}>Save</Button></View>;\n\
             }\n",
        ),
    ];
    let project = compile::compile_project_with(&files, Some(install_stubs)).expect("compiles");
    // the Rust file is passed through verbatim
    assert_eq!(RUST_SOURCE, project.outputs[0]);
    contains(&project.outputs[1], "crate::format::plural(");
    contains(&project.outputs[1], "crate::format::record(");
}

#[test]
fn rust_import_diagnostics_point_at_imported_names() {
    let app = " export default function App() { return <View/>; }";
    let cases = [
        (
            "import { plurla } from \"./format.rs\";",
            "`plurla` is not a pub fn of `./format.rs`; did you mean `plural`?",
        ),
        (
            "import { hidden } from \"./format.rs\";",
            "`hidden` is not a pub fn of `./format.rs`",
        ),
        (
            "import { value } from \"./format.rs\";",
            "`value` is not a pub fn of `./format.rs`",
        ),
        (
            "import type { plural } from \"./format.rs\";",
            "is not a pub struct or pub enum",
        ),
    ];
    for (import, message) in cases {
        let files = [
            SourceFile::tsx("src/App.tsx", &format!("{import}{app}")),
            SourceFile::rust("src/format.rs", RUST_SOURCE),
        ];
        let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
        assert_eq!(0, error.file);
        assert!(
            error.diag.msg.contains(message),
            "expected `{message}`, got `{}`",
            error.diag.msg
        );
    }
}

#[test]
fn rust_import_calls_check_argument_counts_and_types() {
    let prefix =
        "import { plural } from \"./format.rs\"; export default function App() { return <Text>{";
    let suffix = "}</Text>; }";
    let cases = [
        ("plural(1)", "`plural` expects 2 arguments"),
        ("plural(true, \"note\")", "cannot convert bool to usize"),
        (
            "plural<usize>(1, \"note\")",
            "do not take TSX type arguments",
        ),
    ];
    for (call, message) in cases {
        let files = [
            SourceFile::rust("src/format.rs", RUST_SOURCE),
            SourceFile::tsx("src/App.tsx", &format!("{prefix}{call}{suffix}")),
        ];
        let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
        assert!(
            error.diag.msg.contains(message),
            "expected `{message}`, got `{}`",
            error.diag.msg
        );
    }
}

#[test]
fn rust_imports_require_project_mode() {
    let error = compile::compile(
        "import { plural } from \"./format.rs\"; export default function App() { return <View/>; }",
        "App.tsx",
    )
    .unwrap_err();
    assert_eq!(
        "Rust imports need a project (zant-tsx --out / zant_build::tsx_app)",
        error.msg
    );
}

#[test]
fn a_store_exported_by_one_file_is_shared_by_the_files_importing_it() {
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "import { prefs as shared } from \"./store\";\n\
             export default function App() {\n\
             const dark = useStore(shared, (p) => p.dark);\n\
             return <Text>{`${dark}`}</Text>;\n\
             }\n",
        ),
        SourceFile::tsx(
            "src/store.tsx",
            "export interface Prefs { dark: boolean; name: string; }\n\
             export const prefs = createStore<Prefs>({ dark: false, name: \"\" });\n",
        ),
    ];
    let project = compile::compile_project_with(&files, Some(install_stubs)).expect("compiles");
    contains(&project.outputs[1], "pub fn prefs_read()");
    // the importing file reads through the declaring module's accessor
    contains(&project.outputs[0], "crate::store::prefs_read()");
}

#[test]
fn project_rejects_invalid_rust_sources() {
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "export default function App() { return <View/>; }",
        ),
        SourceFile::rust("src/bad.rs", "pub fn broken("),
    ];
    let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
    assert_eq!(1, error.file);
    assert!(!error.diag.msg.is_empty(), "a syntax error message");
}

#[test]
fn native_module_specs_compile_to_the_generated_spec_module() {
    // WP-5.8 is wired: a spec compiles to `native/spec.rs` (one `pub mod`)
    // instead of failing with the old "implemented by WP-5.8" diagnostic.
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "export default function App() { return <View/>; }",
        ),
        SourceFile::native_spec(
            "native/clips.ts",
            "export interface Clip { row: number }\nexport declare function list(): Clip[];",
        ),
    ];
    let build = compile::compile_project_with(&files, Some(install_stubs)).expect("spec compiles");
    let spec = build
        .outputs
        .iter()
        .find(|source| source.contains("pub mod clips"))
        .expect("the spec module is generated");
    assert!(spec.contains("pub mod clips"), "{spec}");
    assert!(spec.contains("pub trait ClipsSpec"), "{spec}");
    assert!(spec.contains("fn list()"), "{spec}");
    // The project also exposes the combined root written as `native/spec.rs`.
    let root = build.spec_root.as_deref().expect("the spec root");
    assert!(root.contains("pub mod clips"), "{root}");
    assert!(root.contains("pub trait ClipsSpec"), "{root}");
    assert!(root.contains("use zant::tsx;"), "{root}");
}

#[test]
fn a_types_only_spec_omits_the_unused_tsx_import() {
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "export default function App() { return <View/>; }",
        ),
        SourceFile::native_spec(
            "native/system.ts",
            "export type Os = \"macos\" | \"windows\";\n\
             export interface SystemInfo { os: Os; cpus: u32 }\n\
             export declare function info(): SystemInfo;",
        ),
    ];
    let build = compile::compile_project_with(&files, Some(install_stubs)).expect("spec compiles");
    let root = build.spec_root.as_deref().expect("the spec root");
    assert!(root.contains("pub trait SystemSpec"), "{root}");
    // `tsx::List` does not appear in the generated code, so the import would
    // be unused (and warn in the app).
    assert!(!root.contains("use zant::tsx;"), "{root}");
}

/// `source` with every whitespace character removed (the generated code is
/// line-broken by `prettyplease`, so call-site assertions compare tokens).
fn compact(source: &str) -> String {
    source.chars().filter(|ch| !ch.is_whitespace()).collect()
}

#[test]
fn a_component_call_passes_the_props_struct_not_the_component() {
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "import { useState, View } from \"zant\";\n\
             import { Dashboard } from \"./components/Dashboard\";\n\
             import { Badge } from \"./Badge\";\n\
             export default function App() {\n\
             const [count, setCount] = useState(0);\n\
             return <View>\n\
             <Dashboard count={count} onIncrement={() => setCount(count + 1)} onReset={() => setCount(0)} />\n\
             <Badge />\n\
             </View>;\n\
             }\n",
        ),
        SourceFile::tsx(
            "src/components/Dashboard.tsx",
            "import { View } from \"zant\";\n\
             export interface DashboardProps { count: number; onIncrement: () => void; onReset: () => void }\n\
             export function Dashboard({ count, onIncrement, onReset }: DashboardProps) { return <View/>; }\n",
        ),
        SourceFile::tsx(
            "src/Badge.tsx",
            "import { View } from \"zant\";\n\
             export function Badge() { return <View/>; }\n",
        ),
    ];
    // Real JSX emitters (not the snapshot stubs): the call site is what this
    // test is about.
    let build = compile::compile_project_with(&files, None).expect("compiles");
    let app = &build.outputs[0];
    let compact_app = compact(app);
    // The props argument is the target module's props struct, not `Dashboard`.
    assert!(
        compact_app
            .contains("crate::dashboard::Dashboard::build(crate::dashboard::DashboardProps{"),
        "{app}"
    );
    // A component without props gets the unit `Props` default.
    assert!(
        compact_app.contains("crate::badge::Badge::build(Default::default())"),
        "{app}"
    );
    // A root callback for a child's `ui::Action` prop is wrapped for the root
    // state (`ui::action(|s: &mut App| …)`), like a ui event handler.
    assert!(compact_app.contains("onIncrement:ui::action("), "{app}");
    // The props struct's no-op default matches the action type.
    let dashboard = &build.outputs[1];
    assert!(
        dashboard.contains("onIncrement: ui::Action::new(|_| ())"),
        "{dashboard}"
    );
    assert!(
        dashboard.contains("onReset: ui::Action::new(|_| ())"),
        "{dashboard}"
    );
}

#[test]
fn project_output_names_collide_case_insensitively() {
    let files = [
        SourceFile::tsx(
            "src/Worker.tsx",
            "export default function Worker() { return <View/>; }",
        ),
        SourceFile::rust("src/worker.rs", "pub fn f() {}"),
    ];
    let error = compile::compile_project_with(&files, Some(install_stubs)).unwrap_err();
    assert!(
        error.diag.msg.contains("project output collision"),
        "{}",
        error.diag.msg
    );
}

// ── the entry ────────────────────────────────────────────────────────

#[test]
fn the_entry_runs_the_default_export_with_the_project_config() {
    let files = [
        SourceFile::tsx(
            "App.tsx",
            "import { useClipboardChange } from \"zant\";\n\
             export default function Clips() { useClipboardChange(() => {}); return <View/>; }\n",
        ),
        SourceFile::tsx("Row.tsx", "export function Row() { return <View/>; }"),
    ];
    let source = entry::main_source(&files, "App", None).expect("entry");
    contains(&source, "#[path = \"App.rs\"] pub mod app;");
    contains(&source, "#[path = \"Row.rs\"] pub mod row;");
    contains(&source, "zant::tsx::app::run::<app::Clips>(");
    contains(&source, "watch_clipboard: true");
    assert_eq!(
        entry::Error::NoEntry,
        entry::main_source(&files, "Main", None).unwrap_err()
    );
}

#[test]
fn the_entry_carries_app_config_json() {
    let files = [SourceFile::tsx(
        "App.tsx",
        "export default function App() { return <View/>; }",
    )];
    let config = compile::ConfigSpec {
        title: "Clips".to_owned(),
        bundle_id: "io.x.clips".to_owned(),
        single_instance: true,
        window: compile::WindowConfig {
            style: "panel".to_owned(),
            width: 380.0,
            height: 480.0,
            background: 0xff00_0000,
            center: false,
            ..compile::WindowConfig::default()
        },
    };
    let source = entry::main_source(&files, "App", Some(&config)).expect("entry");
    contains(&source, "title: \"Clips\".into()");
    contains(&source, "bundle_id: \"io.x.clips\".into()");
    contains(&source, "single_instance: true");
    contains(&source, "style: zant::desktop::WindowStyle::Panel");
    contains(&source, "width: 380");
    contains(&source, "center: false");
}

// ── the repository corpus ────────────────────────────────────────────

/// Files of the repository corpus that still need a module this worktree does
/// not have (WP-5.4 JSX renderers, WP-5.8 native specs, timers/defaults).
/// Reported, not silently skipped.
const KNOWN_MISSING: &[&str] = &[
    // WP-5.4: JSX prop lowering / event parameter inference / children
    "arrays",
    "children",
    "components",
    "conditional",
    "counter",
    "defaults",
    "events",
    "hooks",
    "layout",
    "list",
    "list_scroll",
    "local_state",
    "named_handlers",
    "narrowing",
    "nesting",
    "react_compat",
    "spread",
    "store",
    "theme",
    "todos",
    "widgets",
    // WP-5.3 bug found by this WP: `testdata/strings.tsx` lowers a string
    // method to `label: { … }.clone()` (a labeled block as a method receiver,
    // which is not valid Rust); reported, not fixed here (5.3 owns
    // `string_methods.rs` / `aggregate.rs`).
    "strings",
    // `setTimeout` / timer codegen (`codegen/timers.zig`) has no owner yet
    "timers",
    // the `Uint8Array` ↔ bytes typing conflict (see status.md pending questions)
    "desktop",
    "platform",
];

#[test]
fn the_repository_corpus_compiles_without_the_missing_modules() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/tsx/testdata");
    let mut failures: Vec<String> = Vec::new();
    let mut compiled = 0usize;
    let mut skipped = 0usize;
    let mut entries: Vec<_> = std::fs::read_dir(&root)
        .expect("testdata directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "tsx"))
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_owned();
        let source = std::fs::read_to_string(&path).expect("read testdata");
        if KNOWN_MISSING.contains(&name.as_str()) {
            skipped += 1;
            continue;
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compile::compile_project_with(
                &[SourceFile::tsx(&format!("src/{name}.tsx"), &source)],
                Some(install_stubs),
            )
        }));
        let result = match result {
            Ok(result) => result,
            Err(_) => {
                failures.push(format!("{name}: the generator panicked (invalid Rust)"));
                continue;
            }
        };
        match result {
            Ok(project) => {
                syn::parse_file(&project.outputs[0]).unwrap_or_else(|error| {
                    panic!("generated module for {name} does not parse: {error}")
                });
                compiled += 1;
            }
            Err(error) => {
                if !KNOWN_MISSING.contains(&name.as_str()) {
                    failures.push(format!(
                        "{name}: {} (at {})",
                        error.diag.msg, error.diag.loc
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "the corpus compiles except the known-missing modules:\n{}",
        failures.join("\n")
    );
    assert!(compiled > 0, "no corpus file compiled");
    assert_eq!(
        KNOWN_MISSING.len(),
        skipped,
        "the known-missing set matches the corpus"
    );
}

// ── golden snapshots ─────────────────────────────────────────────────

fn snapshot(name: &str, source: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.rs"));
    if std::env::var("ZANT_TSX_WRITE_SNAPSHOTS").is_ok() {
        std::fs::create_dir_all(path.parent().expect("directory")).expect("snapshot directory");
        std::fs::write(&path, source).expect("write snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing snapshot {} ({error}); regenerate with ZANT_TSX_WRITE_SNAPSHOTS=1",
            path.display()
        )
    });
    if expected != source {
        let first = expected
            .lines()
            .zip(source.lines())
            .position(|(old, new)| old != new)
            .map(|line| line + 1)
            .unwrap_or(0);
        panic!("snapshot `{name}` differs at line {first}");
    }
}

const COUNTER: &str = r#"
import { useState, View, Row, Text, Button, Input } from "zant";
import type { u32 } from "zant";

export default function Counter() {
  const [count, setCount] = useState<u32>(0);
  const [name, setName] = useState("App");
  const doubled = count * 2;
  return (
    <View padding={24} gap={14}>
      <Text key="title">{`Hello, ${name}!`}</Text>
      <Text key="count">Count: {count} (x2 = {doubled})</Text>
      <Input key="name" value={name} onChange={setName} placeholder="Your name" width={240} />
      <Row gap={8}>
        <Button key="inc" variant="primary" onClick={() => setCount(count + 1)}>+</Button>
        <Button key="reset" disabled={count === 0} onClick={() => setCount(0)}>Reset</Button>
      </Row>
    </View>
  );
}
"#;

#[test]
fn the_application_layer_matches_its_golden_snapshots() {
    snapshot("counter", &generated(COUNTER));
    snapshot("windows", &generated(WINDOWS));
    snapshot("app_hooks", &generated(APP_HOOKS));
    snapshot("platform", &generated(PLATFORM));
    snapshot("desktop", &generated(DESKTOP));
}

// ── the project-root build integration (WP-6.2) ──────────────────────

#[test]
fn compile_project_root_writes_modules_entry_assets_and_the_dep_file() {
    let directory = std::env::temp_dir().join(format!(
        "zant-tsx-wp56-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let src = directory.join("src");
    let assets = src.join("assets");
    std::fs::create_dir_all(&assets).expect("source directory");
    std::fs::write(
        src.join("App.tsx"),
        "import { useState, View, Text, Button } from \"zant\";\n\
         import logo from \"./assets/logo.png\";\n\
         export default function App() {\n\
         const [count, setCount] = useState(0);\n\
         return <View><Text>{`${count}`}</Text><Button onClick={() => setCount(count + 1)}>+</Button></View>;\n\
         }\n",
    )
    .expect("App.tsx");
    std::fs::write(
        src.join("Row.tsx"),
        "export function Row() { return <View/>; }\n",
    )
    .expect("Row.tsx");
    std::fs::write(assets.join("logo.png"), [137u8, 80, 78, 71]).expect("logo.png");
    std::fs::write(
        directory.join("app.config.json"),
        "{\n  \"title\": \"Clips\",\n  \"bundle_id\": \"io.x.clips\",\n  \"window\": {\n    \"style\": \"hidden-titlebar\",\n    \"width\": 680,\n    \"height\": 700,\n    \"background\": \"#112233\"\n  }\n}\n",
    )
    .expect("app.config.json");
    let out = directory.join("generated");
    let dep = directory.join("generated/app.d");
    let options = compile::ProjectOptions {
        root: directory.to_string_lossy().into_owned(),
        out: out.to_string_lossy().into_owned(),
        src: src.to_string_lossy().into_owned(),
        native: directory.join("native").to_string_lossy().into_owned(),
        main: Some("App".to_owned()),
        app_config: None,
        dep: Some(dep.to_string_lossy().into_owned()),
    };
    let build = compile::compile_project_root_with(&options, Some(install_stubs))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(2, build.files.len());
    assert!(build.files.iter().any(|file| file.name == "App.rs"));
    assert!(build.files.iter().any(|file| file.name == "Row.rs"));
    assert_eq!(1, build.assets.len());
    let entry = build.entry.as_deref().expect("entry");
    contains(entry, "#[path = \"App.rs\"] pub mod app;");
    contains(entry, "#[path = \"Row.rs\"] pub mod row;");
    contains(entry, "zant::tsx::app::run::<app::App>(");
    contains(entry, "title: \"Clips\".into()");
    contains(entry, "style: zant::desktop::WindowStyle::HiddenTitlebar");
    contains(entry, "background: 0xff112233");
    build
        .write(&options.out, options.dep.as_deref())
        .expect("writes the build");
    let main = std::fs::read_to_string(out.join("main.rs")).expect("main.rs");
    assert!(main.contains("pub mod app;"), "{main}");
    let app = std::fs::read_to_string(out.join("App.rs")).expect("App.rs");
    contains(&app, "include_bytes!(");
    assert!(
        out.join("_assets")
            .read_dir()
            .expect("assets directory")
            .next()
            .is_some(),
        "the asset was copied"
    );
    let dep = std::fs::read_to_string(out.join("app.d")).expect("dep file");
    assert!(dep.contains("logo.png"), "{dep}");
    std::fs::remove_dir_all(&directory).expect("cleanup");
}
