//! Port of `src/markdown/highlight.zig` — WP-3.2.
//!
//! Lightweight syntax highlighter (stands in for Streamdown's Shiki code
//! plugin). Tokenises per line so streaming code re-highlights cheaply.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
    Function,
    Type,
    Constant,
    Property,
    Tag,
    Attribute,
    Punctuation,
    Meta,
    Inserted,
    Deleted,
}

pub struct Token<'a> {
    pub text: &'a str,
    pub kind: Kind,
}

struct Spec {
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    constants: &'static [&'static str],
    line_comments: &'static [&'static str],
    block_comment: Option<[&'static str; 2]>,
    quotes: &'static str,
    backtick_strings: bool,
    case_insensitive: bool,
    /// `@decorator` / `#include` style meta at line start or after `@`.
    meta_prefix: Option<u8>,
    capitalized_types: bool,
    /// Lisp/shell-like variables `$name`.
    dollar_vars: bool,
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

const C_LIKE_KEYWORDS: &[&str] = &[
    "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "return",
    "goto", "sizeof", "typedef", "struct", "union", "enum", "static", "const", "extern", "inline",
    "volatile", "register", "unsigned", "signed",
];
const C_TYPES: &[&str] = &[
    "int", "char", "float", "double", "void", "long", "short", "bool", "size_t", "uint8_t",
    "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t", "int64_t",
];
const CPP_KEYWORDS: &[&str] = &[
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "return",
    "goto",
    "sizeof",
    "typedef",
    "struct",
    "union",
    "enum",
    "static",
    "const",
    "extern",
    "inline",
    "volatile",
    "register",
    "unsigned",
    "signed",
    "class",
    "public",
    "private",
    "protected",
    "virtual",
    "override",
    "template",
    "typename",
    "namespace",
    "using",
    "new",
    "delete",
    "try",
    "catch",
    "throw",
    "this",
    "auto",
    "constexpr",
    "noexcept",
    "operator",
    "friend",
    "nullptr",
    "co_await",
    "co_return",
];
const JS_KEYWORDS: &[&str] = &[
    "var",
    "let",
    "const",
    "function",
    "return",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "new",
    "delete",
    "typeof",
    "instanceof",
    "in",
    "of",
    "class",
    "extends",
    "super",
    "this",
    "import",
    "export",
    "from",
    "as",
    "async",
    "await",
    "yield",
    "try",
    "catch",
    "finally",
    "throw",
    "void",
    "static",
    "get",
    "set",
];
const TS_KEYWORDS: &[&str] = &[
    "var",
    "let",
    "const",
    "function",
    "return",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "new",
    "delete",
    "typeof",
    "instanceof",
    "in",
    "of",
    "class",
    "extends",
    "super",
    "this",
    "import",
    "export",
    "from",
    "as",
    "async",
    "await",
    "yield",
    "try",
    "catch",
    "finally",
    "throw",
    "void",
    "static",
    "get",
    "set",
    "interface",
    "type",
    "enum",
    "implements",
    "private",
    "public",
    "protected",
    "readonly",
    "declare",
    "namespace",
    "abstract",
    "keyof",
    "satisfies",
    "infer",
    "is",
];
const TS_TYPES: &[&str] = &[
    "string", "number", "boolean", "any", "unknown", "never", "void", "object", "bigint", "symbol",
];
const JS_CONSTANTS: &[&str] = &["true", "false", "null", "undefined", "NaN", "Infinity"];
const PY_KEYWORDS: &[&str] = &[
    "def", "class", "return", "if", "elif", "else", "for", "while", "in", "not", "and", "or", "is",
    "import", "from", "as", "with", "try", "except", "finally", "raise", "pass", "break",
    "continue", "lambda", "yield", "global", "nonlocal", "assert", "del", "async", "await",
    "match", "case",
];
const PY_CONSTANTS: &[&str] = &["True", "False", "None", "self", "cls"];
const PY_TYPES: &[&str] = &[
    "int", "str", "float", "bool", "list", "dict", "set", "tuple", "bytes", "object",
];
const GO_KEYWORDS: &[&str] = &[
    "func",
    "package",
    "import",
    "return",
    "if",
    "else",
    "for",
    "range",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "go",
    "defer",
    "select",
    "chan",
    "map",
    "struct",
    "interface",
    "type",
    "var",
    "const",
    "fallthrough",
    "goto",
];
const GO_TYPES: &[&str] = &[
    "int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint16", "uint32", "uint64",
    "float32", "float64", "string", "bool", "byte", "rune", "error", "any",
];
const RUST_KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", "trait", "for", "in",
    "while", "loop", "if", "else", "match", "return", "break", "continue", "as", "where", "move",
    "ref", "self", "Self", "crate", "super", "unsafe", "async", "await", "dyn", "const", "static",
    "type", "extern",
];
const RUST_TYPES: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32",
    "f64", "bool", "char", "str", "String", "Vec", "Option", "Result", "Box",
];
const ZIG_KEYWORDS: &[&str] = &[
    "const",
    "var",
    "fn",
    "pub",
    "return",
    "if",
    "else",
    "while",
    "for",
    "switch",
    "break",
    "continue",
    "defer",
    "errdefer",
    "try",
    "catch",
    "orelse",
    "and",
    "or",
    "struct",
    "enum",
    "union",
    "error",
    "comptime",
    "inline",
    "test",
    "usingnamespace",
    "export",
    "extern",
    "packed",
    "threadlocal",
    "unreachable",
    "async",
    "await",
    "suspend",
    "resume",
    "anytype",
    "noreturn",
];
const ZIG_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize", "f16", "f32", "f64",
    "bool", "void", "type", "anyerror", "c_int", "u21",
];
const ZIG_CONSTANTS: &[&str] = &["true", "false", "null", "undefined"];
const SWIFT_KEYWORDS: &[&str] = &[
    "let",
    "var",
    "func",
    "return",
    "if",
    "else",
    "guard",
    "for",
    "in",
    "while",
    "repeat",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "class",
    "struct",
    "enum",
    "protocol",
    "extension",
    "import",
    "init",
    "deinit",
    "self",
    "Self",
    "super",
    "throws",
    "throw",
    "try",
    "catch",
    "do",
    "async",
    "await",
    "public",
    "private",
    "internal",
    "fileprivate",
    "open",
    "static",
    "override",
    "mutating",
    "where",
    "some",
    "any",
    "inout",
    "typealias",
    "lazy",
    "weak",
];
const KOTLIN_KEYWORDS: &[&str] = &[
    "val",
    "var",
    "fun",
    "return",
    "if",
    "else",
    "when",
    "for",
    "in",
    "while",
    "do",
    "class",
    "object",
    "interface",
    "data",
    "sealed",
    "enum",
    "import",
    "package",
    "private",
    "public",
    "internal",
    "override",
    "open",
    "is",
    "as",
    "this",
    "super",
    "try",
    "catch",
    "finally",
    "throw",
    "suspend",
    "companion",
    "lateinit",
];
const JAVA_KEYWORDS: &[&str] = &[
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "return",
    "goto",
    "sizeof",
    "typedef",
    "struct",
    "union",
    "enum",
    "static",
    "const",
    "extern",
    "inline",
    "volatile",
    "register",
    "unsigned",
    "signed",
    "class",
    "public",
    "private",
    "protected",
    "virtual",
    "override",
    "template",
    "typename",
    "namespace",
    "using",
    "new",
    "delete",
    "try",
    "catch",
    "throw",
    "this",
    "auto",
    "constexpr",
    "noexcept",
    "operator",
    "friend",
    "nullptr",
    "co_await",
    "co_return",
    "extends",
    "implements",
    "interface",
    "package",
    "import",
    "final",
    "abstract",
    "synchronized",
    "instanceof",
    "record",
    "var",
];
const RUBY_KEYWORDS: &[&str] = &[
    "def",
    "end",
    "class",
    "module",
    "if",
    "elsif",
    "else",
    "unless",
    "while",
    "until",
    "for",
    "in",
    "do",
    "return",
    "yield",
    "begin",
    "rescue",
    "ensure",
    "raise",
    "require",
    "include",
    "self",
    "then",
    "case",
    "when",
    "and",
    "or",
    "not",
    "attr_accessor",
    "puts",
];
const SHELL_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while", "until", "case",
    "esac", "function", "return", "export", "local", "readonly", "echo", "cd", "exit", "source",
    "set", "unset", "sudo", "npm", "pnpm", "yarn", "git", "zig", "brew", "curl",
];
const SQL_KEYWORDS: &[&str] = &[
    "select",
    "from",
    "where",
    "and",
    "or",
    "not",
    "insert",
    "into",
    "values",
    "update",
    "set",
    "delete",
    "create",
    "table",
    "drop",
    "alter",
    "index",
    "join",
    "left",
    "right",
    "inner",
    "outer",
    "on",
    "group",
    "by",
    "order",
    "having",
    "limit",
    "offset",
    "as",
    "distinct",
    "union",
    "all",
    "primary",
    "key",
    "foreign",
    "references",
    "null",
    "is",
    "in",
    "like",
    "between",
    "case",
    "when",
    "then",
    "else",
    "end",
    "with",
    "default",
    "exists",
    "asc",
    "desc",
];
const SQL_TYPES: &[&str] = &[
    "int",
    "integer",
    "text",
    "varchar",
    "char",
    "boolean",
    "date",
    "timestamp",
    "real",
    "float",
    "decimal",
    "blob",
    "bigint",
    "serial",
    "uuid",
    "json",
    "jsonb",
];
const PHP_KEYWORDS: &[&str] = &[
    "function",
    "return",
    "if",
    "else",
    "elseif",
    "foreach",
    "for",
    "while",
    "as",
    "class",
    "public",
    "private",
    "protected",
    "static",
    "new",
    "echo",
    "namespace",
    "use",
    "extends",
    "implements",
    "try",
    "catch",
    "throw",
    "array",
    "fn",
    "match",
];
const LUA_KEYWORDS: &[&str] = &[
    "local", "function", "end", "if", "then", "else", "elseif", "for", "in", "do", "while",
    "repeat", "until", "return", "break", "and", "or", "not", "nil",
];
const CSS_KEYWORDS: &[&str] = &[
    "important",
    "media",
    "import",
    "keyframes",
    "from",
    "to",
    "supports",
    "font-face",
];
const DART_KEYWORDS: &[&str] = &[
    "var",
    "final",
    "const",
    "void",
    "class",
    "extends",
    "implements",
    "with",
    "return",
    "if",
    "else",
    "for",
    "in",
    "while",
    "switch",
    "case",
    "import",
    "async",
    "await",
    "new",
    "this",
    "super",
    "static",
    "late",
    "required",
];
const CS_KEYWORDS: &[&str] = &[
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "return",
    "goto",
    "sizeof",
    "typedef",
    "struct",
    "union",
    "enum",
    "static",
    "const",
    "extern",
    "inline",
    "volatile",
    "register",
    "unsigned",
    "signed",
    "class",
    "public",
    "private",
    "protected",
    "virtual",
    "override",
    "template",
    "typename",
    "namespace",
    "using",
    "new",
    "delete",
    "try",
    "catch",
    "throw",
    "this",
    "auto",
    "constexpr",
    "noexcept",
    "operator",
    "friend",
    "nullptr",
    "co_await",
    "co_return",
    "var",
    "readonly",
    "async",
    "await",
    "interface",
    "get",
    "set",
    "string",
    "object",
    "foreach",
    "in",
    "is",
    "as",
    "out",
    "ref",
    "params",
    "record",
    "init",
];
const DOCKER_KEYWORDS: &[&str] = &[
    "FROM",
    "RUN",
    "CMD",
    "COPY",
    "ADD",
    "WORKDIR",
    "ENV",
    "EXPOSE",
    "ENTRYPOINT",
    "ARG",
    "LABEL",
    "USER",
    "VOLUME",
];

