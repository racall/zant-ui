//! Compiling the regression modules against the real runtime with `cargo check`.

use super::*;

#[test]
fn the_regression_modules_compile_against_the_runtime() {
    let crate_dir = temporary_crate_dir();
    let src_dir = crate_dir.join("src");
    let _ = std::fs::remove_dir_all(&src_dir);
    std::fs::create_dir_all(&src_dir).expect("create temp crate src");
    let zant_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../zant");
    let cargo_toml = format!(
        "[package]\nname = \"zant-tsx-regressions\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nzant = {{ path = {:?} }}\n\n[workspace]\n",
        zant_dir.to_string_lossy()
    );
    std::fs::write(crate_dir.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    let mut modules = String::from("#![allow(dead_code)]\n");
    for (name, source) in MODULES {
        let generated = compile(name, source);
        std::fs::write(src_dir.join(format!("{name}.rs")), generated).expect("write module");
        modules.push_str(&format!("#[path = \"{name}.rs\"]\nmod {name};\n"));
    }
    for (name, generated) in ["callbacks", "imported_defaults"]
        .iter()
        .zip(imported_callback_defaults().outputs)
    {
        std::fs::write(src_dir.join(format!("{name}.rs")), generated)
            .expect("write imported module");
        modules.push_str(&format!("#[path = \"{name}.rs\"]\nmod {name};\n"));
    }
    modules.push_str("fn main() {}\n");
    std::fs::write(src_dir.join("main.rs"), modules).expect("write main.rs");
    let (success, text) = run_cargo_check(&crate_dir);
    assert!(
        success,
        "the regression modules do not compile against zant:\n{text}"
    );
}

pub(super) fn temporary_crate_dir() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let dir = target.join("zant-tsx-regressions");
    std::fs::create_dir_all(&dir).expect("create temp crate directory");
    dir
}

pub(super) fn run_cargo_check(crate_dir: &Path) -> (bool, String) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let run = |offline: bool| {
        let mut command = Command::new(&cargo);
        command
            .current_dir(crate_dir)
            .env("CARGO_TARGET_DIR", crate_dir.join("target"))
            .arg("check")
            .arg("--quiet")
            .arg("--message-format=short");
        if offline {
            command.arg("--offline");
        }
        command.output()
    };
    let output = run(true).expect("run cargo check");
    if output.status.success() {
        return (true, String::from_utf8_lossy(&output.stderr).into_owned());
    }
    match run(false) {
        Ok(retry) if retry.status.success() => {
            (true, String::from_utf8_lossy(&retry.stderr).into_owned())
        }
        Ok(retry) => (false, String::from_utf8_lossy(&retry.stderr).into_owned()),
        Err(error) => (
            false,
            format!(
                "{}\n(and the retry without --offline failed: {error})",
                String::from_utf8_lossy(&output.stderr)
            ),
        ),
    }
}

/// Boolean children render nothing (React); `for…in` visits record keys.
#[test]
fn boolean_children_and_for_in() {
    let out = compile(
        "bool_children",
        r#"import { useState, View, Text } from "zant";
function count(scores: Record<string, number>): number {
  let n = 0;
  for (const key in scores) n += key.length;
  return n;
}
export default function App() {
  const [flag, setFlag] = useState(false);
  return <View>{flag}{true}<Text>{count({ a: 1 })}</Text></View>;
}
"#,
    );
    contains(&out, "let _ = self.flag");
    contains(&out, "scores.clone().keys()");
    let error = zant_tsx::compile::compile(
        "function f(o: Record<string, number>): void { for (const [k, v] in o) {} }\nexport default function App() { return <View />; }",
        "for_in.tsx",
    )
    .unwrap_err();
    assert!(error.msg.contains("Object.entries"), "{}", error.msg);
}

/// Found by the everyday-TypeScript probes (`node_parity/everyday*.ts`).
#[test]
fn everyday_typescript_regressions() {
    // the right side of an assignment still reads the narrowed `let`
    compile(
        "narrowed_assign",
        r#"import { View } from "zant";
function total(rows: number[][]): number {
  let row: number[] | null = rows.length > 0 ? rows[0] : null;
  let n = 0;
  while (row !== null) {
    n += row.length;
    row = row.length > 1 ? null : row;
  }
  return n;
}
export default function App() { return <View />; }
"#,
    );
    // `?.` short-circuits the rest of the chain
    let out = compile(
        "optional_chain",
        r#"import { View } from "zant";
function title(text: string | null): string { return text?.trim().toUpperCase() ?? "x"; }
export default function App() { return <View />; }
"#,
    );
    contains(&out, ".map(|_some|");
    // `satisfies` keeps the object's own type
    compile(
        "satisfies_record",
        r#"import { View } from "zant";
const PALETTE = { red: 1, green: 2 } satisfies Record<string, number>;
function green(): number { return PALETTE.green; }
export default function App() { return <View />; }
"#,
    );
    // a type holding itself directly is reported (Rust needs a pointer)
    let error = zant_tsx::compile::compile(
        "interface Person { name: string; boss: Person | null }\nexport default function App() { return <View />; }",
        "recursive.tsx",
    )
    .unwrap_err();
    assert!(
        error.msg.contains("contains its own type directly"),
        "{}",
        error.msg
    );
}
