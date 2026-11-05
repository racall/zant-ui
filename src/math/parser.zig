//! TeX math parser (KaTeX-compatible subset) producing a `Node` tree.
//!
//! Structural errors (unbalanced braces, missing arguments, unknown
//! environments) fail the whole parse — the caller then renders the source
//! in the error colour like KaTeX with `throwOnError: false`. Unknown control
//! sequences are kept as `.unknown` nodes and coloured individually.
const std = @import("std");
const symbols = @import("symbols.zig");
const fonts = @import("fonts.zig");

pub const Class = symbols.Class;

pub const StyleSwitch = enum { display, text, script, scriptscript };

pub const Node = union(enum) {
    /// A glyph run treated as one atom (letter, digit, symbol).
    atom: Atom,
    /// `{...}`: an ord atom made of a math list.
    group: []const Node,
    supsub: SupSub,
    frac: Frac,
    sqrt: Sqrt,
    left_right: LeftRight,
    /// `\big(` etc.: a fixed-size delimiter.
    sized_delim: SizedDelim,
    accent: AccentNode,
    /// `\overline`, `\underline`, `\overbrace`, `\underbrace`,
    /// `\overrightarrow`, ...
    over_under: OverUnder,
    /// `\sum`, `\int`, `\lim`, `\operatorname{..}`
    op: Op,
    text: Text,
    space: f32,
    array: Array,
    color: Color,
    style: struct { style: StyleSwitch, body: []const Node },
    phantom: struct { body: []const Node, horizontal: bool = true, vertical: bool = true },
    boxed: []const Node,
    cancel: struct { body: []const Node, kind: CancelKind },
    /// `\overset{top}{base}` / `\underset` / `\stackrel`
    stack: struct { base: *const Node, over: ?*const Node, under: ?*const Node },
    /// `\tag{..}`
    tag: []const u8,
    unknown: []const u8,
};

pub const CancelKind = enum { cancel, bcancel, xcancel, sout };

pub const Atom = struct {
    text: []const u8,
    class: Class = .ord,
    variant: fonts.Variant = .normal,
};

pub const SupSub = struct {
    base: ?*const Node,
    sup: ?*const Node = null,
    sub: ?*const Node = null,
};

pub const Frac = struct {
    num: []const Node,
    den: []const Node,
    bar: bool = true,
    left: []const u8 = "",
    right: []const u8 = "",
    style: ?StyleSwitch = null,
    /// `\cfrac`: numerator stays display style.
    continued: bool = false,
};

pub const Sqrt = struct { body: []const Node, index: ?[]const Node = null };

pub const LeftRight = struct {
    left: []const u8,
    right: []const u8,
    body: []const Node,
    // `\middle|` appears in `body` as a `.sized_delim` with size 0.
};

pub const SizedDelim = struct {
    delim: []const u8,
    /// 1..4 for \big..\Bigg; 0 = `\middle` (sized to the enclosing \left).
    size: u8,
    class: Class = .ord,
};

pub const AccentNode = struct {
    base: []const Node,
    glyph: []const u8,
    wide: bool,
    name: []const u8,
};

pub const OverUnderKind = enum { overline, underline, overbrace, underbrace, overrightarrow, overleftarrow, overleftrightarrow, underrightarrow, underleftarrow };

pub const OverUnder = struct { body: []const Node, kind: OverUnderKind };

pub const Op = struct {
    text: []const u8,
    /// true for ∑-like glyph operators (enlarged in display style).
    symbol: bool,
    integral: bool = false,
    limits: bool,
    /// `\limits` / `\nolimits` override.
    forced: ?bool = null,
};

pub const Text = struct {
    text: []const u8,
    variant: fonts.Variant = .roman,
    bold: bool = false,
};

pub const Array = struct {
    rows: []const []const []const Node,
    /// One entry per column: 'l', 'c', 'r'.
    cols: []const u8,
    /// Vertical rule positions: column index before which a `|` is drawn.
    col_rules: []const usize = &.{},
    /// Rows (index before which) a `\hline` is drawn.
    row_rules: []const usize = &.{},
    left: []const u8 = "",
    right: []const u8 = "",
    display: bool = false,
    /// Column separation in em (KaTeX arraycolsep 5pt = 0.5em per side).
    col_sep: f32 = 0.5,
    /// aligned/align: alternate r/l columns with no separation inside pairs.
    aligned: bool = false,
    small: bool = false,
    /// Extra space after each row (em), from `\\[..]`.
    row_gaps: []const f32 = &.{},
};

pub const Color = struct { color: u32, body: []const Node };

pub const Error = error{ OutOfMemory, ParseError };

