//! Tokenizer for the TSX subset. JS tokens come from `next`; the parser
//! switches to `jsxTagToken`, `jsxText` and `templatePart` where TSX's
//! grammar is context sensitive.
const std = @import("std");

pub const Kind = enum {
    eof,
    identifier,
    number,
    /// Quoted string; the token spans the quotes.
    string,
    /// Opening backtick of a template literal.
    template,
    punct,
    /// A byte the lexer does not understand.
    invalid,
};

pub const Token = struct {
    kind: Kind,
    start: u32,
    end: u32,
    /// A line break comes before the token (ASI, `return`, postfix `++`).
    nl_before: bool = false,

    pub fn text(self: Token, source: []const u8) []const u8 {
        return source[self.start..self.end];
    }

    pub fn is(self: Token, source: []const u8, value: []const u8) bool {
        return (self.kind == .punct or self.kind == .identifier) and std.mem.eql(u8, self.text(source), value);
    }
};

/// Longest match first. `>` is always its own token, as in tsc and oxc:
/// the parser joins `>=` where a binary operator is expected
/// (`Parser.rescanGreater`), so `>` closing type arguments or a JSX tag
/// never needs splitting.
const puncts = [_][]const u8{
    "===", "!==", "...", "**=",
    "=>",  "==",  "!=",  "<=",
    "&&",  "||",  "??",  "?.",
    "++",  "--",  "+=",  "-=",
    "*=",  "/=",  "%=",  "**",
};
const single_puncts = "{}()[];,<>+-*/%=!?:.&|^~@#";

