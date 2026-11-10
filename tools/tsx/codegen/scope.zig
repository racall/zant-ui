//! Function scopes, Zig name clashes, unused-value discards, and handler state snapshots.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @This();
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Type = types.Type;
const Value = types.Value;
const Error = core.Error;

pub const Binding = struct {
    name: []const u8,
    zig: []const u8,
    ty: Type,
    mutable: bool = false,
    assigned: bool = false,
    init: ?[]const u8 = null,
    serial: usize,
};

pub const ReturnValue = struct { value: Value, loc: ast.Loc };

pub const Context = struct {
    g: *Generator,
    render: bool,
    ret: Type,
    active: std.ArrayList(*Binding) = .empty,
    bindings: std.ArrayList(*Binding) = .empty,
    params: std.ArrayList(*Binding) = .empty,
    constant: bool = false,
    return_label: ?[]const u8 = null,
    return_values: ?*std.ArrayList(ReturnValue) = null,
    capture_source: ?*Context = null,
    capture_method: ?usize = null,
    snapshots: std.ArrayList(struct { state: usize, binding: *Binding }) = .empty,
};

pub fn local(c: *Context, n: []const u8) ?*Binding {
    var i = c.active.items.len;
    while (i > 0) {
        i -= 1;
        const b = c.active.items[i];
        if (zig_text.eq(b.name, n)) return b;
    }
    return null;
}

/// Allocate a Zig name that cannot shadow imports, methods, parameters, or enclosing locals.
pub fn bind(c: *Context, n: []const u8, t: Type, mutable: bool) Error!*Binding {
    var candidate = n;
    var suffix: usize = 2;
    while (scope.nameTaken(c, candidate)) : (suffix += 1) candidate = try zig_text.fmt(c.g, "{s}_{d}", .{ n, suffix });
    const b = try c.g.a.create(Binding);
    b.* = .{
        .name = n,
        .zig = try zig_text.id(c.g, candidate),
        .ty = t,
        .mutable = mutable,
        .serial = c.bindings.items.len,
    };
    try c.bindings.append(c.g.a, b);
    try c.active.append(c.g.a, b);
    return b;
}

pub fn nameTaken(c: *Context, n: []const u8) bool {
    const receiver = zig_text.eq(n, "self") or (c.capture_method != null and zig_text.eq(n, "cap"));
    if (receiver or zig_text.eq(n, "_") or scope.globalName(c.g, n)) return true;
    for (c.bindings.items) |binding| {
        if (zig_text.eq(binding.zig, n)) return true;
        if (zig_text.eq(binding.zig, zig_text.id(c.g, n) catch return true)) return true;
    }
    return false;
}