pub const Parser = struct {
    arena: std.mem.Allocator,
    src: []const u8,
    i: usize = 0,

    pub fn init(arena: std.mem.Allocator, src: []const u8) Parser {
        return .{ .arena = arena, .src = src };
    }

    pub fn parseAll(self: *Parser) Error![]const Node {
        const list = try self.parseList(.none);
        self.skipSpace();
        if (self.i < self.src.len) return error.ParseError; // stray '}' etc.
        return list;
    }

    const Stop = enum { none, brace, bracket, right, end_env };

    fn peek(self: *Parser) u8 {
        return if (self.i < self.src.len) self.src[self.i] else 0;
    }

    fn skipSpace(self: *Parser) void {
        while (self.i < self.src.len and std.ascii.isWhitespace(self.src[self.i])) self.i += 1;
    }

    fn readCommandName(self: *Parser) []const u8 {
        // at '\'
        self.i += 1;
        const start = self.i;
        if (self.i >= self.src.len) return "";
        if (!std.ascii.isAlphabetic(self.src[self.i])) {
            self.i += 1;
            return self.src[start..self.i];
        }
        while (self.i < self.src.len and std.ascii.isAlphabetic(self.src[self.i])) self.i += 1;
        // `\operatorname*`
        return self.src[start..self.i];
    }

    fn atEnd(self: *Parser, stop: Stop) bool {
        self.skipSpace();
        if (self.i >= self.src.len) return true;
        const c = self.src[self.i];
        return switch (stop) {
            .none => c == '}',
            .brace => c == '}',
            .bracket => c == ']' or c == '}',
            .right => c == '}' or self.lookingAtCommand("right"),
            .end_env => c == '}' or c == '&' or self.lookingAtCommand("end") or self.lookingAt("\\\\") or
                self.lookingAtCommand("cr") or self.lookingAtCommand("hline"),
        };
    }

    fn lookingAt(self: *Parser, s: []const u8) bool {
        return std.mem.startsWith(u8, self.src[self.i..], s);
    }

    fn lookingAtCommand(self: *Parser, name: []const u8) bool {
        if (!self.lookingAt("\\")) return false;
        const rest = self.src[self.i + 1 ..];
        if (!std.mem.startsWith(u8, rest, name)) return false;
        return rest.len == name.len or !std.ascii.isAlphabetic(rest[name.len]);
    }

    fn box(self: *Parser, node: Node) Error!*const Node {
        const p = try self.arena.create(Node);
        p.* = node;
        return p;
    }

    pub fn parseList(self: *Parser, stop: Stop) Error![]const Node {
        var out: std.ArrayListUnmanaged(Node) = .empty;
        while (!self.atEnd(stop)) {
            if (stop == .none and self.peek() == '}') break;
            const c = self.peek();
            if (c == '^' or c == '_') {
                // script without a base
                try self.attachScripts(&out, null);
                continue;
            }
            if (c == '\'') {
                try self.attachScripts(&out, null);
                continue;
            }
            const node = (try self.parseAtom(&out)) orelse continue;
            try out.append(self.arena, node);
            self.skipSpace();
            const n = self.peek();
            if (n == '^' or n == '_' or n == '\'') {
                const base = out.pop().?;
                try self.attachScripts(&out, base);
            }
        }
        return out.toOwnedSlice(self.arena);
    }

    fn attachScripts(self: *Parser, out: *std.ArrayListUnmanaged(Node), base: ?Node) Error!void {
        var sup: ?*const Node = null;
        var sub: ?*const Node = null;
        var primes: usize = 0;
        while (true) {
            self.skipSpace();
            const c = self.peek();
            if (c == '\'') {
                primes += 1;
                self.i += 1;
                continue;
            }
            if (c != '^' and c != '_') break;
            self.i += 1;
            const arg = try self.parseScriptArg();
            if (c == '^') {
                if (sup != null) return error.ParseError; // double superscript
                sup = arg;
            } else {
                if (sub != null) return error.ParseError;
                sub = arg;
            }
        }
        if (primes > 0) {
            var prime_text: std.ArrayListUnmanaged(u8) = .empty;
            for (0..primes) |_| try prime_text.appendSlice(self.arena, "′");
            const prime_node: Node = .{ .atom = .{ .text = try prime_text.toOwnedSlice(self.arena), .variant = .roman } };
            if (sup) |existing| {
                sup = try self.box(.{ .group = try self.arena.dupe(Node, &.{ prime_node, existing.* }) });
            } else sup = try self.box(prime_node);
        }
        // \limits / \nolimits apply to the op base
        var base_node = base;
        if (base_node) |*b| {
            if (b.* == .op) {
                // handled at parse of the op
            }
        }
        try out.append(self.arena, .{ .supsub = .{
            .base = if (base_node) |b| try self.box(b) else null,
            .sup = sup,
            .sub = sub,
        } });
    }

    fn parseScriptArg(self: *Parser) Error!*const Node {
        self.skipSpace();
        if (self.i >= self.src.len) return error.ParseError;
        const c = self.peek();
        if (c == '{') {
            const list = try self.parseGroupBody();
            return self.box(.{ .group = list });
        }
        if (c == '\\') {
            var dummy: std.ArrayListUnmanaged(Node) = .empty;
            const node = (try self.parseAtom(&dummy)) orelse return error.ParseError;
            return self.box(node);
        }
        // a single character
        const len = std.unicode.utf8ByteSequenceLength(c) catch 1;
        const text = self.src[self.i..@min(self.i + len, self.src.len)];
        self.i += len;
        return self.box(try self.charNode(text));
    }

    fn parseGroupBody(self: *Parser) Error![]const Node {
        self.skipSpace();
        if (self.peek() != '{') return error.ParseError;
        self.i += 1;
        const list = try self.parseList(.brace);
        self.skipSpace();
        if (self.peek() != '}') return error.ParseError;
        self.i += 1;
        return list;
    }

    /// A required argument: `{...}` or a single token.
    fn parseArg(self: *Parser) Error![]const Node {
        self.skipSpace();
        if (self.peek() == '{') return self.parseGroupBody();
        if (self.i >= self.src.len) return error.ParseError;
        var dummy: std.ArrayListUnmanaged(Node) = .empty;
        if (self.peek() == '\\') {
            const node = (try self.parseAtom(&dummy)) orelse return error.ParseError;
            return self.arena.dupe(Node, &.{node});
        }
        const len = std.unicode.utf8ByteSequenceLength(self.peek()) catch 1;
        const text = self.src[self.i..@min(self.i + len, self.src.len)];
        self.i += len;
        return self.arena.dupe(Node, &.{try self.charNode(text)});
    }

    /// Raw text of a `{...}` argument (balanced braces).
    fn rawArg(self: *Parser) Error![]const u8 {
        self.skipSpace();
        if (self.peek() != '{') {
            if (self.i >= self.src.len) return error.ParseError;
            const len = std.unicode.utf8ByteSequenceLength(self.peek()) catch 1;
            const t = self.src[self.i..@min(self.i + len, self.src.len)];
            self.i += len;
            return t;
        }
        const start = self.i + 1;
        var depth: usize = 0;
        while (self.i < self.src.len) : (self.i += 1) {
            const c = self.src[self.i];
            if (c == '\\') {
                self.i += 1;
                continue;
            }
            if (c == '{') depth += 1;
            if (c == '}') {
                depth -= 1;
                if (depth == 0) {
                    const out = self.src[start..self.i];
                    self.i += 1;
                    return out;
                }
            }
        }
        return error.ParseError;
    }

    fn optionalArg(self: *Parser) Error!?[]const Node {
        self.skipSpace();
        if (self.peek() != '[') return null;
        self.i += 1;
        const list = try self.parseList(.bracket);
        if (self.peek() != ']') return error.ParseError;
        self.i += 1;
        return list;
    }

    fn charNode(self: *Parser, text: []const u8) Error!Node {
        _ = self;
        const cp = std.unicode.utf8Decode(text) catch 0xFFFD;
        if (text.len == 1) {
            const c = text[0];
            if (c == '-') return .{ .atom = .{ .text = "−", .class = .bin } };
            if (c == '*') return .{ .atom = .{ .text = "∗", .class = .bin } };
            if (c == '~') return .{ .space = 0.25 };
            if (std.ascii.isDigit(c) or c == '.') return .{ .atom = .{ .text = text, .variant = .roman } };
        }
        return .{ .atom = .{ .text = text, .class = symbols.charClass(cp) } };
    }

    fn parseAtom(self: *Parser, out: *std.ArrayListUnmanaged(Node)) Error!?Node {
        self.skipSpace();
        const c = self.peek();
        if (c == '{') {
            const list = try self.parseGroupBody();
            return .{ .group = list };
        }
        if (c == '&') return error.ParseError; // outside an environment
        if (c != '\\') {
            const len = std.unicode.utf8ByteSequenceLength(c) catch 1;
            const text = self.src[self.i..@min(self.i + len, self.src.len)];
            self.i += len;
            // merge runs of digits ("123" is one atom, "1.5" too)
            if (std.ascii.isDigit(c)) {
                const start = self.i - 1;
                while (self.i < self.src.len and (std.ascii.isDigit(self.src[self.i]) or
                    (self.src[self.i] == '.' and self.i + 1 < self.src.len and std.ascii.isDigit(self.src[self.i + 1])))) self.i += 1;
                return .{ .atom = .{ .text = self.src[start..self.i], .variant = .roman } };
            }
            return try self.charNode(text);
        }
        const name = self.readCommandName();
        return self.command(name, out);
    }

    fn command(self: *Parser, name: []const u8, out: *std.ArrayListUnmanaged(Node)) Error!?Node {
        const eql = std.mem.eql;
        // spacing
        if (eql(u8, name, ",") or eql(u8, name, "thinspace")) return .{ .space = 3.0 / 18.0 };
        if (eql(u8, name, ":") or eql(u8, name, ">") or eql(u8, name, "medspace")) return .{ .space = 4.0 / 18.0 };
        if (eql(u8, name, ";") or eql(u8, name, "thickspace")) return .{ .space = 5.0 / 18.0 };
        if (eql(u8, name, "!") or eql(u8, name, "negthinspace")) return .{ .space = -3.0 / 18.0 };
        if (eql(u8, name, "negmedspace")) return .{ .space = -4.0 / 18.0 };
        if (eql(u8, name, "negthickspace")) return .{ .space = -5.0 / 18.0 };
        if (eql(u8, name, "quad")) return .{ .space = 1.0 };
        if (eql(u8, name, "qquad")) return .{ .space = 2.0 };
        if (eql(u8, name, " ") or eql(u8, name, "space") or eql(u8, name, "nobreakspace")) return .{ .space = 0.25 };
        if (eql(u8, name, "enspace")) return .{ .space = 0.5 };
        if (eql(u8, name, "hspace") or eql(u8, name, "kern") or eql(u8, name, "mkern") or eql(u8, name, "mskip") or eql(u8, name, "hskip")) {
            const raw = try self.rawArgOrDimen();
            return .{ .space = parseDimen(raw) };
        }
        // line break outside environments: ignore (KaTeX: newline in display)
        if (eql(u8, name, "\\") or eql(u8, name, "newline") or eql(u8, name, "nonumber") or eql(u8, name, "notag") or
            eql(u8, name, "displaylimits") or eql(u8, name, "allowbreak") or eql(u8, name, "relax")) return null;
        if (eql(u8, name, "limits") or eql(u8, name, "nolimits")) {
            if (out.items.len > 0) {
                const last = &out.items[out.items.len - 1];
                if (last.* == .op) last.op.forced = eql(u8, name, "limits");
            }
            return null;
        }

        if (symbols.lookup(name)) |sym| {
            // .normal italicises only lowercase Greek; other symbols are unchanged
            return .{ .atom = .{ .text = sym.text, .class = sym.class, .variant = .normal } };
        }
        if (symbols.bigOp(name)) |op| return .{ .op = .{ .text = op.text, .symbol = true, .integral = op.integral, .limits = op.limits } };
        if (symbols.function(name)) |f| return .{ .op = .{ .text = f.text, .symbol = false, .limits = f.limits } };
        if (eql(u8, name, "operatorname") or eql(u8, name, "operatornamewithlimits")) {
            var limits = eql(u8, name, "operatornamewithlimits");
            if (self.peek() == '*') {
                self.i += 1;
                limits = true;
            }
            const raw = try self.rawArg();
            return .{ .op = .{ .text = try stripBraces(self.arena, raw), .symbol = false, .limits = limits } };
        }
        if (eql(u8, name, "bmod")) return .{ .atom = .{ .text = "mod", .class = .bin, .variant = .roman } };
        if (eql(u8, name, "pmod") or eql(u8, name, "mod") or eql(u8, name, "pod")) {
            const arg = try self.parseArg();
            var parts: std.ArrayListUnmanaged(Node) = .empty;
            try parts.append(self.arena, .{ .space = if (eql(u8, name, "mod")) 0.75 else 1.0 });
            if (!eql(u8, name, "mod")) try parts.append(self.arena, .{ .atom = .{ .text = "(", .class = .open } });
            if (!eql(u8, name, "pod")) {
                try parts.append(self.arena, .{ .atom = .{ .text = "mod", .variant = .roman } });
                try parts.append(self.arena, .{ .space = 1.0 / 3.0 });
            }
            try parts.append(self.arena, .{ .group = arg });
            if (!eql(u8, name, "mod")) try parts.append(self.arena, .{ .atom = .{ .text = ")", .class = .close } });
            return .{ .group = try parts.toOwnedSlice(self.arena) };
        }

        // fractions
        if (eql(u8, name, "frac") or eql(u8, name, "dfrac") or eql(u8, name, "tfrac") or eql(u8, name, "cfrac")) {
            const num = try self.parseArg();
            const den = try self.parseArg();
            return .{ .frac = .{
                .num = num,
                .den = den,
                .style = if (eql(u8, name, "dfrac") or eql(u8, name, "cfrac")) .display else if (eql(u8, name, "tfrac")) .text else null,
                .continued = eql(u8, name, "cfrac"),
            } };
        }
        if (eql(u8, name, "binom") or eql(u8, name, "dbinom") or eql(u8, name, "tbinom")) {
            const num = try self.parseArg();
            const den = try self.parseArg();
            return .{ .frac = .{ .num = num, .den = den, .bar = false, .left = "(", .right = ")", .style = if (eql(u8, name, "dbinom")) .display else if (eql(u8, name, "tbinom")) .text else null } };
        }
        if (eql(u8, name, "over") or eql(u8, name, "choose") or eql(u8, name, "atop")) {
            // infix: everything before in this list is the numerator
            const num = try out.toOwnedSlice(self.arena);
            const den = try self.parseList(.brace);
            const is_choose = eql(u8, name, "choose");
            return .{ .frac = .{ .num = num, .den = den, .bar = eql(u8, name, "over"), .left = if (is_choose) "(" else "", .right = if (is_choose) ")" else "" } };
        }
        if (eql(u8, name, "sqrt")) {
            const index = try self.optionalArg();
            const body = try self.parseArg();
            return .{ .sqrt = .{ .body = body, .index = index } };
        }
        if (eql(u8, name, "left")) {
            const left = try self.delimToken();
            const body = try self.parseList(.right);
            if (!self.lookingAtCommand("right")) return error.ParseError;
            self.i += "\\right".len;
            const right = try self.delimToken();
            return .{ .left_right = .{ .left = left, .right = right, .body = body } };
        }
        if (eql(u8, name, "middle")) return .{ .sized_delim = .{ .delim = try self.delimToken(), .size = 0, .class = .rel } };
        if (eql(u8, name, "right")) return error.ParseError;
        inline for (.{ .{ "big", 1 }, .{ "Big", 2 }, .{ "bigg", 3 }, .{ "Bigg", 4 } }) |entry| {
            inline for (.{ .{ "", Class.ord }, .{ "l", Class.open }, .{ "r", Class.close }, .{ "m", Class.rel } }) |suffix| {
                if (eql(u8, name, entry[0] ++ suffix[0])) {
                    return .{ .sized_delim = .{ .delim = try self.delimToken(), .size = entry[1], .class = suffix[1] } };
                }
            }
        }
        if (symbols.accent(name)) |acc| {
            const base = try self.parseArg();
            return .{ .accent = .{ .base = base, .glyph = acc.glyph, .wide = acc.wide, .name = acc.name } };
        }
        inline for (.{
            .{ "overline", OverUnderKind.overline },                     .{ "underline", OverUnderKind.underline },
            .{ "overbrace", OverUnderKind.overbrace },                   .{ "underbrace", OverUnderKind.underbrace },
            .{ "overrightarrow", OverUnderKind.overrightarrow },         .{ "overleftarrow", OverUnderKind.overleftarrow },
            .{ "overleftrightarrow", OverUnderKind.overleftrightarrow }, .{ "underrightarrow", OverUnderKind.underrightarrow },
            .{ "underleftarrow", OverUnderKind.underleftarrow },
        }) |entry| {
            if (eql(u8, name, entry[0])) return .{ .over_under = .{ .body = try self.parseArg(), .kind = entry[1] } };
        }
        if (eql(u8, name, "overset") or eql(u8, name, "underset") or eql(u8, name, "stackrel")) {
            const annotation = try self.parseArg();
            const base = try self.parseArg();
            const a = try self.box(.{ .group = annotation });
            const b = try self.box(.{ .group = base });
            return .{ .stack = .{ .base = b, .over = if (eql(u8, name, "underset")) null else a, .under = if (eql(u8, name, "underset")) a else null } };
        }
        if (fonts.fromCommand(name)) |variant| {
            const body = try self.parseArg();
            return .{ .group = try applyVariant(self.arena, body, variant) };
        }
        if (eql(u8, name, "text") or eql(u8, name, "textrm") or eql(u8, name, "textnormal") or eql(u8, name, "mbox") or
            eql(u8, name, "textup") or eql(u8, name, "textsf") or eql(u8, name, "texttt") or eql(u8, name, "textbf") or
            eql(u8, name, "textit") or eql(u8, name, "hbox") or eql(u8, name, "textmd"))
        {
            const raw = try self.rawArg();
            return .{ .text = .{
                .text = try textModeString(self.arena, raw),
                .variant = if (eql(u8, name, "textit")) .italic else if (eql(u8, name, "texttt")) .monospace else if (eql(u8, name, "textsf")) .sans_serif else .roman,
                .bold = eql(u8, name, "textbf"),
            } };
        }
        if (eql(u8, name, "color") or eql(u8, name, "textcolor")) {
            const color_name = try self.rawArg();
            const color = parseColor(color_name) orelse return .{ .unknown = color_name };
            if (eql(u8, name, "textcolor")) return .{ .color = .{ .color = color, .body = try self.parseArg() } };
            // \color switches the rest of the current group
            const rest = try self.parseList(.brace);
            return .{ .color = .{ .color = color, .body = rest } };
        }
        inline for (.{ .{ "displaystyle", StyleSwitch.display }, .{ "textstyle", StyleSwitch.text }, .{ "scriptstyle", StyleSwitch.script }, .{ "scriptscriptstyle", StyleSwitch.scriptscript } }) |entry| {
            if (eql(u8, name, entry[0])) return .{ .style = .{ .style = entry[1], .body = try self.parseList(.brace) } };
        }
        if (eql(u8, name, "phantom")) return .{ .phantom = .{ .body = try self.parseArg() } };
        if (eql(u8, name, "hphantom")) return .{ .phantom = .{ .body = try self.parseArg(), .vertical = false } };
        if (eql(u8, name, "vphantom")) return .{ .phantom = .{ .body = try self.parseArg(), .horizontal = false } };
        if (eql(u8, name, "boxed") or eql(u8, name, "fbox")) return .{ .boxed = try self.parseArg() };
        inline for (.{ .{ "cancel", CancelKind.cancel }, .{ "bcancel", CancelKind.bcancel }, .{ "xcancel", CancelKind.xcancel }, .{ "sout", CancelKind.sout } }) |entry| {
            if (eql(u8, name, entry[0])) return .{ .cancel = .{ .body = try self.parseArg(), .kind = entry[1] } };
        }
        if (eql(u8, name, "not")) {
            self.skipSpace();
            if (self.lookingAt("=")) {
                self.i += 1;
                return .{ .atom = .{ .text = "≠", .class = .rel } };
            }
            if (self.lookingAtCommand("in")) {
                self.i += 3;
                return .{ .atom = .{ .text = "∉", .class = .rel } };
            }
            const next = (try self.parseAtom(out)) orelse return null;
            return .{ .cancel = .{ .body = try self.arena.dupe(Node, &.{next}), .kind = .cancel } };
        }
        if (eql(u8, name, "tag") or eql(u8, name, "tag*")) return .{ .tag = try self.rawArg() };
        if (eql(u8, name, "begin")) return try self.environment();
        if (eql(u8, name, "end")) return error.ParseError;
        if (eql(u8, name, "mathop")) return .{ .op = .{ .text = try stripBraces(self.arena, try self.rawArg()), .symbol = false, .limits = true } };
        if (eql(u8, name, "mathrel") or eql(u8, name, "mathbin") or eql(u8, name, "mathord") or eql(u8, name, "mathopen") or eql(u8, name, "mathclose") or eql(u8, name, "mathpunct") or eql(u8, name, "mathinner")) {
            const body = try self.parseArg();
            const class: Class = if (eql(u8, name, "mathrel")) .rel else if (eql(u8, name, "mathbin")) .bin else if (eql(u8, name, "mathopen")) .open else if (eql(u8, name, "mathclose")) .close else if (eql(u8, name, "mathpunct")) .punct else if (eql(u8, name, "mathinner")) .inner else .ord;
            if (body.len == 1 and body[0] == .atom) {
                var atom = body[0].atom;
                atom.class = class;
                return .{ .atom = atom };
            }
            return .{ .group = body };
        }
        return .{ .unknown = name };
    }

    fn rawArgOrDimen(self: *Parser) Error![]const u8 {
        self.skipSpace();
        if (self.peek() == '{') return self.rawArg();
        const start = self.i;
        while (self.i < self.src.len and (std.ascii.isDigit(self.src[self.i]) or self.src[self.i] == '.' or self.src[self.i] == '-')) self.i += 1;
        while (self.i < self.src.len and std.ascii.isAlphabetic(self.src[self.i]) and self.i - start < 8) self.i += 1;
        return self.src[start..self.i];
    }

    fn delimToken(self: *Parser) Error![]const u8 {
        self.skipSpace();
        if (self.i >= self.src.len) return error.ParseError;
        var token: []const u8 = undefined;
        if (self.peek() == '\\') {
            const start = self.i;
            _ = self.readCommandName();
            token = self.src[start..self.i];
        } else {
            const len = std.unicode.utf8ByteSequenceLength(self.peek()) catch 1;
            token = self.src[self.i..@min(self.i + len, self.src.len)];
            self.i += len;
        }
        return symbols.delimiter(token) orelse error.ParseError;
    }

    fn environment(self: *Parser) Error!Node {
        const name = try self.rawArg();
        const eql = std.mem.eql;
        var arr: Array = .{ .rows = &.{}, .cols = "" };
        var col_spec: []const u8 = "";
        var default_align: u8 = 'c';
        if (eql(u8, name, "matrix") or eql(u8, name, "matrix*")) {} else if (eql(u8, name, "pmatrix")) {
            arr.left = "(";
            arr.right = ")";
        } else if (eql(u8, name, "bmatrix")) {
            arr.left = "[";
            arr.right = "]";
        } else if (eql(u8, name, "Bmatrix")) {
            arr.left = "{";
            arr.right = "}";
        } else if (eql(u8, name, "vmatrix")) {
            arr.left = "|";
            arr.right = "|";
        } else if (eql(u8, name, "Vmatrix")) {
            arr.left = "‖";
            arr.right = "‖";
        } else if (eql(u8, name, "smallmatrix")) {
            arr.small = true;
            arr.col_sep = 0.2778;
        } else if (eql(u8, name, "cases") or eql(u8, name, "dcases")) {
            arr.left = "{";
            col_spec = "ll";
            arr.col_sep = 0.5;
            arr.display = eql(u8, name, "dcases");
        } else if (eql(u8, name, "rcases")) {
            arr.right = "}";
            col_spec = "ll";
        } else if (eql(u8, name, "aligned") or eql(u8, name, "align") or eql(u8, name, "align*") or eql(u8, name, "split") or
            eql(u8, name, "alignat") or eql(u8, name, "alignat*") or eql(u8, name, "alignedat") or eql(u8, name, "eqnarray") or eql(u8, name, "eqnarray*"))
        {
            if (std.mem.startsWith(u8, name, "alignat") or eql(u8, name, "alignedat")) _ = try self.rawArg();
            arr.aligned = true;
            arr.display = true;
            arr.col_sep = 0;
        } else if (eql(u8, name, "gathered") or eql(u8, name, "gather") or eql(u8, name, "gather*") or eql(u8, name, "equation") or eql(u8, name, "equation*")) {
            arr.display = true;
            arr.col_sep = 0;
        } else if (eql(u8, name, "array") or eql(u8, name, "darray")) {
            col_spec = try self.rawArg();
            arr.display = eql(u8, name, "darray");
        } else return error.ParseError;
        if (std.mem.endsWith(u8, name, "matrix*")) {
            if (try self.optionalRaw()) |a| default_align = if (a.len > 0) a[0] else 'c';
        }

        var rows: std.ArrayListUnmanaged([]const []const Node) = .empty;
        var row_rules: std.ArrayListUnmanaged(usize) = .empty;
        var gaps: std.ArrayListUnmanaged(f32) = .empty;
        var cells: std.ArrayListUnmanaged([]const Node) = .empty;
        var max_cols: usize = 0;
        while (true) {
            self.skipSpace();
            if (self.lookingAtCommand("hline")) {
                self.i += "\\hline".len;
                try row_rules.append(self.arena, rows.items.len);
                continue;
            }
            const cell = try self.parseList(.end_env);
            try cells.append(self.arena, cell);
            self.skipSpace();
            if (self.peek() == '&') {
                self.i += 1;
                continue;
            }
            if (self.lookingAt("\\\\") or self.lookingAtCommand("cr")) {
                self.i += if (self.lookingAt("\\\\")) 2 else 3;
                var gap: f32 = 0;
                self.skipSpace();
                if (self.peek() == '[') {
                    const close = std.mem.indexOfScalarPos(u8, self.src, self.i, ']') orelse return error.ParseError;
                    gap = parseDimen(self.src[self.i + 1 .. close]);
                    self.i = close + 1;
                }
                max_cols = @max(max_cols, cells.items.len);
                try rows.append(self.arena, try cells.toOwnedSlice(self.arena));
                try gaps.append(self.arena, gap);
                continue;
            }
            if (self.lookingAtCommand("end")) {
                self.i += "\\end".len;
                const end_name = try self.rawArg();
                if (!eql(u8, end_name, name)) return error.ParseError;
                // drop a trailing empty row (`a \\ b \\`)
                const empty_last = cells.items.len == 1 and cells.items[0].len == 0;
                if (!empty_last or rows.items.len == 0) {
                    max_cols = @max(max_cols, cells.items.len);
                    try rows.append(self.arena, try cells.toOwnedSlice(self.arena));
                    try gaps.append(self.arena, 0);
                }
                break;
            }
            return error.ParseError;
        }
        arr.rows = try rows.toOwnedSlice(self.arena);
        arr.row_rules = try row_rules.toOwnedSlice(self.arena);
        arr.row_gaps = try gaps.toOwnedSlice(self.arena);

        var cols: std.ArrayListUnmanaged(u8) = .empty;
        var col_rules: std.ArrayListUnmanaged(usize) = .empty;
        if (col_spec.len > 0) {
            for (col_spec) |c| switch (c) {
                'l', 'c', 'r' => try cols.append(self.arena, c),
                '|' => try col_rules.append(self.arena, cols.items.len),
                else => {},
            };
        }
        while (cols.items.len < max_cols) {
            const a: u8 = if (arr.aligned) (if (cols.items.len % 2 == 0) 'r' else 'l') else if (col_spec.len > 0) 'l' else default_align;
            try cols.append(self.arena, a);
        }
        arr.cols = try cols.toOwnedSlice(self.arena);
        arr.col_rules = try col_rules.toOwnedSlice(self.arena);
        return .{ .array = arr };
    }

    fn optionalRaw(self: *Parser) Error!?[]const u8 {
        self.skipSpace();
        if (self.peek() != '[') return null;
        const close = std.mem.indexOfScalarPos(u8, self.src, self.i, ']') orelse return error.ParseError;
        const out = self.src[self.i + 1 .. close];
        self.i = close + 1;
        return out;
    }
};

