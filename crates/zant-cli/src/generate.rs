//! Platform generation: `Info.plist` / `app.entitlements`
//! writers, the macOS section completion and the XML / JSON escaping helpers.

use crate::app_config::AppConfig;
use crate::{escape_json, fail, ok, read_text, write_file};
use std::path::Path;

/// Extra Info.plist entries `zant build` / `zant package` can add.
#[derive(Debug, Default, Clone)]
pub struct PlistExtras {
    pub icon: bool,
    /// CFBundleVersion (App Store Connect needs a new one for every upload of
    /// the same version); defaults to `version`
    pub build_number: Option<String>,
    /// LSApplicationCategoryType, e.g. public.app-category.utilities
    pub category: Option<String>,
}

pub fn generate_info_plist(cfg: &AppConfig) -> String {
    generate_info_plist_for(cfg, &PlistExtras::default())
}

pub fn generate_info_plist_for(cfg: &AppConfig, extras: &PlistExtras) -> String {
    let title = escape_xml(&cfg.title);
    let bundle = escape_xml(&cfg.bundle_id);
    let version = escape_xml(&cfg.version);
    let build = escape_xml(extras.build_number.as_deref().unwrap_or(&cfg.version));
    let category = match extras.category.as_ref().or(cfg.category.as_ref()) {
        Some(category) => format!(
            "\n<key>LSApplicationCategoryType</key><string>{}</string>",
            escape_xml(category)
        ),
        None => String::new(),
    };
    let icon = if extras.icon {
        "\n<key>CFBundleIconFile</key><string>AppIcon</string>"
    } else {
        ""
    };
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
    out.push_str("<plist version=\"1.0\"><dict>\n");
    out.push_str("<key>CFBundleDevelopmentRegion</key><string>en</string>\n");
    out.push_str(&format!(
        "<key>CFBundleDisplayName</key><string>{title}</string>\n"
    ));
    out.push_str(&format!(
        "<key>CFBundleExecutable</key><string>app</string>{icon}\n"
    ));
    out.push_str(&format!(
        "<key>CFBundleIdentifier</key><string>{bundle}</string>\n"
    ));
    out.push_str("<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>\n");
    out.push_str(&format!(
        "<key>CFBundleName</key><string>{title}</string>\n"
    ));
    out.push_str("<key>CFBundlePackageType</key><string>APPL</string>\n");
    out.push_str(&format!(
        "<key>CFBundleShortVersionString</key><string>{version}</string>\n"
    ));
    out.push_str(&format!(
        "<key>CFBundleVersion</key><string>{build}</string>{category}\n"
    ));
    out.push_str(&format!(
        "<key>LSMinimumSystemVersion</key><string>{}</string>\n",
        escape_xml(&cfg.minimum_macos)
    ));
    out.push_str("<key>NSHighResolutionCapable</key><true/>\n");
    if !cfg.copyright.is_empty() {
        // the About panel's copyright line
        out.push_str(&format!(
            "<key>NSHumanReadableCopyright</key><string>{}</string>\n",
            escape_xml(&cfg.copyright)
        ));
    }
    // app.config.json macos.info_plist: extra string keys (usage descriptions)
    for (key, value) in &cfg.info_plist {
        if out.contains(&format!("<key>{}</key>", escape_xml(key))) {
            continue;
        }
        out.push_str(&format!(
            "<key>{}</key><string>{}</string>\n",
            escape_xml(key),
            escape_xml(value)
        ));
    }
    // Integrated GPU unless the app asks for more: better battery life.
    out.push_str("<key>NSSupportsAutomaticGraphicsSwitching</key><true/>\n");
    out.push_str("<key>NSPrincipalClass</key><string>NSApplication</string>\n");
    out.push_str("</dict></plist>\n");
    out
}