fn spec_for(lang: &str) -> Option<Spec> {
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

fn is_markup(lang: &str) -> bool {
    let eq = |s: &str| lang.eq_ignore_ascii_case(s);
    eq("html") || eq("xml") || eq("svg") || eq("vue") || eq("svelte") || eq("htm") || eq("plist")
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

fn contains(list: &[&str], word: &str, ci: bool) -> bool {
    for w in list {
        if ci {
            if w.eq_ignore_ascii_case(word) {
                return true;
            }
        } else if *w == word {
            return true;
        }
    }
    false
}

/// Highlighter state carried across lines (open block comments / strings).
#[derive(Clone, Copy, Default)]
pub struct State {
    pub in_block_comment: bool,
    pub in_backtick: bool,
}

fn flush<'a>(out: &mut Vec<Token<'a>>, text: &'a str, from: usize, to: usize) {
    if to > from {
        out.push(Token {
            text: &text[from..to],
            kind: Kind::Plain,
        });
    }
}

fn index_of_pos(text: &str, from: usize, needle: &str) -> Option<usize> {
    let from = from.min(text.len());
    text.as_bytes()[from..]
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())
        .map(|p| p + from)
}

fn index_of_any_pos(text: &str, from: usize, set: &[u8]) -> Option<usize> {
    let from = from.min(text.len());
    text.as_bytes()[from..]
        .iter()
        .position(|c| set.contains(c))
        .map(|p| p + from)
}

