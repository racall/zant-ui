use zant_tsx::i18n::{catalog, contract, emit, message};

fn catalogs(files: &[(&str, &str)]) -> Vec<(catalog::Catalog, String)> {
    files
        .iter()
        .map(|(tag, src)| {
            (
                catalog::parse(&format!("locales/{tag}.json"), src).unwrap(),
                (*src).into(),
            )
        })
        .collect()
}

fn fixture() -> Vec<(catalog::Catalog, String)> {
    catalogs(&[
        (
            "zh-CN",
            r#"{
                "items": "{n, plural, =0 {没有} one {# 个} other {{who, select, friend {朋友有 # 个} other {# 个}}}}",
                "greeting": "{who}：{n, number}",
                "zant.only": "ignored {"
            }"#,
        ),
        (
            "en",
            r#"{
                "items": "{n, plural, =0 {none} one {# item} other {{who, select, friend {# items for a friend} other {# items}}}}",
                "greeting": "{who, select, friend {Hello} other {Welcome}} {n, number} {who}",
                "escapes": "Quote: \" backslash: \\ newline:\n\t\r\u0000\u0008\u007f\u0085\u2028\u2029 中",
                "zant.title": "ignored {"
            }"#,
        ),
    ])
}

#[test]
fn compiles_sorted_catalogs_and_matches_runtime_golden() {
    let catalogs = fixture();
    let compiled = emit::compile(&catalogs, "en").unwrap();
    let default = &catalogs[1].0;
    let expected: Vec<_> = default
        .entries
        .iter()
        .filter(|e| !e.key.starts_with("zant."))
        .map(|e| {
            (
                e.key.clone(),
                contract::contract(&message::parse(&e.value).unwrap()).unwrap(),
            )
        })
        .collect();
    assert_eq!(compiled.keys, expected);
    assert_eq!(
        compiled
            .keys
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>(),
        ["escapes", "greeting", "items"]
    );
    assert_eq!(
        compiled.keys[2]
            .1
            .params
            .iter()
            .map(|p| (p.name.as_str(), p.kind))
            .collect::<Vec<_>>(),
        [
            ("n", contract::ParamKind::Number),
            ("who", contract::ParamKind::String)
        ]
    );
    assert_eq!(compiled.locales, ["en", "zh-CN"]);
    assert_eq!(compiled.default, 0);
    assert_eq!(
        compiled.warnings,
        ["locales/zh-CN.json: missing translation for `escapes` (falls back to en)"]
    );
    syn::parse_str::<syn::Item>(&compiled.rust).unwrap();
    let mut reversed = catalogs.clone();
    reversed.reverse();
    assert_eq!(emit::compile(&reversed, "en").unwrap().rust, compiled.rust);
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/i18n_description.rs");
    if std::env::var_os("ZANT_BLESS").is_some() {
        std::fs::write(&path, &compiled.rust).unwrap();
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), compiled.rust);
}

#[test]
fn errors_have_exact_source_locations() {
    for (en, zh, loc, msg) in [
        (
            r#"{"x":"ok"}"#,
            r#"{"x":"\u4e2d\u007d"}"#,
            12,
            "unbalanced `}`",
        ),
        (
            r#"{"x":"ok"}"#,
            r#"{"x":"ok","extra":"no"}"#,
            10,
            "`extra` is not in the default language (`en`); add it there or remove it",
        ),
        (
            r#"{"x":"{n}"}"#,
            r#"{"x":"\u4e2d{n, number}"}"#,
            12,
            "`n` is a string in the default-language message but is used as a number",
        ),
        (
            r#"{"x":"ok"}"#,
            r#"{"x":"{n, number}{n, select, other {}}"}"#,
            17,
            "`n` is used as a number and as a select value",
        ),
    ] {
        let err = emit::compile(&catalogs(&[("en", en), ("zh-CN", zh)]), "en").unwrap_err();
        assert_eq!(err.path, "locales/zh-CN.json");
        assert_eq!(err.diag.loc, loc, "{zh}");
        assert_eq!(err.diag.msg, msg);
        assert_eq!(err.related, None);
    }
    let err = emit::compile(&fixture(), "fr").unwrap_err();
    assert_eq!(err.path, "locales/fr.json");
    assert_eq!(err.diag.loc, 0);
    assert_eq!(
        err.diag.msg,
        "default language `fr` has no catalog; available languages: en, zh-CN"
    );
    let err = emit::compile(&[], "en").unwrap_err();
    assert_eq!(err.path, "locales/en.json");
    assert_eq!(err.diag.loc, 0);
}

#[test]
fn translated_arguments_use_default_indices_and_default_need_not_sort_first() {
    let compiled = emit::compile(
        &catalogs(&[
            ("en", r#"{"x":"{z}"}"#),
            ("fr", r#"{"x":"{a}{z, number}"}"#),
        ]),
        "fr",
    )
    .unwrap();
    assert_eq!(compiled.default, 1);
    assert!(compiled.rust.contains("Op::Arg(1)"));
    assert!(compiled.warnings.is_empty());
    let empty = emit::compile(&catalogs(&[("xx", "{}")]), "xx").unwrap();
    assert!(empty.keys.is_empty());
    syn::parse_str::<syn::Item>(&empty.rust).unwrap();
}
