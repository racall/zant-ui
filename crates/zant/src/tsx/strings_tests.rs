//! Ported tests of `src/tsx_strings_test.zig` (plus the inline tests of
//! `src/tsx_strings.zig`): same names, same assertions.

use super::*;
use crate::tsx::List;

fn eq(expected: &str, got: Str) {
    assert_eq!(expected, got.as_str());
}

#[test]
fn string_queries_do_not_need_a_ui_frame_and_report_utf16_offsets() {
    assert!(includes("Hello", "ell"));
    assert!(!includes("", "中"));
    assert!(includes("", ""));
    assert!(starts_with("中😀abc", "中😀"));
    assert!(!starts_with("abc", "bc"));
    assert!(starts_with("", ""));
    assert!(ends_with("中😀abc", "abc"));
    assert!(ends_with("", ""));
    assert!(!ends_with("abc", "ab"));
    assert_eq!(index_of("中😀abc😀", "abc"), 3);
    assert_eq!(last_index_of("中😀abc😀", "😀"), 6);
    assert_eq!(index_of("", "a"), -1);
    assert_eq!(last_index_of("中😀", "a"), -1);
    assert_eq!(index_of("中😀", ""), 0);
    assert_eq!(last_index_of("中😀", ""), 3);
    assert_eq!(last_index_of("", ""), 0);
    assert_eq!(last_index_of("aaaa", "aa"), 2);
}

#[test]
fn trim_recognizes_js_whitespace_excluding_nel_and_zero_width_space() {
    let ws = " \t\r\n\u{0b}\u{0c}\u{a0}\u{feff}\u{2028}\u{2029}\
              \u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\
              \u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{202f}\u{205f}\u{3000}";
    let text = format!("{ws}中😀 A{ws}");
    eq("中😀 A", trim(&text));
    let expected_start = format!("中😀 A{ws}");
    eq(&expected_start, trim_start(&text));
    let expected_end = format!("{ws}中😀 A");
    eq(&expected_end, trim_end(&text));
    eq("\u{85}\u{200b}", trim("\u{85}\u{200b}"));
    eq("", trim(ws));
    eq("", trim_start(""));
    eq("", trim_end(""));
}

#[test]
fn ascii_case_mapping_preserves_non_ascii_bytes() {
    eq("hello 中😀 Äé", to_lower_case("HeLLo 中😀 Äé"));
    eq("HELLO 中😀 Äé", to_upper_case("HeLLo 中😀 Äé"));
    eq("", to_lower_case(""));
    eq("", to_upper_case(""));
}

#[test]
fn slice_and_substring_clamp_truncate_swap_and_round_utf16_boundaries_down() {
    eq("bc", slice("abcd", 1.9, Some(3.8)));
    eq("abcd", slice("abcd", -0.5, None));
    eq("", slice("abcd", 0.0, Some(-0.5)));
    eq("cd", slice("abcd", -2.0, None));
    eq("bc", slice("abcd", -3.0, Some(-1.0)));
    eq("", slice("abcd", 3.0, Some(1.0)));
    eq("中😀", slice("A中😀Z", 1.0, Some(-1.0)));
    eq("", slice("😀", 0.0, Some(1.0)));
    eq("😀", slice("😀", 1.0, Some(2.0)));
    eq("", slice("", -1.0, None));
    eq("abcd", slice("abcd", -100.0, Some(100.0)));
    eq("bc", substring("abcd", 3.0, Some(1.0)));
    eq("ab", substring("abcd", -3.0, Some(2.0)));
    eq("中😀Z", substring("A中😀Z", 1.0, None));
    eq("", substring("😀", 1.0, Some(0.0)));
    eq("", substring("", -1.0, Some(99.0)));
    eq("a", substring("abc", f64::NAN, Some(1.0)));
    eq("", slice("abc", f64::INFINITY, None));
    eq(
        "abc",
        substring("abc", f64::NEG_INFINITY, Some(f64::INFINITY)),
    );
}

#[test]
fn padding_and_repeat_use_frame_memory_with_utf16_target_lengths() {
    eq("ababx", pad_start("x", 5.0, "ab"));
    eq("xabab", pad_end("x", 5.9, "ab"));
    eq(" 中😀", pad_start("中😀", 4.0, " "));
    eq("中😀ab", pad_end("中😀", 5.0, "ab"));
    eq("😀x", pad_start("x", 4.0, "😀"));
    eq("中", pad_end("中", -2.0, "ab"));
    eq("中", pad_start("中", 9.0, ""));
    eq("", pad_start("", 0.0, "x"));
    eq("xx", pad_end("", 2.0, "x"));
    eq("a中😀a中😀", repeat("a中😀", 2.9));
    eq("", repeat("x", -0.5));
    eq("", repeat("", 100.0));
    eq("", repeat("x", f64::NAN));
}