/// Tokenises one line (without the newline).
pub fn line<'a>(lang: &str, text: &'a str, state: &mut State) -> Vec<Token<'a>> {
    let mut out: Vec<Token<'a>> = Vec::new();
    let bytes = text.as_bytes();
    if lang.eq_ignore_ascii_case("diff") || lang.eq_ignore_ascii_case("patch") {
        let kind: Kind = if !text.is_empty() && bytes[0] == b'+' && !text.starts_with("+++") {
            Kind::Inserted
        } else if !text.is_empty() && bytes[0] == b'-' && !text.starts_with("---") {
            Kind::Deleted
        } else if text.starts_with("@@") {
            Kind::Meta
        } else {
            Kind::Plain
        };
        out.push(Token { text, kind });
        return out;
    }
    if is_markup(lang) {
        return markup_line(text);
    }
    let Some(spec) = spec_for(lang) else {
        out.push(Token {
            text,
            kind: Kind::Plain,
        });
        return out;
    };
    let is_json = lang.eq_ignore_ascii_case("json") || lang.eq_ignore_ascii_case("jsonc");
    let is_yaml = lang.eq_ignore_ascii_case("yaml") || lang.eq_ignore_ascii_case("yml");
    let is_toml = lang.eq_ignore_ascii_case("toml") || lang.eq_ignore_ascii_case("ini");
    let mut i: usize = 0;
    let mut plain_start: usize = 0;

    // yaml / toml keys
    if is_yaml || is_toml {
        let trimmed_start = text
            .bytes()
            .position(|c| c != b' ' && c != b'\t' && c != b'-')
            .unwrap_or(text.len());
        if is_toml && trimmed_start < text.len() && bytes[trimmed_start] == b'[' {
            out.push(Token {
                text,
                kind: Kind::Type,
            });
            return out;
        }
        let sep = index_of_any_pos(text, trimmed_start, if is_yaml { b":" } else { b"=" });
        if let Some(sep) = sep {
            let key = &text[trimmed_start..sep];
            if !key.is_empty() && !key.bytes().any(|c| c == b'"' || c == b'\'' || c == b'#') {
                flush(&mut out, text, 0, trimmed_start);
                out.push(Token {
                    text: key,
                    kind: Kind::Property,
                });
                i = sep;
                plain_start = sep;
            }
        }
    }

    while i < text.len() {
        if state.in_block_comment {
            let close = spec.block_comment.expect("block comment spec")[1];
            let end = match index_of_pos(text, i, close) {
                Some(e) => e + close.len(),
                None => text.len(),
            };
            out.push(Token {
                text: &text[i..end],
                kind: Kind::Comment,
            });
            if end < text.len() || text[i..end].ends_with(close) {
                state.in_block_comment = false;
            }
            i = end;
            plain_start = i;
            continue;
        }
        if state.in_backtick {
            let end = match text[i..].find('`') {
                Some(e) => i + e + 1,
                None => text.len(),
            };
            out.push(Token {
                text: &text[i..end],
                kind: Kind::String,
            });
            if end <= text.len() && end > i && bytes[end - 1] == b'`' {
                state.in_backtick = false;
            }
            i = end;
            plain_start = i;
            continue;
        }
        let c = bytes[i];
        // comments
        let mut matched_comment = false;
        for lc in spec.line_comments {
            if text[i..].starts_with(lc) {
                // `#` in shell-ish langs only starts a comment after whitespace
                if lc.len() == 1
                    && lc.as_bytes()[0] == b'#'
                    && i > 0
                    && !bytes[i - 1].is_ascii_whitespace()
                {
                    break;
                }
                flush(&mut out, text, plain_start, i);
                out.push(Token {
                    text: &text[i..],
                    kind: Kind::Comment,
                });
                i = text.len();
                plain_start = i;
                matched_comment = true;
                break;
            }
        }
        if matched_comment {
            break;
        }
        if let Some(bc) = spec.block_comment
            && text[i..].starts_with(bc[0])
        {
            flush(&mut out, text, plain_start, i);
            state.in_block_comment = true;
            let end = match index_of_pos(text, i + bc[0].len(), bc[1]) {
                Some(e) => {
                    state.in_block_comment = false;
                    e + bc[1].len()
                }
                None => text.len(),
            };
            out.push(Token {
                text: &text[i..end],
                kind: Kind::Comment,
            });
            i = end;
            plain_start = i;
            continue;
        }
        // strings
        if spec.quotes.as_bytes().contains(&c) || (c == b'`' && spec.backtick_strings) {
            flush(&mut out, text, plain_start, i);
            let mut j = i + 1;
            while j < text.len() && bytes[j] != c {
                if bytes[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(text.len());
            if c == b'`' && j >= text.len() {
                state.in_backtick = true;
            }
            let mut kind: Kind = Kind::String;
            // JSON keys: a string followed by ':'
            if is_json {
                let mut k = end;
                while k < text.len() && bytes[k] == b' ' {
                    k += 1;
                }
                if k < text.len() && bytes[k] == b':' {
                    kind = Kind::Property;
                }
            }
            out.push(Token {
                text: &text[i..end],
                kind,
            });
            i = end;
            plain_start = i;
            continue;
        }
        // numbers
        if c.is_ascii_digit() && (i == 0 || !is_ident(bytes[i - 1])) {
            flush(&mut out, text, plain_start, i);
            let mut j = i + 1;
            while j < text.len()
                && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'.' || bytes[j] == b'_')
            {
                j += 1;
            }
            out.push(Token {
                text: &text[i..j],
                kind: Kind::Number,
            });
            i = j;
            plain_start = i;
            continue;
        }
        // meta: @decorator, #include, @builtin
        if let Some(mp) = spec.meta_prefix
            && c == mp
            && i + 1 < text.len()
            && is_ident_start(bytes[i + 1])
        {
            let at_line_start = text[..i].trim_matches([' ', '\t']).is_empty();
            if mp == b'@' || at_line_start {
                flush(&mut out, text, plain_start, i);
                let mut j = i + 1;
                while j < text.len() && is_ident(bytes[j]) {
                    j += 1;
                }
                let kind = if mp == b'@' && !lang.eq_ignore_ascii_case("zig") {
                    Kind::Function
                } else {
                    Kind::Meta
                };
                out.push(Token {
                    text: &text[i..j],
                    kind,
                });
                i = j;
                plain_start = i;
                continue;
            }
        }
        if spec.dollar_vars
            && c == b'$'
            && i + 1 < text.len()
            && (is_ident_start(bytes[i + 1]) || bytes[i + 1] == b'{')
        {
            flush(&mut out, text, plain_start, i);
            let mut j = i + 1;
            if bytes[j] == b'{' {
                j = match index_of_any_pos(text, j, b"}") {
                    Some(e) => e + 1,
                    None => text.len(),
                };
            } else {
                while j < text.len() && is_ident(bytes[j]) {
                    j += 1;
                }
            }
            out.push(Token {
                text: &text[i..j],
                kind: Kind::Constant,
            });
            i = j;
            plain_start = i;
            continue;
        }
        // identifiers
        if is_ident_start(c) && (i == 0 || !is_ident(bytes[i - 1])) {
            let mut j = i + 1;
            while j < text.len()
                && (is_ident(bytes[j]) || (bytes[j] == b'-' && lang.eq_ignore_ascii_case("css")))
            {
                j += 1;
            }
            let word = &text[i..j];
            let mut next = j;
            while next < text.len() && bytes[next] == b' ' {
                next += 1;
            }
            let kind: Option<Kind> = if contains(spec.keywords, word, spec.case_insensitive) {
                Some(Kind::Keyword)
            } else if contains(spec.constants, word, spec.case_insensitive) {
                Some(Kind::Constant)
            } else if contains(spec.types, word, spec.case_insensitive) {
                Some(Kind::Type)
            } else if next < text.len() && bytes[next] == b'(' {
                Some(Kind::Function)
            } else if spec.capitalized_types && bytes[i].is_ascii_uppercase() && word.len() > 1 {
                Some(Kind::Type)
            } else if i > 0 && bytes[i - 1] == b'.' {
                Some(Kind::Property)
            } else {
                None
            };
            if let Some(k) = kind {
                flush(&mut out, text, plain_start, i);
                out.push(Token {
                    text: word,
                    kind: k,
                });
                plain_start = j;
            }
            i = j;
            continue;
        }
        if b"{}[]();,.:=<>+-*/%!&|^~?".contains(&c) {
            flush(&mut out, text, plain_start, i);
            out.push(Token {
                text: &text[i..i + 1],
                kind: Kind::Punctuation,
            });
            i += 1;
            plain_start = i;
            continue;
        }
        i += 1;
    }
    flush(&mut out, text, plain_start, text.len());
    out
}

