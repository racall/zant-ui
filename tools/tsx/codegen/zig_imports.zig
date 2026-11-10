//! Public Zig functions form a small, typed boundary for project TSX code.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const zig_types = @import("zig_types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const text = @import("zig_text.zig");
const Type = types.Type;
const Error = core.Error;
const Generator = core.Generator;

/// A hand-written project file; its AST and sentinel source live in the compile arena.
pub const Source = struct {
    name: []const u8,
    tree: std.zig.Ast,
};

/// An imported function, after its signature has been checked and mapped to TSX types.
pub const Function = struct {
    local: []const u8,
    target: []const u8,
    params: []const Type,
    ret: Type,
};

/// Register named imports and append one module alias per Zig file.
pub fn process(g: *Generator, imp: ast.Import, header: *std.ArrayList(u8)) Error!void {
    if (g.project.len == 0) return core.fail(
        g,
        imp.source_loc,
        "Zig imports need a project (zant-tsx --out / addTsxProject)",
        .{},
    );
    const base = std.fs.path.basename(imp.source);
    const name = base[0 .. base.len - 4];
    const source = for (g.zig_sources) |*source| {
        if (text.eq(source.name, name)) break source;
    } else return core.fail(g, imp.source_loc, "cannot find `{s}` among the project's .zig files", .{imp.source});
    const alias = try text.id(g, try text.fmt(g, "_{s}", .{name}));
    const line = try text.fmt(g, "const {s} = @import({s});\n", .{ alias, try text.quote(g, base) });
    if (std.mem.indexOf(u8, header.items, line) == null) try header.appendSlice(g.a, line);
    const file = zig_types.File{ .tree = &source.tree, .alias = alias };
    for (imp.names) |n| {
        if (n.type_only or imp.type_only) {
            const ty = (try zig_types.resolve(g, file, n.imported.name)) orelse return core.fail(
                g,
                n.imported.loc,
                "`{s}` is not a pub struct or enum of `{s}`",
                .{ n.imported.name, imp.source },
            );
            try g.zig_types.append(g.a, .{ .local = n.local.name, .ty = ty });
            continue;
        }
        const decl = find(&source.tree, n.imported.name) orelse {
            if (closest(&source.tree, n.imported.name)) |near| return core.fail(
                g,
                n.imported.loc,
                "`{s}` is not a pub fn of `{s}`; did you mean `{s}`?",
                .{ n.imported.name, imp.source, near },
            );
            return core.fail(g, n.imported.loc, "`{s}` is not a pub fn of `{s}`", .{ n.imported.name, imp.source });
        };
        var function = try signatureIn(g, &source.tree, file, decl, n.imported);
        function.local = n.local.name;
        function.target = try text.fmt(g, "{s}.{s}", .{ alias, try text.id(g, n.imported.name) });
        try g.zig_functions.append(g.a, function);
    }
}

/// Lower imported calls with exactly the same contextual coercions as component methods.
pub fn tryCall(g: *Generator, c: *scope.Context, loc: ast.Loc, call: ast.Call) Error!?types.Value {
    if (call.callee.data != .ident) return null;
    const name = call.callee.data.ident;
    if (scope.local(c, name) != null) return null;
    for (g.zig_functions.items) |function| {
        if (!text.eq(function.local, name)) continue;
        if (call.type_args.len != 0) return core.fail(g, loc, "Zig functions do not take TSX type arguments", .{});
        if (call.args.len != function.params.len) {
            return core.fail(g, loc, "`{s}` expects {d} arguments", .{ name, function.params.len });
        }
        var args: std.ArrayList([]const u8) = .empty;
        for (call.args, function.params) |arg, ty| {
            try args.append(g.a, (try expr.emit(g, c, arg, ty, false)).code);
        }
        return .{
            .ty = function.ret,
            .code = try text.fmt(g, "{s}({s})", .{ function.target, try std.mem.join(g.a, ", ", args.items) }),
        };
    }
    return null;
}

