use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use zant_tsx::i18n::{discover, parse};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "zant-i18n-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn discovery_sorts_skips_hidden_files_and_does_not_recurse() {
    let dir = TempDir::new();
    for name in [
        "zh-hans-cn.json",
        "EN-us.json",
        "es-419.json",
        ".invalid.json",
        "README.txt",
    ] {
        std::fs::write(dir.0.join(name), "{}").unwrap();
    }
    std::fs::create_dir(dir.0.join("nested")).unwrap();
    std::fs::write(dir.0.join("nested/en.json"), "{}").unwrap();
    std::fs::create_dir(dir.0.join("de.json")).unwrap();
    let names: Vec<_> = discover(&dir.0)
        .unwrap()
        .into_iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["EN-us.json", "es-419.json", "zh-hans-cn.json"]);
}

#[test]
fn canonical_duplicate_names_report_both_paths() {
    let dir = TempDir::new();
    // Both spellings coexist only on case-sensitive filesystems.
    for name in ["EN-us.json", "en-US.json"] {
        std::fs::write(dir.0.join(name), "{}").unwrap();
    }
    // The pure ordering unit test covers this conflict on every filesystem.
    let files = std::fs::read_dir(&dir.0).unwrap().count();
    if files == 1 {
        return;
    }
    let err = discover(&dir.0).unwrap_err();
    assert!(err.diag.msg.contains("EN-us.json"));
    assert!(err.diag.msg.contains("en-US.json"));
    assert!(err.path.ends_with("en-US.json"));
    assert!(err.related.unwrap().0.ends_with("EN-us.json"));
}

#[test]
fn missing_directory_is_empty_and_invalid_tag_is_diagnosed() {
    let dir = TempDir::new();
    assert!(discover(&dir.0.join("missing")).unwrap().is_empty());
    std::fs::write(dir.0.join("zh_CN.json"), "{}").unwrap();
    let err = discover(&dir.0).unwrap_err();
    assert!(err.diag.msg.contains("use `zh-CN`"));
}

#[test]
fn bom_and_multibyte_text_have_source_byte_offsets() {
    let src = "\u{feff}{\"标题\": \"Hello\\n\\uD83D\\uDE00\"}";
    let catalog = parse("zh-hans-cn.json", src).unwrap();
    assert_eq!(catalog.locale, "zh-Hans-CN");
    assert_eq!(catalog.entries[0].key, "标题");
    assert_eq!(catalog.entries[0].value, "Hello\n😀");
    assert_eq!(catalog.entries[0].key_loc, 4);
    assert_eq!(catalog.entries[0].value_loc, 14);
}

#[test]
fn resource_limits_accept_boundaries_and_reject_excess() {
    let key = "é".repeat(128);
    let value = "é".repeat(32 * 1024);
    assert!(parse("en.json", &format!("{{{key:?}:{value:?}}}")).is_ok());
    assert!(
        parse("en.json", &format!("{{\"{key}x\":\"ok\"}}"))
            .unwrap_err()
            .diag
            .msg
            .contains("256 bytes")
    );
    assert!(
        parse("en.json", &format!("{{\"k\":\"{value}x\"}}"))
            .unwrap_err()
            .diag
            .msg
            .contains("64 KiB")
    );
    let file = format!("{{}}{}", " ".repeat(4 * 1024 * 1024 - 2));
    assert!(parse("en.json", &file).is_ok());
    assert!(
        parse("en.json", &(file + " "))
            .unwrap_err()
            .diag
            .msg
            .contains("4 MiB")
    );
    let entries = (0..20_000)
        .map(|i| format!("\"k{i}\":\"v\""))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        parse("en.json", &format!("{{{entries}}}"))
            .unwrap()
            .entries
            .len(),
        20_000
    );
    assert!(
        parse("en.json", &format!("{{{entries},\"extra\":\"v\"}}"))
            .unwrap_err()
            .diag
            .msg
            .contains("20000 entries")
    );
}
