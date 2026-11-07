//! mhchem: chemical equations (`\ce{...}`) and physical units (`\pu{...}`).
//!
//! A faithful Zig port of the mhchem parser and texifier shipped with KaTeX
//! (`katex/contrib/mhchem/mhchem.js`, KaTeX's adaptation of mhchem 3.3.0):
//!
//!   mhchem — Copyright (c) 2015-2018 Martin Hensel,
//!            Copyright (c) 2011-2015 The MathJax Consortium.
//!            Licensed under the Apache License, Version 2.0
//!            (http://www.apache.org/licenses/LICENSE-2.0).
//!   KaTeX  — Copyright (c) 2013-2020 Khan Academy and other contributors
//!            (MIT); KaTeX's changes: \mathrlap/\mathllap instead of
//!            \rlap/\llap, \raisebox instead of \raise, extensible reaction
//!            arrows, adjusted \tripledash.
//!
//! `toTex(arena, input, .ce)` returns byte-for-byte the string that
//! `texify.go(mhchemParser.go(input, "ce"))` returns upstream (likewise for
//! `.pu`); upstream exceptions become `error.MhchemError`. The result still
//! needs KaTeX's `\tripledash` macro (defined by mhchem.js) when bonds of
//! the `~` family are used.
//!
//! The structure follows upstream so fixes can be diffed against it:
//!   * `Pat` + `Parser.match` = `mhchemParser.patterns.patterns` /
//!     `patterns.match_`; every regexp is a hand-written matcher that
//!     reproduces the JS (sticky, backtracking) semantics, named exactly
//!     like the upstream pattern.
//!   * `findObserveGroups` = `patterns.findObserveGroups`.
//!   * the `*_transitions` tables = `mhchemParser.stateMachines[...]`
//!     (same pattern/state keys, same order, same actions/options).
//!   * `Parser.runAction` = `mhchemParser.actions` + the per-machine
//!     `actions` (looked up machine first, like upstream).
//!   * `Parser.go` = `mhchemParser.go`, `Parser.texify`/`go2` =
//!     `texify.go`/`texify._go2`.
//!
//! Known representation difference: JS strings are UTF-16, this port works
//! on UTF-8 code points. They agree for every BMP character; a pattern such
//! as `/^./` consumes a whole astral (non-BMP) code point here where JS
//! would take one surrogate half.
const std = @import("std");

pub const Machine = enum { ce, pu };

pub const Error = error{ OutOfMemory, MhchemError };

/// TeX for `\ce{input}` (`.ce`) or `\pu{input}` (`.pu`). All memory comes
/// from `arena` (nothing is freed individually).
pub fn toTex(arena: std.mem.Allocator, input: []const u8, machine: Machine) Error![]const u8 {
    const p: Parser = .{ .arena = arena };
    const parsed = try p.go(input, switch (machine) {
        .ce => .ce,
        .pu => .pu,
    });
    return p.texify(parsed, false);
}

const Str = []const u8;
const Items = []const Item;

// ---------------------------------------------------------------------------
// Parser output (upstream `ParserOutput`: bare strings or `{ type_: ... }`)
// ---------------------------------------------------------------------------

const Chemfive = struct {
    a: Items,
    b: Items,
    p: Items,
    o: Items,
    q: Items,
    d: Items,
    dType: ?Str,
};

const Item = union(enum) {
    /// A bare string pushed into the output array.
    str: Str,
    chemfive: *const Chemfive,
    rm: Str,
    text: Str,
    @"roman numeral": Str,
    @"state of aggregation": Items,
    @"state of aggregation subscript": Items,
    bond: Str,
    frac: [2]Str,
    @"pu-frac": [2]Items,
    @"tex-math": Str,
    @"frac-ce": [2]Items,
    overset: [2]Items,
    underset: [2]Items,
    underbrace: [2]Items,
    color: struct { color1: Str, color2: Items },
    color0: Str,
    arrow: struct { r: Str, rd: Items, rq: Items },
    operator: Str,
    @"1st-level escape": Str,
    @"comma enumeration L": Str,
    @"comma enumeration M": Str,
    @"comma enumeration S": Str,
    space: void,
    entitySkip: void,
    @"pu-space-1": void,
    @"pu-space-2": void,
    @"1000 separator": void,
    commaDecimal: void,
    hyphen: void,
    @"addition compound": void,
    @"electron dot": void,
    @"KV x": void,
    prime: void,
    cdot: void,
    @"tight cdot": void,
    times: void,
    circa: void,
    @"^": void,
    v: void,
    ellipsis: void,
    @"/": void,
    @" / ": void,
};
const ItemTag = std.meta.Tag(Item);

/// `{ type_: option }` (action 'insert').
fn voidItem(tag: ItemTag) Error!Item {
    switch (tag) {
        inline else => |t| {
            if (@FieldType(Item, @tagName(t)) == void) return @unionInit(Item, @tagName(t), {});
            return error.MhchemError;
        },
    }
}

/// `{ type_: option, p1: m }` (action 'insert+p1').
fn p1Item(tag: ItemTag, p1: Str) Error!Item {
    switch (tag) {
        inline else => |t| {
            if (@FieldType(Item, @tagName(t)) == Str) return @unionInit(Item, @tagName(t), p1);
            return error.MhchemError;
        },
    }
}

/// Return value of an action: `undefined`, one value, or an array.
const Ret = union(enum) {
    none,
    one: Item,
    many: Items,
};

/// `matches.match_`: a string or (for two-group patterns) an array.
const MV = union(enum) {
    s: Str,
    arr: []const Str,
};

const Match = struct { m: MV, rem: Str };

// ---------------------------------------------------------------------------
// Character helpers (JS regexp semantics)
// ---------------------------------------------------------------------------

const Cp = struct { c: u21, n: usize };

fn decode(s: Str, i: usize) Cp {
    const b = s[i];
    const n = std.unicode.utf8ByteSequenceLength(b) catch return .{ .c = b, .n = 1 };
    if (i + n > s.len) return .{ .c = b, .n = 1 };
    const c = std.unicode.utf8Decode(s[i..][0..n]) catch return .{ .c = b, .n = 1 };
    return .{ .c = c, .n = n };
}

/// JS `\s`.
fn isJsSpace(c: u21) bool {
    return switch (c) {
        0x09...0x0D, 0x20, 0xA0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000, 0xFEFF => true,
        else => false,
    };
}

/// JS line terminators (not matched by `.`).
fn isLineTerminator(c: u21) bool {
    return c == 0x0A or c == 0x0D or c == 0x2028 or c == 0x2029;
}

/// Byte length of the `\s` character at `i`, 0 if none.
fn spaceLen(s: Str, i: usize) usize {
    if (i >= s.len) return 0;
    const c = decode(s, i);
    return if (isJsSpace(c.c)) c.n else 0;
}

/// Index after `\s*` starting at `i`.
fn skipSpaces(s: Str, i: usize) usize {
    var j = i;
    while (true) {
        const n = spaceLen(s, j);
        if (n == 0) return j;
        j += n;
    }
}

fn isDigit(c: u8) bool {
    return c >= '0' and c <= '9';
}
fn isLower(c: u8) bool {
    return c >= 'a' and c <= 'z';
}
fn isUpper(c: u8) bool {
    return c >= 'A' and c <= 'Z';
}
fn isAlpha(c: u8) bool {
    return isLower(c) or isUpper(c);
}
fn isRoman(c: u8) bool {
    return c == 'I' or c == 'V' or c == 'X';
}
fn isSign(c: u8) bool {
    return c == '+' or c == '-';
}

fn digitAt(s: Str, i: usize) bool {
    return i < s.len and isDigit(s[i]);
}
fn lowerAt(s: Str, i: usize) bool {
    return i < s.len and isLower(s[i]);
}
fn charAt(s: Str, i: usize, c: u8) bool {
    return i < s.len and s[i] == c;
}
fn anyAt(s: Str, i: usize, set: Str) bool {
    return i < s.len and std.mem.indexOfScalar(u8, set, s[i]) != null;
}
fn startsAt(s: Str, i: usize, prefix: Str) bool {
    return i <= s.len and std.mem.startsWith(u8, s[i..], prefix);
}

/// Index after `[0-9]*` starting at `i`.
fn digitsEnd(s: Str, i: usize) usize {
    var j = i;
    while (digitAt(s, j)) j += 1;
    return j;
}

fn eql(a: Str, b: Str) bool {
    return std.mem.eql(u8, a, b);
}

/// JS truthiness of an optional string (`undefined` and `""` are falsy).
fn truthy(s: ?Str) bool {
    return if (s) |x| x.len > 0 else false;
}

fn optEql(s: ?Str, b: Str) bool {
    return if (s) |x| eql(x, b) else false;
}

/// Full match of `[0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+)` (`optional_frac`
/// selects the `(?:\.[0-9]+)?` variant of the second alternative).
fn isNumberLiteral(s: Str, optional_frac: bool) bool {
    // alternative 1
    if (s.len > 0 and isDigit(s[0])) {
        var i = digitsEnd(s, 0);
        if (i == s.len) return true;
        if ((s[i] == ',' or s[i] == '.') and digitAt(s, i + 1)) {
            i = digitsEnd(s, i + 1);
            if (i == s.len) return true;
        }
    }
    // alternative 2
    const i = digitsEnd(s, 0);
    if (i == s.len) return optional_frac;
    if (s[i] == '.' and digitAt(s, i + 1)) return digitsEnd(s, i + 1) == s.len;
    return false;
}

/// `[+\-]?[0-9]+|\{[+\-]?[0-9]+\}` at `i`; returns the end index.
fn exponentEnd(s: Str, i: usize) ?usize {
    var j = i;
    if (j < s.len and isSign(s[j])) j += 1;
    if (digitAt(s, j)) return digitsEnd(s, j);
    if (charAt(s, i, '{')) {
        j = i + 1;
        if (j < s.len and isSign(s[j])) j += 1;
        if (!digitAt(s, j)) return null;
        j = digitsEnd(s, j);
        if (charAt(s, j, '}')) return j + 1;
    }
    return null;
}

const greek_lower = [_]Str{ "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa", "lambda", "mu", "nu", "xi", "omicron", "pi", "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega" };
const greek_upper = [_]Str{ "Gamma", "Delta", "Theta", "Lambda", "Xi", "Pi", "Sigma", "Upsilon", "Phi", "Psi", "Omega" };

/// `\\(?:alpha|beta|...)` at `i` (no name is a prefix of another, so the
/// first hit is the only one); returns the end index.
fn greekMacroEnd(s: Str, i: usize, with_upper: bool) ?usize {
    if (!charAt(s, i, '\\')) return null;
    for (greek_lower) |n| if (startsAt(s, i + 1, n)) return i + 1 + n.len;
    if (with_upper) for (greek_upper) |n| if (startsAt(s, i + 1, n)) return i + 1 + n.len;
    return null;
}

/// `(?:\s+|\{\}|(?![a-zA-Z]))` at `j`.
fn greekSuffixEnd(s: Str, j: usize) ?usize {
    if (spaceLen(s, j) > 0) return skipSpaces(s, j);
    if (startsAt(s, j, "{}")) return j + 2;
    if (j < s.len and isAlpha(s[j])) return null;
    return j;
}

/// `(?:\s+|\{\}|(?![a-zA-Z]))$` at `j`.
fn greekSuffixToEnd(s: Str, j: usize) bool {
    if (j == s.len) return true;
    if (eql(s[j..], "{}")) return true;
    return skipSpaces(s, j) == s.len;
}

/// Character class of `letters`: `[a-zA-Zα-ωΑ-Ω?@]`.
fn isLetterChar(c: u21) bool {
    return (c < 128 and (isAlpha(@intCast(c)) or c == '?' or c == '@')) or
        (c >= 0x3B1 and c <= 0x3C9) or (c >= 0x391 and c <= 0x3A9);
}

/// Position-set simulation for small anchored regexps (exact for boolean
/// full matches, independent of backtracking order).
const PosSet = struct {
    s: Str,
    set: []bool,

    fn init(arena: std.mem.Allocator, s: Str) Error!PosSet {
        const set = try arena.alloc(bool, s.len + 1);
        @memset(set, false);
        set[0] = true;
        return .{ .s = s, .set = set };
    }
    fn clone(self: PosSet, arena: std.mem.Allocator) Error!PosSet {
        return .{ .s = self.s, .set = try arena.dupe(bool, self.set) };
    }
    /// `set := one(class)(set)`.
    fn one(self: PosSet, comptime pred: fn (u8) bool) void {
        var i = self.s.len;
        while (i > 0) : (i -= 1) self.set[i] = self.set[i - 1] and pred(self.s[i - 1]);
        self.set[0] = false;
    }
    /// `set := set ∪ one(class)(set)`.
    fn opt(self: PosSet, comptime pred: fn (u8) bool) void {
        var i = self.s.len;
        while (i > 0) : (i -= 1) {
            if (self.set[i - 1] and pred(self.s[i - 1])) self.set[i] = true;
        }
    }
    /// `set := star(class)(set)`.
    fn star(self: PosSet, comptime pred: fn (u8) bool) void {
        for (0..self.s.len) |i| {
            if (self.set[i] and pred(self.s[i])) self.set[i + 1] = true;
        }
    }
    fn unionWith(self: PosSet, other: PosSet) void {
        for (self.set, other.set) |*a, b| a.* = a.* or b;
    }
};

fn isOpenParen(c: u8) bool {
    return c == '(';
}
fn isCloseParen(c: u8) bool {
    return c == ')';
}

/// Replaces every occurrence of any of `alts` (leftmost, first alternative
/// first, like a global JS regexp of literal alternatives) by `repl`.
fn replaceAlternatives(arena: std.mem.Allocator, s: Str, alts: []const Str, repl: Str) Error!Str {
    var out: std.ArrayList(u8) = .empty;
    var i: usize = 0;
    outer: while (i < s.len) {
        for (alts) |a| {
            if (startsAt(s, i, a)) {
                try out.appendSlice(arena, repl);
                i += a.len;
                continue :outer;
            }
        }
        try out.append(arena, s[i]);
        i += 1;
    }
    return out.items;
}

/// JS `String.prototype.replace(string, string)`: first occurrence only.
fn replaceFirst(arena: std.mem.Allocator, s: Str, needle: Str, repl: Str) Error!Str {
    const i = std.mem.indexOf(u8, s, needle) orelse return s;
    return std.mem.concat(arena, u8, &.{ s[0..i], repl, s[i + needle.len ..] });
}

fn m0(s: Str, n: usize) Match {
    return .{ .m = .{ .s = s[0..n] }, .rem = s[n..] };
}
fn opt0(s: Str, n: ?usize) ?Match {
    return if (n) |k| m0(s, k) else null;
}
/// `patterns.match_` for a regexp with one group: group 1 if truthy, else
/// the whole match.
fn group1(s: Str, n: usize, g: Str) Match {
    return .{ .m = .{ .s = if (g.len > 0) g else s[0..n] }, .rem = s[n..] };
}

// ---------------------------------------------------------------------------
// patterns.findObserveGroups
// ---------------------------------------------------------------------------

const FM = union(enum) {
    str: Str,
    re: *const fn (Str) ?usize,

    /// `_match`: length of the match at the start of `s`.
    fn at(self: FM, s: Str) ?usize {
        return switch (self) {
            .str => |p| if (std.mem.startsWith(u8, s, p)) p.len else null,
            .re => |f| f(s),
        };
    }
    fn truthy(self: FM) bool {
        return switch (self) {
            .str => |p| p.len > 0,
            .re => true,
        };
    }
};

fn lit(s: Str) FM {
    return .{ .str = s };
}