fn stripBraces(arena: std.mem.Allocator, raw: []const u8) ![]const u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var i: usize = 0;
    while (i < raw.len) : (i += 1) {
        const c = raw[i];
        if (c == '{' or c == '}') continue;
        if (c == '\\' and i + 1 < raw.len) {
            i += 1;
            if (raw[i] == ',' or raw[i] == ' ') {
                try out.append(arena, ' ');
                continue;
            }
        }
        try out.append(arena, raw[i]);
    }
    return out.toOwnedSlice(arena);
}

/// `\text{...}` content: strip braces, handle `\$`, `\%`, `\&`, `~`.
fn textModeString(arena: std.mem.Allocator, raw: []const u8) ![]const u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var i: usize = 0;
    while (i < raw.len) : (i += 1) {
        const c = raw[i];
        if (c == '{' or c == '}') continue;
        if (c == '~') {
            try out.append(arena, ' ');
            continue;
        }
        if (c == '\\' and i + 1 < raw.len) {
            const n = raw[i + 1];
            if (!std.ascii.isAlphabetic(n)) {
                try out.append(arena, if (n == ' ' or n == ',') ' ' else n);
                i += 1;
                continue;
            }
        }
        try out.append(arena, c);
    }
    return out.toOwnedSlice(arena);
}

