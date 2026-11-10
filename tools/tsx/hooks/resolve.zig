//! Names an inlined hook reads from its own file, made available in the
//! file of the component it is inlined into (`expand.zig`): the same import
//! (its relative path recomputed), an import of the hook file's top-level
//! declaration, or the name itself when both are one file. A name the
//! component's file already uses for something else gets an alias.
const std = @import("std");
const ast = @import("../ast.zig");
const assets = @import("../assets.zig");
const rewrite = @import("rewrite.zig");
const Error = rewrite.Error;

/// A parsed .tsx file of the project.
pub const Unit = struct {
    /// Its stem: relative imports name files by it (`imports.zig`).
    name: []const u8,
    path: []const u8,
    module: *ast.Module,
    /// Imports added while inlining hooks here.
    added: std.ArrayList(ast.Import) = .empty,
    /// Top-level declarations other files now import.
    exported: std.ArrayList([]const u8) = .empty,
};

pub fn isHookName(name: []const u8) bool {
    return name.len > 3 and std.mem.startsWith(u8, name, "use") and std.ascii.isUpper(name[3]);
}

/// Inlining one hook call: from the hook's file into the caller's.
pub const Context = struct {
    arena: std.mem.Allocator,
    units: []Unit,
    caller: usize,
    from: usize,
    hook: []const u8,
    component: []const u8,
    /// Names the component declares at its top level.
    locals: *const std.StringHashMapUnmanaged(void),
    failure: *rewrite.Failure,
    cache: std.StringHashMapUnmanaged([]const u8) = .empty,

    pub fn free(self: *Context) rewrite.Free {
        return .{ .ctx = self, .resolve = resolveOpaque };
    }

    fn fail(self: *Context, loc: ast.Loc, comptime f: []const u8, args: anytype) Error {
        self.failure.* = .{ .loc = loc, .msg = try std.fmt.allocPrint(self.arena, f, args) };
        return error.Hook;
    }
};

fn resolveOpaque(ctx: *anyopaque, name: []const u8, loc: ast.Loc, kind: rewrite.Kind) Error![]const u8 {
    const self: *Context = @ptrCast(@alignCast(ctx));
    return resolve(self, name, loc, kind);
}

/// What a name of the hook's file refers to.
const Origin = union(enum) {
    /// `import { imported as name } from source`
    named: struct { source: []const u8, imported: []const u8, type_only: bool },
    /// `import name from source` / `import * as name from source`
    default: []const u8,
    namespace: []const u8,
    /// A top-level declaration of the hook's file.
    top,
};

fn resolve(self: *Context, name: []const u8, loc: ast.Loc, kind: rewrite.Kind) Error![]const u8 {
    if (self.cache.get(name)) |known| return known;
    const result = try resolveNew(self, name, loc, kind);
    try self.cache.put(self.arena, name, result);
    return result;
}

fn resolveNew(self: *Context, name: []const u8, loc: ast.Loc, kind: rewrite.Kind) Error![]const u8 {
    const from = &self.units[self.from];
    // another hook: called (and inlined) later, from wherever it was found
    if (kind == .value and isHookName(name) and findHookIn(self.units, self.from, name) != null) {
        return std.fmt.allocPrint(self.arena, "{s}@{d}", .{ name, self.from });
    }
    const origin = originOf(from.module, name, kind) orelse return name;
    if (self.from == self.caller) {
        if (kind != .type_name and self.locals.contains(name)) return self.fail(
            loc,
            "`{s}` reads `{s}`, which {s} also declares; rename one of them",
            .{ self.hook, name, self.component },
        );
        return name;
    }
    const key = try originKey(self.arena, self.units, self.from, origin, name);
    // the caller's file already has it
    if (try existing(self, key)) |local| return local;
    var local = name;
    var n: usize = 1;
    while (taken(self, local)) : (n += 1) {
        local = try std.fmt.allocPrint(self.arena, "{s}_{s}{d}", .{ name, self.hook, n });
    }
    try addImport(self, origin, name, local, kind);
    return local;
}

fn originOf(module: *const ast.Module, name: []const u8, kind: rewrite.Kind) ?Origin {
    for (module.imports) |imp| {
        for (imp.names) |n| if (std.mem.eql(u8, n.local.name, name)) return .{ .named = .{
            .source = imp.source,
            .imported = n.imported.name,
            .type_only = n.type_only or imp.type_only,
        } };
        if (imp.default_name) |d| if (std.mem.eql(u8, d.name, name)) return .{ .default = imp.source };
        if (imp.namespace_name) |d| if (std.mem.eql(u8, d.name, name)) return .{ .namespace = imp.source };
    }
    if (kind == .type_name) {
        for (module.types) |t| if (std.mem.eql(u8, t.name.name, name)) return .top;
        return null;
    }
    for (module.functions) |f| if (std.mem.eql(u8, f.func.name, name)) return .top;
    for (module.consts) |c| if (std.mem.eql(u8, c.name.name, name)) return .top;
    return null;
}