/// `/^\([a-z]{1,3}(?=[\),])/`
fn reAggregationBegin(s: Str) ?usize {
    if (!charAt(s, 0, '(')) return null;
    var k: usize = 0;
    while (k < 3 and lowerAt(s, 1 + k)) k += 1;
    if (k == 0) return null;
    if (!anyAt(s, 1 + k, "),")) return null;
    return 1 + k;
}
/// `/^\\[a-zA-Z]+\{/`
fn reMacroBrace(s: Str) ?usize {
    if (!charAt(s, 0, '\\')) return null;
    var i: usize = 1;
    while (i < s.len and isAlpha(s[i])) i += 1;
    if (i == 1 or !charAt(s, i, '{')) return null;
    return i + 1;
}
/// `/^(?=\{)/`
fn reBeforeBrace(s: Str) ?usize {
    return if (charAt(s, 0, '{')) 0 else null;
}

const ObserveEnd = struct { begin: usize, end: usize };

/// `_findObserveGroups`
fn findObserveEnd(input: Str, start: usize, endChars: FM) Error!?ObserveEnd {
    var braces: usize = 0;
    var i = start;
    while (i < input.len) : (i += 1) {
        const a = input[i];
        const mm = endChars.at(input[i..]);
        if (mm != null and braces == 0) {
            return .{ .begin = i, .end = i + mm.? };
        } else if (a == '{') {
            braces += 1;
        } else if (a == '}') {
            // upstream throws ["ExtraCloseMissingOpen", ...]
            if (braces == 0) return error.MhchemError;
            braces -= 1;
        }
    }
    return null;
}

/// `findObserveGroups(input, begExcl, begIncl, endIncl, endExcl)`
fn findObserveGroups(s: Str, begExcl: FM, begIncl: FM, endIncl: FM, endExcl: FM) Error!?Match {
    const n1 = begExcl.at(s) orelse return null;
    const input = s[n1..];
    const n2 = begIncl.at(input) orelse return null;
    const e = (try findObserveEnd(input, n2, if (endIncl.truthy()) endIncl else endExcl)) orelse return null;
    const match1 = input[0..if (endIncl.truthy()) e.end else e.begin];
    return .{ .m = .{ .s = match1 }, .rem = input[e.end..] };
}

/// `findObserveGroups` with the second group (beg2Excl/beg2Incl given).
fn findObserveGroups2(
    arena: std.mem.Allocator,
    s: Str,
    begExcl: FM,
    begIncl: FM,
    endIncl: FM,
    endExcl: FM,
    beg2Excl: FM,
    beg2Incl: FM,
    end2Incl: FM,
    end2Excl: FM,
    combine: bool,
) Error!?Match {
    const g1 = (try findObserveGroups(s, begExcl, begIncl, endIncl, endExcl)) orelse return null;
    const g2 = (try findObserveGroups(g1.rem, beg2Excl, beg2Incl, end2Incl, end2Excl)) orelse return null;
    if (combine) {
        return .{ .m = .{ .s = try std.mem.concat(arena, u8, &.{ g1.m.s, g2.m.s }) }, .rem = g2.rem };
    }
    const arr = try arena.alloc(Str, 2);
    arr[0] = g1.m.s;
    arr[1] = g2.m.s;
    return .{ .m = .{ .arr = arr }, .rem = g2.rem };
}

// ---------------------------------------------------------------------------
// mhchemParser.patterns.patterns
// ---------------------------------------------------------------------------

const Pat = enum {
    empty,
    @"else",
    else2,
    space,
    @"space A",
    @"space$",
    @"a-z",
    x,
    @"x$",
    @"i$",
    letters,
    @"\\greek",
    @"one lowercase latin letter $",
    @"$one lowercase latin letter$ $",
    @"one lowercase greek letter $",
    digits,
    @"-9.,9",
    @"-9.,9 no missing 0",
    @"(-)(9.,9)(e)(99)",
    @"(-)(9)^(-9)",
    @"state of aggregation $",
    @"_{(state of aggregation)}$",
    @"{[(",
    @")]}",
    @", ",
    @",",
    @".",
    @". ",
    @"...",
    @"* ",
    @"^{(...)}",
    @"^($...$)",
    @"^a",
    @"^\\x{}{}",
    @"^\\x{}",
    @"^\\x",
    @"^(-1)",
    @"'",
    @"_{(...)}",
    @"_($...$)",
    _9,
    @"_\\x{}{}",
    @"_\\x{}",
    @"_\\x",
    @"^_",
    @"{}",
    @"{...}",
    @"{(...)}",
    @"$...$",
    @"${(...)}$",
    @"$(...)$",
    @"=<>",
    @"#",
    @"+",
    @"-$",
    @"-9",
    @"- orbital overlap",
    @"-",
    @"pm-operator",
    operator,
    arrowUpDown,
    @"\\bond{(...)}",
    @"->",
    CMT,
    @"[(...)]",
    @"1st-level escape",
    @"\\,",
    @"\\x{}{}",
    @"\\x{}",
    @"\\ca",
    @"\\x",
    orbital,
    others,
    @"\\frac{(...)}",
    @"\\overset{(...)}",
    @"\\underset{(...)}",
    @"\\underbrace{(...)}",
    @"\\color{(...)}0",
    @"\\color{(...)}{(...)}1",
    @"\\color(...){(...)}2",
    @"\\ce{(...)}",
    @"oxidation$",
    @"d-oxidation$",
    @"roman numeral",
    @"1/2$",
    amount,
    amount2,
    @"(KV letters),",
    @"formula$",
    uprightEntities,
    @"/",
    @"//",
    @"*",
};

/// `'letters'`: `/^(?:[a-zA-Zα-ωΑ-Ω?@]|(?:\\(?:alpha|...|Omega)(?:\s+|\{\}|(?![a-zA-Z]))))+/`
fn lettersEnd(s: Str) ?usize {
    var i: usize = 0;
    while (i < s.len) {
        const c = decode(s, i);
        if (isLetterChar(c.c)) {
            i += c.n;
            continue;
        }
        if (greekMacroEnd(s, i, true)) |j| {
            if (greekSuffixEnd(s, j)) |k| {
                i = k;
                continue;
            }
        }
        break;
    }
    return if (i == 0) null else i;
}

/// `'one lowercase greek letter $'`:
/// `/^(?:\$?[α-ω]\$?|\$?\\(?:alpha|...|omega)\s*\$?)(?:\s+|\{\}|(?![a-zA-Z]))$/`
fn oneLowercaseGreekLetter(s: Str) bool {
    var d1: usize = 0;
    while (d1 <= 1) : (d1 += 1) {
        if (d1 == 1 and !charAt(s, 0, '$')) continue;
        if (d1 < s.len) {
            const c = decode(s, d1);
            if (c.c >= 0x3B1 and c.c <= 0x3C9) {
                const p = d1 + c.n;
                if (charAt(s, p, '$') and greekSuffixToEnd(s, p + 1)) return true;
                if (greekSuffixToEnd(s, p)) return true;
            }
        }
        if (greekMacroEnd(s, d1, false)) |j| {
            var e = j;
            while (true) {
                if (charAt(s, e, '$') and greekSuffixToEnd(s, e + 1)) return true;
                if (greekSuffixToEnd(s, e)) return true;
                const n = spaceLen(s, e);
                if (n == 0) break;
                e += n;
            }
        }
    }
    return false;
}

/// `'(-)(9.,9)(e)(99)'` regexp:
/// `/^(\+\-|\+\/\-|\+|\-|\\pm\s?)?([0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+))?(\((?:[0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+))\))?(?:([eE]|\s*(\*|x|\\times|×)\s*10\^)([+\-]?[0-9]+|\{[+\-]?[0-9]+\}))?/`
/// Returns the end index and groups 1..6 ("" for groups that did not participate).
fn enumberRe(s: Str, g: *[6]Str) usize {
    for (g) |*x| x.* = "";
    var i: usize = 0;
    // group 1
    for ([_]Str{ "+-", "+/-", "+", "-" }) |alt| {
        if (startsAt(s, 0, alt)) {
            i = alt.len;
            break;
        }
    } else if (startsAt(s, 0, "\\pm")) {
        i = 3 + spaceLen(s, 3);
    }
    g[0] = s[0..i];
    // group 2
    {
        var j = i;
        if (digitAt(s, j)) {
            j = digitsEnd(s, j);
            if (anyAt(s, j, ",.") and digitAt(s, j + 1)) j = digitsEnd(s, j + 1);
        } else if (charAt(s, j, '.') and digitAt(s, j + 1)) {
            j = digitsEnd(s, j + 1);
        }
        g[1] = s[i..j];
        i = j;
    }
    // group 3
    if (charAt(s, i, '(')) {
        var k = i + 1;
        while (anyAt(s, k, "0123456789.,")) k += 1;
        if (charAt(s, k, ')') and isNumberLiteral(s[i + 1 .. k], false)) {
            g[2] = s[i .. k + 1];
            i = k + 1;
        }
    }
    // groups 4, 5, 6
    if (anyAt(s, i, "eE")) {
        if (exponentEnd(s, i + 1)) |e| {
            g[3] = s[i .. i + 1];
            g[5] = s[i + 1 .. e];
            return e;
        }
    }
    {
        const j = skipSpaces(s, i);
        var op: Str = "";
        for ([_]Str{ "*", "x", "\\times", "\u{00D7}" }) |alt| {
            if (startsAt(s, j, alt)) {
                op = alt;
                break;
            }
        }
        if (op.len > 0) {
            const k = skipSpaces(s, j + op.len);
            if (startsAt(s, k, "10^")) {
                if (exponentEnd(s, k + 3)) |e| {
                    g[3] = s[i .. k + 3];
                    g[4] = op;
                    g[5] = s[k + 3 .. e];
                    return e;
                }
            }
        }
    }
    return i;
}

/// `'(-)(9)^(-9)'` regexp:
/// `/^(\+\-|\+\/\-|\+|\-|\\pm\s?)?([0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+)?)\^([+\-]?[0-9]+|\{[+\-]?[0-9]+\})/`
fn numberPowRe(s: Str, g: *[3]Str) ?usize {
    // group 1 alternatives in backtracking order
    var cands: [7]?usize = @splat(null);
    if (startsAt(s, 0, "+-")) cands[0] = 2;
    if (startsAt(s, 0, "+/-")) cands[1] = 3;
    if (startsAt(s, 0, "+")) cands[2] = 1;
    if (startsAt(s, 0, "-")) cands[3] = 1;
    if (startsAt(s, 0, "\\pm")) {
        const n = spaceLen(s, 3);
        if (n > 0) cands[4] = 3 + n;
        cands[5] = 3;
    }
    cands[6] = 0;
    for (cands) |cand| {
        const i = cand orelse continue;
        var k = i;
        while (anyAt(s, k, "0123456789.,")) k += 1;
        if (!charAt(s, k, '^')) continue;
        if (!isNumberLiteral(s[i..k], true)) continue;
        const e = exponentEnd(s, k + 1) orelse continue;
        g[0] = s[0..i];
        g[1] = s[i..k];
        g[2] = s[k + 1 .. e];
        return e;
    }
    return null;
}

/// First regexp of `'amount'`:
/// `/^(?:(?:(?:\([+\-]?[0-9]+\/[0-9]+\)|[+\-]?(?:[0-9]+|\$[a-z]\$|[a-z])\/[0-9]+|[+\-]?[0-9]+[.,][0-9]+|[+\-]?\.[0-9]+|[+\-]?[0-9]+)(?:[a-z](?=\s*[A-Z]))?)|[+\-]?[a-z](?=\s*[A-Z])|\+(?!\s))/`
fn amountRe(s: Str) ?usize {
    if (amountNumber(s)) |n| {
        if (lowerAt(s, n) and upperAfterSpaces(s, n + 1)) return n + 1;
        return n;
    }
    {
        const i: usize = if (s.len > 0 and isSign(s[0])) 1 else 0;
        if (lowerAt(s, i) and upperAfterSpaces(s, i + 1)) return i + 1;
    }
    if (charAt(s, 0, '+') and spaceLen(s, 1) == 0) return 1;
    return null;
}

/// `(?=\s*[A-Z])` at `i`.
fn upperAfterSpaces(s: Str, i: usize) bool {
    const k = skipSpaces(s, i);
    return k < s.len and isUpper(s[k]);
}

/// The five number alternatives of `amountRe`.
fn amountNumber(s: Str) ?usize {
    // \([+\-]?[0-9]+\/[0-9]+\)
    if (charAt(s, 0, '(')) {
        var i: usize = 1;
        if (i < s.len and isSign(s[i])) i += 1;
        if (digitAt(s, i)) {
            i = digitsEnd(s, i);
            if (charAt(s, i, '/') and digitAt(s, i + 1)) {
                i = digitsEnd(s, i + 1);
                if (charAt(s, i, ')')) return i + 1;
            }
        }
    }
    const start: usize = if (s.len > 0 and isSign(s[0])) 1 else 0;
    // [+\-]?(?:[0-9]+|\$[a-z]\$|[a-z])\/[0-9]+
    {
        var i = start;
        var ok = true;
        if (digitAt(s, i)) {
            i = digitsEnd(s, i);
        } else if (charAt(s, i, '$') and lowerAt(s, i + 1) and charAt(s, i + 2, '$')) {
            i += 3;
        } else if (lowerAt(s, i)) {
            i += 1;
        } else ok = false;
        if (ok and charAt(s, i, '/') and digitAt(s, i + 1)) return digitsEnd(s, i + 1);
    }
    // [+\-]?[0-9]+[.,][0-9]+
    if (digitAt(s, start)) {
        const i = digitsEnd(s, start);
        if (anyAt(s, i, ".,") and digitAt(s, i + 1)) return digitsEnd(s, i + 1);
    }
    // [+\-]?\.[0-9]+
    if (charAt(s, start, '.') and digitAt(s, start + 1)) return digitsEnd(s, start + 1);
    // [+\-]?[0-9]+
    if (digitAt(s, start)) return digitsEnd(s, start);
    return null;
}

/// Second regexp of `'amount'` (full match of a `$...$` group):
/// `/^\$(?:\(?[+\-]?(?:[0-9]*[a-z]?[+\-])?[0-9]*[a-z](?:[+\-][0-9]*[a-z]?)?\)?|\+|-)\$$/`
fn amountMath(arena: std.mem.Allocator, s: Str) Error!bool {
    if (s.len < 2 or s[0] != '$' or s[s.len - 1] != '$') return false;
    const c = s[1 .. s.len - 1];
    var set = try PosSet.init(arena, c);
    set.opt(isOpenParen);
    set.opt(isSign);
    {
        var t = try set.clone(arena);
        t.star(isDigit);
        t.opt(isLower);
        t.one(isSign);
        set.unionWith(t);
    }
    set.star(isDigit);
    set.one(isLower);
    {
        var t = try set.clone(arena);
        t.one(isSign);
        t.star(isDigit);
        t.opt(isLower);
        set.unionWith(t);
    }
    set.opt(isCloseParen);
    if (set.set[c.len]) return true;
    return eql(c, "+") or eql(c, "-");
}

/// `'formula$'` regexp `/^(?:[a-z]|(?:[0-9\ \+\-\,\.\(\)]+[a-z])+[0-9\ \+\-\,\.\(\)]*|(?:[a-z][0-9\ \+\-\,\.\(\)]+)+[a-z]?)$/`
/// is exactly: only `[a-z]` and `[0-9 +\-,.()]`, at least one letter, no
/// two letters adjacent.
fn isFormula(s: Str) bool {
    var letters: usize = 0;
    var prev_letter = false;
    for (s) |c| {
        if (isLower(c)) {
            if (prev_letter) return false;
            prev_letter = true;
            letters += 1;
        } else if (isDigit(c) or std.mem.indexOfScalar(u8, " +-,.()", c) != null) {
            prev_letter = false;
        } else return false;
    }
    return letters > 0;
}