/// Resolve declaration placeholders after prop replacement, then insert only required discards.
pub fn finish(c: *Context, body: []const u8) Error![]const u8 {
    var result = body;
    var remaining = c.bindings.items.len;
    while (remaining > 0) {
        remaining -= 1;
        const binding = c.bindings.items[remaining];
        const init = binding.init orelse continue;
        const marker = try zig_text.fmt(c.g, "\x01{d}\x02", .{binding.serial});
        const decl = try zig_text.fmt(
            c.g,
            "{s} {s}: {s} = {s};\n\x03{d}\x04",
            .{ if (binding.assigned) "var" else "const", binding.zig, binding.ty.zig(c.g), init, binding.serial },
        );
        result = try std.mem.replaceOwned(u8, c.g.a, result, marker, decl);
    }
    // Count identifiers in the surviving output, rather than in visited
    // expressions: a later prop alias may have replaced an earlier value.
    var usage_body = result;
    for (c.bindings.items) |binding| if (binding.init != null) {
        usage_body = try std.mem.replaceOwned(
            u8,
            c.g.a,
            usage_body,
            try zig_text.fmt(c.g, "\x03{d}\x04", .{binding.serial}),
            "",
        );
    };
    const source = try c.g.a.allocSentinel(u8, usage_body.len, 0);
    @memcpy(source, usage_body);
    var tokenizer = std.zig.Tokenizer.init(source);
    var counts = try c.g.a.alloc(usize, c.bindings.items.len);
    @memset(counts, 0);
    var self_used = false;
    var after_period = false;
    while (true) {
        const tok = tokenizer.next();
        if (tok.tag == .eof) break;
        const field_name = after_period;
        after_period = tok.tag == .period;
        if (tok.tag != .identifier or field_name) continue;
        const spelling = source[tok.loc.start..tok.loc.end];
        if (zig_text.eq(spelling, "self")) self_used = true;
        for (c.bindings.items, 0..) |binding, i| {
            if (zig_text.eq(spelling, binding.zig)) counts[i] += 1;
        }
    }
    for (c.bindings.items, counts) |binding, count| if (binding.init != null) {
        const discard = if (count == 1) try zig_text.fmt(c.g, "_ = {s};\n", .{binding.zig}) else "";
        result = try std.mem.replaceOwned(
            u8,
            c.g.a,
            result,
            try zig_text.fmt(c.g, "\x03{d}\x04", .{binding.serial}),
            discard,
        );
    };
    var prefix: std.ArrayList(u8) = .empty;
    for (c.params.items) |binding| if (counts[binding.serial] == 0) {
        try prefix.appendSlice(c.g.a, try zig_text.fmt(c.g, "_ = {s};\n", .{binding.zig}));
    };
    for (c.snapshots.items) |snap| {
        if (counts[snap.binding.serial] == 0) continue;
        self_used = true;
        const state = c.g.states.items[snap.state];
        const field = try zig_text.id(c.g, state.name);
        const init = if (@import("model.zig").owned(c.g, state.ty))
            try zig_text.fmt(c.g, "self.{s}.get()", .{field})
        else if (state.ty == .string) try zig_text.fmt(
            c.g,
            "ui.frameString(self.{s}.get())",
            .{field},
        ) else try zig_text.fmt(c.g, "self.{s}", .{field});
        try prefix.appendSlice(
            c.g.a,
            try zig_text.fmt(c.g, "const {s}: {s} = {s};\n", .{ snap.binding.zig, state.ty.zig(c.g), init }),
        );
    }
    if (!self_used) try prefix.appendSlice(c.g.a, "_ = self;\n");
    return zig_text.fmt(c.g, "{s}{s}", .{ prefix.items, result });
}

pub fn stateIndex(g: *Generator, n: []const u8) ?usize {
    for (g.states.items, 0..) |s, i| if (zig_text.eq(s.name, n)) return i;
    return null;
}

pub fn setterIndex(g: *Generator, n: []const u8) ?usize {
    for (g.states.items, 0..) |s, i| if (zig_text.eq(s.setter, n)) return i;
    return null;
}

pub fn methodIndex(g: *Generator, n: []const u8) ?usize {
    for (g.methods.items, 0..) |m, i| if (zig_text.eq(m.name, n)) return i;
    return null;
}

pub fn sourceMethod(g: *Generator, n: []const u8) ?usize {
    for (g.methods.items, 0..) |m, i| if (m.user and zig_text.eq(m.name, n)) return i;
    return null;
}

pub fn globalName(g: *Generator, n: []const u8) bool {
    for (g.module_names) |name| if (zig_text.eq(n, name)) return true;
    inline for (.{ "self", "std", "zant", "ui", "tsx", "render" }) |reserved| {
        if (zig_text.eq(n, reserved)) return true;
    }
    for (g.methods.items) |method| if (method.capture_name) |name| {
        if (zig_text.eq(name, n)) return true;
    };
    for (g.compounds.items) |entry| if (entry.declaration != null and zig_text.eq(entry.zig, n)) return true;
    for (g.type_aliases.items) |alias| if (zig_text.eq(alias.name, n)) return true;
    for (g.constants.items) |constant| if (zig_text.eq(constant.name, n)) return true;
    return zig_text.eq(n, g.name) or scope.methodIndex(g, n) != null;
}

