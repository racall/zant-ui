//! Lightweight syntax highlighter (stands in for Streamdown's Shiki code
//! plugin). Tokenises per line so streaming code re-highlights cheaply.
const std = @import("std");

pub const Kind = enum { plain, keyword, string, comment, number, function, type, constant, property, tag, attribute, punctuation, meta, inserted, deleted };

pub const Token = struct { text: []const u8, kind: Kind };

const Spec = struct {
    keywords: []const []const u8 = &.{},
    types: []const []const u8 = &.{},
    constants: []const []const u8 = &.{ "true", "false", "null" },
    line_comments: []const []const u8 = &.{"//"},
    block_comment: ?[2][]const u8 = .{ "/*", "*/" },
    quotes: []const u8 = "\"'",
    backtick_strings: bool = false,
    case_insensitive: bool = false,
    /// `@decorator` / `#include` style meta at line start or after `@`.
    meta_prefix: ?u8 = null,
    capitalized_types: bool = true,
    /// Lisp/shell-like variables `$name`.
    dollar_vars: bool = false,
};

const c_like_keywords = [_][]const u8{ "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "return", "goto", "sizeof", "typedef", "struct", "union", "enum", "static", "const", "extern", "inline", "volatile", "register", "unsigned", "signed" };
const c_types = [_][]const u8{ "int", "char", "float", "double", "void", "long", "short", "bool", "size_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t", "int64_t" };
const cpp_keywords = c_like_keywords ++ [_][]const u8{ "class", "public", "private", "protected", "virtual", "override", "template", "typename", "namespace", "using", "new", "delete", "try", "catch", "throw", "this", "auto", "constexpr", "noexcept", "operator", "friend", "nullptr", "co_await", "co_return" };
const js_keywords = [_][]const u8{ "var", "let", "const", "function", "return", "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "new", "delete", "typeof", "instanceof", "in", "of", "class", "extends", "super", "this", "import", "export", "from", "as", "async", "await", "yield", "try", "catch", "finally", "throw", "void", "static", "get", "set" };
const ts_keywords = js_keywords ++ [_][]const u8{ "interface", "type", "enum", "implements", "private", "public", "protected", "readonly", "declare", "namespace", "abstract", "keyof", "satisfies", "infer", "is" };
const ts_types = [_][]const u8{ "string", "number", "boolean", "any", "unknown", "never", "void", "object", "bigint", "symbol" };
const js_constants = [_][]const u8{ "true", "false", "null", "undefined", "NaN", "Infinity" };
const py_keywords = [_][]const u8{ "def", "class", "return", "if", "elif", "else", "for", "while", "in", "not", "and", "or", "is", "import", "from", "as", "with", "try", "except", "finally", "raise", "pass", "break", "continue", "lambda", "yield", "global", "nonlocal", "assert", "del", "async", "await", "match", "case" };
const py_constants = [_][]const u8{ "True", "False", "None", "self", "cls" };
const py_types = [_][]const u8{ "int", "str", "float", "bool", "list", "dict", "set", "tuple", "bytes", "object" };
const go_keywords = [_][]const u8{ "func", "package", "import", "return", "if", "else", "for", "range", "switch", "case", "default", "break", "continue", "go", "defer", "select", "chan", "map", "struct", "interface", "type", "var", "const", "fallthrough", "goto" };
const go_types = [_][]const u8{ "int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint16", "uint32", "uint64", "float32", "float64", "string", "bool", "byte", "rune", "error", "any" };
const rust_keywords = [_][]const u8{ "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", "trait", "for", "in", "while", "loop", "if", "else", "match", "return", "break", "continue", "as", "where", "move", "ref", "self", "Self", "crate", "super", "unsafe", "async", "await", "dyn", "const", "static", "type", "extern" };
const rust_types = [_][]const u8{ "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "bool", "char", "str", "String", "Vec", "Option", "Result", "Box" };
const zig_keywords = [_][]const u8{ "const", "var", "fn", "pub", "return", "if", "else", "while", "for", "switch", "break", "continue", "defer", "errdefer", "try", "catch", "orelse", "and", "or", "struct", "enum", "union", "error", "comptime", "inline", "test", "usingnamespace", "export", "extern", "packed", "threadlocal", "unreachable", "async", "await", "suspend", "resume", "anytype", "noreturn" };
const zig_types = [_][]const u8{ "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize", "f16", "f32", "f64", "bool", "void", "type", "anyerror", "c_int", "u21" };
const zig_constants = [_][]const u8{ "true", "false", "null", "undefined" };
const swift_keywords = [_][]const u8{ "let", "var", "func", "return", "if", "else", "guard", "for", "in", "while", "repeat", "switch", "case", "default", "break", "continue", "class", "struct", "enum", "protocol", "extension", "import", "init", "deinit", "self", "Self", "super", "throws", "throw", "try", "catch", "do", "async", "await", "public", "private", "internal", "fileprivate", "open", "static", "override", "mutating", "where", "some", "any", "inout", "typealias", "lazy", "weak" };
const kotlin_keywords = [_][]const u8{ "val", "var", "fun", "return", "if", "else", "when", "for", "in", "while", "do", "class", "object", "interface", "data", "sealed", "enum", "import", "package", "private", "public", "internal", "override", "open", "is", "as", "this", "super", "try", "catch", "finally", "throw", "suspend", "companion", "lateinit" };
const java_keywords = cpp_keywords ++ [_][]const u8{ "extends", "implements", "interface", "package", "import", "final", "abstract", "synchronized", "instanceof", "record", "var" };
const ruby_keywords = [_][]const u8{ "def", "end", "class", "module", "if", "elsif", "else", "unless", "while", "until", "for", "in", "do", "return", "yield", "begin", "rescue", "ensure", "raise", "require", "include", "self", "then", "case", "when", "and", "or", "not", "attr_accessor", "puts" };
const shell_keywords = [_][]const u8{ "if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while", "until", "case", "esac", "function", "return", "export", "local", "readonly", "echo", "cd", "exit", "source", "set", "unset", "sudo", "npm", "pnpm", "yarn", "git", "zig", "brew", "curl" };
const sql_keywords = [_][]const u8{ "select", "from", "where", "and", "or", "not", "insert", "into", "values", "update", "set", "delete", "create", "table", "drop", "alter", "index", "join", "left", "right", "inner", "outer", "on", "group", "by", "order", "having", "limit", "offset", "as", "distinct", "union", "all", "primary", "key", "foreign", "references", "null", "is", "in", "like", "between", "case", "when", "then", "else", "end", "with", "default", "exists", "asc", "desc" };
const sql_types = [_][]const u8{ "int", "integer", "text", "varchar", "char", "boolean", "date", "timestamp", "real", "float", "decimal", "blob", "bigint", "serial", "uuid", "json", "jsonb" };
const php_keywords = [_][]const u8{ "function", "return", "if", "else", "elseif", "foreach", "for", "while", "as", "class", "public", "private", "protected", "static", "new", "echo", "namespace", "use", "extends", "implements", "try", "catch", "throw", "array", "fn", "match" };
const lua_keywords = [_][]const u8{ "local", "function", "end", "if", "then", "else", "elseif", "for", "in", "do", "while", "repeat", "until", "return", "break", "and", "or", "not", "nil" };
const css_keywords = [_][]const u8{ "important", "media", "import", "keyframes", "from", "to", "supports", "font-face" };
const dart_keywords = [_][]const u8{ "var", "final", "const", "void", "class", "extends", "implements", "with", "return", "if", "else", "for", "in", "while", "switch", "case", "import", "async", "await", "new", "this", "super", "static", "late", "required" };
const cs_keywords = cpp_keywords ++ [_][]const u8{ "using", "namespace", "var", "readonly", "async", "await", "interface", "get", "set", "string", "object", "foreach", "in", "is", "as", "out", "ref", "params", "record", "init" };

fn specFor(lang: []const u8) ?Spec {
    const eq = std.ascii.eqlIgnoreCase;
    if (eq(lang, "zig")) return .{ .keywords = &zig_keywords, .types = &zig_types, .constants = &zig_constants, .block_comment = null, .meta_prefix = '@' };
    if (eq(lang, "c") or eq(lang, "h")) return .{ .keywords = &c_like_keywords, .types = &c_types, .constants = &.{ "NULL", "true", "false" }, .meta_prefix = '#' };
    if (eq(lang, "cpp") or eq(lang, "c++") or eq(lang, "cc") or eq(lang, "hpp") or eq(lang, "objc") or eq(lang, "objective-c")) return .{ .keywords = &cpp_keywords, .types = &c_types, .constants = &.{ "nullptr", "true", "false", "NULL", "nil", "YES", "NO" }, .meta_prefix = '#' };
    if (eq(lang, "js") or eq(lang, "javascript") or eq(lang, "jsx") or eq(lang, "mjs") or eq(lang, "cjs")) return .{ .keywords = &js_keywords, .constants = &js_constants, .quotes = "\"'", .backtick_strings = true };
    if (eq(lang, "ts") or eq(lang, "typescript") or eq(lang, "tsx")) return .{ .keywords = &ts_keywords, .types = &ts_types, .constants = &js_constants, .backtick_strings = true, .meta_prefix = '@' };
    if (eq(lang, "py") or eq(lang, "python")) return .{ .keywords = &py_keywords, .types = &py_types, .constants = &py_constants, .line_comments = &.{"#"}, .block_comment = null, .meta_prefix = '@' };
    if (eq(lang, "go") or eq(lang, "golang")) return .{ .keywords = &go_keywords, .types = &go_types, .constants = &.{ "true", "false", "nil", "iota" }, .backtick_strings = true };
    if (eq(lang, "rust") or eq(lang, "rs")) return .{ .keywords = &rust_keywords, .types = &rust_types, .constants = &.{ "true", "false", "None", "Some", "Ok", "Err" }, .quotes = "\"", .meta_prefix = '#' };
    if (eq(lang, "swift")) return .{ .keywords = &swift_keywords, .constants = &.{ "true", "false", "nil" }, .meta_prefix = '@' };
    if (eq(lang, "kotlin") or eq(lang, "kt")) return .{ .keywords = &kotlin_keywords, .constants = &.{ "true", "false", "null" }, .meta_prefix = '@' };
    if (eq(lang, "java")) return .{ .keywords = &java_keywords, .types = &c_types, .meta_prefix = '@' };
    if (eq(lang, "cs") or eq(lang, "csharp") or eq(lang, "c#")) return .{ .keywords = &cs_keywords, .types = &c_types, .meta_prefix = '#' };
    if (eq(lang, "dart")) return .{ .keywords = &dart_keywords, .meta_prefix = '@' };
    if (eq(lang, "rb") or eq(lang, "ruby")) return .{ .keywords = &ruby_keywords, .constants = &.{ "true", "false", "nil" }, .line_comments = &.{"#"}, .block_comment = null };
    if (eq(lang, "php")) return .{ .keywords = &php_keywords, .line_comments = &.{ "//", "#" }, .dollar_vars = true };
    if (eq(lang, "lua")) return .{ .keywords = &lua_keywords, .constants = &.{ "true", "false", "nil" }, .line_comments = &.{"--"}, .block_comment = .{ "--[[", "]]" } };
    if (eq(lang, "sh") or eq(lang, "bash") or eq(lang, "shell") or eq(lang, "zsh") or eq(lang, "console") or eq(lang, "fish")) return .{ .keywords = &shell_keywords, .line_comments = &.{"#"}, .block_comment = null, .dollar_vars = true, .capitalized_types = false };
    if (eq(lang, "sql") or eq(lang, "postgres") or eq(lang, "mysql") or eq(lang, "sqlite")) return .{ .keywords = &sql_keywords, .types = &sql_types, .line_comments = &.{"--"}, .case_insensitive = true, .capitalized_types = false };
    if (eq(lang, "css") or eq(lang, "scss") or eq(lang, "less")) return .{ .keywords = &css_keywords, .line_comments = if (eq(lang, "css")) &.{} else &.{"//"}, .meta_prefix = '@', .capitalized_types = false };
    if (eq(lang, "json") or eq(lang, "jsonc") or eq(lang, "json5")) return .{ .line_comments = &.{"//"}, .quotes = "\"", .capitalized_types = false };
    if (eq(lang, "yaml") or eq(lang, "yml") or eq(lang, "toml") or eq(lang, "ini") or eq(lang, "env") or eq(lang, "dotenv")) return .{ .constants = &.{ "true", "false", "null", "yes", "no", "on", "off" }, .line_comments = &.{"#"}, .block_comment = null, .capitalized_types = false };
    if (eq(lang, "dockerfile") or eq(lang, "docker") or eq(lang, "makefile") or eq(lang, "make")) return .{ .keywords = &.{ "FROM", "RUN", "CMD", "COPY", "ADD", "WORKDIR", "ENV", "EXPOSE", "ENTRYPOINT", "ARG", "LABEL", "USER", "VOLUME" }, .line_comments = &.{"#"}, .block_comment = null, .dollar_vars = true, .capitalized_types = false };
    return null;
}

fn isMarkup(lang: []const u8) bool {
    const eq = std.ascii.eqlIgnoreCase;
    return eq(lang, "html") or eq(lang, "xml") or eq(lang, "svg") or eq(lang, "vue") or eq(lang, "svelte") or eq(lang, "htm") or eq(lang, "plist");
}

fn isIdentStart(c: u8) bool {
    return std.ascii.isAlphabetic(c) or c == '_' or c >= 0x80;
}

fn isIdent(c: u8) bool {
    return std.ascii.isAlphanumeric(c) or c == '_' or c >= 0x80;
}

fn contains(list: []const []const u8, word: []const u8, ci: bool) bool {
    for (list) |w| {
        if (ci) {
            if (std.ascii.eqlIgnoreCase(w, word)) return true;
        } else if (std.mem.eql(u8, w, word)) return true;
    }
    return false;
}

/// Highlighter state carried across lines (open block comments / strings).
pub const State = struct {
    in_block_comment: bool = false,
    in_backtick: bool = false,
};

/// Tokenises one line (without the newline).
pub fn line(allocator: std.mem.Allocator, lang: []const u8, text: []const u8, state: *State) ![]const Token {
    var out: std.ArrayListUnmanaged(Token) = .empty;
    if (std.ascii.eqlIgnoreCase(lang, "diff") or std.ascii.eqlIgnoreCase(lang, "patch")) {
        const kind: Kind = if (text.len > 0 and text[0] == '+' and !std.mem.startsWith(u8, text, "+++")) .inserted else if (text.len > 0 and text[0] == '-' and !std.mem.startsWith(u8, text, "---")) .deleted else if (std.mem.startsWith(u8, text, "@@")) .meta else .plain;
        try out.append(allocator, .{ .text = text, .kind = kind });
        return out.toOwnedSlice(allocator);
    }
    if (isMarkup(lang)) return markupLine(allocator, text);
    const spec = specFor(lang) orelse {
        try out.append(allocator, .{ .text = text, .kind = .plain });
        return out.toOwnedSlice(allocator);
    };
    const is_json = std.ascii.eqlIgnoreCase(lang, "json") or std.ascii.eqlIgnoreCase(lang, "jsonc");
    const is_yaml = std.ascii.eqlIgnoreCase(lang, "yaml") or std.ascii.eqlIgnoreCase(lang, "yml");
    const is_toml = std.ascii.eqlIgnoreCase(lang, "toml") or std.ascii.eqlIgnoreCase(lang, "ini");
    var i: usize = 0;
    var plain_start: usize = 0;
    const flush = struct {
        fn f(o: *std.ArrayListUnmanaged(Token), a: std.mem.Allocator, t: []const u8, from: usize, to: usize) !void {
            if (to > from) try o.append(a, .{ .text = t[from..to], .kind = .plain });
        }
    }.f;

    // yaml / toml keys
    if (is_yaml or is_toml) {
        const trimmed_start = std.mem.indexOfNone(u8, text, " \t-") orelse text.len;
        if (is_toml and trimmed_start < text.len and text[trimmed_start] == '[') {
            try out.append(allocator, .{ .text = text, .kind = .type });
            return out.toOwnedSlice(allocator);
        }
        if (std.mem.indexOfAnyPos(u8, text, trimmed_start, if (is_yaml) ":" else "=")) |sep| {
            const key = text[trimmed_start..sep];
            if (key.len > 0 and std.mem.indexOfAny(u8, key, "\"'#") == null) {
                try flush(&out, allocator, text, 0, trimmed_start);
                try out.append(allocator, .{ .text = key, .kind = .property });
                i = sep;
                plain_start = sep;
            }
        }
    }

    while (i < text.len) {
        if (state.in_block_comment) {
            const close = spec.block_comment.?[1];
            const end = if (std.mem.indexOfPos(u8, text, i, close)) |e| e + close.len else text.len;
            try out.append(allocator, .{ .text = text[i..end], .kind = .comment });
            if (end < text.len or std.mem.endsWith(u8, text[i..end], close)) state.in_block_comment = false;
            i = end;
            plain_start = i;
            continue;
        }
        if (state.in_backtick) {
            const end = if (std.mem.indexOfScalarPos(u8, text, i, '`')) |e| e + 1 else text.len;
            try out.append(allocator, .{ .text = text[i..end], .kind = .string });
            if (end <= text.len and end > i and text[end - 1] == '`') state.in_backtick = false;
            i = end;
            plain_start = i;
            continue;
        }
        const c = text[i];
        // comments
        var matched_comment = false;
        for (spec.line_comments) |lc| {
            if (std.mem.startsWith(u8, text[i..], lc)) {
                // `#` in shell-ish langs only starts a comment after whitespace
                if (lc.len == 1 and lc[0] == '#' and i > 0 and !std.ascii.isWhitespace(text[i - 1])) break;
                try flush(&out, allocator, text, plain_start, i);
                try out.append(allocator, .{ .text = text[i..], .kind = .comment });
                i = text.len;
                plain_start = i;
                matched_comment = true;
                break;
            }
        }
        if (matched_comment) break;
        if (spec.block_comment) |bc| {
            if (std.mem.startsWith(u8, text[i..], bc[0])) {
                try flush(&out, allocator, text, plain_start, i);
                state.in_block_comment = true;
                const end = if (std.mem.indexOfPos(u8, text, i + bc[0].len, bc[1])) |e| blk: {
                    state.in_block_comment = false;
                    break :blk e + bc[1].len;
                } else text.len;
                try out.append(allocator, .{ .text = text[i..end], .kind = .comment });
                i = end;
                plain_start = i;
                continue;
            }
        }
        // strings
        if (std.mem.indexOfScalar(u8, spec.quotes, c) != null or (c == '`' and spec.backtick_strings)) {
            try flush(&out, allocator, text, plain_start, i);
            var j = i + 1;
            while (j < text.len and text[j] != c) : (j += 1) {
                if (text[j] == '\\') j += 1;
            }
            const end = @min(j + 1, text.len);
            if (c == '`' and j >= text.len) state.in_backtick = true;
            var kind: Kind = .string;
            // JSON keys: a string followed by ':'
            if (is_json) {
                var k = end;
                while (k < text.len and text[k] == ' ') k += 1;
                if (k < text.len and text[k] == ':') kind = .property;
            }
            try out.append(allocator, .{ .text = text[i..end], .kind = kind });
            i = end;
            plain_start = i;
            continue;
        }
        // numbers
        if (std.ascii.isDigit(c) and (i == 0 or !isIdent(text[i - 1]))) {
            try flush(&out, allocator, text, plain_start, i);
            var j = i + 1;
            while (j < text.len and (std.ascii.isAlphanumeric(text[j]) or text[j] == '.' or text[j] == '_')) j += 1;
            try out.append(allocator, .{ .text = text[i..j], .kind = .number });
            i = j;
            plain_start = i;
            continue;
        }
        // meta: @decorator, #include, @builtin
        if (spec.meta_prefix) |mp| {
            if (c == mp and i + 1 < text.len and isIdentStart(text[i + 1])) {
                const at_line_start = std.mem.trim(u8, text[0..i], " \t").len == 0;
                if (mp == '@' or at_line_start) {
                    try flush(&out, allocator, text, plain_start, i);
                    var j = i + 1;
                    while (j < text.len and isIdent(text[j])) j += 1;
                    try out.append(allocator, .{ .text = text[i..j], .kind = if (mp == '@' and !std.ascii.eqlIgnoreCase(lang, "zig")) .function else .meta });
                    i = j;
                    plain_start = i;
                    continue;
                }
            }
        }
        if (spec.dollar_vars and c == '$' and i + 1 < text.len and (isIdentStart(text[i + 1]) or text[i + 1] == '{')) {
            try flush(&out, allocator, text, plain_start, i);
            var j = i + 1;
            if (text[j] == '{') {
                j = if (std.mem.indexOfScalarPos(u8, text, j, '}')) |e| e + 1 else text.len;
            } else while (j < text.len and isIdent(text[j])) j += 1;
            try out.append(allocator, .{ .text = text[i..j], .kind = .constant });
            i = j;
            plain_start = i;
            continue;
        }
        // identifiers
        if (isIdentStart(c) and (i == 0 or !isIdent(text[i - 1]))) {
            var j = i + 1;
            while (j < text.len and (isIdent(text[j]) or (text[j] == '-' and std.ascii.eqlIgnoreCase(lang, "css")))) j += 1;
            const word = text[i..j];
            var next = j;
            while (next < text.len and text[next] == ' ') next += 1;
            const kind: ?Kind = if (contains(spec.keywords, word, spec.case_insensitive))
                .keyword
            else if (contains(spec.constants, word, spec.case_insensitive))
                .constant
            else if (contains(spec.types, word, spec.case_insensitive))
                .type
            else if (next < text.len and text[next] == '(')
                .function
            else if (spec.capitalized_types and std.ascii.isUpper(word[0]) and word.len > 1)
                .type
            else if (i > 0 and text[i - 1] == '.')
                .property
            else
                null;
            if (kind) |k| {
                try flush(&out, allocator, text, plain_start, i);
                try out.append(allocator, .{ .text = word, .kind = k });
                plain_start = j;
            }
            i = j;
            continue;
        }
        if (std.mem.indexOfScalar(u8, "{}[]();,.:=<>+-*/%!&|^~?", c) != null) {
            try flush(&out, allocator, text, plain_start, i);
            try out.append(allocator, .{ .text = text[i .. i + 1], .kind = .punctuation });
            i += 1;
            plain_start = i;
            continue;
        }
        i += 1;
    }
    try flush(&out, allocator, text, plain_start, text.len);
    return out.toOwnedSlice(allocator);
}

fn markupLine(allocator: std.mem.Allocator, text: []const u8) ![]const Token {
    var out: std.ArrayListUnmanaged(Token) = .empty;
    var i: usize = 0;
    var plain: usize = 0;
    while (i < text.len) {
        if (std.mem.startsWith(u8, text[i..], "<!--")) {
            if (i > plain) try out.append(allocator, .{ .text = text[plain..i], .kind = .plain });
            const end = if (std.mem.indexOfPos(u8, text, i, "-->")) |e| e + 3 else text.len;
            try out.append(allocator, .{ .text = text[i..end], .kind = .comment });
            i = end;
            plain = i;
            continue;
        }
        if (text[i] == '<') {
            if (i > plain) try out.append(allocator, .{ .text = text[plain..i], .kind = .plain });
            var j = i + 1;
            if (j < text.len and (text[j] == '/' or text[j] == '!' or text[j] == '?')) j += 1;
            try out.append(allocator, .{ .text = text[i..j], .kind = .punctuation });
            const name_start = j;
            while (j < text.len and (isIdent(text[j]) or text[j] == '-' or text[j] == ':')) j += 1;
            if (j > name_start) try out.append(allocator, .{ .text = text[name_start..j], .kind = .tag });
            // attributes until '>'
            while (j < text.len and text[j] != '>') {
                if (text[j] == '"' or text[j] == '\'') {
                    const q = text[j];
                    const end = if (std.mem.indexOfScalarPos(u8, text, j + 1, q)) |e| e + 1 else text.len;
                    try out.append(allocator, .{ .text = text[j..end], .kind = .string });
                    j = end;
                } else if (isIdentStart(text[j])) {
                    const s = j;
                    while (j < text.len and (isIdent(text[j]) or text[j] == '-' or text[j] == ':')) j += 1;
                    try out.append(allocator, .{ .text = text[s..j], .kind = .attribute });
                } else {
                    try out.append(allocator, .{ .text = text[j .. j + 1], .kind = .punctuation });
                    j += 1;
                }
            }
            if (j < text.len) {
                try out.append(allocator, .{ .text = text[j .. j + 1], .kind = .punctuation });
                j += 1;
            }
            i = j;
            plain = i;
            continue;
        }
        i += 1;
    }
    if (plain < text.len) try out.append(allocator, .{ .text = text[plain..], .kind = .plain });
    return out.toOwnedSlice(allocator);
}

/// Download file extension for a fence language (Streamdown
/// `languageExtensionMap`, common subset). Unknown → "txt".
pub fn extension(lang: []const u8) []const u8 {
    const map = .{
        .{ "bash", "sh" },   .{ "sh", "sh" },             .{ "shell", "sh" },              .{ "zsh", "zsh" }, .{ "console", "sh" },
        .{ "c", "c" },       .{ "c++", "cpp" },           .{ "cpp", "cpp" },               .{ "c#", "cs" },   .{ "cs", "cs" },
        .{ "csharp", "cs" }, .{ "css", "css" },           .{ "scss", "scss" },             .{ "csv", "csv" }, .{ "dart", "dart" },
        .{ "diff", "diff" }, .{ "docker", "dockerfile" }, .{ "dockerfile", "dockerfile" }, .{ "go", "go" },   .{ "golang", "go" },
        .{ "html", "html" }, .{ "java", "java" },         .{ "javascript", "js" },         .{ "js", "js" },   .{ "jsx", "jsx" },
        .{ "json", "json" }, .{ "kotlin", "kt" },         .{ "kt", "kt" },                 .{ "lua", "lua" }, .{ "markdown", "md" },
        .{ "md", "md" },     .{ "mermaid", "mmd" },       .{ "objc", "m" },                .{ "php", "php" }, .{ "python", "py" },
        .{ "py", "py" },     .{ "ruby", "rb" },           .{ "rb", "rb" },                 .{ "rust", "rs" }, .{ "rs", "rs" },
        .{ "sql", "sql" },   .{ "swift", "swift" },       .{ "toml", "toml" },             .{ "ts", "ts" },   .{ "typescript", "ts" },
        .{ "tsx", "tsx" },   .{ "xml", "xml" },           .{ "yaml", "yaml" },             .{ "yml", "yml" }, .{ "zig", "zig" },
        .{ "latex", "tex" }, .{ "tex", "tex" },           .{ "makefile", "mk" },           .{ "vue", "vue" }, .{ "svelte", "svelte" },
    };
    inline for (map) |entry| if (std.ascii.eqlIgnoreCase(lang, entry[0])) return entry[1];
    return "txt";
}

test "zig line" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    var state: State = .{};
    const toks = try line(arena.allocator(), "zig", "const x: u32 = 42; // hi", &state);
    try std.testing.expectEqual(Kind.keyword, toks[0].kind);
    var has_num = false;
    var has_comment = false;
    for (toks) |t| {
        if (t.kind == .number) has_num = true;
        if (t.kind == .comment) has_comment = true;
    }
    try std.testing.expect(has_num and has_comment);
}

test "block comments span lines" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    var state: State = .{};
    _ = try line(arena.allocator(), "js", "a /* start", &state);
    try std.testing.expect(state.in_block_comment);
    const toks = try line(arena.allocator(), "js", "end */ b", &state);
    try std.testing.expectEqual(Kind.comment, toks[0].kind);
    try std.testing.expect(!state.in_block_comment);
}