/// `(?:[0-9]+|\$[a-z]\$|[a-z])` at `i`; returns the end index.
fn fracNumeratorEnd(s: Str, i: usize) ?usize {
    if (digitAt(s, i)) return digitsEnd(s, i);
    if (charAt(s, i, '$') and lowerAt(s, i + 1) and charAt(s, i + 2, '$')) return i + 3;
    if (lowerAt(s, i)) return i + 1;
    return null;
}

/// `(?:\$[a-z]\$|[a-z])?$` at `i`; returns the optional group or null if no match.
fn fracSuffix(s: Str, i: usize) ??Str {
    if (i == s.len) return @as(?Str, null);
    if (s.len - i == 3 and s[i] == '$' and isLower(s[i + 1]) and s[i + 2] == '$') return @as(?Str, s[i..]);
    if (s.len - i == 1 and isLower(s[i])) return @as(?Str, s[i..]);
    return null;
}

/// `'1/2$'`: `/^[+\-]?(?:[0-9]+|\$[a-z]\$|[a-z])\/[0-9]+(?:\$[a-z]\$|[a-z])?$/`
fn isHalf(s: Str) bool {
    const start: usize = if (s.len > 0 and isSign(s[0])) 1 else 0;
    const i = fracNumeratorEnd(s, start) orelse return false;
    if (!charAt(s, i, '/') or !digitAt(s, i + 1)) return false;
    return fracSuffix(s, digitsEnd(s, i + 1)) != null;
}

/// `'orbital'`: `/^(?:[0-9]{1,2}[spdfgh]|[0-9]{0,2}sp)(?=$|[^a-zA-Z])/`
fn orbitalEnd(s: Str) ?usize {
    const run: usize = @min(digitsEnd(s, 0), 2);
    var k = run;
    while (k >= 1) : (k -= 1) {
        if (anyAt(s, k, "spdfgh") and notLetterOrEnd(s, k + 1)) return k + 1;
    }
    k = run;
    while (true) : (k -= 1) {
        if (startsAt(s, k, "sp") and notLetterOrEnd(s, k + 2)) return k + 2;
        if (k == 0) break;
    }
    return null;
}

/// `(?=$|[^a-zA-Z])` at `i`.
fn notLetterOrEnd(s: Str, i: usize) bool {
    return i >= s.len or !isAlpha(s[i]);
}

/// `(?=$|[\s,;\)\]\}])` at `i`.
fn delimiterOrEnd(s: Str, i: usize) bool {
    return i >= s.len or spaceLen(s, i) > 0 or anyAt(s, i, ",;)]}");
}

/// `'oxidation$'` (`signed`) / `'d-oxidation$'`:
/// `/^(?:[+-][IVX]+|\\pm\s*0|\$\\pm\$\s*0)$/`, `/^(?:[+-]?\s?[IVX]+|\\pm\s*0|\$\\pm\$\s*0)$/`
fn isOxidation(s: Str, d_variant: bool) bool {
    roman: {
        var i: usize = 0;
        if (s.len > 0 and isSign(s[0])) {
            i = 1;
        } else if (!d_variant) break :roman;
        if (d_variant) i += spaceLen(s, i);
        if (i >= s.len) break :roman;
        for (s[i..]) |c| if (!isRoman(c)) break :roman;
        return true;
    }
    for ([_]Str{ "\\pm", "$\\pm$" }) |pm| {
        if (startsAt(s, 0, pm)) {
            const j = skipSpaces(s, pm.len);
            if (j + 1 == s.len and s[j] == '0') return true;
        }
    }
    return false;
}