/// Identifies what an import names, whichever file it is written in.
fn importKey(arena: std.mem.Allocator, importer: []const u8, source: []const u8, what: []const u8) Error![]const u8 {
    if (!std.mem.startsWith(u8, source, "./") and !std.mem.startsWith(u8, source, "../")) {
        return std.fmt.allocPrint(arena, "{s}|{s}", .{ source, what });
    }
    if (assets.isAsset(source)) return std.fmt.allocPrint(arena, "asset:{s}|{s}", .{
        try normalized(arena, try assets.path(arena, importer, source)), what,
    });
    // project files are named by their stems (`imports.zig`)
    const base = std.fs.path.basename(source);
    const kind = if (std.mem.endsWith(u8, base, ".zig")) "zig" else "tsx";
    const stem = if (std.mem.endsWith(u8, base, ".tsx")) base[0 .. base.len - 4] else base;
    return std.fmt.allocPrint(arena, "{s}:{s}|{s}", .{ kind, stem, what });
}

fn originKey(
    arena: std.mem.Allocator,
    units: []const Unit,
    from: usize,
    origin: Origin,
    name: []const u8,
) Error![]const u8 {
    const path = units[from].path;
    return switch (origin) {
        .named => |n| importKey(arena, path, n.source, n.imported),
        .default => |source| importKey(arena, path, source, "default"),
        .namespace => |source| importKey(arena, path, source, "*"),
        .top => std.fmt.allocPrint(arena, "tsx:{s}|{s}", .{ units[from].name, name }),
    };
}

/// The caller's local name for `key`, if its file has it.
fn existing(self: *Context, key: []const u8) Error!?[]const u8 {
    const caller = &self.units[self.caller];
    // its own declaration
    const own = try std.fmt.allocPrint(self.arena, "tsx:{s}|", .{caller.name});
    if (std.mem.startsWith(u8, key, own)) {
        const name = key[own.len..];
        if (!self.locals.contains(name)) return name;
    }
    for ([_][]const ast.Import{ caller.module.imports, caller.added.items }) |list| for (list) |imp| {
        for (imp.names) |n| {
            if (std.mem.eql(u8, try importKey(self.arena, caller.path, imp.source, n.imported.name), key)) {
                if (!self.locals.contains(n.local.name)) return n.local.name;
            }
        }
        const default_key = try importKey(self.arena, caller.path, imp.source, "default");
        if (imp.default_name) |d| if (std.mem.eql(u8, default_key, key)) {
            if (!self.locals.contains(d.name)) return d.name;
        };
        if (imp.namespace_name) |d| if (std.mem.eql(u8, try importKey(self.arena, caller.path, imp.source, "*"), key)) {
            if (!self.locals.contains(d.name)) return d.name;
        };
    };
    return null;
}

/// Is `name` used in the caller's file (or its component) already?
fn taken(self: *Context, name: []const u8) bool {
    if (self.locals.contains(name)) return true;
    const caller = &self.units[self.caller];
    if (originOf(caller.module, name, .value) != null or originOf(caller.module, name, .type_name) != null) return true;
    for (caller.added.items) |imp| {
        for (imp.names) |n| if (std.mem.eql(u8, n.local.name, name)) return true;
        if (imp.default_name) |d| if (std.mem.eql(u8, d.name, name)) return true;
        if (imp.namespace_name) |d| if (std.mem.eql(u8, d.name, name)) return true;
    }
    return false;
}

fn addImport(self: *Context, origin: Origin, name: []const u8, local: []const u8, kind: rewrite.Kind) Error!void {
    const caller = &self.units[self.caller];
    const from = &self.units[self.from];
    const at: ast.Name = .{ .name = local, .loc = 0 };
    var imp: ast.Import = .{ .loc = 0, .source = "", .source_loc = 0, .type_only = false, .names = &.{} };
    switch (origin) {
        .named => |n| {
            imp.source = try sourceFor(self.arena, from.path, caller.path, n.source);
            imp.names = try self.arena.dupe(ast.ImportName, &.{.{
                .type_only = n.type_only or kind == .type_name,
                .imported = .{ .name = n.imported, .loc = 0 },
                .local = at,
            }});
        },
        .default => |source| {
            imp.source = try sourceFor(self.arena, from.path, caller.path, source);
            imp.default_name = at;
        },
        .namespace => |source| {
            imp.source = try sourceFor(self.arena, from.path, caller.path, source);
            imp.namespace_name = at;
        },
        .top => {
            imp.source = try std.fmt.allocPrint(self.arena, "./{s}", .{from.name});
            imp.names = try self.arena.dupe(ast.ImportName, &.{.{
                .type_only = kind == .type_name,
                .imported = .{ .name = name, .loc = 0 },
                .local = at,
            }});
            try from.exported.append(self.arena, name);
        },
    }
    try caller.added.append(self.arena, imp);
}

