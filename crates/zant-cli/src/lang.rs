//! Language selection is explicit and independent of the system locale.

use std::sync::OnceLock;

static CHINESE: OnceLock<bool> = OnceLock::new();

pub fn zh() -> bool {
    *CHINESE.get().unwrap_or(&false)
}

/// `"zh"` or `"en"`: passed as `ZANT_LANG` to the scripts zant runs, so
/// they print in the same language.
pub fn tag() -> &'static str {
    if zh() { "zh" } else { "en" }
}

pub fn init(chinese: bool) {
    CHINESE.set(chinese).expect("language initialized once");
}

// Literal messages remain borrowed; formatted messages pass arguments once,
// and both format strings are checked by the compiler.
macro_rules! tr {
    (en: $en:literal, zh: $zh:literal) => {
        if $crate::lang::zh() { $zh } else { $en }
    };
    (en: $en:literal, zh: $zh:literal, $($args:tt)*) => {
        if $crate::lang::zh() {
            format!($zh, $($args)*)
        } else {
            format!($en, $($args)*)
        }
    };
}

fn parse(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "en" => Ok(false),
        "zh" | "zh-cn" | "zh-hans" => Ok(true),
        _ => Err(format!("Unknown language {value:?}; expected en or zh")),
    }
}

/// Remove language flags before clap parses subcommands. The last flag wins.
/// An explicit flag also overrides an invalid environment value.
pub fn extract(args: Vec<String>, env: Option<&str>) -> Result<(Vec<String>, bool), String> {
    let mut filtered = Vec::with_capacity(args.len());
    let mut language = None;
    let mut args = args.into_iter();
    if let Some(program) = args.next() {
        filtered.push(program);
    }
    while let Some(arg) = args.next() {
        if arg == "--lang" {
            let value = args.next().ok_or("--lang requires a value: en or zh")?;
            language = Some(parse(&value)?);
        } else if let Some(value) = arg.strip_prefix("--lang=") {
            language = Some(parse(value)?);
        } else {
            filtered.push(arg);
        }
    }
    let chinese = match language {
        Some(chinese) => chinese,
        None => env.map(parse).transpose()?.unwrap_or(false),
    };
    Ok((filtered, chinese))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select(args: &[&str], env: Option<&str>) -> Result<(Vec<String>, bool), String> {
        extract(args.iter().map(|arg| arg.to_string()).collect(), env)
    }

    #[test]
    fn flags_can_appear_anywhere_and_accept_aliases() {
        for args in [
            vec!["zant", "--lang", "ZH", "create", "demo"],
            vec!["zant", "create", "--lang=zh-CN", "demo"],
            vec!["zant", "create", "demo", "--lang", "zH-hAnS"],
        ] {
            assert_eq!(
                select(&args, None).unwrap(),
                (vec!["zant".into(), "create".into(), "demo".into()], true)
            );
        }
    }

    #[test]
    fn defaults_environment_and_precedence() {
        assert!(!select(&["zant"], None).unwrap().1);
        assert!(select(&["zant"], Some("zh")).unwrap().1);
        assert!(!select(&["zant"], Some("EN")).unwrap().1);
        assert!(!select(&["zant", "--lang=en"], Some("zh")).unwrap().1);
        assert!(select(&["zant", "--lang=zh"], Some("invalid")).unwrap().1);
        assert!(
            !select(&["zant", "--lang=zh", "--lang", "en"], None)
                .unwrap()
                .1
        );
    }

    #[test]
    fn invalid_and_missing_values_list_supported_languages() {
        for result in [
            select(&["zant", "--lang=fr"], None),
            select(&["zant", "--lang"], None),
            select(&["zant", "--lang="], None),
            select(&["zant"], Some("fr")),
        ] {
            let error = result.unwrap_err();
            assert!(error.contains("en") && error.contains("zh"), "{error}");
        }
    }

    #[test]
    fn translations_default_to_english_and_format_shared_arguments() {
        assert_eq!(tr!(en: "Hello", zh: "你好"), "Hello");
        assert_eq!(tr!(en: "Hello {0}", zh: "你好 {0}", "Zant"), "Hello Zant");
    }
}