const Parser = struct {
    arena: std.mem.Allocator,

    /// `mhchemParser.patterns.match_(m, input)`
    fn match(self: Parser, pat: Pat, s: Str) Error!?Match {
        return switch (pat) {
            .empty => if (s.len == 0) m0(s, 0) else null,
            // /^./
            .@"else", .else2 => blk: {
                if (s.len == 0) break :blk null;
                const c = decode(s, 0);
                break :blk if (isLineTerminator(c.c)) null else m0(s, c.n);
            },
            // /^\s/
            .space => opt0(s, if (spaceLen(s, 0) > 0) spaceLen(s, 0) else null),
            // /^\s(?=[A-Z\\$])/
            .@"space A" => blk: {
                const n = spaceLen(s, 0);
                if (n == 0) break :blk null;
                if (n < s.len and (isUpper(s[n]) or s[n] == '\\' or s[n] == '$')) break :blk m0(s, n);
                break :blk null;
            },
            // /^\s$/
            .@"space$" => if (s.len > 0 and spaceLen(s, 0) == s.len) m0(s, s.len) else null,
            // /^[a-z]/
            .@"a-z" => if (lowerAt(s, 0)) m0(s, 1) else null,
            // /^x/
            .x => if (charAt(s, 0, 'x')) m0(s, 1) else null,
            // /^x$/
            .@"x$" => if (eql(s, "x")) m0(s, 1) else null,
            // /^i$/
            .@"i$" => if (eql(s, "i")) m0(s, 1) else null,
            .letters => opt0(s, lettersEnd(s)),
            // /^\\(?:alpha|...|Omega)(?:\s+|\{\}|(?![a-zA-Z]))/
            .@"\\greek" => blk: {
                const j = greekMacroEnd(s, 0, true) orelse break :blk null;
                break :blk opt0(s, greekSuffixEnd(s, j));
            },
            // /^(?:([a-z])(?:$|[^a-zA-Z]))$/
            .@"one lowercase latin letter $" => blk: {
                if (!lowerAt(s, 0)) break :blk null;
                if (s.len == 1) break :blk group1(s, 1, s[0..1]);
                const c = decode(s, 1);
                if (1 + c.n == s.len and c.c <= 0xFFFF and !(c.c < 128 and isAlpha(@intCast(c.c))))
                    break :blk group1(s, s.len, s[0..1]);
                break :blk null;
            },
            // /^\$(?:([a-z])(?:$|[^a-zA-Z]))\$$/
            .@"$one lowercase latin letter$ $" => blk: {
                if (!charAt(s, 0, '$') or !lowerAt(s, 1) or s.len < 3) break :blk null;
                const c = decode(s, 2);
                if (2 + c.n + 1 == s.len and s[s.len - 1] == '$' and c.c <= 0xFFFF and !(c.c < 128 and isAlpha(@intCast(c.c))))
                    break :blk group1(s, s.len, s[1..2]);
                break :blk null;
            },
            .@"one lowercase greek letter $" => if (oneLowercaseGreekLetter(s)) m0(s, s.len) else null,
            // /^[0-9]+/
            .digits => if (digitAt(s, 0)) m0(s, digitsEnd(s, 0)) else null,
            // /^[+\-]?(?:[0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+))/
            .@"-9.,9" => blk: {
                const i: usize = if (s.len > 0 and isSign(s[0])) 1 else 0;
                if (digitAt(s, i)) {
                    var j = digitsEnd(s, i);
                    if (anyAt(s, j, ",.") and digitAt(s, j + 1)) j = digitsEnd(s, j + 1);
                    break :blk m0(s, j);
                }
                if (charAt(s, i, '.') and digitAt(s, i + 1)) break :blk m0(s, digitsEnd(s, i + 1));
                break :blk null;
            },
            // /^[+\-]?[0-9]+(?:[.,][0-9]+)?/
            .@"-9.,9 no missing 0" => blk: {
                const i: usize = if (s.len > 0 and isSign(s[0])) 1 else 0;
                if (!digitAt(s, i)) break :blk null;
                var j = digitsEnd(s, i);
                if (anyAt(s, j, ",.") and digitAt(s, j + 1)) j = digitsEnd(s, j + 1);
                break :blk m0(s, j);
            },
            .@"(-)(9.,9)(e)(99)" => blk: {
                var g: [6]Str = undefined;
                const n = enumberRe(s, &g);
                if (n == 0) break :blk null;
                break :blk .{ .m = .{ .arr = try self.arena.dupe(Str, &g) }, .rem = s[n..] };
            },
            .@"(-)(9)^(-9)" => blk: {
                var g: [3]Str = undefined;
                const n = numberPowRe(s, &g) orelse break :blk null;
                break :blk .{ .m = .{ .arr = try self.arena.dupe(Str, &g) }, .rem = s[n..] };
            },
            // ... or crystal system
            .@"state of aggregation $" => blk: {
                // (aq), (aq,$\infty$), (aq, sat)
                if (try findObserveGroups(s, lit(""), .{ .re = reAggregationBegin }, lit(")"), lit(""))) |a| {
                    //  AND end of 'phrase': /^($|[\s,;\)\]\}])/
                    if (delimiterOrEnd(a.rem, 0)) break :blk a;
                }
                // OR crystal system ($o$) (\ca$c$): /^(?:\((?:\\ca\s?)?\$[amothc]\$\))/
                if (!charAt(s, 0, '(')) break :blk null;
                var i: usize = 1;
                if (startsAt(s, 1, "\\ca")) i = 4 + spaceLen(s, 4);
                if (charAt(s, i, '$') and anyAt(s, i + 1, "amothc") and charAt(s, i + 2, '$') and charAt(s, i + 3, ')'))
                    break :blk m0(s, i + 4);
                break :blk null;
            },
            // /^_\{(\([a-z]{1,3}\))\}/
            .@"_{(state of aggregation)}$" => blk: {
                if (!startsAt(s, 0, "_{(")) break :blk null;
                var k: usize = 0;
                while (lowerAt(s, 3 + k)) k += 1;
                if (k < 1 or k > 3 or !startsAt(s, 3 + k, ")}")) break :blk null;
                break :blk group1(s, 3 + k + 2, s[2 .. 3 + k + 1]);
            },
            // /^(?:\\\{|\[|\()/
            .@"{[(" => if (startsAt(s, 0, "\\{")) m0(s, 2) else if (anyAt(s, 0, "[(")) m0(s, 1) else null,
            // /^(?:\)|\]|\\\})/
            .@")]}" => if (anyAt(s, 0, ")]")) m0(s, 1) else if (startsAt(s, 0, "\\}")) m0(s, 2) else null,
            // /^[,;]\s*/
            .@", " => if (anyAt(s, 0, ",;")) m0(s, skipSpaces(s, 1)) else null,
            // /^[,;]/
            .@"," => if (anyAt(s, 0, ",;")) m0(s, 1) else null,
            // /^[.]/
            .@"." => if (charAt(s, 0, '.')) m0(s, 1) else null,
            // /^([.⋅·•])\s*/
            .@". " => blk: {
                if (s.len == 0) break :blk null;
                const c = decode(s, 0);
                if (c.c != '.' and c.c != 0x22C5 and c.c != 0xB7 and c.c != 0x2022) break :blk null;
                break :blk group1(s, skipSpaces(s, c.n), s[0..c.n]);
            },
            // /^\.\.\.(?=$|[^.])/
            .@"..." => if (startsAt(s, 0, "...") and !charAt(s, 3, '.')) m0(s, 3) else null,
            // /^([*])\s*/
            .@"* " => if (charAt(s, 0, '*')) group1(s, skipSpaces(s, 1), s[0..1]) else null,
            .@"^{(...)}" => try findObserveGroups(s, lit("^{"), lit(""), lit(""), lit("}")),
            .@"^($...$)" => try findObserveGroups(s, lit("^"), lit("$"), lit("$"), lit("")),
            // /^\^([0-9]+|[^\\_])/
            .@"^a" => blk: {
                if (!charAt(s, 0, '^') or s.len < 2) break :blk null;
                if (digitAt(s, 1)) {
                    const e = digitsEnd(s, 1);
                    break :blk group1(s, e, s[1..e]);
                }
                if (s[1] == '\\' or s[1] == '_') break :blk null;
                const c = decode(s, 1);
                break :blk group1(s, 1 + c.n, s[1 .. 1 + c.n]);
            },
            .@"^\\x{}{}" => try findObserveGroups2(self.arena, s, lit("^"), .{ .re = reMacroBrace }, lit("}"), lit(""), lit(""), lit("{"), lit("}"), lit(""), true),
            .@"^\\x{}" => try findObserveGroups(s, lit("^"), .{ .re = reMacroBrace }, lit("}"), lit("")),
            // /^\^(\\[a-zA-Z]+)\s*/
            .@"^\\x" => blk: {
                if (!startsAt(s, 0, "^\\")) break :blk null;
                var i: usize = 2;
                while (i < s.len and isAlpha(s[i])) i += 1;
                if (i == 2) break :blk null;
                break :blk group1(s, skipSpaces(s, i), s[1..i]);
            },
            // /^\^(-?\d+)/
            .@"^(-1)" => blk: {
                if (!charAt(s, 0, '^')) break :blk null;
                const i: usize = if (charAt(s, 1, '-')) 2 else 1;
                if (!digitAt(s, i)) break :blk null;
                const e = digitsEnd(s, i);
                break :blk group1(s, e, s[1..e]);
            },
            // /^'/
            .@"'" => if (charAt(s, 0, '\'')) m0(s, 1) else null,
            .@"_{(...)}" => try findObserveGroups(s, lit("_{"), lit(""), lit(""), lit("}")),
            .@"_($...$)" => try findObserveGroups(s, lit("_"), lit("$"), lit("$"), lit("")),
            // /^_([+\-]?[0-9]+|[^\\])/
            ._9 => blk: {
                if (!charAt(s, 0, '_') or s.len < 2) break :blk null;
                const i: usize = if (isSign(s[1])) 2 else 1;
                if (digitAt(s, i)) {
                    const e = digitsEnd(s, i);
                    break :blk group1(s, e, s[1..e]);
                }
                if (s[1] == '\\') break :blk null;
                const c = decode(s, 1);
                break :blk group1(s, 1 + c.n, s[1 .. 1 + c.n]);
            },
            .@"_\\x{}{}" => try findObserveGroups2(self.arena, s, lit("_"), .{ .re = reMacroBrace }, lit("}"), lit(""), lit(""), lit("{"), lit("}"), lit(""), true),
            .@"_\\x{}" => try findObserveGroups(s, lit("_"), .{ .re = reMacroBrace }, lit("}"), lit("")),
            // /^_(\\[a-zA-Z]+)\s*/
            .@"_\\x" => blk: {
                if (!startsAt(s, 0, "_\\")) break :blk null;
                var i: usize = 2;
                while (i < s.len and isAlpha(s[i])) i += 1;
                if (i == 2) break :blk null;
                break :blk group1(s, skipSpaces(s, i), s[1..i]);
            },
            // /^(?:\^(?=_)|\_(?=\^)|[\^_]$)/
            .@"^_" => blk: {
                if (startsAt(s, 0, "^_") or startsAt(s, 0, "_^")) break :blk m0(s, 1);
                if (eql(s, "^") or eql(s, "_")) break :blk m0(s, 1);
                break :blk null;
            },
            // /^\{\}/
            .@"{}" => if (startsAt(s, 0, "{}")) m0(s, 2) else null,
            .@"{...}" => try findObserveGroups(s, lit(""), lit("{"), lit("}"), lit("")),
            .@"{(...)}" => try findObserveGroups(s, lit("{"), lit(""), lit(""), lit("}")),
            .@"$...$" => try findObserveGroups(s, lit(""), lit("$"), lit("$"), lit("")),
            .@"${(...)}$" => try findObserveGroups(s, lit("${"), lit(""), lit(""), lit("}$")),
            .@"$(...)$" => try findObserveGroups(s, lit("$"), lit(""), lit(""), lit("$")),
            // /^[=<>]/
            .@"=<>" => if (anyAt(s, 0, "=<>")) m0(s, 1) else null,
            // /^[#≡]/
            .@"#" => if (charAt(s, 0, '#')) m0(s, 1) else if (startsAt(s, 0, "\u{2261}")) m0(s, 3) else null,
            // /^\+/
            .@"+" => if (charAt(s, 0, '+')) m0(s, 1) else null,
            // /^-(?=[\s_},;\]/]|$|\([a-z]+\))/  -space -, -; -] -/ -$ -state-of-aggregation
            .@"-$" => blk: {
                if (!charAt(s, 0, '-')) break :blk null;
                if (s.len == 1 or spaceLen(s, 1) > 0 or anyAt(s, 1, "_},;]/")) break :blk m0(s, 1);
                if (charAt(s, 1, '(') and lowerAt(s, 2)) {
                    var i: usize = 2;
                    while (lowerAt(s, i)) i += 1;
                    if (charAt(s, i, ')')) break :blk m0(s, 1);
                }
                break :blk null;
            },
            // /^-(?=[0-9])/
            .@"-9" => if (charAt(s, 0, '-') and digitAt(s, 1)) m0(s, 1) else null,
            // /^-(?=(?:[spd]|sp)(?:$|[\s,;\)\]\}]))/
            .@"- orbital overlap" => blk: {
                if (!charAt(s, 0, '-')) break :blk null;
                if (anyAt(s, 1, "spd") and delimiterOrEnd(s, 2)) break :blk m0(s, 1);
                if (startsAt(s, 1, "sp") and delimiterOrEnd(s, 3)) break :blk m0(s, 1);
                break :blk null;
            },
            // /^-/
            .@"-" => if (charAt(s, 0, '-')) m0(s, 1) else null,
            // /^(?:\\pm|\$\\pm\$|\+-|\+\/-)/
            .@"pm-operator" => blk: {
                for ([_]Str{ "\\pm", "$\\pm$", "+-", "+/-" }) |alt| {
                    if (startsAt(s, 0, alt)) break :blk m0(s, alt.len);
                }
                break :blk null;
            },
            // /^(?:\+|(?:[\-=<>]|<<|>>|\\approx|\$\\approx\$)(?=\s|$|-?[0-9]))/
            .operator => blk: {
                if (charAt(s, 0, '+')) break :blk m0(s, 1);
                const la = struct {
                    fn ok(t: Str, j: usize) bool {
                        return j == t.len or spaceLen(t, j) > 0 or digitAt(t, j) or (charAt(t, j, '-') and digitAt(t, j + 1));
                    }
                };
                if (anyAt(s, 0, "-=<>") and la.ok(s, 1)) break :blk m0(s, 1);
                for ([_]Str{ "<<", ">>", "\\approx", "$\\approx$" }) |alt| {
                    if (startsAt(s, 0, alt) and la.ok(s, alt.len)) break :blk m0(s, alt.len);
                }
                break :blk null;
            },
            // /^(?:v|\(v\)|\^|\(\^\))(?=$|[\s,;\)\]\}])/
            .arrowUpDown => blk: {
                for ([_]Str{ "v", "(v)", "^", "(^)" }) |alt| {
                    if (startsAt(s, 0, alt) and delimiterOrEnd(s, alt.len)) break :blk m0(s, alt.len);
                }
                break :blk null;
            },
            .@"\\bond{(...)}" => try findObserveGroups(s, lit("\\bond{"), lit(""), lit(""), lit("}")),
            // /^(?:<->|<-->|->|<-|<=>>|<<=>|<=>|[→⟶⇌])/
            .@"->" => blk: {
                for ([_]Str{ "<->", "<-->", "->", "<-", "<=>>", "<<=>", "<=>", "\u{2192}", "\u{27F6}", "\u{21CC}" }) |alt| {
                    if (startsAt(s, 0, alt)) break :blk m0(s, alt.len);
                }
                break :blk null;
            },
            // /^[CMT](?=\[)/
            .CMT => if (anyAt(s, 0, "CMT") and charAt(s, 1, '[')) m0(s, 1) else null,
            .@"[(...)]" => try findObserveGroups(s, lit("["), lit(""), lit(""), lit("]")),
            // /^(&|\\\\|\\hline)\s*/
            .@"1st-level escape" => blk: {
                for ([_]Str{ "&", "\\\\", "\\hline" }) |alt| {
                    if (startsAt(s, 0, alt)) break :blk group1(s, skipSpaces(s, alt.len), alt);
                }
                break :blk null;
            },
            // /^(?:\\[,\ ;:])/  \\x - but output no space before
            .@"\\," => if (charAt(s, 0, '\\') and anyAt(s, 1, ", ;:")) m0(s, 2) else null,
            .@"\\x{}{}" => try findObserveGroups2(self.arena, s, lit(""), .{ .re = reMacroBrace }, lit("}"), lit(""), lit(""), lit("{"), lit("}"), lit(""), true),
            .@"\\x{}" => try findObserveGroups(s, lit(""), .{ .re = reMacroBrace }, lit("}"), lit("")),
            // /^\\ca(?:\s+|(?![a-zA-Z]))/
            .@"\\ca" => blk: {
                if (!startsAt(s, 0, "\\ca")) break :blk null;
                if (spaceLen(s, 3) > 0) break :blk m0(s, skipSpaces(s, 3));
                if (notLetterOrEnd(s, 3)) break :blk m0(s, 3);
                break :blk null;
            },
            // /^(?:\\[a-zA-Z]+\s*|\\[_&{}%])/
            .@"\\x" => blk: {
                if (!charAt(s, 0, '\\')) break :blk null;
                var i: usize = 1;
                while (i < s.len and isAlpha(s[i])) i += 1;
                if (i > 1) break :blk m0(s, skipSpaces(s, i));
                if (anyAt(s, 1, "_&{}%")) break :blk m0(s, 2);
                break :blk null;
            },
            // only those with numbers in front, because the others will be formatted correctly anyway
            .orbital => opt0(s, orbitalEnd(s)),
            // /^[\/~|]/
            .others => if (anyAt(s, 0, "/~|")) m0(s, 1) else null,
            .@"\\frac{(...)}" => try findObserveGroups2(self.arena, s, lit("\\frac{"), lit(""), lit(""), lit("}"), lit("{"), lit(""), lit(""), lit("}"), false),
            .@"\\overset{(...)}" => try findObserveGroups2(self.arena, s, lit("\\overset{"), lit(""), lit(""), lit("}"), lit("{"), lit(""), lit(""), lit("}"), false),
            .@"\\underset{(...)}" => try findObserveGroups2(self.arena, s, lit("\\underset{"), lit(""), lit(""), lit("}"), lit("{"), lit(""), lit(""), lit("}"), false),
            .@"\\underbrace{(...)}" => try findObserveGroups2(self.arena, s, lit("\\underbrace{"), lit(""), lit(""), lit("}_"), lit("{"), lit(""), lit(""), lit("}"), false),
            .@"\\color{(...)}0" => try findObserveGroups(s, lit("\\color{"), lit(""), lit(""), lit("}")),
            .@"\\color{(...)}{(...)}1" => try findObserveGroups2(self.arena, s, lit("\\color{"), lit(""), lit(""), lit("}"), lit("{"), lit(""), lit(""), lit("}"), false),
            .@"\\color(...){(...)}2" => try findObserveGroups2(self.arena, s, lit("\\color"), lit("\\"), lit(""), .{ .re = reBeforeBrace }, lit("{"), lit(""), lit(""), lit("}"), false),
            .@"\\ce{(...)}" => try findObserveGroups(s, lit("\\ce{"), lit(""), lit(""), lit("}")),
            .@"oxidation$" => if (isOxidation(s, false)) m0(s, s.len) else null,
            // 0 could be oxidation or charge
            .@"d-oxidation$" => if (isOxidation(s, true)) m0(s, s.len) else null,
            // /^[IVX]+/
            .@"roman numeral" => blk: {
                var i: usize = 0;
                while (i < s.len and isRoman(s[i])) i += 1;
                break :blk if (i == 0) null else m0(s, i);
            },
            .@"1/2$" => if (isHalf(s)) m0(s, s.len) else null,
            // e.g. 2, 0.5, 1/2, -2, n/2, +;  $a$ could be added later in parsing
            .amount, .amount2 => blk: {
                if (amountRe(s)) |n| break :blk m0(s, n);
                const a = (try findObserveGroups(s, lit(""), lit("$"), lit("$"), lit(""))) orelse break :blk null;
                // e.g. $2n-1$, $-$
                if (try amountMath(self.arena, a.m.s)) break :blk m0(s, a.m.s.len);
                break :blk null;
            },
            // /^(?:[A-Z][a-z]{0,2}|i)(?=,)/
            .@"(KV letters)," => blk: {
                if (s.len > 0 and isUpper(s[0])) {
                    var k: usize = 0;
                    while (k < 2 and lowerAt(s, 1 + k)) k += 1;
                    if (charAt(s, 1 + k, ',')) break :blk m0(s, 1 + k);
                }
                if (charAt(s, 0, 'i') and charAt(s, 1, ',')) break :blk m0(s, 1);
                break :blk null;
            },
            .@"formula$" => blk: {
                // state of aggregation = no formula: /^\([a-z]+\)$/
                if (s.len >= 3 and s[0] == '(' and s[s.len - 1] == ')') {
                    for (s[1 .. s.len - 1]) |c| {
                        if (!isLower(c)) break;
                    } else break :blk null;
                }
                break :blk if (isFormula(s)) m0(s, s.len) else null;
            },
            // /^(?:pH|pOH|pC|pK|iPr|iBu)(?=$|[^a-zA-Z])/
            .uprightEntities => blk: {
                for ([_]Str{ "pH", "pOH", "pC", "pK", "iPr", "iBu" }) |alt| {
                    if (startsAt(s, 0, alt) and notLetterOrEnd(s, alt.len)) break :blk m0(s, alt.len);
                }
                break :blk null;
            },
            // /^\s*(\/)\s*/
            .@"/" => blk: {
                const i = skipSpaces(s, 0);
                if (!charAt(s, i, '/')) break :blk null;
                break :blk group1(s, skipSpaces(s, i + 1), s[i .. i + 1]);
            },
            // /^\s*(\/\/)\s*/
            .@"//" => blk: {
                const i = skipSpaces(s, 0);
                if (!startsAt(s, i, "//")) break :blk null;
                break :blk group1(s, skipSpaces(s, i + 2), s[i .. i + 2]);
            },
            // /^\s*[*.]\s*/
            .@"*" => blk: {
                const i = skipSpaces(s, 0);
                if (!anyAt(s, i, "*.")) break :blk null;
                break :blk m0(s, skipSpaces(s, i + 1));
            },
        };
    }

    // -----------------------------------------------------------------------
    // mhchemParser.go
    // -----------------------------------------------------------------------

    fn go(self: Parser, input0: ?Str, sm: SM) Error!Items {
        const raw = input0 orelse return &.{};
        if (raw.len == 0) return &.{};
        var input = try self.normalize(raw);
        var state: Str = "0";
        var buffer: Buffer = .{};
        var output: std.ArrayList(Item) = .empty;
        const transitions = machineTransitions(sm);
        var last_len: ?usize = null;
        var watchdog: i32 = 10;
        while (true) {
            if (last_len != input.len) {
                watchdog = 10;
                last_len = input.len;
            } else {
                watchdog -= 1;
            }
            // `machine.transitions[state] || machine.transitions['*']`
            const key: Str = if (stateExists(transitions, state)) state else "*";
            iterateTransitions: for (transitions) |t| {
                if (!stateIn(t.states, key) and !stateIn(t.states, "*")) continue;
                for (t.patterns) |pat| {
                    const matches = (try self.match(pat, input)) orelse continue;
                    for (t.task.action_) |action| {
                        try self.concatArray(&output, try self.runAction(sm, &buffer, matches.m, action));
                    }
                    state = t.task.nextState orelse state;
                    if (input.len > 0) {
                        if (!t.task.revisit) input = matches.rem;
                        if (!t.task.toContinue) break :iterateTransitions;
                    } else {
                        return output.items;
                    }
                }
            }
            // Prevent infinite loop
            if (watchdog <= 0) return error.MhchemError; // "MhchemBugU"
        }
    }

    /// `input.replace(/\n/g, " ").replace(/[−–—‐]/g, "-").replace(/[…]/g, "...")`
    fn normalize(self: Parser, s: Str) Error!Str {
        const needs = std.mem.indexOfScalar(u8, s, '\n') != null or
            std.mem.indexOf(u8, s, "\u{2212}") != null or std.mem.indexOf(u8, s, "\u{2013}") != null or
            std.mem.indexOf(u8, s, "\u{2014}") != null or std.mem.indexOf(u8, s, "\u{2010}") != null or
            std.mem.indexOf(u8, s, "\u{2026}") != null;
        if (!needs) return s;
        var out: std.ArrayList(u8) = .empty;
        var i: usize = 0;
        while (i < s.len) {
            if (s[i] == '\n') {
                try out.append(self.arena, ' ');
                i += 1;
                continue;
            }
            const c = decode(s, i);
            switch (c.c) {
                0x2212, 0x2013, 0x2014, 0x2010 => try out.append(self.arena, '-'),
                0x2026 => try out.appendSlice(self.arena, "..."),
                else => try out.appendSlice(self.arena, s[i .. i + c.n]),
            }
            i += c.n;
        }
        return out.items;
    }

    /// `mhchemParser.concatArray`
    fn concatArray(self: Parser, a: *std.ArrayList(Item), b: Ret) Error!void {
        switch (b) {
            .none => {},
            .one => |it| {
                if (it == .str and it.str.len == 0) return; // falsy ""
                try a.append(self.arena, it);
            },
            .many => |items| try a.appendSlice(self.arena, items),
        }
    }

    fn cat(self: Parser, parts: []const Str) Error!Str {
        return std.mem.concat(self.arena, u8, parts);
    }

    /// JS `String(m)` of a match (arrays join with ",").
    fn mstr(self: Parser, m: MV) Error!Str {
        return switch (m) {
            .s => |s| s,
            .arr => |a| std.mem.join(self.arena, ",", a),
        };
    }

    /// `(buffer.x || "") + m`
    fn app(self: Parser, x: ?Str, m: MV) Error!Str {
        return self.cat(&.{ x orelse "", try self.mstr(m) });
    }

    /// `[ "{", ...items, "}" ]`
    fn braced(self: Parser, items: Items) Error!Ret {
        var ret: std.ArrayList(Item) = .empty;
        try ret.append(self.arena, .{ .str = "{" });
        try ret.appendSlice(self.arena, items);
        try ret.append(self.arena, .{ .str = "}" });
        return .{ .many = ret.items };
    }

    // -----------------------------------------------------------------------
    // Actions: per-machine `actions` first, then `mhchemParser.actions`
    // -----------------------------------------------------------------------

    fn runAction(self: Parser, sm: SM, buffer: *Buffer, m: MV, action: A) Error!Ret {
        switch (action.type_) {
            // --- mhchemParser.actions (generic) ---
            .@"a=" => buffer.a = try self.app(buffer.a, m),
            .@"b=" => buffer.b = try self.app(buffer.b, m),
            .@"p=" => buffer.p = try self.app(buffer.p, m),
            .@"o=" => buffer.o = try self.app(buffer.o, m),
            .@"q=" => buffer.q = try self.app(buffer.q, m),
            .@"d=" => buffer.d = try self.app(buffer.d, m),
            .@"rm=" => buffer.rm = try self.app(buffer.rm, m),
            .@"text=" => buffer.text_ = try self.app(buffer.text_, m),
            .insert => return .{ .one = try voidItem(action.option.item) },
            .@"insert+p1" => return .{ .one = try p1Item(action.option.item, try self.mstr(m)) },
            .copy => return switch (m) {
                .s => |s| .{ .one = .{ .str = s } },
                .arr => |arr| blk: {
                    const items = try self.arena.alloc(Item, arr.len);
                    for (items, arr) |*it, s| it.* = .{ .str = s };
                    break :blk .{ .many = items };
                },
            },
            .rm => return .{ .one = .{ .rm = try self.mstr(m) } },
            .text => return .{ .many = try self.go(try self.mstr(m), .text) },
            .@"{text}" => return self.braced(try self.go(try self.mstr(m), .text)),
            .@"tex-math" => return .{ .many = try self.go(try self.mstr(m), .@"tex-math") },
            .@"tex-math tight" => return .{ .many = try self.go(try self.mstr(m), .@"tex-math tight") },
            .bond => return .{ .one = .{ .bond = if (action.option == .str) action.option.str else try self.mstr(m) } },
            .@"color0-output" => return .{ .one = .{ .color0 = firstElement(m) } },
            .ce => return .{ .many = try self.go(try self.mstr(m), .ce) },
            .@"1/2" => return self.half(try self.mstr(m)),
            .@"9,9" => return .{ .many = try self.go(try self.mstr(m), .@"9,9") },

            // --- shared names, resolved per machine ---
            .output => return switch (sm) {
                .ce => self.ceOutput(buffer, if (action.option == .int) action.option.int else 0),
                .text => blk: {
                    if (!truthy(buffer.text_)) break :blk .none;
                    const ret: Item = .{ .text = buffer.text_.? };
                    buffer.* = .{};
                    break :blk .{ .one = ret };
                },
                .@"tex-math", .@"tex-math tight" => blk: {
                    if (!truthy(buffer.o)) break :blk .none;
                    const ret: Item = .{ .@"tex-math" = buffer.o.? };
                    buffer.* = .{};
                    break :blk .{ .one = ret };
                },
                .pu => self.puOutput(buffer),
                .@"pu-2" => self.pu2Output(buffer),
                else => error.MhchemError, // "MhchemBugA"
            },
            .@"state of aggregation" => return switch (sm) {
                .ce => .{ .one = .{ .@"state of aggregation" = try self.go(try self.mstr(m), .o) } },
                .pq => .{ .one = .{ .@"state of aggregation subscript" = try self.go(try self.mstr(m), .o) } },
                else => error.MhchemError,
            },
            .@"color-output" => {
                if (m != .arr) return error.MhchemError;
                const machine: SM = switch (sm) {
                    .ce => .ce,
                    .pq => .pq,
                    .bd => .bd,
                    else => return error.MhchemError,
                };
                return .{ .one = .{ .color = .{ .color1 = m.arr[0], .color2 = try self.go(m.arr[1], machine) } } };
            },
            .comma => switch (sm) {
                .ce => {
                    const s = try self.mstr(m);
                    // m.replace(/\s*$/, '')
                    var end = s.len;
                    while (end > 0) {
                        var start = end - 1;
                        while (start > 0 and (s[start] & 0xC0) == 0x80) start -= 1;
                        if (!isJsSpace(decode(s, start).c)) break;
                        end = start;
                    }
                    const a = s[0..end];
                    const withSpace = a.len != s.len;
                    if (withSpace and buffer.parenthesisLevel == 0) {
                        return .{ .one = .{ .@"comma enumeration L" = a } };
                    }
                    return .{ .one = .{ .@"comma enumeration M" = a } };
                },
                .@"9,9", .@"pu-9,9" => return .{ .one = .commaDecimal },
                else => return error.MhchemError,
            },
            .operator => return .{ .one = .{ .operator = if (action.option == .str) action.option.str else try self.mstr(m) } },
            .space => return switch (sm) {
                .pu => .{ .one = .@"pu-space-1" },
                .@"pu-2" => .{ .one = .@"pu-space-2" },
                else => error.MhchemError,
            },

            // --- stateMachines.ce.actions ---
            .@"o after d" => {
                var ret: Ret = undefined;
                if (isAllDigits(buffer.d orelse "")) {
                    const tmp = buffer.d;
                    buffer.d = null;
                    ret = try self.ceOutput(buffer, 0);
                    buffer.b = tmp;
                } else {
                    ret = try self.ceOutput(buffer, 0);
                }
                buffer.o = try self.app(buffer.o, m);
                return ret;
            },
            .@"d= kv" => {
                buffer.d = try self.mstr(m);
                buffer.dType = "kv";
            },
            .@"charge or bond" => {
                if (buffer.beginsWithBond) {
                    var ret: std.ArrayList(Item) = .empty;
                    try self.concatArray(&ret, try self.ceOutput(buffer, 0));
                    try ret.append(self.arena, .{ .bond = "-" });
                    return .{ .many = ret.items };
                }
                buffer.d = try self.mstr(m);
            },
            .@"- after o/d" => return self.afterOD(buffer, try self.mstr(m), action.option == .bool and action.option.bool),
            .@"a to o" => {
                buffer.o = buffer.a;
                buffer.a = null;
            },
            .@"sb=true" => buffer.sb = true,
            .@"sb=false" => buffer.sb = false,
            .@"beginsWithBond=true" => buffer.beginsWithBond = true,
            .@"beginsWithBond=false" => buffer.beginsWithBond = false,
            .@"parenthesisLevel++" => buffer.parenthesisLevel += 1,
            .@"parenthesisLevel--" => buffer.parenthesisLevel -= 1,
            .@"oxidation-output" => return self.braced(try self.go(try self.mstr(m), .oxidation)),
            .@"frac-output", .@"overset-output", .@"underset-output", .@"underbrace-output" => {
                if (m != .arr) return error.MhchemError;
                const pair: [2]Items = .{ try self.go(m.arr[0], .ce), try self.go(m.arr[1], .ce) };
                return .{ .one = switch (action.type_) {
                    .@"frac-output" => .{ .@"frac-ce" = pair },
                    .@"overset-output" => .{ .overset = pair },
                    .@"underset-output" => .{ .underset = pair },
                    else => .{ .underbrace = pair },
                } };
            },
            .@"r=" => buffer.r = try self.mstr(m),
            .@"rdt=" => buffer.rdt = try self.mstr(m),
            .@"rd=" => buffer.rd = try self.mstr(m),
            .@"rqt=" => buffer.rqt = try self.mstr(m),
            .@"rq=" => buffer.rq = try self.mstr(m),

            // --- stateMachines.oxidation.actions ---
            .@"roman-numeral" => return .{ .one = .{ .@"roman numeral" = try self.mstr(m) } },
            // --- stateMachines['tex-math tight'].actions ---
            .@"tight operator" => buffer.o = try self.cat(&.{ buffer.o orelse "", "{", try self.mstr(m), "}" }),
            // --- stateMachines.pu.actions ---
            .enumber => return self.enumber(m),
            .@"number^" => {
                if (m != .arr) return error.MhchemError;
                var ret: std.ArrayList(Item) = .empty;
                try self.pushSign(&ret, m.arr[0]);
                try self.concatArray(&ret, .{ .many = try self.go(m.arr[1], .@"pu-9,9") });
                try ret.append(self.arena, .{ .str = try self.cat(&.{ "^{", m.arr[2], "}" }) });
                return .{ .many = ret.items };
            },
            // --- stateMachines['pu-2'].actions ---
            .cdot => return .{ .one = .@"tight cdot" },
            .@"^(-1)" => buffer.rm = try self.cat(&.{ buffer.rm orelse "undefined", "^{", try self.mstr(m), "}" }),
            // --- stateMachines['pu-9,9'].actions ---
            .@"output-0" => return self.output9(buffer, false),
            .@"output-o" => return self.output9(buffer, true),
        }
        return .none;
    }

    /// `ret.push("\\pm ")` / `ret.push(m[0])` in 'enumber' and 'number^'.
    fn pushSign(self: Parser, ret: *std.ArrayList(Item), sign: Str) Error!void {
        if (eql(sign, "+-") or eql(sign, "+/-")) {
            try ret.append(self.arena, .{ .str = "\\pm " });
        } else if (sign.len > 0) {
            try ret.append(self.arena, .{ .str = sign });
        }
    }

    /// `mhchemParser.actions['1/2']`
    fn half(self: Parser, m0_: Str) Error!Ret {
        var ret: std.ArrayList(Item) = .empty;
        var m = m0_;
        if (m.len > 0 and isSign(m[0])) {
            try ret.append(self.arena, .{ .str = m[0..1] });
            m = m[1..];
        }
        // /^([0-9]+|\$[a-z]\$|[a-z])\/([0-9]+)(\$[a-z]\$|[a-z])?$/
        const e1 = fracNumeratorEnd(m, 0) orelse return error.MhchemError;
        if (!charAt(m, e1, '/') or !digitAt(m, e1 + 1)) return error.MhchemError;
        const e2 = digitsEnd(m, e1 + 1);
        const n3 = fracSuffix(m, e2) orelse return error.MhchemError;
        const n1 = try std.mem.replaceOwned(u8, self.arena, m[0..e1], "$", "");
        try ret.append(self.arena, .{ .frac = .{ n1, m[e1 + 1 .. e2] } });
        if (n3) |x| {
            try ret.append(self.arena, .{ .@"tex-math" = try std.mem.replaceOwned(u8, self.arena, x, "$", "") });
        }
        return .{ .many = ret.items };
    }

    /// `stateMachines.ce.actions['output']`
    fn ceOutput(self: Parser, buffer: *Buffer, entityFollows: u8) Error!Ret {
        // entityFollows:
        //   undefined = if we have nothing else to output, also ignore the just read space (buffer.sb)
        //   1 = an entity follows, never omit the space if there was one just read before (can only apply to state 1)
        //   2 = 1 + the entity can have an amount, so output a\, instead of converting it to o (can only apply to states a|as)
        var ret: Ret = undefined;
        if (!truthy(buffer.r)) {
            var out: std.ArrayList(Item) = .empty;
            const b = buffer;
            if (!truthy(b.a) and !truthy(b.b) and !truthy(b.p) and !truthy(b.o) and !truthy(b.q) and !truthy(b.d) and entityFollows == 0) {
                //ret = [];
            } else {
                if (b.sb) try out.append(self.arena, .entitySkip);
                if (!truthy(b.o) and !truthy(b.q) and !truthy(b.d) and !truthy(b.b) and !truthy(b.p) and entityFollows != 2) {
                    b.o = b.a;
                    b.a = null;
                } else if (!truthy(b.o) and !truthy(b.q) and !truthy(b.d) and (truthy(b.b) or truthy(b.p))) {
                    b.o = b.a;
                    b.d = b.b;
                    b.q = b.p;
                    b.a = null;
                    b.b = null;
                    b.p = null;
                } else {
                    if (truthy(b.o) and optEql(b.dType, "kv") and (try self.match(.@"d-oxidation$", b.d orelse "")) != null) {
                        b.dType = "oxidation";
                    } else if (truthy(b.o) and optEql(b.dType, "kv") and !truthy(b.q)) {
                        b.dType = null;
                    }
                }
                const cf = try self.arena.create(Chemfive);
                cf.* = .{
                    .a = try self.go(b.a, .a),
                    .b = try self.go(b.b, .bd),
                    .p = try self.go(b.p, .pq),
                    .o = try self.go(b.o, .o),
                    .q = try self.go(b.q, .pq),
                    .d = try self.go(b.d, if (optEql(b.dType, "oxidation")) .oxidation else .bd),
                    .dType = b.dType,
                };
                try out.append(self.arena, .{ .chemfive = cf });
            }
            ret = .{ .many = out.items };
        } else { // r
            const rd: Items = if (optEql(buffer.rdt, "M"))
                try self.go(buffer.rd, .@"tex-math")
            else if (optEql(buffer.rdt, "T"))
                try self.arena.dupe(Item, &.{.{ .text = buffer.rd orelse "" }})
            else
                try self.go(buffer.rd, .ce);
            const rq: Items = if (optEql(buffer.rqt, "M"))
                try self.go(buffer.rq, .@"tex-math")
            else if (optEql(buffer.rqt, "T"))
                try self.arena.dupe(Item, &.{.{ .text = buffer.rq orelse "" }})
            else
                try self.go(buffer.rq, .ce);
            ret = .{ .one = .{ .arrow = .{ .r = buffer.r.?, .rd = rd, .rq = rq } } };
        }
        buffer.* = .{ .parenthesisLevel = buffer.parenthesisLevel, .beginsWithBond = buffer.beginsWithBond };
        return ret;
    }

    /// `stateMachines.ce.actions['- after o/d']`
    fn afterOD(self: Parser, buffer: *Buffer, m: Str, isAfterD: bool) Error!Ret {
        const o = buffer.o orelse "";
        var c1 = try self.match(.orbital, o);
        const c2 = try self.match(.@"one lowercase greek letter $", o);
        const c3 = try self.match(.@"one lowercase latin letter $", o);
        const c4 = try self.match(.@"$one lowercase latin letter$ $", o);
        const hyphenFollows = eql(m, "-") and ((c1 != null and c1.?.rem.len == 0) or c2 != null or c3 != null or c4 != null);
        if (hyphenFollows and !truthy(buffer.a) and !truthy(buffer.b) and !truthy(buffer.p) and !truthy(buffer.d) and !truthy(buffer.q) and c1 == null and c3 != null) {
            buffer.o = try self.cat(&.{ "$", buffer.o orelse "undefined", "$" });
        }
        var ret: std.ArrayList(Item) = .empty;
        if (hyphenFollows) {
            try self.concatArray(&ret, try self.ceOutput(buffer, 0));
            try ret.append(self.arena, .hyphen);
        } else {
            c1 = try self.match(.digits, buffer.d orelse "");
            if (isAfterD and c1 != null and c1.?.rem.len == 0) {
                buffer.d = try self.cat(&.{ buffer.d orelse "", m });
                try self.concatArray(&ret, try self.ceOutput(buffer, 0));
            } else {
                try self.concatArray(&ret, try self.ceOutput(buffer, 0));
                try ret.append(self.arena, .{ .bond = "-" });
            }
        }
        return .{ .many = ret.items };
    }

    /// `stateMachines.pu.actions['enumber']`
    fn enumber(self: Parser, m: MV) Error!Ret {
        if (m != .arr) return error.MhchemError;
        var g: [6]Str = undefined;
        @memcpy(&g, m.arr[0..6]);
        var ret: std.ArrayList(Item) = .empty;
        try self.pushSign(&ret, g[0]);
        if (g[1].len > 0) {
            try self.concatArray(&ret, .{ .many = try self.go(g[1], .@"pu-9,9") });
            if (g[2].len > 0) {
                if (std.mem.indexOfAny(u8, g[2], ",.") != null) {
                    try self.concatArray(&ret, .{ .many = try self.go(g[2], .@"pu-9,9") });
                } else {
                    try ret.append(self.arena, .{ .str = g[2] });
                }
            }
            g[3] = if (g[4].len > 0) g[4] else g[3];
            if (g[3].len > 0) {
                g[3] = std.mem.trim(u8, g[3], " \t\n\r\x0B\x0C");
                if (eql(g[3], "e") or (g[3].len > 0 and g[3][0] == '*')) {
                    try ret.append(self.arena, .cdot);
                } else {
                    try ret.append(self.arena, .times);
                }
            }
        }
        if (g[3].len > 0) {
            try ret.append(self.arena, .{ .str = try self.cat(&.{ "10^{", g[5], "}" }) });
        }
        return .{ .many = ret.items };
    }

    /// `stateMachines.pu.actions['output']`
    fn puOutput(self: Parser, buffer: *Buffer) Error!Ret {
        var ret: Ret = undefined;
        if (try self.match(.@"{(...)}", buffer.d orelse "")) |md| {
            if (md.rem.len == 0) buffer.d = md.m.s;
        }
        if (try self.match(.@"{(...)}", buffer.q orelse "")) |mq| {
            if (mq.rem.len == 0) buffer.q = mq.m.s;
        }
        const celsius = [_]Str{ "\u{00B0}C", "^oC", "^{o}C" };
        const fahrenheit = [_]Str{ "\u{00B0}F", "^oF", "^{o}F" };
        if (truthy(buffer.d)) {
            buffer.d = try replaceAlternatives(self.arena, buffer.d.?, &celsius, "{}^{\\circ}C");
            buffer.d = try replaceAlternatives(self.arena, buffer.d.?, &fahrenheit, "{}^{\\circ}F");
        }
        if (truthy(buffer.q)) { // fraction
            buffer.q = try replaceAlternatives(self.arena, buffer.q.?, &celsius, "{}^{\\circ}C");
            buffer.q = try replaceAlternatives(self.arena, buffer.q.?, &fahrenheit, "{}^{\\circ}F");
            const b5d = try self.go(buffer.d, .pu);
            const b5q = try self.go(buffer.q, .pu);
            if (optEql(buffer.o, "//")) {
                ret = .{ .one = .{ .@"pu-frac" = .{ b5d, b5q } } };
            } else {
                var out: std.ArrayList(Item) = .empty;
                try out.appendSlice(self.arena, b5d);
                if (b5d.len > 1 or b5q.len > 1) {
                    try out.append(self.arena, .@" / ");
                } else {
                    try out.append(self.arena, .@"/");
                }
                try out.appendSlice(self.arena, b5q);
                ret = .{ .many = out.items };
            }
        } else { // no fraction
            ret = .{ .many = try self.go(buffer.d, .@"pu-2") };
        }
        buffer.* = .{};
        return ret;
    }

    /// `stateMachines['pu-2'].actions['output']`
    fn pu2Output(self: Parser, buffer: *Buffer) Error!Ret {
        var ret: Ret = .{ .many = &.{} };
        if (truthy(buffer.rm)) {
            const mrm = try self.match(.@"{(...)}", buffer.rm.?);
            if (mrm != null and mrm.?.rem.len == 0) {
                ret = .{ .many = try self.go(mrm.?.m.s, .pu) };
            } else {
                ret = .{ .one = .{ .rm = buffer.rm.? } };
            }
        }
        buffer.* = .{};
        return ret;
    }

    /// `stateMachines['pu-9,9'].actions['output-0']` / `['output-o']`
    fn output9(self: Parser, buffer: *Buffer, is_o: bool) Error!Ret {
        var ret: std.ArrayList(Item) = .empty;
        const t = buffer.text_ orelse "";
        if (t.len > 4) {
            if (!is_o) {
                var a = t.len % 3;
                if (a == 0) a = 3;
                // pushed in reverse: substr(i,3), separator, ..., substr(0,a); then reversed
                var tmp: std.ArrayList(Item) = .empty;
                var i: isize = @as(isize, @intCast(t.len)) - 3;
                while (i > 0) : (i -= 3) {
                    const u: usize = @intCast(i);
                    try tmp.append(self.arena, .{ .str = t[u..@min(u + 3, t.len)] });
                    try tmp.append(self.arena, .@"1000 separator");
                }
                try tmp.append(self.arena, .{ .str = t[0..a] });
                std.mem.reverse(Item, tmp.items);
                ret = tmp;
            } else {
                const a = t.len - 3;
                var i: usize = 0;
                while (i < a) : (i += 3) {
                    try ret.append(self.arena, .{ .str = t[i..@min(i + 3, t.len)] });
                    try ret.append(self.arena, .@"1000 separator");
                }
                try ret.append(self.arena, .{ .str = t[i..] });
            }
        } else {
            try ret.append(self.arena, .{ .str = t });
        }
        buffer.* = .{};
        return .{ .many = ret.items };
    }

    // -----------------------------------------------------------------------
    // texify
    // -----------------------------------------------------------------------

    /// `texify.go(input, isInner)` (recursive, max 4 levels)
    fn texify(self: Parser, input: Items, isInner: bool) Error!Str {
        var res: std.ArrayList(u8) = .empty;
        var cee = false;
        for (input) |inputi| {
            switch (inputi) {
                .str => |s| try res.appendSlice(self.arena, s),
                else => {
                    try res.appendSlice(self.arena, try self.go2(inputi));
                    if (inputi == .@"1st-level escape") cee = true;
                },
            }
        }
        if (!isInner and !cee and res.items.len > 0) {
            return self.cat(&.{ "{", res.items, "}" });
        }
        return res.items;
    }

    /// `texify._goInner`
    fn inner(self: Parser, input: Items) Error!Str {
        return self.texify(input, true);
    }

    /// `texify._go2`
    fn go2(self: Parser, buf: Item) Error!Str {
        return switch (buf) {
            .str => |s| s,
            .chemfive => |c| blk: {
                var res: std.ArrayList(u8) = .empty;
                var a = try self.inner(c.a);
                const b = try self.inner(c.b);
                const p = try self.inner(c.p);
                var o = try self.inner(c.o);
                const q = try self.inner(c.q);
                const d = try self.inner(c.d);
                //
                // a
                //
                if (a.len > 0) {
                    if (isSign(a[0])) a = try self.cat(&.{ "{", a, "}" });
                    try res.appendSlice(self.arena, a);
                    try res.appendSlice(self.arena, "\\,");
                }
                //
                // b and p
                //
                if (b.len > 0 or p.len > 0) {
                    try res.appendSlice(self.arena, try self.cat(&.{
                        "{\\vphantom{X}}",
                        "^{\\hphantom{",
                        b,
                        "}}_{\\hphantom{",
                        p,
                        "}}",
                        "{\\vphantom{X}}",
                        "^{\\smash[t]{\\vphantom{2}}\\mathllap{",
                        b,
                        "}}",
                        "_{\\vphantom{2}\\mathllap{\\smash[t]{",
                        p,
                        "}}}",
                    }));
                }
                //
                // o
                //
                if (o.len > 0) {
                    if (isSign(o[0])) o = try self.cat(&.{ "{", o, "}" });
                    try res.appendSlice(self.arena, o);
                }
                //
                // q and d
                //
                if (optEql(c.dType, "kv")) {
                    if (d.len > 0 or q.len > 0) try res.appendSlice(self.arena, "{\\vphantom{X}}");
                    if (d.len > 0) try res.appendSlice(self.arena, try self.cat(&.{ "^{", d, "}" }));
                    if (q.len > 0) try res.appendSlice(self.arena, try self.cat(&.{ "_{\\smash[t]{", q, "}}" }));
                } else if (optEql(c.dType, "oxidation")) {
                    if (d.len > 0) try res.appendSlice(self.arena, try self.cat(&.{ "{\\vphantom{X}}", "^{", d, "}" }));
                    if (q.len > 0) try res.appendSlice(self.arena, try self.cat(&.{ "{\\vphantom{X}}", "_{\\smash[t]{", q, "}}" }));
                } else {
                    if (q.len > 0) try res.appendSlice(self.arena, try self.cat(&.{ "{\\vphantom{X}}", "_{\\smash[t]{", q, "}}" }));
                    if (d.len > 0) try res.appendSlice(self.arena, try self.cat(&.{ "{\\vphantom{X}}", "^{", d, "}" }));
                }
                break :blk res.items;
            },
            .rm => |p1| self.cat(&.{ "\\mathrm{", p1, "}" }),
            .text => |p1| blk: {
                if (std.mem.indexOfAny(u8, p1, "^_") != null) {
                    const t = try replaceFirst(self.arena, try replaceFirst(self.arena, p1, " ", "~"), "-", "\\text{-}");
                    break :blk self.cat(&.{ "\\mathrm{", t, "}" });
                }
                break :blk self.cat(&.{ "\\text{", p1, "}" });
            },
            .@"roman numeral" => |p1| self.cat(&.{ "\\mathrm{", p1, "}" }),
            .@"state of aggregation" => |p1| self.cat(&.{ "\\mskip2mu ", try self.inner(p1) }),
            .@"state of aggregation subscript" => |p1| self.cat(&.{ "\\mskip1mu ", try self.inner(p1) }),
            .bond => |kind| getBond(kind) orelse error.MhchemError, // "MhchemErrorBond"
            .frac => |f| blk: {
                const c = try self.cat(&.{ "\\frac{", f[0], "}{", f[1], "}" });
                break :blk self.cat(&.{ "\\mathchoice{\\textstyle", c, "}{", c, "}{", c, "}{", c, "}" });
            },
            .@"pu-frac" => |f| blk: {
                const d = try self.cat(&.{ "\\frac{", try self.inner(f[0]), "}{", try self.inner(f[1]), "}" });
                break :blk self.cat(&.{ "\\mathchoice{\\textstyle", d, "}{", d, "}{", d, "}{", d, "}" });
            },
            .@"tex-math" => |p1| self.cat(&.{ p1, " " }),
            .@"frac-ce" => |f| self.cat(&.{ "\\frac{", try self.inner(f[0]), "}{", try self.inner(f[1]), "}" }),
            .overset => |f| self.cat(&.{ "\\overset{", try self.inner(f[0]), "}{", try self.inner(f[1]), "}" }),
            .underset => |f| self.cat(&.{ "\\underset{", try self.inner(f[0]), "}{", try self.inner(f[1]), "}" }),
            .underbrace => |f| self.cat(&.{ "\\underbrace{", try self.inner(f[0]), "}_{", try self.inner(f[1]), "}" }),
            .color => |c| self.cat(&.{ "{\\color{", c.color1, "}{", try self.inner(c.color2), "}}" }),
            .color0 => |c| self.cat(&.{ "\\color{", c, "}" }),
            .arrow => |r| blk: {
                const rd = try self.inner(r.rd);
                const rq = try self.inner(r.rq);
                const name = getArrow(r.r) orelse return error.MhchemError;
                var arrow: std.ArrayList(u8) = .empty;
                try arrow.appendSlice(self.arena, "\\x");
                try arrow.appendSlice(self.arena, name);
                if (rq.len > 0) try arrow.appendSlice(self.arena, try self.cat(&.{ "[{", rq, "}]" }));
                if (rd.len > 0) {
                    try arrow.appendSlice(self.arena, try self.cat(&.{ "{", rd, "}" }));
                } else {
                    try arrow.appendSlice(self.arena, "{}");
                }
                break :blk arrow.items;
            },
            .operator => |kind| getOperator(kind) orelse error.MhchemError,
            .@"1st-level escape" => |p1| self.cat(&.{ p1, " " }), // &, \\\\, \\hlin
            .space => " ",
            .entitySkip => "~",
            .@"pu-space-1" => "~",
            .@"pu-space-2" => "\\mkern3mu ",
            .@"1000 separator" => "\\mkern2mu ",
            .commaDecimal => "{,}",
            .@"comma enumeration L" => |p1| self.cat(&.{ "{", p1, "}\\mkern6mu " }),
            .@"comma enumeration M" => |p1| self.cat(&.{ "{", p1, "}\\mkern3mu " }),
            .@"comma enumeration S" => |p1| self.cat(&.{ "{", p1, "}\\mkern1mu " }),
            .hyphen => "\\text{-}",
            .@"addition compound" => "\\,{\\cdot}\\,",
            .@"electron dot" => "\\mkern1mu \\bullet\\mkern1mu ",
            .@"KV x" => "{\\times}",
            .prime => "\\prime ",
            .cdot => "\\cdot ",
            .@"tight cdot" => "\\mkern1mu{\\cdot}\\mkern1mu ",
            .times => "\\times ",
            .circa => "{\\sim}",
            .@"^" => "uparrow",
            .v => "downarrow",
            .ellipsis => "\\ldots ",
            .@"/" => "/",
            .@" / " => "\\,/\\,",
        };
    }
};