fn applyVariant(arena: std.mem.Allocator, body: []const Node, variant: fonts.Variant) ![]const Node {
    const out = try arena.alloc(Node, body.len);
    for (body, 0..) |node, i| {
        out[i] = switch (node) {
            .atom => |a| blk: {
                var atom = a;
                atom.variant = variant;
                break :blk .{ .atom = atom };
            },
            .group => |g| .{ .group = try applyVariant(arena, g, variant) },
            .supsub => |s| blk: {
                var copy = s;
                if (s.base) |base| {
                    const nb = try arena.create(Node);
                    const mapped = try applyVariant(arena, &.{base.*}, variant);
                    nb.* = mapped[0];
                    copy.base = nb;
                }
                break :blk .{ .supsub = copy };
            },
            else => node,
        };
    }
    return out;
}

/// Parses a TeX dimension into em (`3mu`, `1em`, `2pt`, `0.5ex`).
pub fn parseDimen(raw: []const u8) f32 {
    const t = std.mem.trim(u8, raw, " \t");
    var end: usize = 0;
    while (end < t.len and (std.ascii.isDigit(t[end]) or t[end] == '.' or t[end] == '-' or t[end] == '+')) end += 1;
    const value = std.fmt.parseFloat(f32, t[0..end]) catch 0;
    const unit = std.mem.trim(u8, t[end..], " ");
    if (std.mem.eql(u8, unit, "mu")) return value / 18.0;
    if (std.mem.eql(u8, unit, "pt")) return value / 10.0;
    if (std.mem.eql(u8, unit, "ex")) return value * 0.431;
    if (std.mem.eql(u8, unit, "px")) return value / 16.0;
    if (std.mem.eql(u8, unit, "mm")) return value * 0.2845;
    if (std.mem.eql(u8, unit, "cm")) return value * 2.845;
    if (std.mem.eql(u8, unit, "in")) return value * 7.227;
    return value; // em (default)
}

