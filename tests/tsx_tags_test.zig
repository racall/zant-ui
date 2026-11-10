//! Checks the merged TSX table against actual builder parameter and field types.
const std = @import("std");
const tags = @import("tsx_tags");
const zant = @import("zant");
const ui = zant.ui;

fn builderName(comptime builder: []const u8) []const u8 {
    if (!std.mem.startsWith(u8, builder, "ui.")) @compileError("Expected ui.* builder");
    const name = builder[3..];
    if (std.mem.startsWith(u8, name, "@\"") and std.mem.endsWith(u8, name, "\"")) return name[2 .. name.len - 1];
    return name;
}

/// The builder's last parameter is its options type; no tag-to-type map.
fn optionsType(comptime tag: tags.Tag) type {
    const f = @typeInfo(@TypeOf(@field(ui, builderName(tag.builder)))).@"fn";
    if (f.return_type != ui.Node) @compileError("Expected Node builder");
    return f.param_types[f.param_types.len - 1].?;
}

fn unwrap(comptime T: type) type {
    return if (@typeInfo(T) == .optional) @typeInfo(T).optional.child else T;
}

fn checkEnum(comptime T: type, values: []const tags.Value) !void {
    const Enum = unwrap(T);
    if (@typeInfo(Enum) != .@"enum") return error.ExpectedEnumField;
    try std.testing.expect(values.len > 0);
    for (values) |value| {
        var found = false;
        inline for (@typeInfo(Enum).@"enum".field_names) |name| {
            if (std.mem.eql(u8, value.zig, name)) found = true;
        }
        try std.testing.expect(found);
    }
    // Aliases must not hide native enum members.
    inline for (@typeInfo(Enum).@"enum".field_names) |name| {
        var found = false;
        for (values) |value| {
            if (std.mem.eql(u8, value.zig, name)) found = true;
        }
        try std.testing.expect(found);
    }
}

fn checkType(comptime T: type, prop: tags.Prop) anyerror!void {
    const U = unwrap(T);
    switch (prop.kind) {
        .number, .dimension => try std.testing.expect(U == f32),
        .integer => {
            try std.testing.expect(@typeInfo(U) == .int);
            try std.testing.expectEqualStrings(prop.int_type, @typeName(U));
        },
        .boolean, .flag => try std.testing.expect(U == bool),
        .string => try std.testing.expect(U == []const u8),
        .color => try std.testing.expect(T == ?u32 or T == u32),
        .edges => try std.testing.expect(U == zant.geometry.Edges),
        .action => try std.testing.expect(T == ?zant.actions.Action),
        .bool_action => try std.testing.expect(T == ?zant.actions.BoolAction),
        .text_action => try std.testing.expect(T == ?zant.actions.TextAction),
        .index_action => try std.testing.expect(T == ?zant.actions.IndexAction),
        .enumeration => try checkEnum(T, prop.values),
        .flex => unreachable,
        .node => try std.testing.expect(T == ui.Node),
        .optional_node => try std.testing.expect(T == ?ui.Node),
        .children => try std.testing.expect(U == []const ui.Node),
        .string_list => try std.testing.expect(U == []const []const u8),
        .struct_list => {
            if (@typeInfo(U) != .pointer) return error.ExpectedStructSlice;
            const p = @typeInfo(U).pointer;
            try std.testing.expect(p.size == .slice and p.attrs.@"const" and p.sentinel_ptr == null);
            if (@typeInfo(p.child) != .@"struct") return error.ExpectedStructItem;
            var found = false;
            inline for (@typeInfo(ui).@"struct".decl_names) |name| {
                if (@TypeOf(@field(ui, name)) == type) {
                    if (@field(ui, name) == p.child) {
                        const expected = comptime std.fmt.comptimePrint("ui.{f}", .{std.zig.fmtId(name)});
                        if (std.mem.eql(u8, prop.item_type, expected)) found = true;
                    }
                }
            }
            try std.testing.expect(found);
            try checkProps(p.child, prop.item_props);
        },
    }
}

/// Check the real leaf's default (including flattened and object-array fields).
fn checkField(comptime Options: type, prop: tags.Prop) anyerror!void {
    const info = @typeInfo(Options).@"struct";
    inline for (info.field_names, info.field_types, info.field_attrs) |name, T, attr| {
        if (std.mem.eql(u8, prop.field, name)) {
            try checkType(T, prop);
            try std.testing.expectEqual(attr.default_value_ptr == null and @typeInfo(T) != .optional, prop.required);
            return;
        }
    }
    std.debug.print("Missing field {s}.{s} for prop {s}\n", .{ @typeName(Options), prop.field, prop.name });
    return error.MissingOptionField;
}

fn expectFieldType(comptime Options: type, field: []const u8, comptime Expected: type) !void {
    const info = @typeInfo(Options).@"struct";
    inline for (info.field_names, info.field_types) |name, T| {
        if (std.mem.eql(u8, field, name)) {
            try std.testing.expect(T == Expected);
            return;
        }
    }
    return error.MissingOptionField;
}

