//! Windows, desktop hooks, app hooks and the `zant` API.

use super::*;

#[test]
fn a_window_becomes_a_thread_local_handle_and_a_window_component() {
    let source = generated(WINDOWS);
    // the `createWindow` constant: a lazy slot plus accessor
    contains(&source, "static PANEL");
    contains(&source, "tsx::window::Window < PanelWindow");
    contains(&source, "pub fn panel_with<R>(");
    contains(&source, "zant::desktop::OpenOptions {");
    contains(&source, "name: Some(\"panel\".into())");
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
    contains(&source, "window.focus(zant::Str::from(\"count\").as_str())");
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
            "a window has show, hide, activate, toggle, visible, active, minimized, fullscreen, place, focus, setTitle",
        ),
    ];
    for (marked, message) in cases {
        expect_diagnostic(&marked, message);
    }
}

// ── desktop hooks ────────────────────────────────────────────────────

pub(super) const DESKTOP: &str = r#"
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
    contains(&source, "tooltip: zant::Str::from(\"Clips\")");
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

pub(super) const APP_HOOKS: &str = r#"
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
        // a child without props has no trait that would call its hooks
        (
            format!(
                "function Child() {{\n  ~useCloseRequest(() => true);\n  return <View/>;\n}}\n{APP}  return <Child/>;\n}}\n"
            ),
            "useCloseRequest belongs to the root component or a window's component",
        ),
        (
            format!(
                "function Child() {{\n  ~useKeyDown((e: KeyEvent) => false);\n  return <View/>;\n}}\n{APP}  return <Child/>;\n}}\n"
            ),
            "useKeyDown belongs to the root component or a window's component",
        ),
        // `ui::App` has no focus hook: useWindowFocus is a window's
        (
            format!("{APP}  ~useWindowFocus((focused: boolean) => {{}});\n{END}"),
            "useWindowFocus belongs to a window's component",
        ),
        // quitting is an app event: a window's trait has no on_quit_request
        (
            format!(
                "function Panel() {{\n  ~useQuitRequest(() => true);\n  return <View/>;\n}}\nconst panel = createWindow(Panel, {{ title: \"P\" }});\n{APP}  return <View/>;\n}}\n"
            ),
            "useQuitRequest belongs to the root component (the app)",
        ),
        (
            format!(
                "function Panel() {{\n  ~useWake(() => {{}});\n  return <View/>;\n}}\nconst panel = createWindow(Panel, {{ title: \"P\" }});\n{APP}  return <View/>;\n}}\n"
            ),
            "useWake belongs to the root component (the app)",
        ),
        (
            format!("{APP}  ~useKeyDown(1);\n{END}"),
            "useKeyDown takes a function: an inline arrow or the name of a function",
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

pub(super) const PLATFORM: &str = r#"
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