/// `m[0]` in 'color0-output' (on a string this is its first character).
fn firstElement(m: MV) Str {
    switch (m) {
        .arr => |a| return if (a.len > 0) a[0] else "undefined",
        .s => |s| {
            if (s.len == 0) return "undefined";
            return s[0..decode(s, 0).n];
        },
    }
}

/// `/^[0-9]+$/`
fn isAllDigits(s: Str) bool {
    return s.len > 0 and digitsEnd(s, 0) == s.len;
}

/// `texify._getArrow`
fn getArrow(a: Str) ?Str {
    const table = [_][2]Str{
        .{ "->", "rightarrow" },
        .{ "\u{2192}", "rightarrow" },
        .{ "\u{27F6}", "rightarrow" },
        .{ "<-", "leftarrow" },
        .{ "<->", "leftrightarrow" },
        .{ "<-->", "rightleftarrows" },
        .{ "<=>", "rightleftharpoons" },
        .{ "\u{21CC}", "rightleftharpoons" },
        .{ "<=>>", "rightequilibrium" },
        .{ "<<=>", "leftequilibrium" },
    };
    for (table) |e| if (eql(a, e[0])) return e[1];
    return null;
}

/// `texify._getBond`
fn getBond(a: Str) ?Str {
    const table = [_][2]Str{
        .{ "-", "{-}" },
        .{ "1", "{-}" },
        .{ "=", "{=}" },
        .{ "2", "{=}" },
        .{ "#", "{\\equiv}" },
        .{ "3", "{\\equiv}" },
        .{ "~", "{\\tripledash}" },
        .{ "~-", "{\\mathrlap{\\raisebox{-.1em}{$-$}}\\raisebox{.1em}{$\\tripledash$}}" },
        .{ "~=", "{\\mathrlap{\\raisebox{-.2em}{$-$}}\\mathrlap{\\raisebox{.2em}{$\\tripledash$}}-}" },
        .{ "~--", "{\\mathrlap{\\raisebox{-.2em}{$-$}}\\mathrlap{\\raisebox{.2em}{$\\tripledash$}}-}" },
        .{ "-~-", "{\\mathrlap{\\raisebox{-.2em}{$-$}}\\mathrlap{\\raisebox{.2em}{$-$}}\\tripledash}" },
        .{ "...", "{{\\cdot}{\\cdot}{\\cdot}}" },
        .{ "....", "{{\\cdot}{\\cdot}{\\cdot}{\\cdot}}" },
        .{ "->", "{\\rightarrow}" },
        .{ "<-", "{\\leftarrow}" },
        .{ "<", "{<}" },
        .{ ">", "{>}" },
    };
    for (table) |e| if (eql(a, e[0])) return e[1];
    return null;
}