fn checkLeaf(comptime Options: type, prop: tags.Prop) anyerror!void {
    switch (prop.kind) {
        .flex => {
            try std.testing.expectEqualStrings("", prop.field);
            try std.testing.expect(!prop.required);
            try expectFieldType(Options, "grow", f32);
            try expectFieldType(Options, "shrink", f32);
            try expectFieldType(Options, "basis", ?f32);
        },
        .flag => {
            if (prop.field.len > 0) try checkField(Options, prop);
            try std.testing.expect(!prop.required);
            try std.testing.expect(prop.values.len > 0);
            for (prop.values) |value| {
                if (value.zig.len > 0) try expectFieldType(Options, value.zig, bool);
            }
        },
        else => try checkField(Options, prop),
    }
    if (prop.kind == .dimension) try std.testing.expect(prop.percent_field.len > 0);
    if (prop.percent_field.len > 0) try expectFieldType(Options, prop.percent_field, ?f32);
    if (prop.kind == .edges) try std.testing.expect(@as(u4, @bitCast(prop.sides)) != 0);
    for (prop.values, 0..) |value, i| {
        for (prop.values[0..i]) |earlier| {
            try std.testing.expect(!std.mem.eql(u8, value.ts, earlier.ts));
        }
    }
}

fn checkProp(comptime Options: type, prop: tags.Prop) anyerror!void {
    if (prop.parent.len == 0) return checkLeaf(Options, prop);
    const info = @typeInfo(Options).@"struct";
    inline for (info.field_names, info.field_types) |name, T| {
        if (@typeInfo(T) == .@"struct") {
            if (std.mem.eql(u8, prop.parent, name)) return checkLeaf(T, prop);
        }
    }
    return error.MissingParentField;
}

fn checkProps(comptime Options: type, props: []const tags.Prop) anyerror!void {
    for (props, 0..) |prop, i| {
        try checkProp(Options, prop);
        for (props[0..i]) |earlier| {
            try std.testing.expect(!std.mem.eql(u8, prop.name, earlier.name));
        }
    }
}

test "tsx tag table matches reflected ui options recursively" {
    @setEvalBranchQuota(200_000);
    inline for (tags.all, 0..) |tag, i| {
        const Options = optionsType(tag);
        switch (tag.content) {
            .children => {
                var found = false;
                for (tag.props) |prop| {
                    if (std.mem.eql(u8, prop.field, "children") and prop.kind == .children) found = true;
                }
                try std.testing.expect(found);
            },
            .text_field => try expectFieldType(Options, tag.content_field, []const u8),
            .text_arg => {
                const f = @typeInfo(@TypeOf(@field(ui, builderName(tag.builder)))).@"fn";
                try std.testing.expectEqual(@as(usize, 2), f.param_types.len);
                try std.testing.expect(f.param_types[0] == []const u8);
                try std.testing.expectEqualStrings("", tag.content_field);
            },
            .none => try std.testing.expectEqualStrings("", tag.content_field),
            .child => {
                try expectFieldType(Options, tag.content_field, ui.Node);
                const prop = tags.findProp(&tag, tag.content_field).?;
                try std.testing.expect(prop.required);
            },
        }
        try checkProps(Options, tag.props);
        for (tags.all[0..i]) |earlier| {
            try std.testing.expect(!std.mem.eql(u8, earlier.name, tag.name));
            try std.testing.expect(!std.mem.eql(u8, earlier.builder, tag.builder));
        }
    }
}

// Cover every Node-returning declaration, including helpers with rejected
// signatures. A skipped builder must have a reason and cannot also be a tag.
test "every ui Node builder is generated or explicitly skipped" {
    @setEvalBranchQuota(200_000);
    inline for (@typeInfo(ui).@"struct".decl_names) |name| {
        const F = @TypeOf(@field(ui, name));
        if (@typeInfo(F) == .@"fn") {
            if (@typeInfo(F).@"fn".return_type == ui.Node) {
                var count: usize = 0;
                inline for (tags.all) |tag| {
                    if (comptime std.mem.eql(u8, builderName(tag.builder), name)) count += 1;
                }
                inline for (tags.skipped) |skip| {
                    if (comptime std.mem.eql(u8, builderName(skip.builder), name)) {
                        try std.testing.expect(skip.reason.len > 0);
                        count += 1;
                    }
                }
                try std.testing.expectEqual(@as(usize, 1), count);
            }
        }
    }
    inline for (tags.skipped, 0..) |skip, i| {
        const F = @TypeOf(@field(ui, builderName(skip.builder)));
        try std.testing.expect(@typeInfo(F).@"fn".return_type == ui.Node);
        for (tags.skipped[0..i]) |earlier| {
            try std.testing.expect(!std.mem.eql(u8, earlier.builder, skip.builder));
        }
    }
}