pub const Lexer = struct {
    source: []const u8,
    pos: u32 = 0,

    pub fn init(source: []const u8) Lexer {
        return .{ .source = source };
    }

    fn peekByte(self: *const Lexer, offset: u32) u8 {
        const i = self.pos + offset;
        return if (i < self.source.len) self.source[i] else 0;
    }

    /// Skips whitespace and comments. Returns false for an unterminated
    /// block comment (left at its start).
    pub fn skipTrivia(self: *Lexer) bool {
        while (self.pos < self.source.len) {
            const c = self.source[self.pos];
            if (c == ' ' or c == '\t' or c == '\n' or c == '\r') {
                self.pos += 1;
            } else if (c == '/' and self.peekByte(1) == '/') {
                while (self.pos < self.source.len and self.source[self.pos] != '\n') self.pos += 1;
            } else if (c == '/' and self.peekByte(1) == '*') {
                const end = std.mem.indexOfPos(u8, self.source, self.pos + 2, "*/") orelse return false;
                self.pos = @intCast(end + 2);
            } else if (c == 0xEF and self.peekByte(1) == 0xBB and self.peekByte(2) == 0xBF) {
                self.pos += 3; // UTF-8 BOM
            } else break;
        }
        return true;
    }

    pub fn next(self: *Lexer) Token {
        const before = self.pos;
        var t = self.nextToken();
        t.nl_before = std.mem.indexOfScalar(u8, self.source[before..t.start], '\n') != null;
        return t;
    }

    fn nextToken(self: *Lexer) Token {
        if (!self.skipTrivia()) return self.make(.invalid, self.pos, @intCast(self.source.len));
        const start = self.pos;
        if (start >= self.source.len) return self.make(.eof, start, start);
        const c = self.source[start];
        if (isIdentStart(c)) {
            self.pos += 1;
            while (self.pos < self.source.len and isIdentChar(self.source[self.pos])) self.pos += 1;
            return self.make(.identifier, start, self.pos);
        }
        if (isDigit(c) or (c == '.' and isDigit(self.peekByte(1)))) return self.number(start);
        if (c == '"' or c == '\'') return self.string(start, c);
        if (c == '`') {
            self.pos += 1;
            return self.make(.template, start, self.pos);
        }
        for (puncts) |p| {
            if (std.mem.startsWith(u8, self.source[start..], p)) {
                self.pos += @intCast(p.len);
                return self.make(.punct, start, self.pos);
            }
        }
        if (std.mem.indexOfScalar(u8, single_puncts, c) != null) {
            self.pos += 1;
            return self.make(.punct, start, self.pos);
        }
        self.pos += 1;
        return self.make(.invalid, start, self.pos);
    }

    /// Inside a JSX tag: names (with `-`), strings, and single-character
    /// punctuation so `>` never merges into `>=`.
    pub fn jsxTagToken(self: *Lexer) Token {
        if (!self.skipTrivia()) return self.make(.invalid, self.pos, @intCast(self.source.len));
        const start = self.pos;
        if (start >= self.source.len) return self.make(.eof, start, start);
        const c = self.source[start];
        if (isIdentStart(c)) {
            self.pos += 1;
            while (self.pos < self.source.len and (isIdentChar(self.source[self.pos]) or self.source[self.pos] == '-')) self.pos += 1;
            return self.make(.identifier, start, self.pos);
        }
        if (c == '"' or c == '\'') {
            // JSX attribute strings have no escapes
            const end = std.mem.indexOfScalarPos(u8, self.source, start + 1, c) orelse {
                self.pos = @intCast(self.source.len);
                return self.make(.invalid, start, self.pos);
            };
            self.pos = @intCast(end + 1);
            return self.make(.string, start, self.pos);
        }
        self.pos += 1;
        if (std.mem.indexOfScalar(u8, "{}=/>.:", c) != null) return self.make(.punct, start, self.pos);
        return self.make(.invalid, start, self.pos);
    }

    /// Raw JSX text up to the next `{` or `<` (or the end).
    pub fn jsxText(self: *Lexer) Token {
        const start = self.pos;
        while (self.pos < self.source.len and self.source[self.pos] != '{' and self.source[self.pos] != '<') self.pos += 1;
        return self.make(.string, start, self.pos);
    }

    pub const TemplateEnd = enum { backtick, substitution, eof };

    /// Raw template text from `pos` to the closing backtick or the next
    /// `${`, which is consumed.
    pub fn templatePart(self: *Lexer) struct { raw: []const u8, start: u32, end: TemplateEnd } {
        const start = self.pos;
        while (self.pos < self.source.len) {
            const c = self.source[self.pos];
            if (c == '\\') {
                self.pos = @min(self.pos + 2, @as(u32, @intCast(self.source.len)));
                continue;
            }
            if (c == '`') {
                const raw = self.source[start..self.pos];
                self.pos += 1;
                return .{ .raw = raw, .start = start, .end = .backtick };
            }
            if (c == '$' and self.peekByte(1) == '{') {
                const raw = self.source[start..self.pos];
                self.pos += 2;
                return .{ .raw = raw, .start = start, .end = .substitution };
            }
            self.pos += 1;
        }
        return .{ .raw = self.source[start..], .start = start, .end = .eof };
    }

    fn number(self: *Lexer, start: u32) Token {
        const c = self.source[start];
        if (c == '0' and (self.peekByte(1) | 0x20) == 'x' or c == '0' and (self.peekByte(1) | 0x20) == 'b' or c == '0' and (self.peekByte(1) | 0x20) == 'o') {
            self.pos += 2;
            while (self.pos < self.source.len and (std.ascii.isHex(self.source[self.pos]) or self.source[self.pos] == '_')) self.pos += 1;
            return self.make(.number, start, self.pos);
        }
        while (self.pos < self.source.len and (isDigit(self.source[self.pos]) or self.source[self.pos] == '_')) self.pos += 1;
        if (self.peekByte(0) == '.' and isDigit(self.peekByte(1)) or self.peekByte(0) == '.' and !isIdentStart(self.peekByte(1)) and self.peekByte(1) != '.') {
            self.pos += 1;
            while (self.pos < self.source.len and (isDigit(self.source[self.pos]) or self.source[self.pos] == '_')) self.pos += 1;
        }
        if ((self.peekByte(0) | 0x20) == 'e') {
            const sign: u32 = if (self.peekByte(1) == '+' or self.peekByte(1) == '-') 1 else 0;
            if (isDigit(self.peekByte(1 + sign))) {
                self.pos += 1 + sign;
                while (self.pos < self.source.len and isDigit(self.source[self.pos])) self.pos += 1;
            }
        }
        return self.make(.number, start, self.pos);
    }

    fn string(self: *Lexer, start: u32, quote: u8) Token {
        self.pos += 1;
        while (self.pos < self.source.len) {
            const c = self.source[self.pos];
            if (c == '\\') {
                self.pos += 2;
                continue;
            }
            if (c == '\n') break;
            self.pos += 1;
            if (c == quote) return self.make(.string, start, self.pos);
        }
        self.pos = @min(self.pos, @as(u32, @intCast(self.source.len)));
        return self.make(.invalid, start, self.pos);
    }

    fn make(_: *const Lexer, kind: Kind, start: u32, end: u32) Token {
        return .{ .kind = kind, .start = start, .end = end };
    }
};

