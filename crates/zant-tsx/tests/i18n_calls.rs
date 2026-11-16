//! `t(…)` and the other i18n functions (`codegen::i18n_calls`), compiled
//! against a project's message keys.

use zant_tsx::codegen::i18n_calls::Index;
use zant_tsx::compile::{self, SourceFile};
use zant_tsx::i18n::contract::{Contract, Param, ParamKind};

fn index() -> Index {
    let param = |name: &str, kind| Param {
        name: name.to_owned(),
        kind,
        loc: 0,
    };
    Index {
        keys: vec![
            ("home.add".to_owned(), Contract { params: vec![] }),
            (
                "home.items".to_owned(),
                Contract {
                    params: vec![param("count", ParamKind::Number)],
                },
            ),
            (
                "home.welcome".to_owned(),
                Contract {
                    params: vec![param("name", ParamKind::String)],
                },
            ),
            ("status.done".to_owned(), Contract { params: vec![] }),
            ("status.open".to_owned(), Contract { params: vec![] }),
        ],
        locales: vec!["en".to_owned(), "zh-CN".to_owned()],
        default: "en".to_owned(),
    }
}

fn app(body: &str) -> String {
    format!(
        "import {{ Column, Text, Button, t, useLocale, setLocale, locales, localeName, localePreference }} from \"zant\";\n{body}\n"
    )
}

fn compile_app(source: &str) -> Result<String, String> {
    let files = [SourceFile::tsx("src/App.tsx", source)];
    compile::compile_project_i18n(&files, None, Some(index()))
        .map(|project| project.outputs[0].clone())
        .map_err(|error| error.diag.msg)
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect()
}

#[test]
fn calls_lower_to_key_ids_and_typed_arguments() {
    let rust = compile_app(&app(r#"type Status = "status.open" | "status.done";
export default function App() {
  const status: Status = "status.done";
  const n = 3;
  const locale = useLocale();
  return <Column>
    <Text>{t("home.add")}</Text>
    <Text>{t("home.items", { count: n })}</Text>
    <Text>{t("home.welcome", { name: "Ada" })}</Text>
    <Text>{t(status)}</Text>
    <Text>{locale + localePreference() + localeName("zh-CN") + String(locales().length)}</Text>
    <Button onClick={() => { setLocale("zh-CN"); }}>{t("home.add")}</Button>
  </Column>;
}"#))
    .unwrap();
    let rust = squash(&rust);
    for needle in [
        "zant::i18n::context::text(0)",
        "let_t0:f64=n;zant::i18n::context::format(1,&[zant::i18n::format::Arg::Num(_t0)])",
        "zant::i18n::format::Arg::Str(_t0.as_str())",
        "zant::i18n::context::text(matchstatus{Status::StatusOpen=>4,Status::StatusDone=>3,})",
        "zant::i18n::context::locale()",
        "zant::i18n::context::set_locale(",
        "tsx::List::from_vec(zant::i18n::context::locales())",
    ] {
        assert!(rust.contains(&squash(needle)), "missing `{needle}` in:\n{rust}");
    }
}

#[test]
fn mistakes_are_compile_errors() {
    for (body, expected) in [
        (r#"t("home.ad")"#, "unknown message key `home.ad` (not in locales/en.json); did you mean `home.add`?"),
        (r#"t("home.welcome")"#, "this message needs { name }"),
        (r#"t("home.welcome", { name: 1 })"#, "cannot convert"),
        (r#"t("home.welcome", { name: "a", extra: 1 })"#, "`extra` is not a parameter of this message"),
        (r#"t("home.add", { x: 1 })"#, "this message has no parameters"),
        (r#"t("home." + "add")"#, "`t` keys are string literals"),
    ] {
        let source = app(&format!(
            "export default function App() {{ return <Text>{{{body}}}</Text>; }}"
        ));
        let message = compile_app(&source).unwrap_err();
        assert!(message.contains(expected), "{body}: {message}");
    }
    let mixed = app(r#"type K = "home.add" | "home.items";
export default function App() { const k: K = "home.add"; return <Text>{t(k)}</Text>; }"#);
    assert!(compile_app(&mixed).unwrap_err().contains("need the same parameters"));
    let render = app(r#"export default function App() { setLocale("en"); return <Text>x</Text>; }"#);
    assert!(compile_app(&render).unwrap_err().contains("not while rendering"));
    // without locales/ the app gets a pointer to them
    let files = [SourceFile::tsx(
        "src/App.tsx",
        &app(r#"export default function App() { return <Text>{t("x")}</Text>; }"#),
    )];
    let error = compile::compile_project(&files).unwrap_err();
    assert!(error.diag.msg.contains("add locales/<language>.json"), "{}", error.diag.msg);
}
