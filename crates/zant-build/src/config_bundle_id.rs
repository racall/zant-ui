//! `bundle_id` validation shared by `zant create`, `zant build` and the
//! config parser.

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