#[test]
fn replace_handles_literal_patterns_empty_matches_and_js_dollar_substitutions() {
    eq("x-b-a", replace("a-b-a", "a", "x"));
    eq("x-b-x", replace_all("a-b-a", "a", "x"));
    eq("[中]-[中]", replace_all("中-中", "中", "[$&]"));
    eq("中x中x", replace_all("中😀中😀", "😀", "x"));
    eq("aaa", replace_all("aaaaa", "aa", "a"));
    eq("abc", replace("abc", "z", "x"));
    eq("a$-b-a-c-$1c", replace("abc", "b", "$$-$&-$`-$'-$1"));
    eq("-a-b-", replace_all("ab", "", "-"));
    eq("--😀-", replace_all("😀", "", "-"));
    eq("x中😀", replace("中😀", "", "x"));
    eq("", replace("", "a", "b"));
    eq("x", replace_all("", "", "x"));
}

#[test]
fn split_handles_empty_fields_cjk_emoji_and_empty_separators_by_code_point() {
    let parts = split(",中,😀,", ",");
    assert_eq!(parts.len(), 4);
    eq("", parts[0].clone());
    eq("中", parts[1].clone());
    eq("😀", parts[2].clone());
    eq("", parts[3].clone());
    assert_eq!(split("", "").len(), 0);
    let empty = split("", ",");
    eq("", empty[0].clone());
    let plain = split("abc", "z");
    eq("abc", plain[0].clone());
    let code_points = split("A中😀", "");
    assert_eq!(code_points.len(), 3);
    eq("A", code_points[0].clone());
    eq("中", code_points[1].clone());
    eq("😀", code_points[2].clone());
}

#[test]
fn char_at_and_at_share_rounded_boundaries_at_supports_negative_positions_and_null() {
    eq("中", char_at("A中😀", 1.9));
    eq("", char_at("A中😀", -1.0));
    eq("", char_at("A中😀", 4.0));
    eq("", char_at("😀", 0.0));
    eq("😀", char_at("😀", 1.0));
    eq("", char_at("", 0.0));
    eq("A", char_at("ABC", f64::NAN));
    eq("", char_at("ABC", f64::INFINITY));
    eq("😀", at("A中😀", -1.0).expect("in range"));
    eq("中", at("A中😀", -3.0).expect("in range"));
    eq("A", at("ABC", -0.5).expect("in range"));
    assert_eq!(at("A中😀", -5.0), None);
    assert_eq!(at("A", 1.0), None);
    assert_eq!(at("", 0.0), None);
    assert_eq!(at("ABC", f64::NEG_INFINITY), None);
}

#[test]
fn chars_iterates_by_code_point() {
    let got = chars("a剪b😀");
    assert_eq!(got.len(), 4);
    eq("a", got[0].clone());
    eq("剪", got[1].clone());
    eq("b", got[2].clone());
    eq("😀", got[3].clone());
    assert_eq!(chars("").len(), 0);
    // the Zig test also covered a raw invalid byte (0xff) on the byte-slice
    // side; Rust `Str` is always valid UTF-8, so there is no such element.
}

#[test]
fn char_code_at_reports_utf16_units() {
    assert_eq!(char_code_at("a|", 0.0), 97.0);
    assert_eq!(char_code_at("a|", 1.0), 124.0);
    assert!(char_code_at("a", 3.0).is_nan());
    assert_eq!(char_code_at("剪", 0.0), 0x526a as f64);
    assert_eq!(char_code_at("😀", 0.0), 0xd83d as f64);
    assert_eq!(char_code_at("😀", 1.0), 0xde00 as f64);
    assert_eq!(code_point_at("😀", 0.0), Some(0x1f600 as f64));
    assert_eq!(code_point_at("😀", 1.0), Some(0xde00 as f64));
    assert_eq!(code_point_at("a", 1.0), None);
}

#[test]
fn lists_hold_every_produced_string() {
    let parts: List<Str> = split("a,b", ",");
    assert_eq!(parts, List::from_vec(vec![Str::from("a"), Str::from("b")]));
}