fn markup_line(text: &str) -> Vec<Token<'_>> {
    let mut out: Vec<Token<'_>> = Vec::new();
    let bytes = text.as_bytes();
    let mut i: usize = 0;
    let mut plain: usize = 0;
    while i < text.len() {
        if text[i..].starts_with("<!--") {
            if i > plain {
                out.push(Token {
                    text: &text[plain..i],
                    kind: Kind::Plain,
                });
            }
            let end = match text[i..].find("-->") {
                Some(e) => i + e + 3,
                None => text.len(),
            };
            out.push(Token {
                text: &text[i..end],
                kind: Kind::Comment,
            });
            i = end;
            plain = i;
            continue;
        }
        if bytes[i] == b'<' {
            if i > plain {
                out.push(Token {
                    text: &text[plain..i],
                    kind: Kind::Plain,
                });
            }
            let mut j = i + 1;
            if j < text.len() && (bytes[j] == b'/' || bytes[j] == b'!' || bytes[j] == b'?') {
                j += 1;
            }
            out.push(Token {
                text: &text[i..j],
                kind: Kind::Punctuation,
            });
            let name_start = j;
            while j < text.len() && (is_ident(bytes[j]) || bytes[j] == b'-' || bytes[j] == b':') {
                j += 1;
            }
            if j > name_start {
                out.push(Token {
                    text: &text[name_start..j],
                    kind: Kind::Tag,
                });
            }
            // attributes until '>'
            while j < text.len() && bytes[j] != b'>' {
                if bytes[j] == b'"' || bytes[j] == b'\'' {
                    let q = bytes[j];
                    let end = match text[j + 1..].find(q as char) {
                        Some(e) => j + 1 + e + 1,
                        None => text.len(),
                    };
                    out.push(Token {
                        text: &text[j..end],
                        kind: Kind::String,
                    });
                    j = end;
                } else if is_ident_start(bytes[j]) {
                    let s = j;
                    while j < text.len()
                        && (is_ident(bytes[j]) || bytes[j] == b'-' || bytes[j] == b':')
                    {
                        j += 1;
                    }
                    out.push(Token {
                        text: &text[s..j],
                        kind: Kind::Attribute,
                    });
                } else {
                    out.push(Token {
                        text: &text[j..j + 1],
                        kind: Kind::Punctuation,
                    });
                    j += 1;
                }
            }
            if j < text.len() {
                out.push(Token {
                    text: &text[j..j + 1],
                    kind: Kind::Punctuation,
                });
                j += 1;
            }
            i = j;
            plain = i;
            continue;
        }
        i += 1;
    }
    if plain < text.len() {
        out.push(Token {
            text: &text[plain..],
            kind: Kind::Plain,
        });
    }
    out
}