/// `texify._getOperator`
fn getOperator(a: Str) ?Str {
    const table = [_][2]Str{
        .{ "+", " {}+{} " },
        .{ "-", " {}-{} " },
        .{ "=", " {}={} " },
        .{ "<", " {}<{} " },
        .{ ">", " {}>{} " },
        .{ "<<", " {}\\ll{} " },
        .{ ">>", " {}\\gg{} " },
        .{ "\\pm", " {}\\pm{} " },
        .{ "\\approx", " {}\\approx{} " },
        .{ "$\\approx$", " {}\\approx{} " },
        .{ "v", " \\downarrow{} " },
        .{ "(v)", " \\downarrow{} " },
        .{ "^", " \\uparrow{} " },
        .{ "(^)", " \\uparrow{} " },
    };
    for (table) |e| if (eql(a, e[0])) return e[1];
    return null;
}

// ---------------------------------------------------------------------------
// Buffer and state machine tables (mhchemParser.stateMachines)
// ---------------------------------------------------------------------------

/// Parser buffer. a == amount, o == element, b == left-side superscript,
/// p == left-side subscript, q == right-side subscript, d == right-side
/// superscript, r == arrow, rdt/rd == arrow script above (type, content),
/// rqt/rq == arrow script below. `null` == JS `undefined`.
const Buffer = struct {
    a: ?Str = null,
    b: ?Str = null,
    p: ?Str = null,
    o: ?Str = null,
    q: ?Str = null,
    d: ?Str = null,
    r: ?Str = null,
    rdt: ?Str = null,
    rd: ?Str = null,
    rqt: ?Str = null,
    rq: ?Str = null,
    text_: ?Str = null,
    rm: ?Str = null,
    dType: ?Str = null,
    sb: bool = false,
    parenthesisLevel: i32 = 0,
    beginsWithBond: bool = false,
};

