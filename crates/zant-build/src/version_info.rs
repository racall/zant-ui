//! The Windows `VERSIONINFO` resource of an app: what Explorer shows under
//! Properties › Details (description, company, version, copyright) and what
//! installers and the Task Manager read. Written into the app's `.rc` next to
//! the icon and the manifest.

use crate::config::AppConfig;

/// `1.2.3` → `1,2,3,0` (missing or non-numeric parts are 0).
fn numeric_version(version: &str) -> String {
    let mut parts: Vec<u16> = version
        .split(['.', '-', '+'])
        .take(4)
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    parts.resize(4, 0);
    parts
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// An `.rc` string literal: quotes doubled, backslashes kept literal.
fn rc_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\"\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The `VERSIONINFO` block for `cfg`; `exe` is the executable file name.
pub fn rc_block(cfg: &AppConfig, exe: &str) -> String {
    let number = numeric_version(&cfg.version);
    let description = if cfg.description.is_empty() {
        &cfg.title
    } else {
        &cfg.description
    };
    let mut values = vec![
        ("FileDescription", description.as_str()),
        ("FileVersion", cfg.version.as_str()),
        ("ProductName", cfg.title.as_str()),
        ("ProductVersion", cfg.version.as_str()),
        ("InternalName", exe),
        ("OriginalFilename", exe),
    ];
    if !cfg.company.is_empty() {
        values.push(("CompanyName", cfg.company.as_str()));
    }
    if !cfg.copyright.is_empty() {
        values.push(("LegalCopyright", cfg.copyright.as_str()));
    }
    let mut out = String::new();
    // UTF-8 source: rc.exe reads the strings below in this code page
    out.push_str("#pragma code_page(65001)\n");
    out.push_str("1 VERSIONINFO\n");
    out.push_str(&format!("FILEVERSION {number}\nPRODUCTVERSION {number}\n"));
    out.push_str(
        "FILEFLAGSMASK 0x3fL\nFILEFLAGS 0x0L\nFILEOS 0x40004L\nFILETYPE 0x1L\nFILESUBTYPE 0x0L\n",
    );
    out.push_str("BEGIN\n  BLOCK \"StringFileInfo\"\n  BEGIN\n    BLOCK \"040904b0\"\n    BEGIN\n");
    for (key, value) in values {
        out.push_str(&format!("      VALUE \"{key}\", {}\n", rc_string(value)));
    }
    out.push_str("    END\n  END\n  BLOCK \"VarFileInfo\"\n  BEGIN\n    VALUE \"Translation\", 0x409, 1200\n  END\nEND\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_become_four_numbers() {
        assert_eq!(numeric_version("1.2.3"), "1,2,3,0");
        assert_eq!(numeric_version("0.1.0-beta.2"), "0,1,0,0");
        assert_eq!(numeric_version("2"), "2,0,0,0");
        assert_eq!(numeric_version("1.2.3.4.5"), "1,2,3,4");
    }

    #[test]
    fn the_block_carries_the_app_metadata() {
        let cfg = AppConfig {
            title: "My \"App\"".into(),
            version: "1.4.0".into(),
            company: "Acme".into(),
            copyright: "© 2026 Acme".into(),
            ..AppConfig::default()
        };
        let rc = rc_block(&cfg, "my_app.exe");
        assert!(rc.contains("FILEVERSION 1,4,0,0"));
        assert!(rc.contains("VALUE \"ProductName\", \"My \"\"App\"\"\""));
        // no description given: Explorer shows the title
        assert!(rc.contains("VALUE \"FileDescription\", \"My \"\"App\"\"\""));
        assert!(rc.contains("VALUE \"CompanyName\", \"Acme\""));
        assert!(rc.contains("VALUE \"LegalCopyright\", \"© 2026 Acme\""));
        assert!(rc.contains("VALUE \"OriginalFilename\", \"my_app.exe\""));
        let bare = rc_block(&AppConfig::default(), "app.exe");
        assert!(!bare.contains("CompanyName"));
        assert!(!bare.contains("LegalCopyright"));
    }
}
