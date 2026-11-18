//! Language rules used by the line tokenizer.

use super::keywords::{
    C_LIKE_KEYWORDS, C_TYPES, CPP_KEYWORDS, CS_KEYWORDS, DART_KEYWORDS, GO_KEYWORDS, GO_TYPES,
    JAVA_KEYWORDS, KOTLIN_KEYWORDS, RUST_KEYWORDS, RUST_TYPES, SWIFT_KEYWORDS, ZIG_CONSTANTS,
    ZIG_KEYWORDS, ZIG_TYPES,
};
use super::script_keywords::{
    CSS_KEYWORDS, DOCKER_KEYWORDS, JS_CONSTANTS, JS_KEYWORDS, LUA_KEYWORDS, PHP_KEYWORDS,
    PY_CONSTANTS, PY_KEYWORDS, PY_TYPES, RUBY_KEYWORDS, SHELL_KEYWORDS, SQL_KEYWORDS, SQL_TYPES,
    TS_KEYWORDS, TS_TYPES,
};

pub(super) struct Spec {
    pub(super) keywords: &'static [&'static str],
    pub(super) types: &'static [&'static str],
    pub(super) constants: &'static [&'static str],
    pub(super) line_comments: &'static [&'static str],
    pub(super) block_comment: Option<[&'static str; 2]>,
    pub(super) quotes: &'static str,
    pub(super) backtick_strings: bool,
    pub(super) case_insensitive: bool,
    /// `@decorator` / `#include` style meta at line start or after `@`.
    pub(super) meta_prefix: Option<u8>,
    pub(super) capitalized_types: bool,
    /// Lisp/shell-like variables `$name`.
    pub(super) dollar_vars: bool,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            keywords: &[],
            types: &[],
            constants: &["true", "false", "null"],
            line_comments: &["//"],
            block_comment: Some(["/*", "*/"]),
            quotes: "\"'",
            backtick_strings: false,
            case_insensitive: false,
            meta_prefix: None,
            capitalized_types: true,
            dollar_vars: false,
        }
    }
}