const SM = enum { ce, a, o, text, pq, bd, oxidation, @"tex-math", @"tex-math tight", @"9,9", pu, @"pu-2", @"pu-9,9" };

const Act = enum {
    // mhchemParser.actions
    @"a=",
    @"b=",
    @"p=",
    @"o=",
    @"q=",
    @"d=",
    @"rm=",
    @"text=",
    insert,
    @"insert+p1",
    copy,
    rm,
    text,
    @"{text}",
    @"tex-math",
    @"tex-math tight",
    bond,
    @"color0-output",
    ce,
    @"1/2",
    @"9,9",
    // per-machine actions
    output,
    @"state of aggregation",
    @"color-output",
    comma,
    operator,
    space,
    @"o after d",
    @"d= kv",
    @"charge or bond",
    @"- after o/d",
    @"a to o",
    @"sb=true",
    @"sb=false",
    @"beginsWithBond=true",
    @"beginsWithBond=false",
    @"parenthesisLevel++",
    @"parenthesisLevel--",
    @"oxidation-output",
    @"frac-output",
    @"overset-output",
    @"underset-output",
    @"underbrace-output",
    @"r=",
    @"rdt=",
    @"rd=",
    @"rqt=",
    @"rq=",
    @"roman-numeral",
    @"tight operator",
    enumber,
    @"number^",
    cdot,
    @"^(-1)",
    @"output-0",
    @"output-o",
};

const Option = union(enum) {
    none,
    str: Str,
    int: u8,
    bool: bool,
    item: ItemTag,
};

/// `{ type_: ..., option: ... }`
const A = struct { type_: Act, option: Option = .none };

const Task = struct {
    action_: []const A = &.{},
    nextState: ?Str = null,
    revisit: bool = false,
    toContinue: bool = false,
};

const Transition = struct {
    patterns: []const Pat,
    states: Str,
    task: Task,
};

/// One `'pattern|pattern': { 'state|state': task }` entry of
/// `createTransitions`. The transition list of a state is every entry whose
/// states contain it or `*`, in table order (JS object key order; no key
/// here is integer-like within a conflicting group, so it is source order).
fn tr(comptime patterns: Str, comptime states: Str, comptime task: Task) Transition {
    return .{ .patterns = comptime pats(patterns), .states = states, .task = task };
}

fn pats(comptime s: Str) []const Pat {
    comptime {
        @setEvalBranchQuota(200_000);
        const n = std.mem.count(u8, s, "|") + 1;
        var arr: [n]Pat = undefined;
        var it = std.mem.splitScalar(u8, s, '|');
        var i: usize = 0;
        while (it.next()) |name| : (i += 1) {
            arr[i] = for (std.meta.fieldNames(Pat)) |f| {
                if (std.mem.eql(u8, f, name)) break @field(Pat, f);
            } else @compileError("unknown mhchem pattern: " ++ name);
        }
        const final = arr;
        return &final;
    }
}

fn act(comptime t: Act) A {
    return .{ .type_ = t };
}

/// Action with an `option` (int, bool, string or output type).
fn with(comptime t: Act, comptime option: anytype) A {
    const T = @TypeOf(option);
    const o: Option = switch (@typeInfo(T)) {
        .comptime_int => .{ .int = option },
        .bool => .{ .bool = option },
        .enum_literal => .{ .item = option },
        else => .{ .str = option },
    };
    return .{ .type_ = t, .option = o };
}

fn stateIn(states: Str, state: Str) bool {
    var it = std.mem.splitScalar(u8, states, '|');
    while (it.next()) |s| if (eql(s, state)) return true;
    return false;
}

fn stateExists(transitions: []const Transition, state: Str) bool {
    for (transitions) |t| if (stateIn(t.states, state)) return true;
    return false;
}

fn machineTransitions(sm: SM) []const Transition {
    return switch (sm) {
        .ce => &ce_transitions,
        .a => &a_transitions,
        .o => &o_transitions,
        .text => &text_transitions,
        .pq => &pq_transitions,
        .bd => &bd_transitions,
        .oxidation => &oxidation_transitions,
        .@"tex-math" => &tex_math_transitions,
        .@"tex-math tight" => &tex_math_tight_transitions,
        .@"9,9" => &nine_nine_transitions,
        .pu => &pu_transitions,
        .@"pu-2" => &pu_2_transitions,
        .@"pu-9,9" => &pu_nine_nine_transitions,
    };
}

// stateMachines.ce (main parser). States: 0 == begin of main part (arrow/
// operator unlikely), 1 == next entity, 2 == next entity (arrow/operator
// unlikely), 3 == next atom, c == macro; buffer letters double as states.
const ce_transitions = [_]Transition{
    tr("empty", "*", .{ .action_ = &.{act(.output)} }),
    tr("else", "0|1|2", .{ .action_ = &.{act(.@"beginsWithBond=false")}, .revisit = true, .toContinue = true }),
    tr("oxidation$", "0", .{ .action_ = &.{act(.@"oxidation-output")} }),
    tr("CMT", "r", .{ .action_ = &.{act(.@"rdt=")}, .nextState = "rt" }),
    tr("CMT", "rd", .{ .action_ = &.{act(.@"rqt=")}, .nextState = "rdt" }),
    tr("arrowUpDown", "0|1|2|as", .{ .action_ = &.{ act(.@"sb=false"), act(.output), act(.operator) }, .nextState = "1" }),
    tr("uprightEntities", "0|1|2", .{ .action_ = &.{ act(.@"o="), act(.output) }, .nextState = "1" }),
    tr("orbital", "0|1|2|3", .{ .action_ = &.{act(.@"o=")}, .nextState = "o" }),
    tr("->", "0|1|2|3", .{ .action_ = &.{act(.@"r=")}, .nextState = "r" }),
    tr("->", "a|as", .{ .action_ = &.{ act(.output), act(.@"r=") }, .nextState = "r" }),
    tr("->", "*", .{ .action_ = &.{ act(.output), act(.@"r=") }, .nextState = "r" }),
    tr("+", "o", .{ .action_ = &.{act(.@"d= kv")}, .nextState = "d" }),
    tr("+", "d|D", .{ .action_ = &.{act(.@"d=")}, .nextState = "d" }),
    tr("+", "q", .{ .action_ = &.{act(.@"d=")}, .nextState = "qd" }),
    tr("+", "qd|qD", .{ .action_ = &.{act(.@"d=")}, .nextState = "qd" }),
    tr("+", "dq", .{ .action_ = &.{ act(.output), act(.@"d=") }, .nextState = "d" }),
    tr("+", "3", .{ .action_ = &.{ act(.@"sb=false"), act(.output), act(.operator) }, .nextState = "0" }),
    tr("amount", "0|2", .{ .action_ = &.{act(.@"a=")}, .nextState = "a" }),
    tr("pm-operator", "0|1|2|a|as", .{ .action_ = &.{ act(.@"sb=false"), act(.output), with(.operator, "\\pm") }, .nextState = "0" }),
    tr("operator", "0|1|2|a|as", .{ .action_ = &.{ act(.@"sb=false"), act(.output), act(.operator) }, .nextState = "0" }),
    tr("-$", "o|q", .{ .action_ = &.{ act(.@"charge or bond"), act(.output) }, .nextState = "qd" }),
    tr("-$", "d", .{ .action_ = &.{act(.@"d=")}, .nextState = "d" }),
    tr("-$", "D", .{ .action_ = &.{ act(.output), with(.bond, "-") }, .nextState = "3" }),
    tr("-$", "q", .{ .action_ = &.{act(.@"d=")}, .nextState = "qd" }),
    tr("-$", "qd", .{ .action_ = &.{act(.@"d=")}, .nextState = "qd" }),
    tr("-$", "qD|dq", .{ .action_ = &.{ act(.output), with(.bond, "-") }, .nextState = "3" }),
    tr("-9", "3|o", .{ .action_ = &.{ act(.output), with(.insert, .hyphen) }, .nextState = "3" }),
    tr("- orbital overlap", "o", .{ .action_ = &.{ act(.output), with(.insert, .hyphen) }, .nextState = "2" }),
    tr("- orbital overlap", "d", .{ .action_ = &.{ act(.output), with(.insert, .hyphen) }, .nextState = "2" }),
    tr("-", "0|1|2", .{ .action_ = &.{ with(.output, 1), act(.@"beginsWithBond=true"), with(.bond, "-") }, .nextState = "3" }),
    tr("-", "3", .{ .action_ = &.{with(.bond, "-")} }),
    tr("-", "a", .{ .action_ = &.{ act(.output), with(.insert, .hyphen) }, .nextState = "2" }),
    tr("-", "as", .{ .action_ = &.{ with(.output, 2), with(.bond, "-") }, .nextState = "3" }),
    tr("-", "b", .{ .action_ = &.{act(.@"b=")} }),
    tr("-", "o", .{ .action_ = &.{with(.@"- after o/d", false)}, .nextState = "2" }),
    tr("-", "q", .{ .action_ = &.{with(.@"- after o/d", false)}, .nextState = "2" }),
    tr("-", "d|qd|dq", .{ .action_ = &.{with(.@"- after o/d", true)}, .nextState = "2" }),
    tr("-", "D|qD|p", .{ .action_ = &.{ act(.output), with(.bond, "-") }, .nextState = "3" }),
    tr("amount2", "1|3", .{ .action_ = &.{act(.@"a=")}, .nextState = "a" }),
    tr("letters", "0|1|2|3|a|as|b|p|bp|o", .{ .action_ = &.{act(.@"o=")}, .nextState = "o" }),
    tr("letters", "q|dq", .{ .action_ = &.{ act(.output), act(.@"o=") }, .nextState = "o" }),
    tr("letters", "d|D|qd|qD", .{ .action_ = &.{act(.@"o after d")}, .nextState = "o" }),
    tr("digits", "o", .{ .action_ = &.{act(.@"q=")}, .nextState = "q" }),
    tr("digits", "d|D", .{ .action_ = &.{act(.@"q=")}, .nextState = "dq" }),
    tr("digits", "q", .{ .action_ = &.{ act(.output), act(.@"o=") }, .nextState = "o" }),
    tr("digits", "a", .{ .action_ = &.{act(.@"o=")}, .nextState = "o" }),
    tr("space A", "b|p|bp", .{}),
    tr("space", "a", .{ .nextState = "as" }),
    tr("space", "0", .{ .action_ = &.{act(.@"sb=false")} }),
    tr("space", "1|2", .{ .action_ = &.{act(.@"sb=true")} }),
    tr("space", "r|rt|rd|rdt|rdq", .{ .action_ = &.{act(.output)}, .nextState = "0" }),
    tr("space", "*", .{ .action_ = &.{ act(.output), act(.@"sb=true") }, .nextState = "1" }),
    tr("1st-level escape", "1|2", .{ .action_ = &.{ act(.output), with(.@"insert+p1", .@"1st-level escape") } }),
    tr("1st-level escape", "*", .{ .action_ = &.{ act(.output), with(.@"insert+p1", .@"1st-level escape") }, .nextState = "0" }),
    tr("[(...)]", "r|rt", .{ .action_ = &.{act(.@"rd=")}, .nextState = "rd" }),
    tr("[(...)]", "rd|rdt", .{ .action_ = &.{act(.@"rq=")}, .nextState = "rdq" }),
    tr("...", "o|d|D|dq|qd|qD", .{ .action_ = &.{ act(.output), with(.bond, "...") }, .nextState = "3" }),
    tr("...", "*", .{ .action_ = &.{ with(.output, 1), with(.insert, .ellipsis) }, .nextState = "1" }),
    tr(". |* ", "*", .{ .action_ = &.{ act(.output), with(.insert, .@"addition compound") }, .nextState = "1" }),
    tr("state of aggregation $", "*", .{ .action_ = &.{ act(.output), act(.@"state of aggregation") }, .nextState = "1" }),
    tr("{[(", "a|as|o", .{ .action_ = &.{ act(.@"o="), act(.output), act(.@"parenthesisLevel++") }, .nextState = "2" }),
    tr("{[(", "0|1|2|3", .{ .action_ = &.{ act(.@"o="), act(.output), act(.@"parenthesisLevel++") }, .nextState = "2" }),
    tr("{[(", "*", .{ .action_ = &.{ act(.output), act(.@"o="), act(.output), act(.@"parenthesisLevel++") }, .nextState = "2" }),
    tr(")]}", "0|1|2|3|b|p|bp|o", .{ .action_ = &.{ act(.@"o="), act(.@"parenthesisLevel--") }, .nextState = "o" }),
    tr(")]}", "a|as|d|D|q|qd|qD|dq", .{ .action_ = &.{ act(.output), act(.@"o="), act(.@"parenthesisLevel--") }, .nextState = "o" }),
    tr(", ", "*", .{ .action_ = &.{ act(.output), act(.comma) }, .nextState = "0" }),
    // ^ and _ without a sensible argument
    tr("^_", "*", .{}),
    tr("^{(...)}|^($...$)", "0|1|2|as", .{ .action_ = &.{act(.@"b=")}, .nextState = "b" }),
    tr("^{(...)}|^($...$)", "p", .{ .action_ = &.{act(.@"b=")}, .nextState = "bp" }),
    tr("^{(...)}|^($...$)", "3|o", .{ .action_ = &.{act(.@"d= kv")}, .nextState = "D" }),
    tr("^{(...)}|^($...$)", "q", .{ .action_ = &.{act(.@"d=")}, .nextState = "qD" }),
    tr("^{(...)}|^($...$)", "d|D|qd|qD|dq", .{ .action_ = &.{ act(.output), act(.@"d=") }, .nextState = "D" }),
    tr("^a|^\\x{}{}|^\\x{}|^\\x|'", "0|1|2|as", .{ .action_ = &.{act(.@"b=")}, .nextState = "b" }),
    tr("^a|^\\x{}{}|^\\x{}|^\\x|'", "p", .{ .action_ = &.{act(.@"b=")}, .nextState = "bp" }),
    tr("^a|^\\x{}{}|^\\x{}|^\\x|'", "3|o", .{ .action_ = &.{act(.@"d= kv")}, .nextState = "d" }),
    tr("^a|^\\x{}{}|^\\x{}|^\\x|'", "q", .{ .action_ = &.{act(.@"d=")}, .nextState = "qd" }),
    tr("^a|^\\x{}{}|^\\x{}|^\\x|'", "d|qd|D|qD", .{ .action_ = &.{act(.@"d=")} }),
    tr("^a|^\\x{}{}|^\\x{}|^\\x|'", "dq", .{ .action_ = &.{ act(.output), act(.@"d=") }, .nextState = "d" }),
    tr("_{(state of aggregation)}$", "d|D|q|qd|qD|dq", .{ .action_ = &.{ act(.output), act(.@"q=") }, .nextState = "q" }),
    tr("_{(...)}|_($...$)|_9|_\\x{}{}|_\\x{}|_\\x", "0|1|2|as", .{ .action_ = &.{act(.@"p=")}, .nextState = "p" }),
    tr("_{(...)}|_($...$)|_9|_\\x{}{}|_\\x{}|_\\x", "b", .{ .action_ = &.{act(.@"p=")}, .nextState = "bp" }),
    tr("_{(...)}|_($...$)|_9|_\\x{}{}|_\\x{}|_\\x", "3|o", .{ .action_ = &.{act(.@"q=")}, .nextState = "q" }),
    tr("_{(...)}|_($...$)|_9|_\\x{}{}|_\\x{}|_\\x", "d|D", .{ .action_ = &.{act(.@"q=")}, .nextState = "dq" }),
    tr("_{(...)}|_($...$)|_9|_\\x{}{}|_\\x{}|_\\x", "q|qd|qD|dq", .{ .action_ = &.{ act(.output), act(.@"q=") }, .nextState = "q" }),
    tr("=<>", "0|1|2|3|a|as|o|q|d|D|qd|qD|dq", .{ .action_ = &.{ with(.output, 2), act(.bond) }, .nextState = "3" }),
    tr("#", "0|1|2|3|a|as|o", .{ .action_ = &.{ with(.output, 2), with(.bond, "#") }, .nextState = "3" }),
    tr("{}", "*", .{ .action_ = &.{with(.output, 1)}, .nextState = "1" }),
    tr("{...}", "0|1|2|3|a|as|b|p|bp", .{ .action_ = &.{act(.@"o=")}, .nextState = "o" }),
    tr("{...}", "o|d|D|q|qd|qD|dq", .{ .action_ = &.{ act(.output), act(.@"o=") }, .nextState = "o" }),
    tr("$...$", "a", .{ .action_ = &.{act(.@"a=")} }), // 2$n$
    tr("$...$", "0|1|2|3|as|b|p|bp|o", .{ .action_ = &.{act(.@"o=")}, .nextState = "o" }), // not 'amount'
    tr("$...$", "as|o", .{ .action_ = &.{act(.@"o=")} }),
    tr("$...$", "q|d|D|qd|qD|dq", .{ .action_ = &.{ act(.output), act(.@"o=") }, .nextState = "o" }),
    tr("\\bond{(...)}", "*", .{ .action_ = &.{ with(.output, 2), act(.bond) }, .nextState = "3" }),
    tr("\\frac{(...)}", "*", .{ .action_ = &.{ with(.output, 1), act(.@"frac-output") }, .nextState = "3" }),
    tr("\\overset{(...)}", "*", .{ .action_ = &.{ with(.output, 2), act(.@"overset-output") }, .nextState = "3" }),
    tr("\\underset{(...)}", "*", .{ .action_ = &.{ with(.output, 2), act(.@"underset-output") }, .nextState = "3" }),
    tr("\\underbrace{(...)}", "*", .{ .action_ = &.{ with(.output, 2), act(.@"underbrace-output") }, .nextState = "3" }),
    tr("\\color{(...)}{(...)}1|\\color(...){(...)}2", "*", .{ .action_ = &.{ with(.output, 2), act(.@"color-output") }, .nextState = "3" }),
    tr("\\color{(...)}0", "*", .{ .action_ = &.{ with(.output, 2), act(.@"color0-output") } }),
    tr("\\ce{(...)}", "*", .{ .action_ = &.{ with(.output, 2), act(.ce) }, .nextState = "3" }),
    tr("\\,", "*", .{ .action_ = &.{ with(.output, 1), act(.copy) }, .nextState = "1" }),
    tr("\\x{}{}|\\x{}|\\x", "0|1|2|3|a|as|b|p|bp|o|c0", .{ .action_ = &.{ act(.@"o="), act(.output) }, .nextState = "3" }),
    tr("\\x{}{}|\\x{}|\\x", "*", .{ .action_ = &.{ act(.output), act(.@"o="), act(.output) }, .nextState = "3" }),
    tr("others", "*", .{ .action_ = &.{ with(.output, 1), act(.copy) }, .nextState = "3" }),
    tr("else2", "a", .{ .action_ = &.{act(.@"a to o")}, .nextState = "o", .revisit = true }),
    tr("else2", "as", .{ .action_ = &.{ act(.output), act(.@"sb=true") }, .nextState = "1", .revisit = true }),
    tr("else2", "r|rt|rd|rdt|rdq", .{ .action_ = &.{act(.output)}, .nextState = "0", .revisit = true }),
    tr("else2", "*", .{ .action_ = &.{ act(.output), act(.copy) }, .nextState = "3" }),
};