const EXTENSION_MAP: &[(&str, &str)] = &[
    ("bash", "sh"),
    ("sh", "sh"),
    ("shell", "sh"),
    ("zsh", "zsh"),
    ("console", "sh"),
    ("c", "c"),
    ("c++", "cpp"),
    ("cpp", "cpp"),
    ("c#", "cs"),
    ("cs", "cs"),
    ("csharp", "cs"),
    ("css", "css"),
    ("scss", "scss"),
    ("csv", "csv"),
    ("dart", "dart"),
    ("diff", "diff"),
    ("docker", "dockerfile"),
    ("dockerfile", "dockerfile"),
    ("go", "go"),
    ("golang", "go"),
    ("html", "html"),
    ("java", "java"),
    ("javascript", "js"),
    ("js", "js"),
    ("jsx", "jsx"),
    ("json", "json"),
    ("kotlin", "kt"),
    ("kt", "kt"),
    ("lua", "lua"),
    ("markdown", "md"),
    ("md", "md"),
    ("mermaid", "mmd"),
    ("objc", "m"),
    ("php", "php"),
    ("python", "py"),
    ("py", "py"),
    ("ruby", "rb"),
    ("rb", "rb"),
    ("rust", "rs"),
    ("rs", "rs"),
    ("sql", "sql"),
    ("swift", "swift"),
    ("toml", "toml"),
    ("ts", "ts"),
    ("typescript", "ts"),
    ("tsx", "tsx"),
    ("xml", "xml"),
    ("yaml", "yaml"),
    ("yml", "yml"),
    ("zig", "zig"),
    ("latex", "tex"),
    ("tex", "tex"),
    ("makefile", "mk"),
    ("vue", "vue"),
    ("svelte", "svelte"),
];

/// Download file extension for a fence language (Streamdown
/// `languageExtensionMap`, common subset). Unknown → "txt".
pub fn extension(lang: &str) -> &'static str {
    for (name, ext) in EXTENSION_MAP {
        if lang.eq_ignore_ascii_case(name) {
            return ext;
        }
    }
    "txt"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zig_line() {
        let mut state = State::default();
        let toks = line("zig", "const x: u32 = 42; // hi", &mut state);
        assert_eq!(toks[0].kind, Kind::Keyword);
        let has_num = toks.iter().any(|t| t.kind == Kind::Number);
        let has_comment = toks.iter().any(|t| t.kind == Kind::Comment);
        assert!(has_num && has_comment);
    }

    #[test]
    fn block_comments_span_lines() {
        let mut state = State::default();
        let _ = line("js", "a /* start", &mut state);
        assert!(state.in_block_comment);
        let toks = line("js", "end */ b", &mut state);
        assert_eq!(toks[0].kind, Kind::Comment);
        assert!(!state.in_block_comment);
    }
}
