use super::*;

#[test]
fn app_identifier_follows_apple_and_microsoft_rules() {
    for id in [
        "com.xx.test",
        "dev.zant.demo-app",
        "io.github.user.App2",
        "Com.Example.MyApp",
    ] {
        assert!(bundle_error(id).is_none(), "{id} should be valid");
    }
    for id in [
        "bad",
        "ab",
        "com.xx_test",
        "com.xx test",
        "com.剪贴板",
        "1com.xx",
        "com..xx",
        "com.xx.",
        ".com.xx",
        "com.-xx",
        "com.xx-",
        "com.apple.notes",
        "com.con.app",
        "com.xx.LPT1",
        "a.xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
    ] {
        assert!(bundle_error(id).is_some(), "{id} should be rejected");
    }
}

#[test]
fn platform_title_bar_overrides_and_the_generated_source() {
    let json = "{ \"title\": \"Clip\", \"bundle_id\": \"io.x.clip\", \"single_instance\": true,
  \"window\": { \"style\": \"hidden-titlebar\", \"titlebar_height\": 40 },
  \"macos\": { \"titlebar_height\": 52 }, \"windows\": { \"titlebar_height\": 32 } }";
    let mac = parse(json, Os::Macos).expect("macos parse");
    let win = parse(json, Os::Windows).expect("windows parse");
    assert_eq!(mac.window.titlebar_height, 52.0);
    assert_eq!(win.window.titlebar_height, 32.0);
    let code = source(&win);
    assert!(code.contains("titlebar_height: 32.0,"), "{code}");
    assert!(
        code.contains("pub const single_instance: bool = true;"),
        "{code}"
    );
    assert_eq!(
        parse("{ \"bundle_id\": \"bad\" }", Os::Macos).expect_err("bad bundle"),
        ParseError::InvalidBundleId(bundle_error("bad").expect("reason").to_string())
    );
}

#[test]
fn generated_source_matches_the_expected_fixture() {
    // `app_config_expected.rs` is what `zant sync` writes for
    // `tests/fixtures/app.config.json`.
    let json = include_str!("../tests/fixtures/app.config.json");
    let expected = include_str!("../tests/fixtures/app_config_expected.rs");
    let cfg = parse(json, Os::Macos).expect("fixture parse");
    assert_eq!(source(&cfg), expected);
    // The platform title bar override picks the Windows section instead.
    let windows = parse(json, Os::Windows).expect("fixture parse");
    assert!(source(&windows).contains("titlebar_height: 32.0,"));
}

#[test]
fn defaults_and_unknown_fields() {
    let cfg = parse("{}", Os::Macos).expect("defaults");
    assert_eq!(cfg.title, "Zant App");
    assert_eq!(cfg.bundle_id, "dev.zant.app");
    assert_eq!(cfg.version, "0.1.0");
    assert_eq!(cfg.window, WindowConfig::default());
    assert!(!cfg.has_sandbox && !cfg.has_signing);
    let cfg = parse(
        "{ \"title\": \"X\", \"unknown_key\": 1, \"window\": { \"unknown\": true } }",
        Os::Macos,
    )
    .expect("unknown fields are ignored");
    assert_eq!(cfg.title, "X");
}

#[test]
fn legacy_platform_keys_and_clamped_min_size() {
    let cfg = parse(
            "{ \"bundle_id\": \"io.x.app\", \"sandbox\": true, \"signing_identity\": \"none\",
               \"window\": { \"width\": 400, \"height\": 300, \"min_width\": 500, \"min_height\": 600 } }",
            Os::Macos,
        )
        .expect("legacy parse");
    assert_eq!(cfg.bundle_id, "io.x.app");
    assert!(cfg.sandbox && cfg.has_sandbox && cfg.legacy_sandbox);
    assert!(cfg.has_signing && cfg.legacy_signing);
    assert_eq!(cfg.window.width, 400.0);
    assert_eq!(cfg.window.min_width, 400.0);
    assert_eq!(cfg.window.height, 300.0);
    assert_eq!(cfg.window.min_height, 300.0);
    // the macos section wins over the legacy top-level keys
    let cfg = parse(
        "{ \"sandbox\": true, \"signing_identity\": \"old\",
               \"macos\": { \"sandbox\": false, \"signing_identity\": \"new\" } }",
        Os::Macos,
    )
    .expect("section wins");
    assert!(!cfg.sandbox && cfg.has_sandbox && !cfg.legacy_sandbox);
    assert_eq!(cfg.signing_identity, "new");
    assert!(cfg.has_signing && !cfg.legacy_signing);
}

#[test]
fn invalid_json_and_entitlements() {
    assert!(matches!(
        parse("{", Os::Macos),
        Err(ParseError::InvalidJson(_))
    ));
    assert_eq!(
        parse("{ \"macos\": { \"entitlements\": [] } }", Os::Macos)
            .expect_err("array entitlements"),
        ParseError::InvalidEntitlements
    );
}

#[test]
fn window_remember_is_opt_in_for_old_configs_and_enabled_in_the_template() {
    for os in [Os::Macos, Os::Windows] {
        for json in [
            "{}",
            r#"{"window": {}}"#,
            r#"{"window": {"remember": false}}"#,
        ] {
            let cfg = parse(json, os).expect("old or disabled config");
            assert!(!cfg.window.remember);
            assert!(source(&cfg).contains("pub const remember: bool = false;"));
        }
        let cfg = parse(r#"{"window": {"remember": true}}"#, os).expect("enabled");
        assert!(cfg.window.remember);
        assert!(source(&cfg).contains("pub const remember: bool = true;"));
        assert!(parse(r#"{"window": {"remember": "true"}}"#, os).is_err());
    }
    // The unexpanded template bundle id is a scaffold placeholder.
    let template = include_str!("../../../scaffold/template-ts/app.config.json")
        .replace("__BUNDLE_ID__", "com.example.app");
    assert!(
        parse(&template, Os::Macos)
            .expect("template config")
            .window
            .remember
    );
}