/// `source` as the caller's file writes it: relative paths are relative to
/// the importing file's directory.
fn sourceFor(
    arena: std.mem.Allocator,
    from_path: []const u8,
    caller_path: []const u8,
    source: []const u8,
) Error![]const u8 {
    if (!std.mem.startsWith(u8, source, "./") and !std.mem.startsWith(u8, source, "../")) return source;
    const target = try normalized(arena, try assets.path(arena, from_path, source));
    const dir = try normalized(arena, std.fs.path.dirname(caller_path) orelse ".");
    return relative(arena, dir, target);
}

fn normalized(arena: std.mem.Allocator, path: []const u8) Error![]const u8 {
    const slashes = try std.mem.replaceOwned(u8, arena, path, "\\", "/");
    return std.fs.path.resolvePosix(arena, &.{slashes});
}

/// `dir` and `target` are both relative to the same directory (or both absolute).
pub fn relative(arena: std.mem.Allocator, dir: []const u8, target: []const u8) Error![]const u8 {
    var dir_parts: std.ArrayList([]const u8) = .empty;
    var it = std.mem.tokenizeScalar(u8, dir, '/');
    while (it.next()) |p| if (!std.mem.eql(u8, p, ".")) try dir_parts.append(arena, p);
    var target_parts: std.ArrayList([]const u8) = .empty;
    it = std.mem.tokenizeScalar(u8, target, '/');
    while (it.next()) |p| if (!std.mem.eql(u8, p, ".")) try target_parts.append(arena, p);
    var common: usize = 0;
    while (common < dir_parts.items.len and common + 1 < target_parts.items.len and
        std.mem.eql(u8, dir_parts.items[common], target_parts.items[common])) common += 1;
    var out: std.ArrayList(u8) = .empty;
    if (common == dir_parts.items.len) try out.appendSlice(arena, "./");
    for (dir_parts.items[common..]) |_| try out.appendSlice(arena, "../");
    for (target_parts.items[common..], 0..) |p, i| {
        if (i > 0) try out.append(arena, '/');
        try out.appendSlice(arena, p);
    }
    return out.items;
}

/// A custom hook: a top-level `function useX` (or `const useX = () => …`).
pub const Hook = struct {
    unit: usize,
    func: *ast.Function,
    exported: bool,
};

pub fn hookIn(module: *const ast.Module, name: []const u8) ?Hook {
    for (module.functions) |top| if (std.mem.eql(u8, top.func.name, name) and isHookName(name)) {
        return .{ .unit = 0, .func = top.func, .exported = top.exported };
    };
    for (module.consts) |c| if (std.mem.eql(u8, c.name.name, name) and isHookName(name) and c.value.data == .arrow) {
        return .{ .unit = 0, .func = c.value.data.arrow, .exported = c.exported };
    };
    return null;
}

/// The hook `name` means in file `unit`: its own, or one it imports from
/// another file. `name@<unit>` names the hook as the file `unit` sees it.
pub fn findHookIn(units: []const Unit, unit: usize, name: []const u8) ?Hook {
    if (std.mem.indexOfScalar(u8, name, '@')) |at| {
        const from = std.fmt.parseInt(usize, name[at + 1 ..], 10) catch return null;
        if (from >= units.len) return null;
        return findHookIn(units, from, name[0..at]);
    }
    if (!isHookName(name)) return null;
    if (hookIn(units[unit].module, name)) |h| {
        var hook = h;
        hook.unit = unit;
        return hook;
    }
    for (units[unit].module.imports) |imp| for (imp.names) |n| {
        if (!std.mem.eql(u8, n.local.name, name)) continue;
        const target = unitOf(units, imp.source) orelse return null;
        var hook = hookIn(units[target].module, n.imported.name) orelse return null;
        hook.unit = target;
        return hook;
    };
    return null;
}

/// The project file a relative import names (by stem, like `imports.zig`).
pub fn unitOf(units: []const Unit, source: []const u8) ?usize {
    if (!std.mem.startsWith(u8, source, "./") and !std.mem.startsWith(u8, source, "../")) return null;
    if (assets.isAsset(source) or std.mem.endsWith(u8, source, ".zig")) return null;
    const base = std.fs.path.basename(source);
    const stem = if (std.mem.endsWith(u8, base, ".tsx")) base[0 .. base.len - 4] else base;
    for (units, 0..) |u, i| if (std.mem.eql(u8, u.name, stem)) return i;
    return null;
}

test "relative paths between directories" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    try std.testing.expectEqualStrings("../assets/x.png", try relative(a, "src", "assets/x.png"));
    try std.testing.expectEqualStrings("./AppStore", try relative(a, "src", "src/AppStore"));
    try std.testing.expectEqualStrings("./hooks/useX", try relative(a, "src", "src/hooks/useX"));
    try std.testing.expectEqualStrings("../AppStore", try relative(a, "src/hooks", "src/AppStore"));
    try std.testing.expectEqualStrings(
        "../../assets/x.png",
        try sourceFor(a, "src/hooks/useTray.tsx", "src/a/App.tsx", "../../assets/x.png"),
    );
    try std.testing.expectEqualStrings(
        "../assets/x.png",
        try sourceFor(a, "src/hooks/useTray.tsx", "src/App.tsx", "../../assets/x.png"),
    );
}