/// The app's entitlements: app-sandbox when `macos.sandbox` is on, the
/// `macos.entitlements` from app.config.json, and for the Mac App Store the
/// application and team identifiers (`store_team` = team id; sandbox is
/// forced).
pub fn entitlements_plist(cfg: &AppConfig, store_team: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
    out.push_str("<plist version=\"1.0\"><dict>\n");
    if cfg.sandbox || store_team.is_some() {
        out.push_str("<key>com.apple.security.app-sandbox</key><true/>\n");
    }
    if let Some(team) = store_team {
        out.push_str(&format!(
            "<key>com.apple.application-identifier</key><string>{}.{}</string>\n",
            escape_xml(team),
            escape_xml(&cfg.bundle_id)
        ));
        out.push_str(&format!(
            "<key>com.apple.developer.team-identifier</key><string>{}</string>\n",
            escape_xml(team)
        ));
    }
    if let Some(ents) = &cfg.entitlements {
        let object = ents.as_object().expect("entitlements is an object");
        for (key, value) in object {
            if key == "com.apple.security.app-sandbox" && (cfg.sandbox || store_team.is_some()) {
                continue;
            }
            out.push_str(&format!("<key>{}</key>", escape_xml(key)));
            match value {
                serde_json::Value::Bool(v) => {
                    out.push_str(if *v { "<true/>" } else { "<false/>" });
                }
                serde_json::Value::String(v) => {
                    out.push_str(&format!("<string>{}</string>", escape_xml(v)));
                }
                serde_json::Value::Array(items) => {
                    out.push_str("<array>");
                    for item in items {
                        let Some(item) = item.as_str() else {
                            fail(format!("macos.entitlements 的 {key}：列表里只能是字符串"));
                        };
                        out.push_str(&format!("<string>{}</string>", escape_xml(item)));
                    }
                    out.push_str("</array>");
                }
                _ => fail(format!(
                    "macos.entitlements 的 {key}：值只能是 true / false、字符串或字符串列表"
                )),
            }
            out.push('\n');
        }
    }
    out.push_str("</dict></plist>\n");
    out
}

/// Writes a generated platform file only when its contents change.
pub fn write_generated(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Ok(old) = crate::read_file(path)
        && old == data
    {
        return Ok(());
    }
    write_file(path, data)
}

