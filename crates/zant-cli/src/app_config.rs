//! `app.config.json` → the app's configuration, and the `src/app_config.rs`
//! the app compiles against. Used by `zant sync`, `run` and packaging.

use serde::Deserialize;

/// The platform the config is for (platform sections override the window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Macos,
    Windows,
    Other,
}

impl Os {
    /// The platform this process runs on.
    pub fn current() -> Os {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else {
            Os::Other
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowConfig {
    pub style: String,
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
    pub min_height: f32,
    pub background: String,
    pub resizable: bool,
    pub always_on_top: bool,
    pub movable_by_background: bool,
    pub center: bool,
    /// `hidden-titlebar` style: the height of the app's own title bar
    /// (where the system buttons sit); 0 for the system's default.
    pub titlebar_height: f32,
    /// Opened by the system at login: start hidden (a tray app).
    pub hidden_at_login: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        WindowConfig {
            style: "native".to_string(),
            width: 560.0,
            height: 420.0,
            min_width: 480.0,
            min_height: 320.0,
            background: "#f5f5f5".to_string(),
            resizable: true,
            always_on_top: false,
            movable_by_background: false,
            center: true,
            titlebar_height: 0.0,
            hidden_at_login: false,
        }
    }
}

/// `app.config.json`. `title`, `bundle_id` (the app identifier: the macOS
/// bundle id, the data folder name on Windows), `version` and `window` are
/// shared by both platforms and written by `zant create`. Platform-only
/// settings live in their own section, added by `zant run` / `build` /
/// `package` on that platform the first time they are needed:
///   "macos": { "sandbox": false, "signing_identity": "none" }
/// (older projects keep `sandbox` / `signing_identity` at the top level).
#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub title: String,
    pub bundle_id: String,
    pub version: String,
    /// A second launch brings the running app forward and exits.
    pub single_instance: bool,
    pub sandbox: bool,
    pub signing_identity: String,
    pub window: WindowConfig,
    /// `macos.entitlements`: extra entitlements for the signed app, e.g.
    /// `{ "com.apple.security.network.client": true }` (values: true / false,
    /// a string or a list of strings). `sandbox` adds app-sandbox itself.
    pub entitlements: Option<serde_json::Value>,
    /// set in the file (`macos` section or legacy top-level keys)
    pub has_sandbox: bool,
    pub has_signing: bool,
    /// read from the legacy top-level keys (moved into `macos` when written)
    pub legacy_sandbox: bool,
    pub legacy_signing: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            title: "Zant App".to_string(),
            bundle_id: "dev.zant.app".to_string(),
            version: "0.1.0".to_string(),
            single_instance: false,
            sandbox: false,
            signing_identity: "none".to_string(),
            window: WindowConfig::default(),
            entitlements: None,
            has_sandbox: false,
            has_signing: false,
            legacy_sandbox: false,
            legacy_signing: false,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawWindow {
    style: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
    min_width: Option<f32>,
    min_height: Option<f32>,
    background: Option<String>,
    resizable: Option<bool>,
    always_on_top: Option<bool>,
    movable_by_background: Option<bool>,
    center: Option<bool>,
    titlebar_height: Option<f32>,
    hidden_at_login: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawMacos {
    sandbox: Option<bool>,
    signing_identity: Option<String>,
    entitlements: Option<serde_json::Value>,
    /// Overrides `window.titlebar_height` on macOS.
    titlebar_height: Option<f32>,
}

/// Windows-only settings (so far the title bar height override).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawWindows {
    titlebar_height: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    title: Option<String>,
    bundle_id: Option<String>,
    version: Option<String>,
    single_instance: Option<bool>,
    sandbox: Option<bool>,
    signing_identity: Option<String>,
    window: Option<RawWindow>,
    macos: Option<RawMacos>,
    windows: Option<RawWindows>,
}

/// Failures of `parse`. `InvalidJson` / `InvalidBundleId` carry the detail.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("{0}")]
    InvalidJson(String),
    #[error("macos.entitlements must be an object")]
    InvalidEntitlements,
    #[error("{0}")]
    InvalidBundleId(String),
}

/// Parses app.config.json. On an error the variant says what is wrong (JSON
/// error, bundle id rule).
pub fn parse(data: &str, os: Os) -> Result<AppConfig, ParseError> {
    let parsed: RawConfig =
        serde_json::from_str(data).map_err(|err| ParseError::InvalidJson(err.to_string()))?;
    let mut cfg = AppConfig::default();
    if let Some(v) = parsed.title {
        cfg.title = v;
    }
    if let Some(v) = parsed.bundle_id {
        cfg.bundle_id = v;
    }
    if let Some(v) = parsed.version {
        cfg.version = v;
    }
    if let Some(v) = parsed.sandbox {
        cfg.sandbox = v;
        cfg.has_sandbox = true;
        cfg.legacy_sandbox = true;
    }
    if let Some(v) = parsed.signing_identity {
        cfg.signing_identity = v;
        cfg.has_signing = true;
        cfg.legacy_signing = true;
    }
    if let Some(m) = parsed.macos.as_ref() {
        if let Some(v) = m.sandbox {
            cfg.sandbox = v;
            cfg.has_sandbox = true;
            cfg.legacy_sandbox = false;
        }
        if let Some(v) = &m.signing_identity {
            cfg.signing_identity = v.clone();
            cfg.has_signing = true;
            cfg.legacy_signing = false;
        }
        if let Some(v) = &m.entitlements {
            if !v.is_object() {
                return Err(ParseError::InvalidEntitlements);
            }
            cfg.entitlements = Some(v.clone());
        }
    }
    if let Some(v) = parsed.single_instance {
        cfg.single_instance = v;
    }
    // the platform's own title bar height (macOS 52 / Windows 32 in ClipHop)
    let platform_titlebar = if os == Os::Windows {
        parsed.windows.as_ref().and_then(|w| w.titlebar_height)
    } else {
        parsed.macos.as_ref().and_then(|m| m.titlebar_height)
    };
    if let Some(w) = parsed.window {
        if let Some(v) = w.style {
            cfg.window.style = v;
        }
        if let Some(v) = w.width {
            cfg.window.width = v;
        }
        if let Some(v) = w.height {
            cfg.window.height = v;
        }
        if let Some(v) = w.min_width {
            cfg.window.min_width = v;
        }
        if let Some(v) = w.min_height {
            cfg.window.min_height = v;
        }
        if let Some(v) = w.background {
            cfg.window.background = v;
        }
        if let Some(v) = w.resizable {
            cfg.window.resizable = v;
        }
        if let Some(v) = w.always_on_top {
            cfg.window.always_on_top = v;
        }
        if let Some(v) = w.movable_by_background {
            cfg.window.movable_by_background = v;
        }
        if let Some(v) = w.center {
            cfg.window.center = v;
        }
        if let Some(v) = w.titlebar_height {
            cfg.window.titlebar_height = v;
        }
        if let Some(v) = w.hidden_at_login {
            cfg.window.hidden_at_login = v;
        }
    }
    if let Some(v) = platform_titlebar {
        cfg.window.titlebar_height = v;
    }
    if let Some(reason) = bundle_error(&cfg.bundle_id) {
        return Err(ParseError::InvalidBundleId(reason.to_string()));
    }
    if cfg.window.min_width > cfg.window.width {
        cfg.window.min_width = cfg.window.width;
    }
    if cfg.window.min_height > cfg.window.height {
        cfg.window.min_height = cfg.window.height;
    }
    Ok(cfg)
}

/// `src/app_config.rs`: what the app reads at compile time (title, bundle id,
/// window options). The constants keep the `app.config.json` field names
/// (snake_case) and every window field is written out.
pub fn source(cfg: &AppConfig) -> String {
    let title = escape_rust(&cfg.title);
    let bundle = escape_rust(&cfg.bundle_id);
    let version = escape_rust(&cfg.version);
    let identity = escape_rust(&cfg.signing_identity);
    let background = parse_color(&cfg.window.background).unwrap_or(0xfff5_f5f5);
    let mut out = String::new();
    out.push_str("// Generated by `zant` from app.config.json. Do not edit by hand.\n");
    // an app may not read every field (version, sandbox, signing identity):
    // the generated module is a file-level declaration table, not app code
    out.push_str("#![allow(dead_code)]\n\n");
    out.push_str(&format!(
        "#[allow(non_upper_case_globals)]\npub const title: &str = \"{title}\";\n"
    ));
    out.push_str(&format!(
        "#[allow(non_upper_case_globals)]\npub const bundle_id: &str = \"{bundle}\";\n"
    ));
    out.push_str(&format!(
        "#[allow(non_upper_case_globals)]\npub const version: &str = \"{version}\";\n"
    ));
    out.push_str(&format!(
        "#[allow(non_upper_case_globals)]\npub const single_instance: bool = {};\n",
        cfg.single_instance
    ));
    out.push_str(&format!(
        "#[allow(non_upper_case_globals)]\npub const sandbox: bool = {};\n",
        cfg.sandbox
    ));
    out.push_str(&format!(
        "#[allow(non_upper_case_globals)]\npub const signing_identity: &str = \"{identity}\";\n"
    ));
    out.push_str("\n#[allow(non_upper_case_globals)]\npub const window: zant::WindowOptions = zant::WindowOptions {\n");
    out.push_str(&format!(
        "    style: zant::WindowStyle::{},\n",
        style_rust(&cfg.window.style)
    ));
    out.push_str(&format!("    width: {},\n", fmt_f32(cfg.window.width)));
    out.push_str(&format!("    height: {},\n", fmt_f32(cfg.window.height)));
    out.push_str(&format!(
        "    min_width: {},\n",
        fmt_f32(cfg.window.min_width)
    ));
    out.push_str(&format!(
        "    min_height: {},\n",
        fmt_f32(cfg.window.min_height)
    ));
    out.push_str(&format!("    background: 0x{background:08x},\n"));
    out.push_str(&format!("    resizable: {},\n", cfg.window.resizable));
    out.push_str(&format!(
        "    always_on_top: {},\n",
        cfg.window.always_on_top
    ));
    out.push_str(&format!(
        "    movable_by_background: {},\n",
        cfg.window.movable_by_background
    ));
    out.push_str(&format!("    center: {},\n", cfg.window.center));
    out.push_str(&format!(
        "    titlebar_height: {},\n",
        fmt_f32(cfg.window.titlebar_height)
    ));
    out.push_str("    hidden: false,\n");
    out.push_str(&format!(
        "    hidden_at_login: {},\n",
        cfg.window.hidden_at_login
    ));
    out.push_str("};\n");
    out
}

/// Whole numbers print as `680.0`: Rust float literals need the fraction.
fn fmt_f32(value: f32) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

fn parse_color(text: &str) -> Option<u32> {
    let value = text.strip_prefix('#').unwrap_or(text);
    match value.len() {
        6 => u32::from_str_radix(value, 16)
            .ok()
            .map(|rgb| 0xff00_0000 | rgb),
        8 => u32::from_str_radix(value, 16).ok(),
        _ => None,
    }
}

fn style_rust(style: &str) -> &'static str {
    if style == "hidden-titlebar" || style == "hidden_titlebar" {
        "HiddenTitlebar"
    } else if style == "borderless" {
        "Borderless"
    } else {
        "Native"
    }
}

