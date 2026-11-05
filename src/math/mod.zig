//! Small native LaTeX math renderer.
//! This is a structural renderer for common formulas, not a full TeX engine:
//! fractions, roots, scripts, groups, Greek letters and operators are laid out
//! with Zant nodes. Unknown commands fall back to readable text.
const std = @import("std");
const ui = @import("../ui.zig");

const Expr = union(enum) {
    text: []const u8,
    row: []const Expr,
    frac: struct { num: *Expr, den: *Expr },
    sqrt: *Expr,
};

const Parser = struct {
    arena: std.mem.Allocator,
    src: []const u8,
    i: usize = 0,

    fn parse(self: *Parser) ![]const Expr {
        return self.sequence(0);
    }

    fn sequence(self: *Parser, stop: u8) ![]const Expr {
        var out: std.ArrayListUnmanaged(Expr) = .empty;
        while (self.i < self.src.len) {
            if (stop != 0 and self.src[self.i] == stop) break;
            if (self.src[self.i] == '&') {
                self.i += 1;
                continue;
            }
            const item = try self.atom();
            if (self.i < self.src.len and (self.src[self.i] == '^' or self.src[self.i] == '_')) {
                try out.append(self.arena, item);
                while (self.i < self.src.len and (self.src[self.i] == '^' or self.src[self.i] == '_')) {
                    const is_sup = self.src[self.i] == '^';
                    self.i += 1;
                    const arg = try self.atom();
                    const plain = try self.flatten(arg);
                    try out.append(self.arena, .{ .text = try self.scriptText(plain, is_sup) });
                }
                continue;
            }
            try out.append(self.arena, item);
        }
        return out.toOwnedSlice(self.arena);
    }

    fn flatten(self: *Parser, expr: Expr) ![]const u8 {
        return switch (expr) {
            .text => |text| text,
            .row => |items| blk: {
                var out: std.ArrayListUnmanaged(u8) = .empty;
                for (items) |item| try out.appendSlice(self.arena, try self.flatten(item));
                break :blk out.toOwnedSlice(self.arena);
            },
            .frac => |f| std.fmt.allocPrint(self.arena, "({s})/({s})", .{ try self.flatten(f.num.*), try self.flatten(f.den.*) }),
            .sqrt => |body| std.fmt.allocPrint(self.arena, "√({s})", .{try self.flatten(body.*)}),
        };
    }

    fn scriptText(self: *Parser, plain: []const u8, sup: bool) ![]const u8 {
        var out: std.ArrayListUnmanaged(u8) = .empty;
        for (plain) |c| {
            const mapped: ?[]const u8 = if (sup) switch (c) {
                '0' => "⁰", '1' => "¹", '2' => "²", '3' => "³", '4' => "⁴",
                '5' => "⁵", '6' => "⁶", '7' => "⁷", '8' => "⁸", '9' => "⁹",
                'n' => "ⁿ", 'i' => "ⁱ", 'x' => "ˣ", 'a' => "ᵃ", 'b' => "ᵇ",
                'c' => "ᶜ", 'd' => "ᵈ", 'e' => "ᵉ", 'k' => "ᵏ", 'm' => "ᵐ",
                'o' => "ᵒ", 'p' => "ᵖ", 't' => "ᵗ", 'u' => "ᵘ", 'v' => "ᵛ",
                'w' => "ʷ", 'y' => "ʸ", '+' => "⁺", '-' => "⁻", '=' => "⁼",
                else => null,
            } else switch (c) {
                '0' => "₀", '1' => "₁", '2' => "₂", '3' => "₃", '4' => "₄",
                '5' => "₅", '6' => "₆", '7' => "₇", '8' => "₈", '9' => "₉",
                'i' => "ᵢ", 'j' => "ⱼ", 'n' => "ₙ", 'x' => "ₓ", '+' => "₊", '-' => "₋",
                else => null,
            };
            if (mapped) |value| {
                try out.appendSlice(self.arena, value);
            } else {
                try out.append(self.arena, if (sup) '^' else '_');
                try out.append(self.arena, c);
            }
        }
        return out.toOwnedSlice(self.arena);
    }

    fn box(self: *Parser, expr: Expr) !*Expr {
        const value = try self.arena.create(Expr);
        value.* = expr;
        return value;
    }

    fn atom(self: *Parser) anyerror!Expr {
        if (self.i >= self.src.len) return .{ .text = "" };
        const ch = self.src[self.i];
        if (ch == '{') {
            self.i += 1;
            const body = try self.sequence('}');
            if (self.i < self.src.len and self.src[self.i] == '}') self.i += 1;
            return .{ .row = body };
        }
        if (ch == '\\') {
            self.i += 1;
            const start = self.i;
            while (self.i < self.src.len and std.ascii.isAlphabetic(self.src[self.i])) self.i += 1;
            const name = self.src[start..self.i];
            if (std.mem.eql(u8, name, "frac")) {
                const num = try self.atom();
                const den = try self.atom();
                return .{ .frac = .{ .num = try self.box(num), .den = try self.box(den) } };
            }
            if (std.mem.eql(u8, name, "sqrt")) {
                const body = try self.atom();
                return .{ .sqrt = try self.box(body) };
            }
            if (std.mem.eql(u8, name, "text") or std.mem.eql(u8, name, "mathrm")) {
                return self.atom();
            }
            if (std.mem.eql(u8, name, "left") or std.mem.eql(u8, name, "right")) {
                if (self.i < self.src.len) self.i += 1;
                return self.atom();
            }
            return .{ .text = try self.arena.dupe(u8, commandText(name)) };
        }
        if (ch == '^' or ch == '_') return .{ .text = "" };
        const start = self.i;
        self.i += 1;
        while (self.i < self.src.len and (self.src[self.i] & 0xC0) == 0x80) self.i += 1;
        return .{ .text = try self.arena.dupe(u8, self.src[start..self.i]) };
    }
};