/// Generate Info.plist and app.entitlements on macOS.
pub fn sync_project(root: &str, cfg: &AppConfig) -> std::io::Result<()> {
    crate::framework::warn_if_stale(root);
    crate::declarations::sync(root);
    if crate::IS_WINDOWS {
        crate::dim_line("本平台无需重新生成打包文件；Rust 配置由 Cargo 构建时生成到 OUT_DIR");
        return Ok(()); // Info.plist and entitlements are macOS bundle files
    }
    let plist = generate_info_plist(cfg);
    write_generated(&Path::new(root).join("Info.plist"), plist.as_bytes())?;
    if cfg.sandbox || cfg.entitlements.is_some() {
        let ent_path = Path::new(root).join("app.entitlements");
        let ent = entitlements_plist(cfg, None);
        write_generated(&ent_path, ent.as_bytes())?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Run,
    Package,
}

/// Adds this platform's settings to app.config.json when they are missing
/// (projects are created with the shared settings only). On macOS: `sandbox`
/// when running or building, the signing identity when packaging (asked in
/// an interactive terminal, ad-hoc otherwise). Windows needs nothing yet.
pub fn complete_platform_config(
    root: &str,
    cfg: &mut AppConfig,
    need: Need,
) -> std::io::Result<()> {
    if crate::IS_WINDOWS {
        return Ok(());
    }
    let want_signing = need == Need::Package && !cfg.has_signing;
    let want_sandbox = !cfg.has_sandbox;
    if !want_sandbox && !want_signing {
        return Ok(());
    }
    if want_signing {
        cfg.signing_identity = if crate::is_tty() {
            ask_signing_identity()?
        } else {
            "none".to_string()
        };
        cfg.has_signing = true;
    }
    cfg.has_sandbox = true;
    write_macos_section(root, cfg, want_sandbox, want_signing)?;
    if want_signing {
        ok(&format!(
            "已在 app.config.json 补充 macOS 打包账号：{}",
            if cfg.signing_identity == "none" {
                "Ad-hoc"
            } else {
                &cfg.signing_identity
            }
        ));
    } else {
        ok(&format!(
            "已在 app.config.json 补充 macOS 设置（macos.sandbox = {}，打包时再选择签名账号）",
            cfg.sandbox
        ));
    }
    Ok(())
}

fn ask_signing_identity() -> std::io::Result<String> {
    let identities = list_identities();
    eprintln!("\n打包开发者账号（macOS）:\n  0) Ad-hoc（无开发者账号）");
    for (index, id) in identities.iter().enumerate() {
        eprintln!("  {}) {id}", index + 1);
    }
    let value = crate::prompt("选择", "0");
    if value == "0" || value.is_empty() {
        return Ok("none".to_string());
    }
    match value.parse::<usize>() {
        Ok(index) => {
            if index >= 1 && index <= identities.len() {
                Ok(identities[index - 1].clone())
            } else {
                Ok("none".to_string())
            }
        }
        Err(_) => Ok(value),
    }
}

/// Code-signing identities in the keychain (`security find-identity`); a
/// macOS concept, empty on Windows.
pub fn list_identities() -> Vec<String> {
    if crate::IS_WINDOWS {
        return Vec::new();
    }
    let Ok(captured) = crate::run_capture(&[
        "security".to_string(),
        "find-identity".to_string(),
        "-v".to_string(),
        "-p".to_string(),
        "codesigning".to_string(),
    ]) else {
        return Vec::new();
    };
    let mut list = Vec::new();
    for line in captured.stdout.split('\n') {
        if let Some(start) = line.find('"')
            && let Some(end) = line[start + 1..].find('"')
        {
            list.push(line[start + 1..start + 1 + end].to_string());
        }
    }
    list
}

/// Adds the missing `macos` keys: into the existing section (its other keys,
/// such as `entitlements`, stay as they are) or as a new section before the
/// closing brace. Legacy top-level `sandbox` / `signing_identity` move into
/// the section.
pub fn write_macos_section(
    root: &str,
    cfg: &AppConfig,
    add_sandbox: bool,
    add_signing: bool,
) -> std::io::Result<()> {
    let path = Path::new(root).join("app.config.json");
    let text = read_text(&path)?;
    let mut keys: Vec<String> = Vec::new();
    if add_sandbox || cfg.legacy_sandbox {
        keys.push(format!("\"sandbox\": {}", cfg.sandbox));
    }
    if add_signing || cfg.legacy_signing {
        keys.push(format!(
            "\"signing_identity\": \"{}\"",
            escape_json(&cfg.signing_identity)
        ));
    }
    if keys.is_empty() {
        return Ok(());
    }
    let mut out: String;
    if let Some(start) = text.find("\"macos\"") {
        let open = match text[start..].find('{') {
            Some(index) => start + index,
            None => fail("app.config.json 的 macos 段格式不对"),
        };
        let rest = text[open + 1..].trim_start_matches([' ', '\t', '\r', '\n']);
        let empty = rest.starts_with('}');
        let mut insert = String::new();
        for (index, key) in keys.iter().enumerate() {
            insert.push_str("\n    ");
            insert.push_str(key);
            if !(empty && index == keys.len() - 1) {
                insert.push(',');
            }
        }
        if empty {
            out = format!("{}{}\n  {}", &text[..open + 1], insert, rest);
        } else {
            out = format!("{}{}{}", &text[..open + 1], insert, &text[open + 1..]);
        }
    } else {
        let end = match text.rfind('}') {
            Some(index) => index,
            None => fail("app.config.json 格式不对"),
        };
        let body = text[..end].trim_end_matches([' ', '\t', '\r', '\n']);
        let joined = keys.join(",\n    ");
        out = format!("{body},\n  \"macos\": {{\n    {joined}\n  }}\n}}\n");
    }
    if cfg.legacy_sandbox {
        out = drop_top_level_key(&out, "sandbox");
    }
    if cfg.legacy_signing {
        out = drop_top_level_key(&out, "signing_identity");
    }
    write_file(&path, out.as_bytes())
}

/// Removes a `"key": value,` line written by older `zant create` (the value
/// is a literal on one line); leaves the text unchanged if it is not there.
pub fn drop_top_level_key(text: &str, key: &str) -> String {
    let needle = format!("\n  \"{key}\":");
    let Some(start) = text.find(&needle) else {
        return text.to_string();
    };
    let Some(line_end) = text[start + 1..].find('\n').map(|index| start + 1 + index) else {
        return text.to_string();
    };
    let line = text[start..line_end].trim_matches([' ', '\t', '\r', '\n']);
    if line.is_empty() || !line.ends_with(',') {
        return text.to_string();
    }
    format!("{}{}", &text[..start], &text[line_end..])
}

pub fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
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
        assert!(plist.contains(
            "<key>NSLocalNetworkUsageDescription</key><string>Find &lt;peers&gt;</string>"
        ));
        assert!(!plist.contains("evil"));
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
        assert!(ent.contains(
            "<key>com.apple.application-identifier</key><string>TEAMID.io.x.app</string>"
        ));
        assert!(
            ent.contains("<key>com.apple.developer.team-identifier</key><string>TEAMID</string>")
        );
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
}