pub fn uniqueMethod(g: *Generator, base: []const u8) Error![]const u8 {
    var n = base;
    var i: usize = 2;
    while (scope.globalName(g, n) or scope.stateIndex(g, n) != null) : (i += 1) {
        n = try zig_text.fmt(g, "{s}_{d}", .{ base, i });
    }
    return n;
}

pub fn moduleNames(g: *Generator) Error!void {
    for ([_][]const u8{ "std", "zant", "ui", "tsx" }, 0..) |name, i| {
        if (scope.methodIndex(g, name) != null) {
            const base = try zig_text.fmt(g, "_{s}", .{name});
            g.module_names[i] = try scope.uniqueMethod(g, base);
        }
    }
}

/// Preserve user method names by renaming conflicting generated module references.
pub fn renameModules(g: *Generator, raw: []const u8) Error![]const u8 {
    var unchanged = true;
    for ([_][]const u8{ "std", "zant", "ui", "tsx" }, g.module_names) |original, current| {
        if (!zig_text.eq(original, current)) {
            unchanged = false;
            break;
        }
    }
    if (unchanged) return raw;
    const source = try g.a.allocSentinel(u8, raw.len, 0);
    @memcpy(source, raw);
    var tokenizer = std.zig.Tokenizer.init(source);
    var previous: std.zig.Token.Tag = .eof;
    var token = tokenizer.next();
    var next = tokenizer.next();
    var out: std.ArrayList(u8) = .empty;
    var start: usize = 0;
    while (token.tag != .eof) {
        if (token.tag == .identifier and previous != .period and previous != .keyword_fn and next.tag != .colon) {
            for ([_][]const u8{ "std", "zant", "ui", "tsx" }, g.module_names) |name, replacement| {
                if (zig_text.eq(source[token.loc.start..token.loc.end], name) and !zig_text.eq(name, replacement)) {
                    try out.appendSlice(g.a, source[start..token.loc.start]);
                    try out.appendSlice(g.a, replacement);
                    start = token.loc.end;
                    break;
                }
            }
        }
        previous = token.tag;
        token = next;
        next = tokenizer.next();
    }
    try out.appendSlice(g.a, source[start..]);
    return out.items;
}

/// Handlers read method-entry snapshots; string snapshots copy into the frame arena.
pub fn read(g: *Generator, c: *Context, n: []const u8, loc: ast.Loc) Error!Value {
    if (scope.local(c, n)) |b| {
        return .{ .ty = b.ty, .code = b.zig };
    }
    if (try @import("captures.zig").read(g, c, n)) |value| return value;
    if (scope.stateIndex(g, n)) |i| {
        const s = g.states.items[i];
        if (c.render) return .{
            .ty = s.ty,
            .code = try zig_text.fmt(
                g,
                "self.{s}{s}",
                .{
                    try zig_text.id(g, n),
                    if (s.ty == .string or @import("model.zig").owned(g, s.ty)) ".get()" else "",
                },
            ),
        };
        for (c.snapshots.items) |snap| if (snap.state == i) return .{ .ty = s.ty, .code = snap.binding.zig };
        const b = try scope.bind(c, n, s.ty, false);
        try c.snapshots.append(g.a, .{ .state = i, .binding = b });
        return .{ .ty = s.ty, .code = b.zig };
    }
    for (g.constants.items) |constant| if (zig_text.eq(constant.name, n)) return constant.value;
    if (!c.render) for (g.render_names.items) |name| {
        if (zig_text.eq(n, name)) return core.fail(
            g,
            loc,
            "component functions cannot capture render locals; use an inline event handler",
            .{},
        );
    };
    return core.fail(g, loc, "unknown name `{s}`", .{n});
}
