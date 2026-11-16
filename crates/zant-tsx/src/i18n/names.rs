//! Native language names for generated locale pickers.

pub(super) fn name(tag: &str) -> &str {
    match tag {
        "zh-Hans" | "zh-CN" | "zh-SG" => "简体中文",
        "zh-Hant" | "zh-TW" | "zh-HK" => "繁體中文",
        "pt-BR" => "Português (Brasil)",
        _ => match tag.split('-').next().unwrap_or(tag) {
            "en" => "English",
            "zh" => {
                if tag.starts_with("zh-Hant-") {
                    "繁體中文"
                } else {
                    "简体中文"
                }
            }
            "ja" => "日本語",
            "ko" => "한국어",
            "fr" => "Français",
            "de" => "Deutsch",
            "es" => "Español",
            "pt" => "Português",
            "it" => "Italiano",
            "ru" => "Русский",
            "ar" => "العربية",
            "hi" => "हिन्दी",
            "th" => "ไทย",
            "vi" => "Tiếng Việt",
            "id" => "Bahasa Indonesia",
            "tr" => "Türkçe",
            "nl" => "Nederlands",
            "pl" => "Polski",
            "uk" => "Українська",
            "sv" => "Svenska",
            _ => tag,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::name;

    #[test]
    fn specific_tags_precede_language_fallbacks() {
        for (tag, expected) in [
            ("zh-CN", "简体中文"),
            ("zh-Hant-HK", "繁體中文"),
            ("pt-BR", "Português (Brasil)"),
            ("pt-PT", "Português"),
            ("en-US", "English"),
            ("xx-YY", "xx-YY"),
        ] {
            assert_eq!(name(tag), expected);
        }
    }
}