fn escape_rust(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// Why `id` is not a valid app identifier, or `None`. The identifier is the
/// macOS CFBundleIdentifier and, on Windows, the data folder name (and fits
/// an MSIX package name), so it follows both:
/// - Apple: only A–Z a–z 0–9 `-` `.`, reverse-DNS ("com.xx.test"); the
///   `com.apple.` prefix is reserved.
/// - Microsoft (MSIX Identity Name): 3–50 characters, no trailing `.`; a part
///   may not be a reserved device name (CON, NUL, COM1…), which Windows also
///   refuses as a folder name.
///
/// Plus: at least two parts, none empty, each starting with a letter or
/// digit and not ending with `-`, the first starting with a letter.
pub fn bundle_error(id: &str) -> Option<&'static str> {
    if id.len() < 3 || id.len() > 50 {
        return Some("长度要在 3～50 个字符之间（微软 MSIX 包名规范）");
    }
    for ch in id.chars() {
        if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '.') {
            return Some(
                "只能包含英文字母、数字、- 和 .（苹果 Bundle ID 规范；不能有空格、下划线、中文）",
            );
        }
    }
    if !id.contains('.') {
        return Some("要用反向域名格式，至少两段，如 com.xx.test");
    }
    if !id.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return Some("要以英文字母开头");
    }
    let lower = id.to_ascii_lowercase();
    if lower.starts_with("com.apple.") || lower == "com.apple" {
        return Some("com.apple 开头的标识是苹果保留的");
    }
    for part in id.split('.') {
        if part.is_empty() {
            return Some("不能以 . 开头或结尾，也不能有连续的 ..");
        }
        if part.starts_with('-') || part.ends_with('-') {
            return Some("每一段不能以 - 开头或结尾");
        }
        if reserved_windows_name(part) {
            return Some("不能有 CON、PRN、AUX、NUL、COM1～9、LPT1～9 这样的段（Windows 保留名）");
        }
    }
    None
}

fn reserved_windows_name(part: &str) -> bool {
    for name in ["con", "prn", "aux", "nul"] {
        if part.eq_ignore_ascii_case(name) {
            return true;
        }
    }
    let bytes = part.as_bytes();
    if bytes.len() == 4
        && bytes[3].is_ascii_digit()
        && bytes[3] != b'0'
        && (part[..3].eq_ignore_ascii_case("com") || part[..3].eq_ignore_ascii_case("lpt"))
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
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
}