fn publicName(tree: *const std.zig.Ast, decl: std.zig.Ast.Node.Index) ?[]const u8 {
    var buffer: [1]std.zig.Ast.Node.Index = undefined;
    const proto = tree.fullFnProto(&buffer, decl) orelse return null;
    if (proto.visib_token == null) return null;
    return tree.tokenSlice(proto.name_token orelse return null);
}

fn find(tree: *const std.zig.Ast, name: []const u8) ?std.zig.Ast.Node.Index {
    for (tree.rootDecls()) |decl| {
        if (text.eq(publicName(tree, decl) orelse continue, name)) return decl;
    }
    return null;
}

fn closest(tree: *const std.zig.Ast, name: []const u8) ?[]const u8 {
    var best: ?[]const u8 = null;
    var distance: usize = 3;
    for (tree.rootDecls()) |decl| {
        const candidate = publicName(tree, decl) orelse continue;
        const d = model.editDistance(candidate, name);
        if (d < distance) {
            best = candidate;
            distance = d;
        }
    }
    return best;
}

fn signature(g: *Generator, tree: *const std.zig.Ast, decl: std.zig.Ast.Node.Index, n: ast.Name) Error!Function {
    return signatureIn(g, tree, null, decl, n);
}

/// The TSX signature of a pub fn; `file` resolves the file's own types.
fn signatureIn(
    g: *Generator,
    tree: *const std.zig.Ast,
    file: ?zig_types.File,
    decl: std.zig.Ast.Node.Index,
    n: ast.Name,
) Error!Function {
    var buffer: [1]std.zig.Ast.Node.Index = undefined;
    const proto = tree.fullFnProto(&buffer, decl).?;
    var params: std.ArrayList(Type) = .empty;
    var it = proto.iterate(tree);
    while (it.next()) |param| {
        if (param.comptime_noalias) |tok| if (tree.tokenTag(tok) == .keyword_comptime) {
            return unsupported(g, n, "takes", "comptime parameters");
        };
        const node = param.type_expr orelse return unsupported(
            g,
            n,
            "takes",
            tree.tokenSlice(param.anytype_ellipsis3.?),
        );
        try params.append(g.a, (try zig_types.mapIn(g, tree, file, node, false)) orelse
            return unsupported(g, n, "takes", tree.getNodeSource(node)));
    }
    const ret_node = proto.ast.return_type.unwrap() orelse return unsupported(g, n, "returns", "an inferred type");
    // Inferred error unions keep `!` as a token outside the return type node.
    if (tree.tokenTag(tree.firstToken(ret_node) - 1) == .bang) {
        return unsupported(g, n, "returns", try text.fmt(g, "!{s}", .{tree.getNodeSource(ret_node)}));
    }
    const ret = (try zig_types.mapIn(g, tree, file, ret_node, true)) orelse
        return unsupported(g, n, "returns", tree.getNodeSource(ret_node));
    return .{ .local = n.name, .target = "", .params = params.items, .ret = ret };
}

fn unsupported(g: *Generator, n: ast.Name, verb: []const u8, spelling: []const u8) Error {
    return core.fail(
        g,
        n.loc,
        "`{s}` {s} `{s}`; Zig functions imported into TSX use bool, string, numbers, optionals and slices of them",
        .{ n.name, verb, spelling },
    );
}

