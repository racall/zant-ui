use super::*;
use crate::app_config::{Os, parse};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/zant-cli-test")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn extra_info_plist_keys_are_written_but_cannot_override() {
    let cfg = parse(
        "{\"title\": \"Demo\", \"bundle_id\": \"io.x.demo\",
              \"macos\": {\"info_plist\": {\"NSLocalNetworkUsageDescription\": \"Find <peers>\",
                                       \"CFBundleIdentifier\": \"evil\"}}}",
        Os::Macos,
    )
    .expect("parse");
    let plist = generate_info_plist(&cfg);
    assert!(
        plist.contains(
            "<key>NSLocalNetworkUsageDescription</key><string>Find &lt;peers&gt;</string>"
        )
    );
    assert!(!plist.contains("evil"));
}

#[test]
fn info_plist_values_can_be_dictionaries_lists_and_numbers() {
    let cfg = parse(
        "{\"title\": \"Demo\", \"bundle_id\": \"io.x.demo\",
              \"macos\": {\"info_plist\": {\"LSEnvironment\": {\"MallocLargeCache\": \"0\"},
                                       \"LSUIElement\": true, \"Level\": 2, \"Kinds\": [\"a\", 1.5]}}}",
        Os::Macos,
    )
    .expect("parse");
    let plist = generate_info_plist(&cfg);
    for needle in [
        "<key>LSEnvironment</key><dict><key>MallocLargeCache</key><string>0</string></dict>",
        "<key>LSUIElement</key><true/>",
        "<key>Level</key><integer>2</integer>",
        "<key>Kinds</key><array><string>a</string><real>1.5</real></array>",
    ] {
        assert!(plist.contains(needle), "missing {needle} in {plist}");
    }
    assert!(
        parse(
            "{\"title\": \"Demo\", \"bundle_id\": \"io.x.demo\", \"macos\": {\"info_plist\": {\"X\": null}}}",
            Os::Macos
        )
        .is_err()
    );
}

#[test]
fn app_metadata_reaches_the_info_plist() {
    let cfg = parse(
        "{\"title\": \"Demo\", \"bundle_id\": \"io.x.demo\", \"copyright\": \"© 2026 Acme\",
              \"macos\": {\"category\": \"public.app-category.productivity\",
                        \"minimum_system_version\": \"14.0\"}}",
        Os::Macos,
    )
    .expect("parse");
    let plist = generate_info_plist(&cfg);
    assert!(plist.contains("<key>NSHumanReadableCopyright</key><string>© 2026 Acme</string>"));
    assert!(plist.contains(
        "<key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>"
    ));
    assert!(plist.contains("<key>LSMinimumSystemVersion</key><string>14.0</string>"));
    // a category given for one build (`zant build`) wins over the config
    let extras = PlistExtras {
        category: Some("public.app-category.utilities".to_string()),
        ..Default::default()
    };
    assert!(generate_info_plist_for(&cfg, &extras).contains("public.app-category.utilities"));
}

