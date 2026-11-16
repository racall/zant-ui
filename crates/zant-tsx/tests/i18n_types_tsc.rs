use std::path::PathBuf;
use std::process::Command;
use zant_tsx::i18n::{contract, parse_message, types::declarations};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("zant-i18n-types-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires ZANT_TSC pointing to a TypeScript compiler"]
fn generated_declarations_typecheck() {
    let tsc = std::env::var_os("ZANT_TSC").expect("set ZANT_TSC to a tsc binary");
    let dir = TempDir::new();
    std::fs::write(
        dir.0.join("zant.d.ts"),
        r#"declare module "zant" {
  export interface I18nMessages {}
  export interface I18nLocales {}
  type Key = keyof I18nMessages & string;
  type All<U> = (U extends unknown ? (x: U) => void : never) extends (x: infer I) => void ? I : never;
  type Args<K extends Key> = [All<I18nMessages[K]>] extends [undefined] ? [] : [params: All<I18nMessages[K]>];
  export function t<K extends Key>(key: K, ...params: Args<K>): string;
  export type Locale = keyof I18nLocales & string;
}
"#,
    )
    .unwrap();
    let add = contract(&parse_message("Add").unwrap()).unwrap();
    let items = contract(&parse_message("{count, number}").unwrap()).unwrap();
    let welcome = contract(&parse_message("{name}{when, number}").unwrap()).unwrap();
    std::fs::write(
        dir.0.join("zant.i18n.d.ts"),
        declarations(
            &["zh-CN", "en"],
            &[
                ("home.welcome", &welcome),
                ("home.add", &add),
                ("home.items", &items),
            ],
        ),
    )
    .unwrap();
    let ok = r#"t("home.add");
t("home.welcome", { name: "a", when: 1 });
declare const key: "home.items" | "home.items";
t(key, { count: 1 });
const l: Locale = "zh-CN";
"#;
    let bad = [
        r#"t("home.nope");"#,
        r#"t("home.add", {});"#,
        r#"t("home.welcome", { name: 1, when: 1 });"#,
        r#"t("home.welcome");"#,
        r#"const l: Locale = "fr";"#,
        r#"declare const key: "home.items" | "home.welcome"; t(key, { count: 1 });"#,
    ];
    let mut failures = Vec::new();
    for (name, source, success) in std::iter::once(("ok.ts".into(), ok, true)).chain(
        bad.iter()
            .enumerate()
            .map(|(index, source)| (format!("bad_{index}.ts"), *source, false)),
    ) {
        std::fs::write(
            dir.0.join(&name),
            format!("import {{ t, type Locale }} from \"zant\";\n{source}\n"),
        )
        .unwrap();
        // The CLI's `--types []` requests a package literally named `[]`.
        // An empty type list must be expressed in a tsconfig instead.
        std::fs::write(
            dir.0.join("tsconfig.json"),
            format!(
                r#"{{"compilerOptions":{{"noEmit":true,"strict":true,"types":[]}},"files":["zant.d.ts","zant.i18n.d.ts","{name}"]}}"#
            ),
        )
        .unwrap();
        let output = Command::new(&tsc)
            .current_dir(&dir.0)
            .args(["--project", "tsconfig.json"])
            .output()
            .expect("run ZANT_TSC");
        if output.status.success() != success {
            failures.push(format!(
                "{name}: expected success={success}, got {}\n{}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