fn commandText(name: []const u8) []const u8 {
    const map = [_]struct { []const u8, []const u8 }{
        .{ "alpha", "α" }, .{ "beta", "β" }, .{ "gamma", "γ" }, .{ "delta", "δ" },
        .{ "theta", "θ" }, .{ "lambda", "λ" }, .{ "mu", "μ" }, .{ "pi", "π" },
        .{ "sigma", "σ" }, .{ "phi", "φ" }, .{ "omega", "ω" }, .{ "Gamma", "Γ" },
        .{ "Delta", "Δ" }, .{ "Theta", "Θ" }, .{ "Lambda", "Λ" }, .{ "Pi", "Π" },
        .{ "Sigma", "Σ" }, .{ "Phi", "Φ" }, .{ "Omega", "Ω" },
        .{ "sum", "∑" }, .{ "prod", "∏" }, .{ "int", "∫" }, .{ "infty", "∞" },
        .{ "times", "×" }, .{ "cdot", "·" }, .{ "pm", "±" }, .{ "le", "≤" },
        .{ "ge", "≥" }, .{ "neq", "≠" }, .{ "approx", "≈" }, .{ "to", "→" },
        .{ "rightarrow", "→" }, .{ "leftarrow", "←" }, .{ "partial", "∂" },
        .{ "nabla", "∇" }, .{ "in", "∈" }, .{ "forall", "∀" }, .{ "exists", "∃" },
        .{ "ldots", "…" }, .{ "cdots", "⋯" },
    };
    for (map) |entry| if (std.mem.eql(u8, name, entry[0])) return entry[1];
    return name;
}

fn richText(text: []const u8, size: f32) ui.Node {
    return ui.text(text, .{ .size = size, .family = .serif, .color = 0xff292d2a });
}

fn renderExpr(expr: Expr, size: f32) ui.Node {
    return switch (expr) {
        .text => |text| richText(text, size),
        .row => |items| blk: {
            var children: [64]ui.Node = undefined;
            var n: usize = 0;
            for (items) |item| {
                if (n >= children.len) break;
                children[n] = renderExpr(item, size);
                n += 1;
            }
            if (n == 1) break :blk children[0];
            break :blk ui.row(.{ .gap = 1, .align_items = .center, .children = children[0..n] });
        },
        .frac => |f| ui.column(.{
            .align_items = .stretch,
            .gap = 2,
            .children = &.{
                renderExpr(f.num.*, size - 1),
                ui.styled(ui.space(.{ .height = 1 }), .{ .background = 0xff292d2a }),
                renderExpr(f.den.*, size - 1),
            },
        }),
        .sqrt => |body| ui.row(.{
            .gap = 1,
            .align_items = .end,
            .children = &.{
                richText("√", size),
                ui.column(.{
                    .align_items = .stretch,
                    .children = &.{
                        ui.styled(ui.space(.{ .height = 1 }), .{ .background = 0xff292d2a }),
                        renderExpr(body.*, size),
                    },
                }),
            },
        }),
    };
}