#[test]
fn info_plist_and_entitlements_have_the_expected_text() {
    let cfg = parse(
            "{\"title\": \"A&B\", \"bundle_id\": \"io.x.app\", \"version\": \"1.2.3\",
              \"sandbox\": true,
              \"macos\": {\"entitlements\": {\"com.apple.security.network.client\": true,
                                           \"com.apple.security.files.user-selected.read-only\": true}}}",
            Os::Macos,
        )
        .expect("parse");
    let plist = generate_info_plist(&cfg);
    assert!(plist.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(plist.contains("<key>CFBundleDisplayName</key><string>A&amp;B</string>"));
    assert!(plist.contains("<key>CFBundleIdentifier</key><string>io.x.app</string>"));
    assert!(plist.contains("<key>CFBundleShortVersionString</key><string>1.2.3</string>"));
    assert!(plist.contains("<key>CFBundleExecutable</key><string>app</string>"));
    assert!(!plist.contains("CFBundleIconFile"));
    assert!(!plist.contains("LSApplicationCategoryType"));
    let extras = PlistExtras {
        icon: true,
        build_number: Some("42".to_string()),
        category: Some("public.app-category.utilities".to_string()),
    };
    let plist = generate_info_plist_for(&cfg, &extras);
    assert!(plist.contains("<key>CFBundleIconFile</key><string>AppIcon</string>"));
    assert!(plist.contains("<key>CFBundleVersion</key><string>42</string>"));
    assert!(plist.contains(
        "<key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>"
    ));
    assert!(plist.contains("<key>LSMinimumSystemVersion</key><string>13.0</string>"));
    assert!(!plist.contains("NSHumanReadableCopyright"));
    let ent = entitlements_plist(&cfg, None);
    assert!(ent.contains("<key>com.apple.security.app-sandbox</key><true/>"));
    assert!(ent.contains("<key>com.apple.security.network.client</key><true/>"));
    let ent = entitlements_plist(&cfg, Some("TEAMID"));
    assert!(
        ent.contains("<key>com.apple.application-identifier</key><string>TEAMID.io.x.app</string>")
    );
    assert!(ent.contains("<key>com.apple.developer.team-identifier</key><string>TEAMID</string>"));
}

#[test]
fn migrates_legacy_top_level_keys_into_the_macos_section() {
    let root = temp_dir("generate-migrate");
    let path = root.join("app.config.json");
    let text = "{\n  \"title\": \"Clip\",\n  \"bundle_id\": \"io.x.clip\",\n  \"sandbox\": true,\n  \"signing_identity\": \"Developer ID Application: Me (ABCDE12345)\",\n  \"window\": {\n    \"width\": 680\n  }\n}\n";
    let mut cfg = parse(text, Os::Macos).expect("parse");
    cfg.has_sandbox = false;
    cfg.has_signing = false;
    std::fs::write(&path, text).expect("write");
    write_macos_section(root.to_str().unwrap(), &cfg, true, true).expect("write section");
    let updated = std::fs::read_to_string(&path).expect("read");
    assert!(updated.contains("\"macos\": {"), "{updated}");
    assert!(updated.contains("\"sandbox\": true"), "{updated}");
    assert!(
        updated.contains("\"signing_identity\": \"Developer ID Application: Me (ABCDE12345)\"")
    );
    assert!(!updated.contains("\n  \"sandbox\""), "{updated}");
    assert!(!updated.contains("\n  \"signing_identity\""), "{updated}");
    let parsed = parse(&updated, Os::Macos).expect("reparse");
    assert!(parsed.sandbox);
    assert!(!parsed.legacy_sandbox);
    assert_eq!(
        parsed.signing_identity,
        "Developer ID Application: Me (ABCDE12345)"
    );

    // An existing (non-empty) macos section keeps its other keys.
    let text = "{\n  \"bundle_id\": \"io.x.clip\",\n  \"macos\": {\n    \"entitlements\": { \"com.apple.security.network.client\": true }\n  }\n}\n";
    std::fs::write(&path, text).expect("write");
    let mut cfg = parse(text, Os::Macos).expect("parse");
    cfg.sandbox = false;
    cfg.has_sandbox = true;
    write_macos_section(root.to_str().unwrap(), &cfg, false, false).expect("noop");
    let unchanged = std::fs::read_to_string(&path).expect("read");
    assert_eq!(unchanged, text);
    write_macos_section(root.to_str().unwrap(), &cfg, true, false).expect("insert");
    let updated = std::fs::read_to_string(&path).expect("read");
    assert!(updated.contains("\"sandbox\": false"), "{updated}");
    assert!(updated.contains("\"entitlements\""), "{updated}");
    parse(&updated, Os::Macos).expect("reparse");
}

#[test]
fn sync_writes_generated_files_only_when_changed() {
    let root = temp_dir("generate-sync");
    std::fs::create_dir_all(root.join("src")).expect("src");
    let cfg = parse(
        "{\"title\": \"Sync\", \"bundle_id\": \"io.x.sync\", \"version\": \"0.2.0\"}",
        Os::Macos,
    )
    .expect("parse");
    sync_project(root.to_str().unwrap(), &cfg).expect("sync");
    assert!(!root.join("src/app_config.rs").exists());
    sync_project(root.to_str().unwrap(), &cfg).expect("sync again");
    if !crate::IS_WINDOWS {
        assert!(root.join("Info.plist").is_file());
    }
}
