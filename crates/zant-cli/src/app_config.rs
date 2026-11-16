//! Shared application configuration parser and Rust source generator.
pub use zant_build::config::*;

/// Translate details returned by the shared validator without changing its API.
pub fn bundle_reason(reason: &str) -> &str {
    macro_rules! reason {
        (en: $en:literal, zh: $zh:literal) => {
            if reason == $zh {
                return tr!(en: $en, zh: $zh);
            }
        };
    }
    reason!(en: "Must be 3–50 characters long (Microsoft MSIX package name rule)", zh: "长度要在 3～50 个字符之间（微软 MSIX 包名规范）");
    reason!(en: "Use only ASCII letters, digits, - and . (Apple Bundle ID rule; no spaces, underscores, or Chinese characters)", zh: "只能包含英文字母、数字、- 和 .（苹果 Bundle ID 规范；不能有空格、下划线、中文）");
    reason!(en: "Use reverse-domain format with at least two parts, e.g. com.xx.test", zh: "要用反向域名格式，至少两段，如 com.xx.test");
    reason!(en: "Must start with an ASCII letter", zh: "要以英文字母开头");
    reason!(en: "Identifiers starting with com.apple are reserved by Apple", zh: "com.apple 开头的标识是苹果保留的");
    reason!(en: "Cannot start or end with . or contain consecutive dots", zh: "不能以 . 开头或结尾，也不能有连续的 ..");
    reason!(en: "Parts cannot start or end with -", zh: "每一段不能以 - 开头或结尾");
    reason!(en: "Parts cannot be CON, PRN, AUX, NUL, COM1–9, or LPT1–9 (Windows reserved names)", zh: "不能有 CON、PRN、AUX、NUL、COM1～9、LPT1～9 这样的段（Windows 保留名）");
    reason
}

pub fn bundle_error(id: &str) -> Option<&'static str> {
    zant_build::config::bundle_error(id).map(bundle_reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_bundle_validation_details_are_english_by_default() {
        for id in [
            "x",
            "com.应用",
            "demo",
            "1.app",
            "com.apple.demo",
            "com..app",
            "com.-app",
            "com.CON",
        ] {
            let reason = bundle_error(id).expect("invalid identifier");
            assert!(
                !reason
                    .chars()
                    .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
                "{id}: {reason}"
            );
        }
        assert!(bundle_error("com.acme.demo").is_none());
    }
}