const a_transitions = [_]Transition{
    tr("empty", "*", .{}),
    tr("1/2$", "0", .{ .action_ = &.{act(.@"1/2")} }),
    tr("else", "0", .{ .nextState = "1", .revisit = true }),
    tr("$(...)$", "*", .{ .action_ = &.{act(.@"tex-math tight")}, .nextState = "1" }),
    tr(",", "*", .{ .action_ = &.{with(.insert, .commaDecimal)} }),
    tr("else2", "*", .{ .action_ = &.{act(.copy)} }),
};

const o_transitions = [_]Transition{
    tr("empty", "*", .{}),
    tr("1/2$", "0", .{ .action_ = &.{act(.@"1/2")} }),
    tr("else", "0", .{ .nextState = "1", .revisit = true }),
    tr("letters", "*", .{ .action_ = &.{act(.rm)} }),
    tr("\\ca", "*", .{ .action_ = &.{with(.insert, .circa)} }),
    tr("\\x{}{}|\\x{}|\\x", "*", .{ .action_ = &.{act(.copy)} }),
    tr("${(...)}$|$(...)$", "*", .{ .action_ = &.{act(.@"tex-math")} }),
    tr("{(...)}", "*", .{ .action_ = &.{act(.@"{text}")} }),
    tr("else2", "*", .{ .action_ = &.{act(.copy)} }),
};

const text_transitions = [_]Transition{
    tr("empty", "*", .{ .action_ = &.{act(.output)} }),
    tr("{...}", "*", .{ .action_ = &.{act(.@"text=")} }),
    tr("${(...)}$|$(...)$", "*", .{ .action_ = &.{act(.@"tex-math")} }),
    tr("\\greek", "*", .{ .action_ = &.{ act(.output), act(.rm) } }),
    tr("\\,|\\x{}{}|\\x{}|\\x", "*", .{ .action_ = &.{ act(.output), act(.copy) } }),
    tr("else", "*", .{ .action_ = &.{act(.@"text=")} }),
};

const pq_transitions = [_]Transition{
    tr("empty", "*", .{}),
    tr("state of aggregation $", "*", .{ .action_ = &.{act(.@"state of aggregation")} }),
    tr("i$", "0", .{ .nextState = "!f", .revisit = true }),
    tr("(KV letters),", "0", .{ .action_ = &.{act(.rm)}, .nextState = "0" }),
    tr("formula$", "0", .{ .nextState = "f", .revisit = true }),
    tr("1/2$", "0", .{ .action_ = &.{act(.@"1/2")} }),
    tr("else", "0", .{ .nextState = "!f", .revisit = true }),
    tr("${(...)}$|$(...)$", "*", .{ .action_ = &.{act(.@"tex-math")} }),
    tr("{(...)}", "*", .{ .action_ = &.{act(.text)} }),
    tr("a-z", "f", .{ .action_ = &.{act(.@"tex-math")} }),
    tr("letters", "*", .{ .action_ = &.{act(.rm)} }),
    tr("-9.,9", "*", .{ .action_ = &.{act(.@"9,9")} }),
    tr(",", "*", .{ .action_ = &.{with(.@"insert+p1", .@"comma enumeration S")} }),
    tr("\\color{(...)}{(...)}1|\\color(...){(...)}2", "*", .{ .action_ = &.{act(.@"color-output")} }),
    tr("\\color{(...)}0", "*", .{ .action_ = &.{act(.@"color0-output")} }),
    tr("\\ce{(...)}", "*", .{ .action_ = &.{act(.ce)} }),
    tr("\\,|\\x{}{}|\\x{}|\\x", "*", .{ .action_ = &.{act(.copy)} }),
    tr("else2", "*", .{ .action_ = &.{act(.copy)} }),
};

const bd_transitions = [_]Transition{
    tr("empty", "*", .{}),
    tr("x$", "0", .{ .nextState = "!f", .revisit = true }),
    tr("formula$", "0", .{ .nextState = "f", .revisit = true }),
    tr("else", "0", .{ .nextState = "!f", .revisit = true }),
    tr("-9.,9 no missing 0", "*", .{ .action_ = &.{act(.@"9,9")} }),
    tr(".", "*", .{ .action_ = &.{with(.insert, .@"electron dot")} }),
    tr("a-z", "f", .{ .action_ = &.{act(.@"tex-math")} }),
    tr("x", "*", .{ .action_ = &.{with(.insert, .@"KV x")} }),
    tr("letters", "*", .{ .action_ = &.{act(.rm)} }),
    tr("'", "*", .{ .action_ = &.{with(.insert, .prime)} }),
    tr("${(...)}$|$(...)$", "*", .{ .action_ = &.{act(.@"tex-math")} }),
    tr("{(...)}", "*", .{ .action_ = &.{act(.text)} }),
    tr("\\color{(...)}{(...)}1|\\color(...){(...)}2", "*", .{ .action_ = &.{act(.@"color-output")} }),
    tr("\\color{(...)}0", "*", .{ .action_ = &.{act(.@"color0-output")} }),
    tr("\\ce{(...)}", "*", .{ .action_ = &.{act(.ce)} }),
    tr("\\,|\\x{}{}|\\x{}|\\x", "*", .{ .action_ = &.{act(.copy)} }),
    tr("else2", "*", .{ .action_ = &.{act(.copy)} }),
};

const oxidation_transitions = [_]Transition{
    tr("empty", "*", .{}),
    tr("roman numeral", "*", .{ .action_ = &.{act(.@"roman-numeral")} }),
    tr("${(...)}$|$(...)$", "*", .{ .action_ = &.{act(.@"tex-math")} }),
    tr("else", "*", .{ .action_ = &.{act(.copy)} }),
};

const tex_math_transitions = [_]Transition{
    tr("empty", "*", .{ .action_ = &.{act(.output)} }),
    tr("\\ce{(...)}", "*", .{ .action_ = &.{ act(.output), act(.ce) } }),
    tr("{...}|\\,|\\x{}{}|\\x{}|\\x", "*", .{ .action_ = &.{act(.@"o=")} }),
    tr("else", "*", .{ .action_ = &.{act(.@"o=")} }),
};

const tex_math_tight_transitions = [_]Transition{
    tr("empty", "*", .{ .action_ = &.{act(.output)} }),
    tr("\\ce{(...)}", "*", .{ .action_ = &.{ act(.output), act(.ce) } }),
    tr("{...}|\\,|\\x{}{}|\\x{}|\\x", "*", .{ .action_ = &.{act(.@"o=")} }),
    tr("-|+", "*", .{ .action_ = &.{act(.@"tight operator")} }),
    tr("else", "*", .{ .action_ = &.{act(.@"o=")} }),
};

const nine_nine_transitions = [_]Transition{
    tr("empty", "*", .{}),
    tr(",", "*", .{ .action_ = &.{act(.comma)} }),
    tr("else", "*", .{ .action_ = &.{act(.copy)} }),
};

// stateMachines.pu
const pu_transitions = [_]Transition{
    tr("empty", "*", .{ .action_ = &.{act(.output)} }),
    tr("space$", "*", .{ .action_ = &.{ act(.output), act(.space) } }),
    tr("{[(|)]}", "0|a", .{ .action_ = &.{act(.copy)} }),
    tr("(-)(9)^(-9)", "0", .{ .action_ = &.{act(.@"number^")}, .nextState = "a" }),
    tr("(-)(9.,9)(e)(99)", "0", .{ .action_ = &.{act(.enumber)}, .nextState = "a" }),
    tr("space", "0|a", .{}),
    tr("pm-operator", "0|a", .{ .action_ = &.{with(.operator, "\\pm")}, .nextState = "0" }),
    tr("operator", "0|a", .{ .action_ = &.{act(.copy)}, .nextState = "0" }),
    tr("//", "d", .{ .action_ = &.{act(.@"o=")}, .nextState = "/" }),
    tr("/", "d", .{ .action_ = &.{act(.@"o=")}, .nextState = "/" }),
    tr("{...}|else", "0|d", .{ .action_ = &.{act(.@"d=")}, .nextState = "d" }),
    tr("{...}|else", "a", .{ .action_ = &.{ act(.space), act(.@"d=") }, .nextState = "d" }),
    tr("{...}|else", "/|q", .{ .action_ = &.{act(.@"q=")}, .nextState = "q" }),
};

const pu_2_transitions = [_]Transition{
    tr("empty", "*", .{ .action_ = &.{act(.output)} }),
    tr("*", "*", .{ .action_ = &.{ act(.output), act(.cdot) }, .nextState = "0" }),
    tr("\\x", "*", .{ .action_ = &.{act(.@"rm=")} }),
    tr("space", "*", .{ .action_ = &.{ act(.output), act(.space) }, .nextState = "0" }),
    tr("^{(...)}|^(-1)", "1", .{ .action_ = &.{act(.@"^(-1)")} }),
    tr("-9.,9", "0", .{ .action_ = &.{act(.@"rm=")}, .nextState = "0" }),
    tr("-9.,9", "1", .{ .action_ = &.{act(.@"^(-1)")}, .nextState = "0" }),
    tr("{...}|else", "*", .{ .action_ = &.{act(.@"rm=")}, .nextState = "1" }),
};

const pu_nine_nine_transitions = [_]Transition{
    tr("empty", "0", .{ .action_ = &.{act(.@"output-0")} }),
    tr("empty", "o", .{ .action_ = &.{act(.@"output-o")} }),
    tr(",", "0", .{ .action_ = &.{ act(.@"output-0"), act(.comma) }, .nextState = "o" }),
    tr(".", "0", .{ .action_ = &.{ act(.@"output-0"), act(.copy) }, .nextState = "o" }),
    tr("else", "*", .{ .action_ = &.{act(.@"text=")} }),
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

test "mhchem fixture (generated by the real mhchem.js)" {
    const gpa = std.testing.allocator;
    const Case = struct {
        input: []const u8,
        machine: []const u8,
        /// null == mhchem.js throws
        expected: ?[]const u8,
    };
    const parsed = try std.json.parseFromSlice([]const Case, gpa, @embedFile("mhchem_cases.json"), .{});
    defer parsed.deinit();
    try std.testing.expect(parsed.value.len >= 300);

    var failures: usize = 0;
    for (parsed.value) |c| {
        var arena_state = std.heap.ArenaAllocator.init(gpa);
        defer arena_state.deinit();
        const machine: Machine = if (eql(c.machine, "pu")) .pu else .ce;
        const actual: ?[]const u8 = toTex(arena_state.allocator(), c.input, machine) catch |e| switch (e) {
            error.MhchemError => null,
            error.OutOfMemory => return e,
        };
        const same = if (actual != null and c.expected != null) eql(actual.?, c.expected.?) else actual == null and c.expected == null;
        if (!same) {
            failures += 1;
            std.debug.print("mhchem mismatch (\\{s}):\n  input:    {s}\n  expected: {?s}\n  actual:   {?s}\n", .{ c.machine, c.input, c.expected, actual });
        }
    }
    if (failures > 0) {
        std.debug.print("{d} of {d} mhchem cases failed\n", .{ failures, parsed.value.len });
        return error.TestUnexpectedResult;
    }
}

test "mhchem basics" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    try std.testing.expectEqualStrings("", try toTex(arena, "", .ce));
    try std.testing.expectEqualStrings("{\\mathrm{H}{\\vphantom{X}}_{\\smash[t]{2}}\\mathrm{O}}", try toTex(arena, "H2O", .ce));
    try std.testing.expectEqualStrings("{123~\\mathrm{kJ}\\mkern3mu \\mathrm{mol^{-1}}}", try toTex(arena, "123 kJ mol-1", .pu));
    try std.testing.expectError(error.MhchemError, toTex(arena, "A\\bond{x}B", .ce));
}

/// The fixture (input, machine, expected TeX), for integration tests that
/// lay every case out.
pub const fixture_json = @embedFile("mhchem_cases.json");