pub(super) fn spec_for(lang: &str) -> Option<Spec> {
    let eq = |s: &str| lang.eq_ignore_ascii_case(s);
    if eq("zig") {
        return Some(Spec {
            keywords: ZIG_KEYWORDS,
            types: ZIG_TYPES,
            constants: ZIG_CONSTANTS,
            block_comment: None,
            meta_prefix: Some(b'@'),
            ..Default::default()
        });
    }
    if eq("c") || eq("h") {
        return Some(Spec {
            keywords: C_LIKE_KEYWORDS,
            types: C_TYPES,
            constants: &["NULL", "true", "false"],
            meta_prefix: Some(b'#'),
            ..Default::default()
        });
    }
    if eq("cpp") || eq("c++") || eq("cc") || eq("hpp") || eq("objc") || eq("objective-c") {
        return Some(Spec {
            keywords: CPP_KEYWORDS,
            types: C_TYPES,
            constants: &["nullptr", "true", "false", "NULL", "nil", "YES", "NO"],
            meta_prefix: Some(b'#'),
            ..Default::default()
        });
    }
    if eq("js") || eq("javascript") || eq("jsx") || eq("mjs") || eq("cjs") {
        return Some(Spec {
            keywords: JS_KEYWORDS,
            constants: JS_CONSTANTS,
            quotes: "\"'",
            backtick_strings: true,
            ..Default::default()
        });
    }
    if eq("ts") || eq("typescript") || eq("tsx") {
        return Some(Spec {
            keywords: TS_KEYWORDS,
            types: TS_TYPES,
            constants: JS_CONSTANTS,
            backtick_strings: true,
            meta_prefix: Some(b'@'),
            ..Default::default()
        });
    }
    if eq("py") || eq("python") {
        return Some(Spec {
            keywords: PY_KEYWORDS,
            types: PY_TYPES,
            constants: PY_CONSTANTS,
            line_comments: &["#"],
            block_comment: None,
            meta_prefix: Some(b'@'),
            ..Default::default()
        });
    }
    if eq("go") || eq("golang") {
        return Some(Spec {
            keywords: GO_KEYWORDS,
            types: GO_TYPES,
            constants: &["true", "false", "nil", "iota"],
            backtick_strings: true,
            ..Default::default()
        });
    }
    if eq("rust") || eq("rs") {
        return Some(Spec {
            keywords: RUST_KEYWORDS,
            types: RUST_TYPES,
            constants: &["true", "false", "None", "Some", "Ok", "Err"],
            quotes: "\"",
            meta_prefix: Some(b'#'),
            ..Default::default()
        });
    }
    if eq("swift") {
        return Some(Spec {
            keywords: SWIFT_KEYWORDS,
            constants: &["true", "false", "nil"],
            meta_prefix: Some(b'@'),
            ..Default::default()
        });
    }
    if eq("kotlin") || eq("kt") {
        return Some(Spec {
            keywords: KOTLIN_KEYWORDS,
            constants: &["true", "false", "null"],
            meta_prefix: Some(b'@'),
            ..Default::default()
        });
    }
    if eq("java") {
        return Some(Spec {
            keywords: JAVA_KEYWORDS,
            types: C_TYPES,
            meta_prefix: Some(b'@'),
            ..Default::default()
        });
    }
    if eq("cs") || eq("csharp") || eq("c#") {
        return Some(Spec {
            keywords: CS_KEYWORDS,
            types: C_TYPES,
            meta_prefix: Some(b'#'),
            ..Default::default()
        });
    }
    if eq("dart") {
        return Some(Spec {
            keywords: DART_KEYWORDS,
            meta_prefix: Some(b'@'),
            ..Default::default()
        });
    }
    if eq("rb") || eq("ruby") {
        return Some(Spec {
            keywords: RUBY_KEYWORDS,
            constants: &["true", "false", "nil"],
            line_comments: &["#"],
            block_comment: None,
            ..Default::default()
        });
    }
    if eq("php") {
        return Some(Spec {
            keywords: PHP_KEYWORDS,
            line_comments: &["//", "#"],
            dollar_vars: true,
            ..Default::default()
        });
    }
    if eq("lua") {
        return Some(Spec {
            keywords: LUA_KEYWORDS,
            constants: &["true", "false", "nil"],
            line_comments: &["--"],
            block_comment: Some(["--[[", "]]"]),
            ..Default::default()
        });
    }
    if eq("sh") || eq("bash") || eq("shell") || eq("zsh") || eq("console") || eq("fish") {
        return Some(Spec {
            keywords: SHELL_KEYWORDS,
            line_comments: &["#"],
            block_comment: None,
            dollar_vars: true,
            capitalized_types: false,
            ..Default::default()
        });
    }
    if eq("sql") || eq("postgres") || eq("mysql") || eq("sqlite") {
        return Some(Spec {
            keywords: SQL_KEYWORDS,
            types: SQL_TYPES,
            line_comments: &["--"],
            case_insensitive: true,
            capitalized_types: false,
            ..Default::default()
        });
    }
    if eq("css") || eq("scss") || eq("less") {
        return Some(Spec {
            keywords: CSS_KEYWORDS,
            line_comments: if eq("css") { &[] } else { &["//"] },
            meta_prefix: Some(b'@'),
            capitalized_types: false,
            ..Default::default()
        });
    }
    if eq("json") || eq("jsonc") || eq("json5") {
        return Some(Spec {
            line_comments: &["//"],
            quotes: "\"",
            capitalized_types: false,
            ..Default::default()
        });
    }
    if eq("yaml") || eq("yml") || eq("toml") || eq("ini") || eq("env") || eq("dotenv") {
        return Some(Spec {
            constants: &["true", "false", "null", "yes", "no", "on", "off"],
            line_comments: &["#"],
            block_comment: None,
            capitalized_types: false,
            ..Default::default()
        });
    }
    if eq("dockerfile") || eq("docker") || eq("makefile") || eq("make") {
        return Some(Spec {
            keywords: DOCKER_KEYWORDS,
            line_comments: &["#"],
            block_comment: None,
            dollar_vars: true,
            capitalized_types: false,
            ..Default::default()
        });
    }
    None
}
