//! End-to-end tests: the application layer of the TSX → Rust compiler
//! (imports, windows, desktop hooks, app hooks, `zant` API, assets, entry,
//! project compilation) and the compiler CLI's project pipeline.
//!
//! To isolate the application layer from the JSX / event emitters
//! (`codegen/jsx.rs` / `events.rs`), these tests inject a small **test-only**
//! stand-in through `compile::compile_project_with(.., install_stubs)`:
//!
//! - it emits `ui::text("stub", …)` for every element, so a module's render
//!   body generates and the app-level lowering around it is exercised, and
//! - it registers inline event arrows / setters the way `events.rs` does.
//!
//! Everything the application layer emits — project imports and their Rust paths,
//! `createWindow` handles, tray / hotkey lowering, app hooks as `impl ui::App`
//! methods, `zant` platform calls, asset `include_bytes!` constants and the
//! generated entry — is real generated code. The render bodies themselves are
//! covered by the JSX tests (`tests/codegen_jsx.rs`) and the goldens.

use zant_tsx::ast;
use zant_tsx::codegen::{self, scope};
use zant_tsx::compile::{self, SourceFile};
use zant_tsx::entry;
use zant_tsx::tags;

// ── the test-only JSX emitter ─────────────────────────────────────────

mod assets_and_rust;
mod project_output;
mod projects;
mod snapshots;
mod windows_and_hooks;

use self::windows_and_hooks::*;

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
            // callback props (`onPick`, `onSelect`, …) are the real emitter's
            // prop lowering; don't try to evaluate their arrow functions here
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
/// asserts the diagnostic message and position.
#[track_caller]
fn expect_diagnostic(marked: &str, message: &str) {
    let at = marked.find('~').expect("`~` marker");
    let src = format!("{}{}", &marked[..at], &marked[at + 1..]);
    // the JSX stub lets the render body generate, so diagnostics in handler
    // bodies (which `scan_body` emits) are reached too
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
