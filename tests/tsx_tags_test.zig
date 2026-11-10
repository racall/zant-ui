//! Checks `tools/tsx/tags.zig` against the real `ui.*` option structs.
const std = @import("std");
const tags = @import("tsx_tags");
const zant = @import("zant");

fn optionsType(comptime name: []const u8) type {
    if (std.mem.eql(u8, name, "View")) return zant.ui.ViewOptions;
    if (std.mem.eql(u8, name, "Column") or std.mem.eql(u8, name, "Row")) return zant.ui.BoxOptions;
    if (std.mem.eql(u8, name, "Text")) return zant.ui.TextOptions;
    if (std.mem.eql(u8, name, "Button")) return zant.ui.ButtonOptions;
    if (std.mem.eql(u8, name, "Input")) return zant.ui.InputOptions;
    if (std.mem.eql(u8, name, "Checkbox") or std.mem.eql(u8, name, "Switch")) return zant.ui.ToggleOptions;
    @compileError("No options type for TSX tag " ++ name);
}

fn builderName(comptime builder: []const u8) []const u8 {
    if (!std.mem.startsWith(u8, builder, "ui.")) @compileError("Expected ui.* builder");
    const name = builder[3..];
    if (std.mem.startsWith(u8, name, "@\"") and std.mem.endsWith(u8, name, "\"")) return name[2 .. name.len - 1];
    return name;
}

fn checkEnum(comptime T: type, values: []const tags.Value) !void {
    const Enum = switch (@typeInfo(T)) {
        .optional => |optional| optional.child,
        else => T,
    };
    if (@typeInfo(Enum) == .@"enum") {
        try std.testing.expect(values.len > 0);
        for (values) |value| {
            var found = false;
            inline for (@typeInfo(Enum).@"enum".field_names) |name| {
                if (std.mem.eql(u8, value.zig, name)) found = true;
            }
            try std.testing.expect(found);
        }
    } else {
        return error.ExpectedEnumField;
    }
}

fn checkType(comptime T: type, prop: tags.Prop) !void {
    switch (prop.kind) {
        .number, .dimension => try std.testing.expect(T == f32 or T == ?f32),
        .integer => {
            try std.testing.expect(@typeInfo(T) == .int);
            try std.testing.expect(prop.int_type.len > 0);
            try std.testing.expectEqualStrings(prop.int_type, @typeName(T));
        },
        .boolean, .flag => try std.testing.expect(T == bool),
        .string => try std.testing.expect(T == []const u8 or T == ?[]const u8),
        .color => try std.testing.expect(T == ?u32),
        .edges => try std.testing.expect(T == zant.geometry.Edges),
        .action => try std.testing.expect(T == ?zant.actions.Action),
        .bool_action => try std.testing.expect(T == ?zant.actions.BoolAction),
        .text_action => try std.testing.expect(T == ?zant.actions.TextAction),
        .index_action => try std.testing.expect(T == ?zant.actions.IndexAction),
        .enumeration => try checkEnum(T, prop.values),
        .flex => unreachable,
    }
}

fn checkField(comptime Options: type, prop: tags.Prop) !void {
    const info = @typeInfo(Options).@"struct";
    inline for (info.field_names, info.field_types) |name, T| {
        if (std.mem.eql(u8, prop.field, name)) {
            try checkType(T, prop);
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
    std.debug.print("Missing field {s}.{s}\n", .{ @typeName(Options), field });
    return error.MissingOptionField;
}

fn checkProp(comptime Options: type, prop: tags.Prop) !void {
    switch (prop.kind) {
        .flex => {
            try std.testing.expectEqualStrings("", prop.field);
            try expectFieldType(Options, "grow", f32);
            try expectFieldType(Options, "shrink", f32);
            try expectFieldType(Options, "basis", ?f32);
        },
        .flag => {
            // Flags select fields through their values instead of prop.field.
            if (prop.field.len > 0) try checkField(Options, prop);
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

test "tsx tag table matches ui options" {
    try std.testing.expectEqual(@as(usize, 8), tags.all.len);
    inline for (tags.all) |tag| {
        const Options = optionsType(tag.name);
        const builder = comptime builderName(tag.builder);
        try std.testing.expect(@hasDecl(zant.ui, builder));
        if (@hasDecl(zant.ui, builder)) {
            try std.testing.expect(@typeInfo(@TypeOf(@field(zant.ui, builder))) == .@"fn");
        }
        switch (tag.content) {
            .children => try expectFieldType(Options, "children", []const zant.ui.Node),
            .text_field => try expectFieldType(Options, tag.content_field, []const u8),
            .text_arg, .none => try std.testing.expectEqualStrings("", tag.content_field),
        }
        for (tag.props, 0..) |prop, i| {
            try checkProp(Options, prop);
            for (tag.props[0..i]) |earlier| {
                try std.testing.expect(!std.mem.eql(u8, prop.name, earlier.name));
            }
        }
    }
}