pub fn parseColor(raw: []const u8) ?u32 {
    const t = std.mem.trim(u8, raw, " \t");
    if (t.len == 0) return null;
    if (t[0] == '#' or (t.len == 6 and isHex(t))) {
        const hex = if (t[0] == '#') t[1..] else t;
        if (hex.len == 6) return 0xff000000 | (std.fmt.parseInt(u32, hex, 16) catch return null);
        if (hex.len == 3) {
            const v = std.fmt.parseInt(u32, hex, 16) catch return null;
            const r = (v >> 8) & 0xf;
            const g = (v >> 4) & 0xf;
            const b = v & 0xf;
            return 0xff000000 | (r * 17) << 16 | (g * 17) << 8 | (b * 17);
        }
        return null;
    }
    const named = .{
        .{ "red", 0xffdf0030 },    .{ "blue", 0xff0000ff },    .{ "green", 0xff008000 },    .{ "black", 0xff000000 },
        .{ "white", 0xffffffff },  .{ "gray", 0xff808080 },    .{ "grey", 0xff808080 },     .{ "orange", 0xffff8000 },
        .{ "purple", 0xff800080 }, .{ "magenta", 0xffff00ff }, .{ "cyan", 0xff00ffff },     .{ "yellow", 0xffffd700 },
        .{ "brown", 0xffa52a2a },  .{ "pink", 0xffff69b4 },    .{ "teal", 0xff008080 },     .{ "olive", 0xff808000 },
        .{ "violet", 0xff8000ff }, .{ "lime", 0xff00ff00 },    .{ "darkgray", 0xff404040 }, .{ "lightgray", 0xffbfbfbf },
    };
    inline for (named) |entry| if (std.ascii.eqlIgnoreCase(t, entry[0])) return entry[1];
    return null;
}

fn isHex(t: []const u8) bool {
    for (t) |c| if (!std.ascii.isHex(c)) return false;
    return true;
}

pub fn parse(arena: std.mem.Allocator, source: []const u8) Error![]const Node {
    var p = Parser.init(arena, source);
    return p.parseAll();
}

test "parse structures" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const nodes = try parse(a, "\\frac{a}{b} + x_i^2 + \\sqrt[3]{y} \\left( \\sum_{k=1}^n k \\right)");
    try std.testing.expect(nodes[0] == .frac);
    try std.testing.expect(nodes[2] == .supsub);
    try std.testing.expect(nodes[4] == .sqrt);
    try std.testing.expect(nodes[5] == .left_right);
    const env = try parse(a, "\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}");
    try std.testing.expectEqual(@as(usize, 2), env[0].array.rows.len);
    try std.testing.expectEqual(@as(usize, 2), env[0].array.cols.len);
    try std.testing.expectError(error.ParseError, parse(a, "\\frac{a}{b"));
    const cases = try parse(a, "f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & \\text{otherwise} \\end{cases}");
    try std.testing.expect(cases[cases.len - 1] == .array);
}