pub fn isIdentStart(c: u8) bool {
    return std.ascii.isAlphabetic(c) or c == '_' or c == '$' or c >= 0x80;
}

pub fn isIdentChar(c: u8) bool {
    return isIdentStart(c) or isDigit(c);
}

fn isDigit(c: u8) bool {
    return c >= '0' and c <= '9';
}

/// Decodes a quoted JS string token (escapes included).
pub fn decodeString(arena: std.mem.Allocator, raw: []const u8) ![]const u8 {
    return cook(arena, raw[1 .. raw.len - 1]);
}

/// Applies JS escapes (`\n`, `\u{1F600}`, ...) to string or template text.
pub fn cook(arena: std.mem.Allocator, raw: []const u8) ![]const u8 {
    if (std.mem.indexOfScalar(u8, raw, '\\') == null) return raw;
    var out: std.ArrayList(u8) = .empty;
    var i: usize = 0;
    while (i < raw.len) {
        const c = raw[i];
        if (c != '\\' or i + 1 >= raw.len) {
            try out.append(arena, c);
            i += 1;
            continue;
        }
        const e = raw[i + 1];
        i += 2;
        switch (e) {
            'n' => try out.append(arena, '\n'),
            't' => try out.append(arena, '\t'),
            'r' => try out.append(arena, '\r'),
            'b' => try out.append(arena, 8),
            'f' => try out.append(arena, 12),
            'v' => try out.append(arena, 11),
            '0' => try out.append(arena, 0),
            '\n' => {}, // line continuation
            'u' => {
                var cp: u21 = 0;
                if (i < raw.len and raw[i] == '{') {
                    const end = std.mem.indexOfScalarPos(u8, raw, i, '}') orelse return error.BadEscape;
                    cp = std.fmt.parseInt(u21, raw[i + 1 .. end], 16) catch return error.BadEscape;
                    i = end + 1;
                } else {
                    if (i + 4 > raw.len) return error.BadEscape;
                    cp = std.fmt.parseInt(u21, raw[i .. i + 4], 16) catch return error.BadEscape;
                    i += 4;
                }
                var buf: [4]u8 = undefined;
                const n = std.unicode.utf8Encode(cp, &buf) catch return error.BadEscape;
                try out.appendSlice(arena, buf[0..n]);
            },
            'x' => {
                if (i + 2 > raw.len) return error.BadEscape;
                const v = std.fmt.parseInt(u8, raw[i .. i + 2], 16) catch return error.BadEscape;
                i += 2;
                var buf: [4]u8 = undefined;
                const n = std.unicode.utf8Encode(v, &buf) catch unreachable;
                try out.appendSlice(arena, buf[0..n]);
            },
            else => try out.append(arena, e),
        }
    }
    return out.items;
}

test "tokens" {
    var lx = Lexer.init("const [n, setN] = useState<u32>(0); // c\n x === 1.5e3 => `");
    const src = lx.source;
    const want = [_][]const u8{ "const", "[", "n", ",", "setN", "]", "=", "useState", "<", "u32", ">", "(", "0", ")", ";", "x", "===", "1.5e3", "=>", "`" };
    for (want) |w| {
        const t = lx.next();
        try std.testing.expectEqualStrings(w, t.text(src));
    }
    try std.testing.expectEqual(Kind.eof, lx.next().kind);
}

test "jsx tag tokens keep > apart" {
    var lx = Lexer.init("Text size={1}>=");
    try std.testing.expectEqualStrings("Text", lx.jsxTagToken().text(lx.source));
    try std.testing.expectEqualStrings("size", lx.jsxTagToken().text(lx.source));
    try std.testing.expectEqualStrings("=", lx.jsxTagToken().text(lx.source));
    try std.testing.expectEqualStrings("{", lx.jsxTagToken().text(lx.source));
    _ = lx.next();
    try std.testing.expectEqualStrings("}", lx.jsxTagToken().text(lx.source));
    try std.testing.expectEqualStrings(">", lx.jsxTagToken().text(lx.source));
    try std.testing.expectEqualStrings("=", lx.jsxText().text(lx.source));
}

test "cook escapes" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    try std.testing.expectEqualStrings("a\nb\u{1F600}é", try cook(arena.allocator(), "a\\nb\\u{1F600}\\u00e9"));
}