test "Zig signatures map primitive, optional and slice types through the AST" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    var diag: ?core.Diag = null;
    var g = Generator{ .a = a, .diag = &diag };
    const source = try a.dupeSentinel(u8,
        \\fn hidden() void {}
        \\pub const value = 1;
        \\const Nested = struct { pub fn nested() void {} };
        \\pub fn plural(n: usize, word: []const u8) []const u8 { return word; }
        \\pub fn all(a: bool, b: ?i32, c: []const f64, d: []const []const u8) ?[]const bool { return null; }
        \\pub fn nothing() void {}
    , 0);
    const tree = try std.zig.Ast.parse(a, source, .{});
    try std.testing.expectEqual(@as(usize, 0), tree.errors.len);
    try std.testing.expect(find(&tree, "hidden") == null);
    try std.testing.expect(find(&tree, "value") == null);
    try std.testing.expect(find(&tree, "nested") == null);
    const plural = try signature(&g, &tree, find(&tree, "plural").?, .{ .name = "plural", .loc = 0 });
    try std.testing.expectEqualSlices(Type, &.{ .usize, .string }, plural.params);
    try std.testing.expectEqual(Type.string, plural.ret);
    const all = try signature(&g, &tree, find(&tree, "all").?, .{ .name = "all", .loc = 0 });
    try std.testing.expectEqual(Type.boolean, all.params[0]);
    try std.testing.expectEqual(Type.i32, model.optionalChild(&g, all.params[1]).?);
    try std.testing.expectEqual(Type.f64, model.element(&g, all.params[2]).?);
    try std.testing.expectEqual(Type.string, model.element(&g, all.params[3]).?);
    const ret_array = model.optionalChild(&g, all.ret).?;
    try std.testing.expectEqual(Type.boolean, model.element(&g, ret_array).?);
    const nothing = try signature(&g, &tree, find(&tree, "nothing").?, .{ .name = "nothing", .loc = 0 });
    try std.testing.expectEqual(Type.void_, nothing.ret);
    inline for (.{ "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "usize", "isize", "f32", "f64" }) |t| {
        const raw = try std.fmt.allocPrint(a, "pub fn f(x: {s}) {s} {{ return x; }}", .{ t, t });
        const src = try a.dupeSentinel(u8, raw, 0);
        const numeric = try std.zig.Ast.parse(a, src, .{});
        const f = try signature(&g, &numeric, find(&numeric, "f").?, .{ .name = "f", .loc = 0 });
        try std.testing.expectEqualStrings(t, f.params[0].zig(&g));
        try std.testing.expectEqual(f.params[0], f.ret);
    }
}

test "unsupported Zig signatures report the import name and offending type" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    for ([_][]const u8{
        "x: Foo",                  "x: *const u8",    "x: anytype", "comptime x: usize",   "x: []u32", "x: []const Foo",
        "x: []align(8) const u32", "x: [:0]const u8", "x: ?*u8",    "x: struct { a: u8 }",
    }) |param| {
        var diag: ?core.Diag = null;
        var g = Generator{ .a = a, .diag = &diag };
        const source = try a.dupeSentinel(u8, try std.fmt.allocPrint(a, "pub fn f({s}) void {{}}", .{param}), 0);
        const tree = try std.zig.Ast.parse(a, source, .{});
        try std.testing.expectEqual(@as(usize, 0), tree.errors.len);
        try std.testing.expectError(error.Tsx, signature(&g, &tree, find(&tree, "f").?, .{ .name = "f", .loc = 42 }));
        try std.testing.expectEqual(@as(u32, 42), diag.?.loc);
        try std.testing.expect(std.mem.startsWith(u8, diag.?.msg, "`f` takes `"));
    }
    for ([_][]const u8{ "!void", "error{Oops}!u32", "Foo", "Array(u8)", "*u8" }) |ret| {
        var diag: ?core.Diag = null;
        var g = Generator{ .a = a, .diag = &diag };
        const raw = try std.fmt.allocPrint(a, "pub fn f() {s} {{ unreachable; }}", .{ret});
        const source = try a.dupeSentinel(u8, raw, 0);
        const tree = try std.zig.Ast.parse(a, source, .{});
        try std.testing.expectError(error.Tsx, signature(&g, &tree, find(&tree, "f").?, .{ .name = "f", .loc = 0 }));
        try std.testing.expect(std.mem.startsWith(u8, diag.?.msg, "`f` returns `"));
    }
}