fn renderCell(allocator: std.mem.Allocator, source: []const u8) ui.Node {
    var arena = std.heap.ArenaAllocator.init(allocator);
    defer arena.deinit();
    var parser = Parser{ .arena = arena.allocator(), .src = source };
    const items = parser.parse() catch return ui.text(source, .{ .size = 18, .family = .serif });
    if (items.len == 0) return ui.space(.{ .width = 1, .height = 1 });
    return renderExpr(.{ .row = items }, 18);
}

fn findEnv(source: []const u8) ?struct { body: []const u8, kind: []const u8 } {
    const begin = std.mem.indexOf(u8, source, "\\begin{") orelse return null;
    const name_start = begin + "\\begin{".len;
    const name_end = std.mem.indexOfScalarPos(u8, source, name_start, '}') orelse return null;
    const kind = source[name_start..name_end];
    const end_marker = std.fmt.allocPrint(std.heap.c_allocator, "{s}{s}{s}", .{ "\\end{", kind, "}" }) catch return null;
    defer std.heap.c_allocator.free(end_marker);
    const end = std.mem.indexOfPos(u8, source, name_end + 1, end_marker) orelse return null;
    return .{ .body = source[name_end + 1 .. end], .kind = kind };
}

fn renderMatrix(allocator: std.mem.Allocator, source: []const u8, kind: []const u8) ui.Node {
    const env = findEnv(source) orelse return ui.text(source, .{ .size = 18, .family = .serif });
    var row_nodes: [24]ui.Node = undefined;
    var row_count: usize = 0;
    var rows = std.mem.splitSequence(u8, env.body, "\\\\");
    while (rows.next()) |row| {
        if (row_count >= row_nodes.len) break;
        var cell_nodes: [24]ui.Node = undefined;
        var cell_count: usize = 0;
        var cells = std.mem.splitScalar(u8, row, '&');
        while (cells.next()) |cell| {
            if (cell_count >= cell_nodes.len) break;
            cell_nodes[cell_count] = renderCell(allocator, std.mem.trim(u8, cell, " \t\n"));
            cell_count += 1;
        }
        row_nodes[row_count] = ui.row(.{
            .gap = 18,
            .align_items = .center,
            .children = cell_nodes[0..cell_count],
        });
        row_count += 1;
    }
    const grid = ui.column(.{ .gap = 8, .align_items = .center, .children = row_nodes[0..row_count] });
    if (std.mem.eql(u8, kind, "matrix")) return grid;
    if (std.mem.eql(u8, kind, "pmatrix") or std.mem.eql(u8, kind, "bmatrix")) {
        const left = if (std.mem.eql(u8, kind, "pmatrix")) "(" else "[";
        const right = if (std.mem.eql(u8, kind, "pmatrix")) ")" else "]";
        return ui.row(.{
            .gap = 7,
            .align_items = .center,
            .children = &.{
                ui.text(left, .{ .size = 30, .family = .serif, .color = 0xff292d2a }),
                grid,
                ui.text(right, .{ .size = 30, .family = .serif, .color = 0xff292d2a }),
            },
        });
    }
    return grid;
}

pub fn render(allocator: std.mem.Allocator, source: []const u8) ui.Node {
    if (findEnv(source)) |env| {
        return ui.column(.{
            .width_percent = 100,
            .align_items = .center,
            .padding = .{ .top = 7, .bottom = 7 },
            .children = &.{renderMatrix(allocator, source, env.kind)},
        });
    }
    var arena = std.heap.ArenaAllocator.init(allocator);
    defer arena.deinit();
    var parser = Parser{ .arena = arena.allocator(), .src = source };
    const items = parser.parse() catch return ui.text(source, .{ .size = 15, .family = .serif });
    if (items.len == 0) return ui.text(source, .{ .size = 15, .family = .serif });
    return ui.column(.{
        .width_percent = 100,
        .align_items = .center,
        .padding = .{ .top = 7, .bottom = 7 },
        .children = &.{renderExpr(.{ .row = items }, 20)},
    });
}

pub fn inlineText(allocator: std.mem.Allocator, source: []const u8) ![]const u8 {
    var arena = std.heap.ArenaAllocator.init(allocator);
    defer arena.deinit();
    var parser = Parser{ .arena = arena.allocator(), .src = source };
    const items = parser.parse() catch return allocator.dupe(u8, source);
    if (items.len == 0) return allocator.dupe(u8, source);
    const flat = try parser.flatten(.{ .row = items });
    return allocator.dupe(u8, flat);
}
